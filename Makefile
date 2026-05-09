OBJS = entry.o vectors.o trapasm.o swtch.o
RS = src/*.rs

# Cross-compiling (e.g., on Mac OS X)
#TOOLPREFIX = i386-jos-elf
# TOOLPREFIX = i686-elf-

# Using native tools (e.g., on X86 Linux)
#TOOLPREFIX =

MAC_CCFLAGS := $(shell if [ "$(shell uname -s)" = "Darwin" ] && [ "$(shell uname -m)" = "arm64" ]; then \
	echo "-Wno-error=infinite-recursion -Wno-error=array-bounds"; \
	else \
	echo ""; \
fi)

# Try to infer the correct TOOLPREFIX if not set
ifndef TOOLPREFIX
TOOLPREFIX := $(shell if command -v i386-jos-elf-gcc >/dev/null 2>&1 && i386-jos-elf-objdump -i 2>&1 | grep '^elf32-i386$$' >/dev/null 2>&1; \
	then echo 'i386-jos-elf-'; \
	elif command -v i686-elf-gcc >/dev/null 2>&1 && i686-elf-objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo 'i686-elf-'; \
	elif command -v gcc >/dev/null 2>&1 && objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo ''; \
	elif i686-elf-objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo 'i686-elf-'; \
	elif i386-elf-objdump -i 2>&1 | grep 'elf32-i386' >/dev/null 2>&1; \
	then echo 'i386-elf-'; \
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
CFLAGS += $(MAC_CCFLAGS)
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
mkfs: src/mkfs.rs src/fs_h.rs
	rustc --edition=2021 -W warnings -o mkfs src/mkfs.rs 
	

fs.img: mkfs *.txt
	./mkfs fs.img *.txt

bootblock: bootasm.S bootmain.c linkers/bootblock.ld
	$(CC) $(CFLAGS) -fno-pic -O -nostdinc -I. -c bootmain.c
	$(CC) $(CFLAGS) -fno-pic -nostdinc -I. -c bootasm.S
	$(LD) $(LDFLAGS) -T linkers/bootblock.ld -o bootblock.o bootasm.o bootmain.o
	$(OBJDUMP) -S -D bootblock.o > bootblock.asm
	$(OBJCOPY) -S -O binary bootblock.o bootblock

initcode: initcode.S
	$(CC) $(CFLAGS) -nostdinc -I. -c initcode.S
	$(LD) $(LDFLAGS) -N -e start -Ttext 0 -o initcode.out initcode.o
	$(OBJCOPY) -S -O binary initcode.out initcode
	$(OBJDUMP) -S initcode.o > initcode.asm

kernel.a: $(RS)
	cargo +nightly rustc \
		-Z build-std=core \
		-Z build-std-features=compiler-builtins-mem \
		-Zjson-target-spec \
		--target ./targets/i686-stage-3.json \
		--lib --release \
		-- -A warnings --emit link=kernel.a

kernel: kernel.a $(OBJS) ./linkers/kernel.ld initcode
	$(LD) -m elf_i386 -T ./linkers/kernel.ld -o kernel $(OBJS) kernel.a -b binary initcode
	$(OBJDUMP) -S -D kernel > kernel.asm
	$(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

vectors.S: vectors.pl
	./vectors.pl > vectors.S

# Prevent deletion of intermediate files, e.g. cat.o, after first build, so
# that disk image changes after first build are persistent until clean.  More
# details:
# http://www.gnu.org/software/make/manual/html_node/Chained-Rules.html
.PRECIOUS: %.o
-include *.d

clean:
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.a *.o *.d *.asm *.sym bootblock kernel xv6.img fs.img mkfs .gdbinit vectors.S initcode initcode.out
	cargo clean

GDBPORT = $(shell expr `id -u` % 5000 + 25000)
QEMUGDB = $(shell if $(QEMU) -help | grep -q '^-gdb'; \
	then echo "-gdb tcp::$(GDBPORT)"; \
	else echo "-s -p $(GDBPORT)"; fi)

ifndef CPUS
CPUS := 1
endif

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
