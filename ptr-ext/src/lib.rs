#![no_std]
#![feature(strict_provenance)]

pub trait PtrExt: Sized {
    fn try_align_up(self, align: usize) -> Option<Self>;
}

impl PtrExt for *mut u8 {
    fn try_align_up(self, align: usize) -> Option<Self> {
        if !align.is_power_of_two() {
            return None;
        }
        Some(if self.addr() % align == 0 {
            self
        } else {
            self.with_addr((self.addr() | (align - 1)).checked_add(1)?)
        })
    }
}
