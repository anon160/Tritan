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
use riscv::register::{stvec::{self, Stvec, TrapMode}, sstatus, sscratch, satp};
use crate::syscall::handle_print_str;

// Import helpers and constants
use crate::kalloc::{
    kinit, kalloc, uvmcreate, uvmmapcode, mappages, 
    TRAMPOLINE, TRAPFRAME, PTE_R, PTE_W, PTE_X, PGSIZE, PTE_U
};
use crate::trap::TrapFrame;

// --- STATIC BOOT STRUCTURES ---
#[repr(align(4096))]
struct PageTable {
    entries: [u64; 512],
}

#[unsafe(link_section = ".data.boot_pt")]
static mut KERNEL_BOOT_PT: PageTable = PageTable {
    entries: [0; 512],
};

unsafe extern "C" {
    pub fn uservec();
    pub fn userret(tf: usize, satp: usize);
    pub fn trampoline_start(); 
}

global_asm!(include_str!("trampoline.S"));
global_asm!(include_str!("../entry.S"));

#[global_allocator]
static ALLOCATOR: Talck<Mutex<()>, talc::ErrOnOom> = Talc::new(talc::ErrOnOom).lock();

static USER_SATP: Mutex<usize> = Mutex::new(0);

#[unsafe(no_mangle)]
pub fn user_code() -> ! {
    unsafe {
        core::arch::asm!(
            ".align 4",
            "lla a0, 2f",      // Load address of the string below into a0
            "li a7, 2",        // Syscall ID 2 (Print String)
            "ecall",
            "1: j 1b",         // Loop forever
            "2: .string \"Hello from Tritan OS with Paging!\\n\"",
            options(noreturn)
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    kinit();
    println!("Tritan OS: Booting...");

    // 1. Initialize Heap
    let heap_phys = kalloc().expect("Failed to allocate heap");
    unsafe {
        let _ = ALLOCATOR.lock().claim(Span::from_base_size(heap_phys, 1024 * 1024));
    }

    // 2. Setup Kernel Page Table
    let kernel_pagetable = unsafe {
        let root = &raw mut KERNEL_BOOT_PT.entries as *mut u64;
        core::ptr::write_bytes(root as *mut u8, 0, 4096);
        
        mappages(root, 0x1000_0000, 0x1000_0000, PGSIZE, PTE_R | PTE_W);
        mappages(root, 0x8000_0000, 0x8000_0000, 1024 * 1024 * 32, PTE_R | PTE_W | PTE_X);
        mappages(root, TRAMPOLINE, trampoline_start as usize, PGSIZE, PTE_R | PTE_X);
        
        root
    };

    let kernel_satp_val = (8usize << 60) | ((kernel_pagetable as usize) >> 12);
    
    unsafe {
        let mut s = sstatus::read();
        s.set_sum(true); 
        sstatus::write(s);

        core::arch::asm!("fence rw, rw", "sfence.vma zero, zero");
        satp::write(satp::Satp::from_bits(kernel_satp_val));
        core::arch::asm!("sfence.vma zero, zero", "fence.i");
    }
    
    println!("Virtual Memory Enabled.");

    // 3. Setup User Environment
    let tf_ptr = kalloc().expect("TF alloc failed") as *mut TrapFrame;
    let kstack = kalloc().expect("KStack alloc failed");
    let user_stack = kalloc().expect("User stack alloc failed");
    
    unsafe {
        let root = &raw mut KERNEL_BOOT_PT.entries as *mut u64;
        mappages(root, TRAPFRAME, tf_ptr as usize, PGSIZE, PTE_R | PTE_W);
    }

    let user_pagetable_va = unsafe { uvmcreate() };
    let user_satp_val = (8usize << 60) | ((user_pagetable_va as usize) >> 12);
    *USER_SATP.lock() = user_satp_val;

    unsafe { 
        core::ptr::write_bytes(tf_ptr as *mut u8, 0, PGSIZE);
        (*tf_ptr).kernel_satp = kernel_satp_val;
        (*tf_ptr).kernel_sp = kstack as usize + PGSIZE; 
        (*tf_ptr).kernel_trap = rust_trap_handler as usize;
        (*tf_ptr).epc = 0x1000; 
        (*tf_ptr).user_satp = user_satp_val;

        // Set User Stack Pointer (sp = regs[2])
        let stack_top = 0x8000_0000;
        (*tf_ptr).regs[2] = stack_top;

        // Map code
        uvmmapcode(user_pagetable_va, 0x1000, user_code as *const u8, PGSIZE);
        // Map user stack
        mappages(user_pagetable_va, stack_top - PGSIZE, user_stack as usize, PGSIZE, PTE_R | PTE_W | PTE_U);
        // Map TrapFrame (Supervisor only)
        mappages(user_pagetable_va, TRAPFRAME, tf_ptr as usize, PGSIZE, PTE_R | PTE_W);
    };

    println!("User SATP: {:#x}. Dropping to User Mode...", user_satp_val);

    unsafe {
        let mut s = sstatus::read();
        s.set_spp(sstatus::SPP::User); 
        s.set_spie(true);
        sstatus::write(s);

        stvec::write(Stvec::new(TRAMPOLINE, TrapMode::Direct));
        sscratch::write(TRAPFRAME);

        core::arch::asm!("fence rw, rw", "sfence.vma zero, zero");

        let userret_offset = (userret as usize) - (trampoline_start as usize);
        let userret_va = TRAMPOLINE + userret_offset;
        let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);
        
        func(TRAPFRAME, user_satp_val);
    }

    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_trap_handler(tf: *mut TrapFrame) {
    let tf_ref = unsafe { &mut *tf };
    let scause = riscv::register::scause::read();

    // Cause 8 is User ECALL
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
        let userret_offset = (userret as usize) - (trampoline_start as usize);
        let userret_va = TRAMPOLINE + userret_offset;
        let func: extern "C" fn(usize, usize) = core::mem::transmute(userret_va);
        func(TRAPFRAME, user_satp);
    }
}

// 2. UPDATE SYSCALL HANDLER FOR REGISTER ARRAY
fn handle_syscall(tf: &mut TrapFrame) {
    let id = tf.regs[17]; // a7
    match id {
        1 => {
            let val = tf.regs[10]; // a0
            println!("[USER CHAR]: {}", val as u8 as char);
        },
        2 => {
            let string_ptr = tf.regs[10]; // a0 now holds the address
            handle_print_str(tf.user_satp, string_ptr);
        },
        _ => println!("Unknown syscall: {}", id),
    }
}

unsafe fn debug_pt(root: *mut u64) {
    println!("--- Root Page Table Contents ---");
    for i in 0..512 {
        let pte = *root.add(i);
        if pte & 1 != 0 {
            println!("  [{:3}]: {:#018x}", i, pte);
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\x1b[31m!!! KERNEL PANIC !!!\x1b[0m\n{}", info);
    loop {}
}