use buddy_system_allocator::LockedHeap;
use core::ptr::{addr_of, write_bytes};
use crate::dtb; 

pub static ALLOCATOR: LockedHeap<32> = LockedHeap::<32>::new();

pub const PGSIZE: usize = 4096;

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
    
    // Dynamically calculate PHYSTOP from DTB
    let phystop = dtb::get_phystop();
    let size = phystop - page_aligned_start;

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
    for level in (1..=2).rev() {
        let idx = (va >> (12 + level * 9)) & 0x1FF;
        let pte_ptr = table.add(idx);
        let pte = *pte_ptr;

        if (pte & PTE_V) != 0 {
            table = ((pte >> 10) << 12) as *mut u64;
        } else {
            if !alloc { return None; }
            let new_page = kalloc().expect("Walk: kalloc failed");
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
        *pte = ((curr_pa as u64 >> 12) << 10) | perm | PTE_V | PTE_A | PTE_D;

        if curr_va == last_va { break; }
        curr_va = curr_va.wrapping_add(PGSIZE);
        curr_pa = curr_pa.wrapping_add(PGSIZE);
    }
}

pub unsafe fn walk_addr(pagetable: *mut u64, va: usize) -> usize {
    let mut table = pagetable;
    let va_u64 = va as u64;

    for level in (1..=2).rev() {
        let vpn = ((va_u64 >> (12 + 9 * level)) & 0x1FF) as usize;
        let pte = *table.add(vpn);
        if (pte & PTE_V) == 0 { return 0; }
        table = ((pte >> 10) << 12) as *mut u64;
    }

    let vpn0 = ((va_u64 >> 12) & 0x1FF) as usize;
    let pte = *table.add(vpn0);
    if (pte & PTE_V) == 0 { return 0; }
    (((pte >> 10) << 12) | (va_u64 & 0xFFF)) as usize
}

pub unsafe fn kmap_dtb(dtb_pa: usize) {
    let root = &raw mut KERNEL_BOOT_PT.entries as *mut u64;
    mappages(root, dtb_pa, dtb_pa, 128 * 1024, PTE_R);
}

// --- PAGE TABLE CONSTRUCTORS ---

pub unsafe fn uvmcreate(trapframe_pa: usize) -> *mut u64 {
    let root = kalloc().expect("Failed user root PT") as *mut u64;
    let trampoline_pa = trampoline_start as usize; 
    let uart_addr = dtb::get_uart_addr();
    
    mappages(root, TRAMPOLINE, trampoline_pa, PGSIZE, PTE_R | PTE_X);
    mappages(root, TRAPFRAME, trapframe_pa, PGSIZE, PTE_R | PTE_W);
    
    // Map UART based on discovery
    mappages(root, uart_addr, uart_addr, PGSIZE, PTE_R | PTE_W);
    
    // Identity map kernel section
    mappages(root, 0x8000_0000, 0x8000_0000, 1024 * 1024 * 32, PTE_R | PTE_W | PTE_X);

    root
}

pub unsafe fn uvmmapcode(pagetable: *mut u64, va: usize, src: *const u8, len: usize) {
    if len > PGSIZE { panic!("uvmmapcode: code too large"); }
    let mem = kalloc().expect("uvmmapcode: kalloc failed");
    core::ptr::copy_nonoverlapping(src, mem, len);
    mappages(pagetable, va, mem as usize, PGSIZE, PTE_R | PTE_X | PTE_U);
}

#[repr(align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

#[unsafe(link_section = ".data.boot_pt")]
pub static mut KERNEL_BOOT_PT: PageTable = PageTable { entries: [0; 512] };

pub unsafe fn kpvminit(dtb_pa: usize) -> usize {
    let root = &raw mut KERNEL_BOOT_PT.entries as *mut u64;
    core::ptr::write_bytes(root as *mut u8, 0, 4096);
    
    let uart_addr = dtb::get_uart_addr();
    let plic_addr = dtb::HW_CONFIG.get().map(|c| c.plic_addr).unwrap_or(0x0c00_0000);
    
    // 1. Identity map Hardware
    mappages(root, uart_addr, uart_addr, PGSIZE, PTE_R | PTE_W);
    mappages(root, plic_addr, plic_addr, 0x400000, PTE_R | PTE_W); 
    
    // 2. Identity map Kernel RAM
    mappages(root, 0x8000_0000, 0x8000_0000, 1024 * 1024 * 32, PTE_R | PTE_W | PTE_X);
    
    // 3. Map the Trampoline
    extern "C" { fn trampoline_start(); }
    mappages(root, TRAMPOLINE, trampoline_start as *const () as usize, PGSIZE, PTE_R | PTE_X);
    
    // 4. Map the DTB
    kmap_dtb(dtb_pa);
    
    let satp_val = (8usize << 60) | ((root as usize) >> 12);
    
    riscv::register::sstatus::set_sum();
    riscv::register::satp::write(riscv::register::satp::Satp::from_bits(satp_val));
    core::arch::asm!("sfence.vma zero, zero", "fence.i");
    
    satp_val
}

pub unsafe fn kmap(va: usize, pa: usize, size: usize, perm: usize) {
    let root = &raw mut KERNEL_BOOT_PT.entries as *mut u64;
    mappages(root, va, pa, size, perm as u64);
}