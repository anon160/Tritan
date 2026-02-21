use crate::syscall::handle_syscall;
use crate::{USER_SATP, TRAMPOLINE, TRAPFRAME};
use crate::kalloc::trampoline_start;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    pub kernel_satp: usize,   
    pub kernel_sp: usize,     
    pub kernel_trap: usize,   
    pub kernel_hartid: usize, 
    pub epc: usize,           
    pub user_satp: usize,
    pub regs: [usize; 32],
}

// We must declare that userret exists in our assembly
extern "C" {
    pub fn userret(tf: usize, satp: usize);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_trap_handler(tf: *mut TrapFrame) {
    let tf_ref = unsafe { &mut *tf };
    let scause = riscv::register::scause::read();

    if !scause.is_interrupt() && (scause.bits() & 0xfff) == 8 {
        tf_ref.epc += 4; 
        handle_syscall(tf_ref);
    } else {
        panic!(
            "Unexpected Trap: cause={:#x}, epc={:#x}, tval={:#x}", 
            scause.bits(), 
            tf_ref.epc,
            riscv::register::stval::read()
        );
    }

    let user_satp = *USER_SATP.lock();
    unsafe {
        let userret_offset = (userret as *const () as usize) - (trampoline_start as *const () as usize);
        let userret_va = TRAMPOLINE + userret_offset;
        
        // Use transmute to turn the calculated address into a callable function
        let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);
        func(TRAPFRAME, user_satp);
    }
}