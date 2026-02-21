// src/cpu.rs
use crate::sched::{Context, Task};
use alloc::boxed::Box;
use riscv::register::sie;

pub struct Cpu {
    pub scheduler_context: Context, 
    pub current_task: Option<Box<Task>>,
    pub hartid: usize, // The logical ID stored in kernel memory
    pub started: bool,
}

impl Cpu {
    /// Returns the Hart ID of this specific CPU struct
    pub fn id(&self) -> usize {
        self.hartid
    }
}

pub const MAX_CPUS: usize = 8; 
pub const TIMER_INTERVAL: u64 = 1_000_000; 

pub struct CpuTable {
    pub cpus: [Cpu; MAX_CPUS],
    pub ncpu: usize,
}

pub static mut CPU_TABLE: CpuTable = CpuTable {
    cpus: [const { Cpu {
        scheduler_context: Context::new(),
        current_task: None,
        hartid: 0,
        started: false,
    } }; MAX_CPUS],
    ncpu: 0,
};

/// The "Anchor": Returns a reference to the current Hart's Cpu struct 
/// by reading the pointer we saved in the 'tp' register.
pub unsafe fn my_cpu() -> &'static mut Cpu {
    let ptr: *mut Cpu;
    // We use 'tp' (Thread Pointer) as our constant reference to the local Cpu struct
    core::arch::asm!("mv {}, tp", out(reg) ptr);
    
    // Safety check: if tp is null, the kernel hasn't initialized this Hart's pointer yet.
    if ptr.is_null() {
        panic!("my_cpu() called before cpu_init on Hart!");
    }
    
    &mut *ptr
}

/// New get_cpu_id: No longer asks the hardware. It asks the struct.
pub unsafe fn get_cpu_id() -> usize {
    my_cpu().id()
}

/// Initializes the CPU struct and anchors it to the hardware 'tp' register.
/// 'hard_id' should be the raw ID passed from OpenSBI in a0.
pub unsafe fn cpu_init(hard_id: usize) {
    if hard_id < MAX_CPUS {
        let cpu_ptr = &raw mut CPU_TABLE.cpus[hard_id];
        
        // 1. Store the pointer to THIS Hart's struct in 'tp'
        // This is the most critical line for stabilizing the "mess".
        core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        
        // 2. Initialize the struct fields
        (*cpu_ptr).hartid = hard_id;
        (*cpu_ptr).started = true;
        
        // 3. Update the global CPU count safely
        if hard_id + 1 > CPU_TABLE.ncpu {
            CPU_TABLE.ncpu = hard_id + 1;
        }
    } else {
        panic!("Hart ID {} exceeds MAX_CPUS!", hard_id);
    }
}

pub unsafe fn timer_init() {
    sie::set_stimer();
    set_next_timer_interrupt();
}

pub unsafe fn set_next_timer_interrupt() {
    let current_time = riscv::register::time::read64();
    let next_tick = current_time + TIMER_INTERVAL;
    sbi_rt::set_timer(next_tick);
}