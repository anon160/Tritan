/* Updated linker.ld */
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  . = 0x80200000;

.text : {
    *(.text.init)    /* entry.S goes here first */
    . = ALIGN(0x1000);    /* Move main to a predictable spot */
    *(.text.main)         /* We will tag our Rust main with this */
    *(.text .text.*) /* Then all the Rust code */

    . = ALIGN(16);
    *(.text.swtch)   /* swtch.S will go here second */
    
    . = ALIGN(16);
    *(.stack)        /* stack0 will go here third */

    
    . = ALIGN(0x1000);
    PROVIDE(trampoline_start = .);
    *(.trampoline)
    . = ALIGN(0x1000);
    PROVIDE(trampoline_end = .);
  }


  .stack : {
      PROVIDE(stack0 = .);
      *(.stack)
      . = ALIGN(4096 * 8); /* Ensure the whole stack is reserved */
      PROVIDE(stack_top = .);
    }

  .rodata : { . = ALIGN(16); *(.rodata .rodata.*) }
  .data   : { . = ALIGN(16); *(.data .data.*) }
  .bss    : { 
      . = ALIGN(16);
      PROVIDE(__sbss = .);
      *(.bss .bss.*) 
      PROVIDE(__ebss = .);
  }

  PROVIDE(end = .);
}