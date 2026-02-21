use core::mem::offset_of;
use crate::syscall::handle_syscall;
use crate::{TRAMPOLINE, info, warn, error}; // Added new macros
use crate::kalloc::trampoline_start;
use crate::cpu::{set_next_timer_interrupt, CPU_TABLE};
use crate::sched::yield_now;
use riscv::register::{sstatus};

#[repr(C, align(4096))]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /* 0   */ pub regs: [usize; 32],    // All 32 general purpose registers
    /* 256 */ pub kernel_satp: usize,   
    /* 264 */ pub kernel_sp: usize,     
    /* 272 */ pub kernel_trap: usize,   
    /* 280 */ pub epc: usize,           // Note: Now at offset 280
    /* 288 */ pub kernel_hartid: usize, 
    /* 296 */ pub user_satp: usize,
}


use core::arch::global_asm;

// We use global_asm! so we can use the labels uservec and userret 
// anywhere in the kernel, and the compiler will calculate the offsets.
global_asm!(
    ".section .trampoline, \"ax\"",
    ".align 12",
    ".globl trampoline_start", // Make it visible to the rest of the kernel
    "trampoline_start:",
    ".globl uservec",
    "uservec:",
        // a0 is sscratch (TrapFrame VA)
        "csrrw a0, sscratch, a0",
        
        // Save General Purpose Registers
        "sd ra, {ra_off}(a0)",
        "sd sp, {sp_off}(a0)",
        "sd gp, {gp_off}(a0)",
        "sd tp, {tp_off}(a0)",
        "sd t0, {t0_off}(a0)",
        "sd t1, {t1_off}(a0)",
        "sd t2, {t2_off}(a0)",
        "sd s0, {s0_off}(a0)",
        "sd s1, {s1_off}(a0)",
        "sd a1, {a1_off}(a0)",
        "sd a2, {a2_off}(a0)",
        "sd a3, {a3_off}(a0)",
        "sd a4, {a4_off}(a0)",
        "sd a5, {a5_off}(a0)",
        "sd a6, {a6_off}(a0)",
        "sd a7, {a7_off}(a0)",
        "sd s2, {s2_off}(a0)",
        "sd s3, {s3_off}(a0)",
        "sd s4, {s4_off}(a0)",
        "sd s5, {s5_off}(a0)",
        "sd s6, {s6_off}(a0)",
        "sd s7, {s7_off}(a0)",
        "sd s8, {s8_off}(a0)",
        "sd s9, {s9_off}(a0)",
        "sd s10, {s10_off}(a0)",
        "sd s11, {s11_off}(a0)",
        "sd t3, {t3_off}(a0)",
        "sd t4, {t4_off}(a0)",
        "sd t5, {t5_off}(a0)",
        "sd t6, {t6_off}(a0)",

        // Save user a0 (which was swapped into sscratch)
        "csrr t0, sscratch",
        "sd t0, {a0_off}(a0)",

        // Save user EPC
        "csrr t1, sepc",
        "sd t1, {epc_off}(a0)",

        // Load Kernel State
        "ld t1, {k_satp_off}(a0)",
        "ld sp, {k_sp_off}(a0)",
        "ld tp, {k_hartid_off}(a0)",
        "ld t0, {k_trap_off}(a0)",

        // Switch to Kernel Page Table
        "csrw satp, t1",
        "sfence.vma zero, zero",

        // Jump to rust_trap_handler
        "jr t0",
    ".globl userret",
    "userret:",
        // a0 = TrapFrame VA, a1 = user_satp
        "csrw satp, a1",
        "sfence.vma zero, zero",

        // Restore EPC
        "ld t0, {epc_off}(a0)",
        "csrw sepc, t0",

        // Set sscratch for next trap
        "csrw sscratch, a0",

        // Restore registers
        "ld ra, {ra_off}(a0)",
        "ld sp, {sp_off}(a0)",
        "ld gp, {gp_off}(a0)",
        "ld tp, {tp_off}(a0)",
        "ld t0, {t0_off}(a0)",
        "ld t1, {t1_off}(a0)",
        "ld t2, {t2_off}(a0)",
        "ld s0, {s0_off}(a0)",
        "ld s1, {s1_off}(a0)",
        "ld a1, {a1_off}(a0)",
        "ld a2, {a2_off}(a0)",
        "ld a3, {a3_off}(a0)",
        "ld a4, {a4_off}(a0)",
        "ld a5, {a5_off}(a0)",
        "ld a6, {a6_off}(a0)",
        "ld a7, {a7_off}(a0)",
        "ld s2, {s2_off}(a0)",
        "ld s3, {s3_off}(a0)",
        "ld s4, {s4_off}(a0)",
        "ld s5, {s5_off}(a0)",
        "ld s6, {s6_off}(a0)",
        "ld s7, {s7_off}(a0)",
        "ld s8, {s8_off}(a0)",
        "ld s9, {s9_off}(a0)",
        "ld s10, {s10_off}(a0)",
        "ld s11, {s11_off}(a0)",
        "ld t3, {t3_off}(a0)",
        "ld t4, {t4_off}(a0)",
        "ld t5, {t5_off}(a0)",
        "ld t6, {t6_off}(a0)",

        // Restore user a0 last
        "ld a0, {a0_off}(a0)",
        "sret",

    // Mapping Rust TrapFrame offsets to Assembly labels
    k_satp_off   = const offset_of!(TrapFrame, kernel_satp),
    k_sp_off     = const offset_of!(TrapFrame, kernel_sp),
    k_trap_off   = const offset_of!(TrapFrame, kernel_trap),
    k_hartid_off = const offset_of!(TrapFrame, kernel_hartid),
    epc_off      = const offset_of!(TrapFrame, epc),
    
    // Register offsets (Base of regs array + index * 8)
    ra_off  = const offset_of!(TrapFrame, regs) + 8,
    sp_off  = const offset_of!(TrapFrame, regs) + 16,
    gp_off  = const offset_of!(TrapFrame, regs) + 24,
    tp_off  = const offset_of!(TrapFrame, regs) + 32,
    t0_off  = const offset_of!(TrapFrame, regs) + 40,
    t1_off  = const offset_of!(TrapFrame, regs) + 48,
    t2_off  = const offset_of!(TrapFrame, regs) + 56,
    s0_off  = const offset_of!(TrapFrame, regs) + 64,
    s1_off  = const offset_of!(TrapFrame, regs) + 72,
    a0_off  = const offset_of!(TrapFrame, regs) + 80, // x10
    a1_off  = const offset_of!(TrapFrame, regs) + 88,
    a2_off  = const offset_of!(TrapFrame, regs) + 96,
    a3_off  = const offset_of!(TrapFrame, regs) + 104,
    a4_off  = const offset_of!(TrapFrame, regs) + 112,
    a5_off  = const offset_of!(TrapFrame, regs) + 120,
    a6_off  = const offset_of!(TrapFrame, regs) + 128,
    a7_off  = const offset_of!(TrapFrame, regs) + 136,
    s2_off  = const offset_of!(TrapFrame, regs) + 144,
    s3_off  = const offset_of!(TrapFrame, regs) + 152,
    s4_off  = const offset_of!(TrapFrame, regs) + 160,
    s5_off  = const offset_of!(TrapFrame, regs) + 168,
    s6_off  = const offset_of!(TrapFrame, regs) + 176,
    s7_off  = const offset_of!(TrapFrame, regs) + 184,
    s8_off  = const offset_of!(TrapFrame, regs) + 192,
    s9_off  = const offset_of!(TrapFrame, regs) + 200,
    s10_off = const offset_of!(TrapFrame, regs) + 208,
    s11_off = const offset_of!(TrapFrame, regs) + 216,
    t3_off  = const offset_of!(TrapFrame, regs) + 224,
    t4_off  = const offset_of!(TrapFrame, regs) + 232,
    t5_off  = const offset_of!(TrapFrame, regs) + 240,
    t6_off  = const offset_of!(TrapFrame, regs) + 248
);

