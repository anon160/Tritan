use core::panic::PanicInfo;
// REMOVED: hart_id from the import list
use crate::{println, error, info, warn}; 
use crate::cpu::my_cpu; // Added this to get the anchored CPU info

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 1. Safely try to get the Hart ID from the 'tp' register
    // We use a raw asm check here because if the panic happens BEFORE 
    // cpu_init, my_cpu() would panic again (a double panic).
    let mut tp_val: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) tp_val);
    }
    
    // If tp is a valid-looking pointer, use it. Otherwise, it's likely 
    // still holding the raw ID from OpenSBI (early boot).
    let hart = if tp_val > 0x8000_0000 {
        unsafe { (*(tp_val as *const crate::cpu::Cpu)).hartid }
    } else {
        tp_val // It's just the raw ID
    };

    let scause = riscv::register::scause::read();
    let stval = riscv::register::stval::read();
    let sepc = riscv::register::sepc::read();
    let sstatus = riscv::register::sstatus::read();

    error!("KERNEL PANIC");
    
    info!("Hart ID: {}", hart);
    warn!("Location: {}", info.location().unwrap_or(core::panic::Location::caller()));
    
    // Updated for latest Rust panic_info API
    error!("Message: {}", info.message());

    // Decode Cause
    let code = scause.code();
    let is_interrupt = scause.is_interrupt();
    let cause_str = match (is_interrupt, code) {
        (false, 1) => "Instruction Access Fault",
        (false, 2) => "Illegal Instruction",
        (false, 5) => "Load Access Fault",
        (false, 7) => "Store/AMO Access Fault",
        (false, 8) => "Environment Call (User)",
        (false, 12) => "Instruction Page Fault",
        (false, 13) => "Load Page Fault",
        (false, 15) => "Store Page Fault",
        (true, 1)  => "Software Interrupt (Supervisor)",
        (true, 5)  => "Timer Interrupt (Supervisor)",
        (true, 9)  => "External Interrupt (Supervisor)",
        _ => "Unknown Cause",
    };

    println!("\x1b[34m[TRAP CONTEXT]\x1b[0m");
    println!("  - Cause:  {} ({} : {})", cause_str, if is_interrupt { "Interrupt" } else { "Exception" }, code);
    println!("  - STVAL:  {:#018x}", stval);
    println!("  - SEPC:   {:#018x}", sepc);
    println!("  - STATUS: {:#018x} (SPP={:?})", sstatus.bits(), sstatus.spp());
    
    println!("\x1b[33m[STACK BACKTRACE]\x1b[0m");
    
    unsafe {
        let mut fp: *const usize;
        core::arch::asm!("mv {}, s0", out(reg) fp);

        let mut depth = 0;
        while !fp.is_null() && depth < 10 {
            // Note: This requires -C force-frame-pointers=yes in .cargo/config.toml
            let ra = fp.offset(-1).read();
            let prev_fp = fp.offset(-2).read() as *const usize;

            println!("  \x1b[90m[{}]\x1b[0m ra={:#018x}", depth, ra);

            if prev_fp <= fp || prev_fp.is_null() { break; }
            fp = prev_fp;
            depth += 1;
        }
    }
    
    loop {}
}