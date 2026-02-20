

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    // Kernel State (filled during process creation)
    pub kernel_satp: usize,   // Kernel Page Table
    pub kernel_sp: usize,     // Kernel Stack Pointer
    pub kernel_trap: usize,   // Rust Trap Handler address
    pub kernel_hartid: usize, // ID of this CPU
    
    // User State (saved/restored during trap)
    pub epc: usize,           // 32: Saved User Program Counter
    pub user_satp: usize,
   pub regs: [usize; 32],
}