extern "C" {
    pub fn userret(tf: usize, satp: usize);
}

// src/trap.rs

pub unsafe fn user_trap_return(tf_va: usize, user_satp: usize) -> ! {
    // 1. Turn off interrupts. We are in a sensitive transition state.
    sstatus::clear_sie();

    // 2. Set S-mode Previous Privilege to User (0), so sret goes to User mode
    let mut status = sstatus::read();
    status.set_spp(sstatus::SPP::User); 
    status.set_spie(true); // User mode will start with interrupts enabled
    sstatus::write(status);

    // 3. Get the absolute address of userret in the TRAMPOLINE section
    extern "C" { fn userret(tf: usize, satp: usize); }
    let userret_offset = (userret as *const () as usize) - (crate::kalloc::trampoline_start as *const () as usize);
    let userret_va = TRAMPOLINE + userret_offset;
    
    let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);

    // 4. THE HANDOFF
    // a0 = tf_va (0x3fffffe000)
    // a1 = user_satp (Page Table PPN)
    func(tf_va, user_satp);

    unreachable!()
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_trap_handler(tf: *mut TrapFrame) {
    let tf_ref = unsafe { &mut *tf };
    let scause = riscv::register::scause::read();
    let stval = riscv::register::stval::read();
    let hartid = tf_ref.kernel_hartid; 

    let sstatus = riscv::register::sstatus::read();
    
    // If the trap came from Supervisor mode, we shouldn't be 
    // doing the complex user-mode register saving logic.
    if sstatus.spp() == riscv::register::sstatus::SPP::Supervisor {
        let scause = riscv::register::scause::read();
        let sepc = riscv::register::sepc::read();
        panic!("Trap from Kernel: Cause {:?}, EPC {:#x}", scause.code(), sepc);
    }

    if scause.is_interrupt() {
        if scause.code() == 5 { // Supervisor Timer
            unsafe {
                set_next_timer_interrupt();
                yield_now();
            }
        }
    } else if (scause.bits() & 0xfff) == 8 {
        // Environment call from User mode
        tf_ref.epc += 4; 
        handle_syscall(tf_ref);
    } else {
        // Use the new error! macro for the panic message
        error!("TRAP FATAL: Hart {}", hartid);
        panic!(
            "Cause: {:#x}, EPC: {:#x}, TVAL: {:#x}", 
            scause.bits(), tf_ref.epc, stval
        );
    }

    // --- Return to User Mode Path ---
    
    let user_satp = tf_ref.user_satp;
    
    // We must pass the VIRTUAL address of the TrapFrame (TRAPFRAME constant)
    // to the userret function, because it will be running under the user pagetable.
    unsafe {
        // Ensure sscratch is set to the VIRTUAL address of the trapframe
        // so the next trap knows where to save registers.
        riscv::register::sscratch::write(crate::kalloc::TRAPFRAME);

        let userret_offset = (userret as *const () as usize) - (trampoline_start as *const () as usize);
        let userret_va = TRAMPOLINE + userret_offset;
        
        let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);
        
        // func(TRAPFRAME_VA, USER_SATP)
        func(crate::kalloc::TRAPFRAME, user_satp);
    }
}
