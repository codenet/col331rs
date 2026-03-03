OUTPUT_FORMAT("elf32-i386", "elf32-i386", "elf32-i386")
OUTPUT_ARCH(i386)
ENTRY(start)

SECTIONS
{
  . = 0x7c00;

  .text : {
    *(.text .text.*)
  }

  .rodata : {
    *(.rodata .rodata.*)
  }

  .data : {
    *(.data .data.*)
  }

  /DISCARD/ : {
    *(.eh_frame .note.GNU-stack .comment)
  }

  . = 0x7c00 + 510;
  .signature : {
    SHORT(0xaa55)
  }
}
