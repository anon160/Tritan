use crate::{print, syscall, println, TrapFrame};

pub fn handle_print_str(user_satp: usize, va: usize) {
    // Convert the satp value (which contains the PPN) into a raw pointer
    // We mask out the MODE bits (top 4 bits) and shift the PPN back to an address
    let root_pt = ((user_satp & 0x000F_FFFF_FFFF_FFFF) << 12) as *mut u64;

    unsafe {
        let mut i = 0;
        loop {
            let pa = crate::kalloc::walk_addr(root_pt, va + i);
            if pa == 0 { break; }
            
            let c = *(pa as *const u8);
            if c == 0 { break; } // Stop at null terminator
            
            print!("{}", c as char);
            i += 1;
        }
    }
}

// 2. UPDATE SYSCALL HANDLER FOR REGISTER ARRAY
pub fn handle_syscall(tf: &mut TrapFrame) {
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