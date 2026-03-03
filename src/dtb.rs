use fdt::Fdt;
use spin::Once;

/// The Global Hardware Configuration discovered from the Device Tree.
pub struct Config {
    pub ram_start: usize,
    pub ram_size: usize,
    pub phystop: usize,   // Added phystop
    pub uart_addr: usize,
    pub plic_addr: usize,
    pub clint_addr: usize,
    pub cpu_count: usize,
}

/// Global static initialized once during boot.
pub static HW_CONFIG: Once<Config> = Once::new();

pub fn init(dtb_pa: usize) {
    let fdt = unsafe { 
        Fdt::from_ptr(dtb_pa as *const u8).expect("DTB: Failed to parse FDT blob") 
    };

    // 1. Discover RAM
    let mem = fdt.memory().regions().next().expect("DTB: No memory node found");
    let ram_start = mem.starting_address as usize;
    let ram_size = mem.size.unwrap_or(0);
    
    // Calculate PHYSTOP (End of physical memory)
    let phystop = ram_start + ram_size;

    // 2. Discover UART Address
    let uart_addr = fdt.chosen().stdout()
        .and_then(|node| node.reg())
        .and_then(|mut reg_list| reg_list.next())
        .map(|reg| reg.starting_address as usize)
        .unwrap_or(0x1000_0000); 

    // 3. Discover PLIC
    let plic_addr = fdt.find_all_nodes("/soc/interrupt-controller")
        .filter(|n| n.compatible().map_or(false, |c| c.all().any(|s| s.contains("plic"))))
        .next()
        .and_then(|n| n.reg())
        .and_then(|mut r| r.next())
        .map(|r| r.starting_address as usize)
        .unwrap_or(0x0c00_0000);

    // 4. Discover CLINT
    let clint_addr = fdt.find_all_nodes("/soc/clint")
        .filter(|n| n.compatible().map_or(false, |c| c.all().any(|s| s.contains("clint"))))
        .next()
        .and_then(|n| n.reg())
        .and_then(|mut r| r.next())
        .map(|r| r.starting_address as usize)
        .unwrap_or(0x0200_0000);

    // 5. Discover CPU Count
    let cpu_count = fdt.cpus().count();

    let config = Config {
        ram_start,
        ram_size,
        phystop,
        uart_addr,
        plic_addr,
        clint_addr, 
        cpu_count,
    };

    HW_CONFIG.call_once(|| config);

    crate::println!("[DTB] RAM: {:#x} - {:#x} ({} MB) | CPUS: {}", 
        ram_start, phystop, ram_size / 1024 / 1024, cpu_count);
}

// --- Getter Methods ---

pub fn get_ram_start() -> usize {
    HW_CONFIG.get().map(|c| c.ram_start).unwrap_or(0x8000_0000)
}

pub fn get_phystop() -> usize {
    HW_CONFIG.get().map(|c| c.phystop).expect("DTB: HW_CONFIG not initialized")
}

pub fn get_uart_addr() -> usize {
    HW_CONFIG.get().map(|c| c.uart_addr).unwrap_or(0x1000_0000)
}

