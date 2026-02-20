use buddy_system_allocator::LockedHeap;
use core::ptr::{addr_of, write_bytes};

pub static ALLOCATOR: LockedHeap<32> = LockedHeap::<32>::new();

pub const PGSIZE: usize = 4096;
pub const PHYSTOP: usize = 0x80000000 + (120 * 1024 * 1024);

// --- Sv39 Page Table Entry Bits ---
pub const PTE_V: u64 = 1 << 0; 
pub const PTE_R: u64 = 1 << 1; 
pub const PTE_W: u64 = 1 << 2; 
pub const PTE_X: u64 = 1 << 3; 
pub const PTE_U: u64 = 1 << 4; 
pub const PTE_A: u64 = 1 << 6; 
pub const PTE_D: u64 = 1 << 7;

pub const TRAMPOLINE: usize = (1 << 38) - PGSIZE;
pub const TRAPFRAME: usize = TRAMPOLINE - PGSIZE;

unsafe extern "C" {
    static mut __ebss: u8;
    pub fn trampoline_start(); 
}

// --- ALLOCATION HELPERS ---

pub fn kinit() {
    let start = addr_of!(__ebss) as usize;
    let page_aligned_start = (start + PGSIZE - 1) & !(PGSIZE - 1);
    let size = PHYSTOP - page_aligned_start;

    unsafe {
        ALLOCATOR.lock().init(page_aligned_start, size);
    }
}

pub fn kalloc() -> Option<*mut u8> {
    let layout = core::alloc::Layout::from_size_align(PGSIZE, PGSIZE).ok()?;
    ALLOCATOR.lock().alloc(layout).ok().map(|ptr| {
        let p = ptr.as_ptr();
        unsafe { write_bytes(p, 0, PGSIZE); }
        p
    })
}

// --- PAGING LOGIC ---

pub unsafe fn walk(pagetable: *mut u64, va: usize, alloc: bool) -> Option<*mut u64> {
    let mut table = pagetable;
    for level in (1..3).rev() {
        let idx = (va >> (12 + level * 9)) & 0x1FF;
        let pte_ptr = table.add(idx);
        let pte = *pte_ptr;

        if (pte & PTE_V) != 0 {
            table = ((pte >> 10) << 12) as *mut u64;
        } else {
            if !alloc { return None; }
            let new_page = kalloc().expect("Walk: kalloc failed");
            
            // Non-leaf (directory) entries should only have PTE_V.
            *pte_ptr = ((new_page as u64 >> 12) << 10) | PTE_V;
            table = new_page as *mut u64;
        }
    }
    Some(table.add((va >> 12) & 0x1FF))
}

pub unsafe fn mappages(pagetable: *mut u64, va: usize, pa: usize, size: usize, perm: u64) {
    let mut curr_va = va & !(PGSIZE - 1);
    let last_va = (va.wrapping_add(size).wrapping_sub(1)) & !(PGSIZE - 1);
    let mut curr_pa = pa & !(PGSIZE - 1);

    loop {
        let pte = walk(pagetable, curr_va, true).expect("mappages: walk failed");
        
        // Leaf nodes need V, A, D, and the requested permissions.
        *pte = ((curr_pa as u64 >> 12) << 10) | perm | PTE_V | PTE_A | PTE_D;

        if curr_va == last_va { break; }
        curr_va = curr_va.wrapping_add(PGSIZE);
        curr_pa = curr_pa.wrapping_add(PGSIZE);
    }
}

// --- PAGE TABLE CONSTRUCTORS ---
pub unsafe fn uvmcreate() -> *mut u64 {
    let root = kalloc().expect("Failed user root PT") as *mut u64;

    // Map Trampoline and UART for kernel-mode operations while on user page table
    mappages(root, TRAMPOLINE, trampoline_start as *const () as usize, PGSIZE, PTE_R | PTE_X);
    mappages(root, 0x1000_0000, 0x1000_0000, PGSIZE, PTE_R | PTE_W);

    // Identity map kernel RAM so the CPU doesn't fault during trap entry/exit
    let kernel_start = 0x8000_0000;
    mappages(root, kernel_start, kernel_start, 1024 * 1024 * 32, PTE_R | PTE_W | PTE_X);

    root
}

pub unsafe fn uvmmapcode(pagetable: *mut u64, va: usize, src: *const u8, len: usize) {
    if len > PGSIZE { panic!("uvmmapcode: code too large"); }
    let mem = kalloc().expect("uvmmapcode: kalloc failed");
    core::ptr::copy_nonoverlapping(src, mem, len);
    
    // User code must have PTE_U to be executable in U-mode
    mappages(pagetable, va, mem as usize, PGSIZE, PTE_R | PTE_X | PTE_U);
}

pub unsafe fn walk_addr(pagetable: *mut u64, va: usize) -> usize {
    let mut table = pagetable;
    let va_u64 = va as u64; // Convert once for easier math

    // Sv39 has 3 levels: VPN[2], VPN[1], VPN[0]
    for level in (1..=2).rev() {
        let vpn = ((va_u64 >> (12 + 9 * level)) & 0x1FF) as usize;
        let pte = *table.add(vpn);

        if (pte & 1) == 0 { 
            return 0; // Page not present
        }

        // Extract PPN from PTE and move to next level
        table = ((pte >> 10) << 12) as *mut u64;
    }

    let vpn0 = ((va_u64 >> 12) & 0x1FF) as usize;
    let pte = *table.add(vpn0);
    
    if (pte & 1) == 0 { return 0; }

    // Math: (PPN << 12) | (Page Offset)
    let pa = ((pte >> 10) << 12) | (va_u64 & 0xFFF);
    
    pa as usize // Cast back to usize for the return
}