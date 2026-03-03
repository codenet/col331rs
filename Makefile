OBJS = entry.o vectors.o trapasm.o
RS = src/*.rs

# Cross-compiling (e.g., on Mac OS X)
#TOOLPREFIX = i386-jos-elf
#TOOLPREFIX = i386-elf-

# Using native tools (e.g., on X86 Linux)
#TOOLPREFIX =

# Try to infer the correct TOOLPREFIX if not set
ifndef TOOLPREFIX
TOOLPREFIX := $(shell if i386-jos-elf-objdump -i 2>&1 | grep '^elf32-i386$$' >/dev/null 2>&1; \
	then echo 'i386-jos-elf-'; \
	elif objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo ''; \
	else echo "***" 1>&2; \
	echo "*** Error: Couldn't find an i386-*-elf version of GCC/binutils." 1>&2; \
	exit 1; fi)
endif

ifndef QEMU
QEMU = $(shell if which qemu > /dev/null; \
	then echo qemu; exit; \
	elif which qemu-system-i386 > /dev/null; \
	then echo qemu-system-i386; exit; \
	elif which qemu-system-x86_64 > /dev/null; \
	then echo qemu-system-x86_64; exit; \
	else echo "*** Error: Couldn't find QEMU." 1>&2; exit 1; fi)
endif

CC = $(TOOLPREFIX)gcc
AS = $(TOOLPREFIX)gas
LD = $(TOOLPREFIX)ld
OBJCOPY = $(TOOLPREFIX)objcopy
OBJDUMP = $(TOOLPREFIX)objdump

CFLAGS = -fno-pic -static -fno-builtin -fno-strict-aliasing -O2 -Wall -MD -ggdb -m32 -Werror -fno-omit-frame-pointer
CFLAGS += $(shell $(CC) -fno-stack-protector -E -x c /dev/null >/dev/null 2>&1 && echo -fno-stack-protector)

ASFLAGS = -m32 -gdwarf-2 -Wa,-divide
LDFLAGS += -m $(shell $(LD) -V | grep elf_i386 2>/dev/null | head -n 1)

ifneq ($(shell $(CC) -dumpspecs 2>/dev/null | grep -e '[^f]no-pie'),)
CFLAGS += -fno-pie -no-pie
endif
ifneq ($(shell $(CC) -dumpspecs 2>/dev/null | grep -e '[^f]nopie'),)
CFLAGS += -fno-pie -nopie
endif

# Disk image with bootblock + kernel
xv6.img: bootblock kernel
	dd if=/dev/zero of=xv6.img count=10000
	dd if=bootblock of=xv6.img conv=notrunc
	dd if=kernel of=xv6.img seek=1 conv=notrunc

# Second disk (fs.img) with welcome text (2 sectors, like C version)
fs: fs.img

fs.img: welcome.txt
	dd if=/dev/zero of=fs.img count=2
	dd if=welcome.txt of=fs.img conv=notrunc

bootblock: bootasm.S bootmain.c linkers/bootblock.ld
	$(CC) $(CFLAGS) -fno-pic -O -nostdinc -I. -c bootmain.c
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c bootasm.S
	$(LD) $(LDFLAGS) -T linkers/bootblock.ld -o bootblock.o bootasm.o bootmain.o
	$(OBJDUMP) -S bootblock.o > bootblock.asm
	$(OBJCOPY) -S -O binary bootblock.o bootblock

kernel.a: $(RS)
	cargo build -Z build-std=core -Z build-std-features=compiler-builtins-mem -Z json-target-spec \
	  --target ./targets/i686.json --release
	@tdir=$$(cargo metadata --format-version=1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'); \
	lib=$$(find "$$tdir" -maxdepth 4 -type f -name 'libkernel.a' | head -n 1); \
	if [ -z "$$lib" ]; then echo "ERROR: libkernel.a not found"; exit 1; fi; \
	cp "$$lib" kernel.a


kernel: kernel.a $(OBJS) ./linkers/kernel.ld
	$(LD) -m elf_i386 -T ./linkers/kernel.ld -o kernel $(OBJS) kernel.a
	$(OBJDUMP) -S -D kernel > kernel.asm
	$(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

vectors.S: vectors.pl
	./vectors.pl > vectors.S

.PRECIOUS: %.o
-include *.d

clean:
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.a *.o *.d *.asm *.sym bootblock kernel xv6.img fs.img .gdbinit vectors.S
	rm -rf target

# run in emulators
GDBPORT = $(shell expr `id -u` % 5000 + 25000)
QEMUGDB = $(shell if $(QEMU) -help | grep -q '^-gdb'; \
	then echo "-gdb tcp::$(GDBPORT)"; \
	else echo "-s -p $(GDBPORT)"; fi)

ifndef CPUS
CPUS := 1
endif

# Attach fs.img as disk1 (index=1), like the C version
QEMUOPTS = -drive file=xv6.img,index=0,media=disk,format=raw \
           -drive file=fs.img,index=1,media=disk,format=raw \
           -smp $(CPUS) -m 512 $(QEMUEXTRA)

qemu: xv6.img fs.img
	$(QEMU) -nographic $(QEMUOPTS)

.gdbinit: .gdbinit.tmpl
	sed "s/localhost:1234/localhost:$(GDBPORT)/" < $^ > $@

qemu-gdb: xv6.img fs .gdbinit
	@echo "*** Now run 'gdb'." 1>&2
	$(QEMU) -nographic $(QEMUOPTS) -S $(QEMUGDB)
