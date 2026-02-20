OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  . = 0x80000000; # QEMU RAM Start

  .text : {
    *(.text.init)
    *(.text .text.*)
    
    . = ALIGN(0x1000);
    PROVIDE(trampoline_start = .);
    *(.trampoline)
    . = ALIGN(0x1000);
    PROVIDE(trampoline_end = .);
  }

  .rodata : { *(.rodata .rodata.*) }
  .data   : { *(.data .data.*) }
  
  .bss : {
    PROVIDE(__sbss = .);
    *(.bss .bss.*)
    PROVIDE(__ebss = .);
  }

  PROVIDE(end = .);
}