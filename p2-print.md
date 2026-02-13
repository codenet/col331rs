# UART Documentation (Rust Version)

UART stands for Universal Asynchronous Receiver/Transmitter. It is an interface for asynchronous (without a synchronizing clock signal) serial communication in PCs.

The `uart.rs` file describes the functions to communicate with the UART interface, which in this case is an 8250-compatible UART (QEMU emulates the classic PC8250/16550-style UART, so the register mapping and basic behavior remain consistent).

The UART exposes 8 programmable registers accessed using port-mapped I/O. On x86, `0x3f8` is the base I/O port for COM1 (equivalent to register 0 in the datasheet). These registers are accessed using `in`/`out` instructions. In Rust, this is done via inline assembly (`core::arch::asm`) inside `unsafe` blocks (similar to how `outb/inb` are used in C).

QEMU simulates the UART. So if we correctly program these UART registers and write bytes to the data register, the output appears on the QEMU terminal (especially when running with `-nographic`). For example, `uartputc` polls the Line Status Register (LSR) and waits until the Transmit Holding Register Empty (THRE) bit (`0x20`) is set, which indicates the UART is ready to transmit another character, before writing the next byte to the data register.

[UART Datasheet](https://sys.cs.fau.de/extern/lehre/ws22/bs/uebung/aufgabe3/uart-8250a.pdf)
