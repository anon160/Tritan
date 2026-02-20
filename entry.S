# entry.S
.section .text.init
.globl _start
_start:
    # 1. Setup a stack so we can perform the jump
    la sp, boot_stack_top

    # 2. Delegate interrupts and exceptions to S-mode.
    # Without this, 'ecall' will try to jump to M-mode, bypassing your trampoline!
    li t0, 0xffff
    csrw medeleg, t0
    csrw mideleg, t0

    # 3. Configure mstatus to return into Supervisor Mode (S-mode)
    # Bits 12:11 are MPP (Machine Previous Privilege). 
    # We set them to 01 (Supervisor).
    li t0, (1 << 11)
    csrw mstatus, t0

    # 4. Set the return address (mepc) to our Rust main function
    la t1, main
    csrw mepc, t1

    # 5. Disable PMP (Physical Memory Protection)
    # By default, some QEMU versions lock down all memory in M-mode.
    # This gives S-mode full access to the entire 64-bit address space.
    li t0, 0x3fffffffffffffff
    csrw pmpaddr0, t0
    li t0, 0xf
    csrw pmpcfg0, t0

    # 6. "Return" from M-mode to S-mode
    # This jumps to the address in 'mepc' (main) and switches to S-mode
    mret

.section .bss.stack
.align 16
boot_stack:
    .space 4096 * 4
boot_stack_top: