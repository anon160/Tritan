use crate::{trap::TrapFrame, kalloc::{kalloc, PGSIZE}};
use crate::cpu::{my_cpu, CPU_TABLE}; // Import our new anchor
use alloc::string::String;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use spin::Mutex;
use riscv::register::{sscratch, sstatus};

#[repr(C)]
pub struct Context {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 12],
}

impl Context {
    pub const fn new() -> Self {
        Self { ra: 0, sp: 0, s: [0; 12] }
    }
}

pub enum TaskState {
    Running,
    Runnable,
    Sleeping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    Async,
    Sync,
    SingleThreaded(usize), 
}

pub struct Task {
    pub name: String,
    pub state: TaskState,
    pub metadata: ExecMode, 
    pub vruntime: u64,             
    pub trapframe: *mut TrapFrame, 
    pub context: Context,          
    pub kstack: usize,             
    pub pagetable: usize,          
}

// CRITICAL: We tell Rust it is safe to move Tasks between Harts.
// Since we wrap the B-Tree in a Mutex, this is sound.
unsafe impl Send for Task {}
unsafe impl Sync for Task {}

extern "C" {
    fn swtch(old: *mut Context, new: *mut Context);
}

pub struct Scheduler {
    pub run_queue: BTreeMap<u64, Box<Task>>,
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler {
    run_queue: BTreeMap::new(),
});

// Inside sched.rs
impl Task {
    pub fn spawn(name: &str, mode: ExecMode, tf: *mut TrapFrame, satp: usize) -> Box<Self> {
        let kstack_ptr = kalloc().expect("failed to alloc kstack");
        let kstack_va = kstack_ptr as usize;

        let mut context = Context::new();
        context.ra = forkret as usize;
        context.sp = kstack_va + PGSIZE;

        Box::new(Self {
            name: String::from(name),
            state: TaskState::Runnable,
            metadata: mode,
            vruntime: 0, 
            trapframe: tf,   // Use the passed TrapFrame
            context,
            kstack: kstack_va,
            pagetable: satp, // Use the passed SATP
        })
    }
}

impl Scheduler {
    /// This method must be public and associated directly with the Scheduler struct
    pub fn pick_next_key(&self, current_hart: usize) -> Option<u64> {
        for (&vruntime, task) in self.run_queue.iter() {
            match task.metadata {
                // If task is pinned to a different Hart, skip it
                ExecMode::SingleThreaded(h) if h != current_hart => continue,
                _ => return Some(vruntime),
            }
        }
        None
    }
}

/// The Idle loop for every CPU. It searches the B-Tree for work.
pub fn scheduler() -> ! {
    // Use the 'tp' anchor to get our CPU struct
    let cpu = unsafe { crate::cpu::my_cpu() };
    
    // Get the actual integer ID from the struct, NOT the raw register value
    let hartid = cpu.id(); 
    
    loop {
        unsafe { sstatus::set_sie(); }

        let mut sched_guard = SCHEDULER.lock();
        
        // Use the guard directly or dereference it
        if let Some(task_key) = sched_guard.pick_next_key(hartid) {
            let mut task = sched_guard.run_queue.remove(&task_key).expect("Task disappeared");
            drop(sched_guard); // Release early to avoid deadlocks

            task.state = TaskState::Running;
            
            unsafe {
                // Update the CPU struct we already have a reference to!
                cpu.current_task = Some(task);
                let task_ref = cpu.current_task.as_mut().unwrap();
                
                // Write the VIRTUAL ADDRESS of the TrapFrame to sscratch
                sscratch::write(task_ref.trapframe as usize);
                              // Perform the switch
                swtch(&mut cpu.scheduler_context, &mut task_ref.context);
                
                // Clear sscratch when we return from the task
                sscratch::write(0);
            }
        } else {
            drop(sched_guard);
        }
        
        unsafe { core::arch::asm!("wfi"); } 
    }
}
pub fn yield_now() {
    unsafe {
        // Use the anchor instead of global indexing
        let cpu = my_cpu();

        if let Some(mut task) = cpu.current_task.take() {
            task.vruntime += 1; 
            task.state = TaskState::Runnable;

            let task_ctx_ptr = &mut task.context as *mut Context;

            let mut sched = SCHEDULER.lock();
            sched.run_queue.insert(task.vruntime, task);
            drop(sched); 

            // Switch back to the scheduler loop
            swtch(task_ctx_ptr, &mut cpu.scheduler_context);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn forkret() {
    unsafe {
        SCHEDULER.force_unlock();
        
        // Use the anchor to find the current task's trapframe
        let cpu = my_cpu();
        let task = cpu.current_task.as_mut().expect("No task in forkret");
        // Jump to trap return logic
        crate::trap::user_trap_return(task.trapframe as usize, task.pagetable);
    }
}