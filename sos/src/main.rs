#![no_std]
#![no_main]
#![feature(never_type)]

use sel4_full_root_task_runtime::main;

#[main]
fn main(bootinfo: &sel4::BootInfo) -> sel4::Result<!> {
    unreachable!()
}