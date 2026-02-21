.section .text.init
.globl _start
_start:
    # 1. Save Hart ID (provided by OpenSBI in a0)
    mv tp, a0

    # 2. Setup a temporary stack for the wake-up logic
    la sp, stack0
    li t0, 4096
    addi t1, a0, 1
    mul t0, t0, t1
    add sp, sp, t0

    # 3. If this is Hart 0, tell OpenSBI to start the other Harts
    bnez a0, secondary_entry

    # --- HART 0 ONLY: The Invitation Loop ---
    # We'll try to wake Harts 1, 2, and 3
    li s0, 1                # Target Hart ID starts at 1
    li s1, 4                # Max Harts (matches -smp 4)
wake_loop:
    li a7, 0x48534D         # HSM Extension
    li a6, 0                # Hart Start Function
    mv a0, s0               # Hart to wake
    la a1, _start           # Point them back to this entry code
    li a2, 0                # Opaque value
    ecall
    
    addi s0, s0, 1
    blt s0, s1, wake_loop

secondary_entry:
    # 4. All Harts (Original Hart 0 + newly woken Harts) jump to main
    j main

.section .stack
.align 16
stack0:
    .space 4096 * 4