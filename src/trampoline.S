.section .trampoline, "ax"
.align 12
.globl trampoline_start
trampoline_start:

.globl uservec
uservec:
    # 1. Swap user a0 with sscratch. 
    # a0 = TrapFrame pointer (VA), sscratch = user a0
    csrrw a0, sscratch, a0

    # 2. Save all user registers
    # We start at 48 because: 
    # 0-39 = Header (satp, sp, trap, hartid, epc)
    # 40 = user_satp
    # 48 = ra (regs[1])
    sd ra, 48(a0)
    sd sp, 56(a0)
    sd gp, 64(a0)
    sd tp, 72(a0)
    sd t0, 80(a0)
    sd t1, 88(a0)
    sd t2, 96(a0)
    sd s0, 104(a0)
    sd s1, 112(a0)
    # user a0 is currently in sscratch, we'll save it to 128(a0) later
    sd a1, 136(a0)
    sd a2, 144(a0)
    sd a3, 152(a0)
    sd a4, 160(a0)
    sd a5, 168(a0)
    sd a6, 176(a0)
    sd a7, 184(a0)
    sd s2, 192(a0)
    sd s3, 200(a0)
    sd s4, 208(a0)
    sd s5, 216(a0)
    sd s6, 224(a0)
    sd s7, 232(a0)
    sd s8, 240(a0)
    sd s9, 248(a0)
    sd s10, 256(a0)
    sd s11, 264(a0)
    sd t3, 272(a0)
    sd t4, 280(a0)
    sd t5, 288(a0)
    sd t6, 296(a0)

    # 3. Save the original user a0 (from sscratch) into regs[10]
    csrr t0, sscratch
    sd t0, 128(a0)

    # 4. Save the user EPC
    csrr t1, sepc
    sd t1, 32(a0)

    # 5. Load Kernel State from TrapFrame
    ld t1, 0(a0)      # kernel_satp
    ld sp, 8(a0)      # kernel_sp
    ld t0, 16(a0)     # kernel_trap (rust_trap_handler)
    ld tp, 24(a0)     # kernel_hartid

    # 6. Switch to Kernel Page Table
    csrw satp, t1
    sfence.vma zero, zero

    # 7. Jump to Rust handler
    jr t0

.globl userret
userret:
    # a0 = TRAPFRAME (VA), a1 = User SATP
    
    # 1. Switch to User Page Table
    csrw satp, a1
    sfence.vma zero, zero

    # 2. Restore User EPC
    ld t0, 32(a0)
    csrw sepc, t0

    # 3. Set sscratch to TrapFrame for the next trap
    csrw sscratch, a0

    # 4. Restore General Purpose Registers (except a0)
    ld ra, 48(a0)
    ld sp, 56(a0)
    ld gp, 64(a0)
    ld tp, 72(a0)
    ld t0, 80(a0)
    ld t1, 88(a0)
    ld t2, 96(a0)
    ld s0, 104(a0)
    ld s1, 112(a0)
    ld a1, 136(a0)
    ld a2, 144(a0)
    ld a3, 152(a0)
    ld a4, 160(a0)
    ld a5, 168(a0)
    ld a6, 176(a0)
    ld a7, 184(a0)
    ld s2, 192(a0)
    ld s3, 200(a0)
    ld s4, 208(a0)
    ld s5, 216(a0)
    ld s6, 224(a0)
    ld s7, 232(a0)
    ld s8, 240(a0)
    ld s9, 248(a0)
    ld s10, 256(a0)
    ld s11, 264(a0)
    ld t3, 272(a0)
    ld t4, 280(a0)
    ld t5, 288(a0)
    ld t6, 296(a0)

    # 5. Restore user a0 last
    ld a0, 128(a0)

    # 6. Back to User Mode
    sret