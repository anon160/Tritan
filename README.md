# Tritan OS — Experimental RISC-V Kernel

An experimental RISC-V kernel designed for learning and exploration, supporting multi-hart booting, device tree parsing, and basic syscall output.

## Current Working features:

* SMP booting (multi-hart) — contains known race conditions
* DTB parsing for basic hardware initialization
* Hello world using ecall
* Simplified HHDM (higher half direct mapping)
* Minimal kernel environment for experimentation

## TODO

* SMP race conditions remain (typical for early bring-up)
* No memory protection yet
* Only basic syscall support
* Early-stage kernel — not production-ready

## Getting started
* Clone this repo using git clone https://github.com/anon160/Tritan.git
* Build it using: `cargo build`.
* Run using: `qemu-system-riscv64 -machine virt -cpu rv64 -m 6G -nographic -kernel target/riscv64gc-unknown-none-elf/debug/tritan -smp 2`
