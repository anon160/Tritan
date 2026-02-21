#![no_std]
#![no_main]

mod trap;
mod kalloc; 
mod console;
mod syscall;
mod cpu;
mod sched;
mod panic;

extern crate alloc; 

use core::arch::global_asm;
use spin::Mutex;
use talc::{Talck, Talc, Span};
use riscv::register::{stvec::{self, Stvec, TrapMode}, sstatus, sscratch};

// Updated Imports
use crate::cpu::{cpu_init, timer_init, my_cpu}; // Added my_cpu
use crate::sched::{SCHEDULER, ExecMode, Task, scheduler};
use crate::kalloc::{kinit, kalloc, TRAMPOLINE, TRAPFRAME, PGSIZE, kpvminit};
use crate::trap::{TrapFrame, rust_trap_handler};

global_asm!(include_str!("../entry.S"));
global_asm!(include_str!("swtch.S"));

#[global_allocator]
static ALLOCATOR: Talck<Mutex<()>, talc::ErrOnOom> = Talc::new(talc::ErrOnOom).lock();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.main")]
pub extern "C" fn main() -> ! {
    // 1. Get the raw Hart ID from 'tp' (where entry.S put it)
    let id: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) id); }
    
    // 2. Initialize the CPU struct (Anchors 'tp' to the Cpu struct pointer)
    unsafe {
        cpu_init(id); 
    }

    // Now we can safely use the high-level my_cpu() helper
    let cpu = unsafe { my_cpu() };
    let hartid = cpu.id();

    if hartid == 0 {
        kinit();
        let heap_phys = kalloc().expect("Heap alloc failed");
        unsafe {
            let _ = ALLOCATOR.lock().claim(Span::from_base_size(heap_phys, 1024 * 1024));
        }
        info!("Tritan OS: Global Hardware Initialized.");
    }
    

    unsafe {
        kpvminit();             
        setup_trap_vectors();  
        timer_init();           
    }

    if hartid == 0 {
        let kernel_satp = (8usize << 60) | (unsafe { crate::kalloc::KERNEL_BOOT_PT.entries.as_ptr() as usize >> 12 });
        
        let (user_satp, tf_ptr) = unsafe { setup_user_test(kernel_satp) };
        
        let first_task = Task::spawn(
            "init_task1", 
            ExecMode::Sync, 
            tf_ptr,      
            user_satp    
        );
        let sec_task = Task::spawn(
            "init_task2", 
            ExecMode::Sync, 
            tf_ptr,      
            user_satp    
        );
        SCHEDULER.lock().run_queue.insert(sec_task.vruntime, sec_task);
        SCHEDULER.lock().run_queue.insert(first_task.vruntime, first_task);
        info!("Initial tasks spawned into B-Tree.");
    }
    info!("Hart {} entering scheduler...", hartid);
    scheduler();
}



unsafe fn setup_user_test(k_satp: usize) -> (usize, *mut TrapFrame) {
    use crate::kalloc::{uvmcreate, uvmmapcode, mappages, PTE_R, PTE_W, PTE_U, PTE_X};
    
    let trapframe_ptr = kalloc().expect("Failed to allocate trapframe") as *mut TrapFrame;
    let trapframe_pa = trapframe_ptr as usize;

    // 1. Create the page table
    let user_pt = uvmcreate(trapframe_pa);

    // --- THE CRITICAL MISSING PIECES ---
    
    // 2. Map the TRAMPOLINE (The code that handles the switch)
    // It MUST be at the same virtual address (0x4000000000) as in the kernel.
    extern "C" { fn trampoline_start(); }
    mappages(
        user_pt, 
        TRAMPOLINE, 
        trampoline_start as usize, 
        PGSIZE, 
        (PTE_R | PTE_X) as u64 // Read + Execute
    );

    // 3. Map the TRAPFRAME (Where registers are saved)
    // It MUST be mapped at the TRAPFRAME virtual address.
    mappages(
        user_pt, 
        TRAPFRAME, 
        trapframe_pa, 
        PGSIZE, 
        (PTE_R | PTE_W) as u64 // Read + Write
    );
    
    mappages(
        user_pt, 
        TRAPFRAME,          // 0x3fffffe000
        trapframe_pa,       // Physical address
        PGSIZE, 
        (PTE_R | PTE_W) as u64 // Read + Write
    );
    // --- END OF CRITICAL FIXES ---

    let user_satp = (8usize << 60) | ((user_pt as usize) >> 12);
    core::ptr::write_bytes(trapframe_ptr as *mut u8, 0, PGSIZE);
    
    let kstack = kalloc().expect("KStack alloc failed");
    (*trapframe_ptr).kernel_satp = k_satp;
    (*trapframe_ptr).kernel_sp = kstack as usize + PGSIZE; 
    (*trapframe_ptr).kernel_trap = rust_trap_handler as *const () as usize;
    (*trapframe_ptr).epc = 0x1000; 
    (*trapframe_ptr).user_satp = user_satp;
    (*trapframe_ptr).regs[2] = 0x8000_0000; 

    uvmmapcode(user_pt, 0x1000, user_code as *const u8, PGSIZE);
    
    let user_stack = kalloc().expect("User stack alloc failed");
    mappages(user_pt, 0x8000_0000 - PGSIZE, user_stack as usize, PGSIZE, (PTE_R | PTE_W | PTE_U) as u64);

    (user_satp, trapframe_ptr) 
}

fn setup_trap_vectors() {
    unsafe {
        // TRAPFRAME is the Virtual Address 0x3fffffe000
        sscratch::write(TRAPFRAME);
        stvec::write(Stvec::new(TRAMPOLINE, TrapMode::Direct));
    }
}

#[unsafe(no_mangle)]
pub fn user_code() -> ! {
    unsafe {
        core::arch::asm!(
            ".align 4",
            "lla a0, 2f",
            "li a7, 2",
            "ecall",
            "1: j 1b",
            "2: .string \"Hello from Tritan OS with Paging!\\n\"",
            options(noreturn)
        );
    }
}