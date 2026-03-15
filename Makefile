OBJS = entry.o vectors.o trapasm.o swtch.o
RS = src/*.rs

# Cross-compiling (e.g., on Mac OS X)
#TOOLPREFIX = i386-jos-elf
# TOOLPREFIX = i686-elf-

# Using native tools (e.g., on X86 Linux)
#TOOLPREFIX =

# Try to infer the correct TOOLPREFIX if not set
ifndef TOOLPREFIX
TOOLPREFIX := $(shell if command -v i386-jos-elf-gcc >/dev/null 2>&1 && i386-jos-elf-objdump -i 2>&1 | grep '^elf32-i386$$' >/dev/null 2>&1; \
	then echo 'i386-jos-elf-'; \
	elif command -v i686-elf-gcc >/dev/null 2>&1 && i686-elf-objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo 'i686-elf-'; \
	elif command -v gcc >/dev/null 2>&1 && objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
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

# Build mkfs utility and create filesystem image
mkfs: src/mkfs.rs
	rustc -W warnings -o mkfs src/mkfs.rs

# User programs (C version)

ULIB = usys.o printf.o

usys.o: usys.S
	$(CC) $(CFLAGS) -c -o usys.o usys.S

printf.o: printf.c
	$(CC) $(CFLAGS) -c -o printf.o printf.c

init.o: init.c
	$(CC) $(CFLAGS) -c -o init.o init.c

_init: init.o $(ULIB)
	$(LD) $(LDFLAGS) -N -e main -Ttext 0 -o _init $^
	$(OBJDUMP) -S _init > init.asm
	$(OBJDUMP) -t _init | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > init.sym

UPROGS=\
	_init

# User Library Programs Pipeline

fs.img: mkfs *.txt $(UPROGS)
	./mkfs fs.img *.txt $(UPROGS)

bootblock: bootasm.S bootmain.c linkers/bootblock.ld
	$(CC) $(CFLAGS) -fno-pic -O -nostdinc -I. -c bootmain.c
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c bootasm.S
	$(LD) $(LDFLAGS) -T linkers/bootblock.ld -o bootblock.o bootasm.o bootmain.o
	$(OBJDUMP) -S bootblock.o > bootblock.asm
	$(OBJCOPY) -S -O binary bootblock.o bootblock

initcode: initcode.S
	$(CC) $(CFLAGS) -nostdinc -I. -c initcode.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0 -o initcode.out initcode.o
	$(OBJCOPY) -S -O binary initcode.out initcode
	$(OBJDUMP) -S initcode.o > initcode.asm

kernel.a: $(RS)
	cargo build -Z build-std=core -Z build-std-features=compiler-builtins-mem -Z json-target-spec\
	  --target ./targets/i686.json --release
	@tdir=$$(cargo metadata --format-version=1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'); \
	lib=$$(find "$$tdir" -maxdepth 4 -type f -name 'libkernel.a' | head -n 1); \
	if [ -z "$$lib" ]; then echo "ERROR: libkernel.a not found"; exit 1; fi; \
	cp "$$lib" kernel.a


kernel: kernel.a $(OBJS) ./linkers/kernel.ld initcode
	$(LD) -m elf_i386 -T ./linkers/kernel.ld -o kernel $(OBJS) kernel.a -b binary initcode
	$(OBJDUMP) -S -D kernel > kernel.asm
	$(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

vectors.S: vectors.pl
	./vectors.pl > vectors.S


# $(LD) $(LDFLAGS) -T kernel.ld -o kernel entry.o kernel.a -b binary
# ld -m    elf_i386 -T kernel.ld -o kernel entry.o kernel.a -b binary
# Prevent deletion of intermediate files, e.g. cat.o, after first build, so
# that disk image changes after first build are persistent until clean.  More
# details:
# http://www.gnu.org/software/make/manual/html_node/Chained-Rules.html
.PRECIOUS: %.o
-include *.d

clean:
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.a *.o *.d *.asm *.sym bootblock kernel xv6.img fs.img mkfs .gdbinit vectors.S initcode initcode.out \
	$(UPROGS) user/src/*.o usys.o
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

qemu-gdb: xv6.img .gdbinit fs.img
	@echo "*** Now run 'gdb'." 1>&2
	$(QEMU) -nographic $(QEMUOPTS) -S $(QEMUGDB)

qemu-full-debug: xv6.img fs.img
	$(QEMU) -nographic $(QEMUOPTS) \
	-debugcon file:qemu_debug.log \
	-global isa-debugcon.iobase=0xe9 \
	-D qemu.log -d guest_errors \
	2> qemu_err.log