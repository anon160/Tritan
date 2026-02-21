#![no_std]
#![no_main]

mod trap;
mod kalloc; 
mod console;
mod syscall;

extern crate alloc; 

use core::{arch::global_asm, panic::PanicInfo};
use spin::Mutex;
use talc::{Talck, Talc, Span};
use riscv::register::{stvec::{self, Stvec, TrapMode}, sstatus, sscratch, mhartid};

// Import constants
use crate::kalloc::{kinit, kalloc, TRAMPOLINE, TRAPFRAME, PGSIZE, kmap, kpvminit};
use crate::trap::{TrapFrame, rust_trap_handler};

global_asm!(include_str!("trampoline.S"));
global_asm!(include_str!("../entry.S"));

#[global_allocator]
static ALLOCATOR: Talck<Mutex<()>, talc::ErrOnOom> = Talc::new(talc::ErrOnOom).lock();

pub static USER_SATP: Mutex<usize> = Mutex::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    // --- Phase 1: Physical Foundations ---
    kinit();
    
    // Initialize Heap
    let heap_phys = kalloc().expect("Failed to allocate heap");
    unsafe {
        let _ = ALLOCATOR.lock().claim(Span::from_base_size(heap_phys, 1024 * 1024));
    }
    
    println!("Tritan OS: Basic Hardware Initialized.");

    // --- Phase 2: Virtual Memory ---
    // We move the messy paging code to vm.rs
    let kernel_satp = unsafe { kpvminit() };
    println!("Virtual Memory: Enabled.");

    // --- Phase 3: Trap & User Setup ---
    setup_trap_vectors();
    
    // Temporary: Still manual user entry for now until Scheduler is ready
    let user_satp = unsafe { setup_user_test(kernel_satp) };
    *USER_SATP.lock() = user_satp;

    println!("Dropping to User Mode...");
    unsafe { drop_to_user(user_satp); }
}

fn setup_trap_vectors() {
    unsafe {
        stvec::write(Stvec::new(TRAMPOLINE, TrapMode::Direct));
        sscratch::write(TRAPFRAME);
    }
}

unsafe fn drop_to_user(satp_val: usize) -> ! {
    let mut s = sstatus::read();
    s.set_spp(sstatus::SPP::User); 
    s.set_spie(true);
    sstatus::write(s);

    core::arch::asm!("fence rw, rw", "sfence.vma zero, zero");

    extern "C" { fn userret(tf: usize, satp: usize); }
    let userret_offset = (userret as *const () as usize) - (crate::kalloc::trampoline_start as *const () as usize);
    let userret_va = TRAMPOLINE + userret_offset;
    let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);
    
    func(TRAPFRAME, satp_val);
    loop {}
}
// Helper for the user test code you currently have
unsafe fn setup_user_test(k_satp: usize) -> usize {
    use crate::kalloc::{uvmcreate, uvmmapcode, mappages, PTE_R, PTE_W, PTE_U};
    
    let tf_ptr = kalloc().expect("TF alloc failed") as *mut TrapFrame;
    let kstack = kalloc().expect("KStack alloc failed");
    let user_stack = kalloc().expect("User stack alloc failed");
    
    // FIX: Cast (PTE_R | PTE_W) to usize
    kmap(TRAPFRAME, tf_ptr as usize, PGSIZE, (PTE_R | PTE_W) as usize);

    let user_pt = uvmcreate();
    let user_satp = (8usize << 60) | ((user_pt as usize) >> 12);

    core::ptr::write_bytes(tf_ptr as *mut u8, 0, PGSIZE);
    (*tf_ptr).kernel_satp = k_satp;
    (*tf_ptr).kernel_sp = kstack as usize + PGSIZE; 
    (*tf_ptr).kernel_trap = rust_trap_handler as *const () as usize;
    (*tf_ptr).epc = 0x1000; 
    (*tf_ptr).user_satp = user_satp;
    (*tf_ptr).regs[2] = 0x8000_0000; // SP

    uvmmapcode(user_pt, 0x1000, user_code as *const u8, PGSIZE);
    mappages(user_pt, 0x8000_0000 - PGSIZE, user_stack as usize, PGSIZE, (PTE_R | PTE_W | PTE_U) as u64);
    mappages(user_pt, TRAPFRAME, tf_ptr as usize, PGSIZE, (PTE_R | PTE_W) as u64);
    
    user_satp
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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let hart = mhartid::read();
    println!("\n\x1b[31;1m!!!!!!!!!!!!!!!! KERNEL PANIC !!!!!!!!!!!!!!!!\x1b[0m");
    println!("\x1b[33mHart ID:\x1b[0m {}", hart);
    println!("\x1b[33mLocation:\x1b[0m {}", info.location().unwrap_or(core::panic::Location::caller()));
    println!("\x1b[33mMessage:\x1b[0m {}", info.message());
    
    // Capture some register state for context
    let scause = riscv::register::scause::read();
    let stval = riscv::register::stval::read();
    let sepc = riscv::register::sepc::read();
    
    println!("\x1b[34mTrap Context:\x1b[0m scause={:#x}, stval={:#x}, sepc={:#x}", scause.bits(), stval, sepc);
    println!("\x1b[31;1m!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\x1b[0m\n");
    
    loop {}
}