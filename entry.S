.section .text.init
.globl _start
_start:
    # 1. Save Hart ID (a0) and DTB Pointer (a1) immediately
    # tp is used for our CPU anchor, s1 stores the DTB address for main
    mv tp, a0
    mv s1, a1

    # 2. Setup a stack for each Hart (4KB per hart)
    la sp, stack0
    li t0, 4096
    addi t1, a0, 1
    mul t0, t0, t1
    add sp, sp, t0

    # 3. Only Hart 0 performs the Wake-up/Invitation Loop
    bnez a0, secondary_entry

    # --- HART 0 ONLY: The Invitation Loop ---
    li s0, 1                # Target Hart ID starts at 1
    li s2, 4                # Max Harts (matching -smp 4)
wake_loop:
    li a7, 0x48534D         # SBI HSM Extension ID
    li a6, 0                # hart_start Function ID
    mv a0, s0               # Hart ID to start
    la a1, _start           # Start address for the target Hart
    mv a2, s1               # Pass our DTB pointer as the 'opaque' value to new harts
    ecall
    
    addi s0, s0, 1
    blt s0, s2, wake_loop

secondary_entry:
    # 4. All Harts jump to main(hartid, dtb_pa)
    # Restore arguments from our saved registers
    mv a0, tp               # Argument 0: Hart ID
    mv a1, s1               # Argument 1: DTB Pointer
    j main

.section .stack
.align 16
stack0:
    .space 4096 * 4         # Reserve space for 4 Harts