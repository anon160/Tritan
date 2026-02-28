/* Tritan OS - Preserved Structure for rust-lld */
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  . = 0x80200000;

  .text : {
    *(.text.init)    /* entry.S goes here first */
    
    . = ALIGN(0x1000);
    *(.text.main)    /* Rust main(hartid, dtb_pa) */
    *(.text .text.*) /* All other Rust code */

    . = ALIGN(16);
    *(.text.swtch)   /* swtch.S code */
    
    . = ALIGN(16);
    *(.stack)        /* Matches any .stack section from assembly */

    /* Ensure trampoline is page-aligned and mapped into the kernel text */
    . = ALIGN(0x1000);
    PROVIDE(trampoline_start = .);
    *(.trampoline)   /* From trampoline.S */
    . = ALIGN(0x1000);
    PROVIDE(trampoline_end = .);
  }

  /* Keep your original .stack section structure */
  .stack : {
    . = ALIGN(4096);
    PROVIDE(stack0 = .);
    
    /* We must use an explicit fill or increment so lld doesn't error on the dot */
    . += 4096 * 8; 
    
    . = ALIGN(4096);
    PROVIDE(stack_top = .);
  }

  .rodata : { 
    . = ALIGN(16); 
    *(.rodata .rodata.*) 
  }

  .data : { 
    . = ALIGN(16); 
    *(.data .data.*)
    *(.data.boot_pt) /* CRITICAL: Must include your PageTable static */
  }

  .bss : { 
    . = ALIGN(16);
    PROVIDE(__sbss = .);
    *(.bss .bss.*) 
    PROVIDE(__ebss = .);
  }

  PROVIDE(end = .);
}