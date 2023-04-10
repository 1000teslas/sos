use core::{
    alloc::Layout,
    ptr::{slice_from_raw_parts_mut, NonNull},
};

use ptr_ext::PtrExt;

pub struct Allocator {
    region: NonNull<[u8]>,
    tip: *mut u8,
    allocations: u64,
}

impl Allocator {
    pub fn new(region: NonNull<[u8]>) -> Allocator {
        Allocator {
            region,
            tip: region.as_mut_ptr(),
            allocations: 0,
        }
    }
}

unsafe impl super::Allocator for Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Option<NonNull<[u8]>> {
        let alloc_start = self.tip.try_align_up(layout.align())?;
        let alloc_end = alloc_start.with_addr(alloc_start.addr().checked_add(layout.size())?);
        if alloc_end.addr() > self.region.addr().get() + self.region.len() {
            return None;
        }
        self.allocations = self.allocations.checked_add(1)?;
        self.tip = alloc_end;
        NonNull::new(slice_from_raw_parts_mut(alloc_start, layout.size()))
    }

    unsafe fn dealloc(&mut self, _ptr: *mut u8, _layout: Layout) {
        self.allocations -= 1;
        if self.allocations == 0 {
            self.tip = self.region.as_mut_ptr();
        }
    }
}

#[cfg(test)]
mod tests {
    use core::{
        alloc::Layout,
        cell::SyncUnsafeCell,
        ptr::{addr_of_mut, slice_from_raw_parts_mut, NonNull},
    };

    use super::Allocator;
    use crate::Allocator as _;

    #[repr(align(8))]
    struct MemPool<const N: usize>([u8; N]);

    #[test]
    fn test() {
        const HEAP_SIZE: usize = 1 << 4;
        static HEAP: SyncUnsafeCell<MemPool<HEAP_SIZE>> =
            SyncUnsafeCell::new(MemPool([0; HEAP_SIZE]));
        let mut alloc = Allocator::new(
            NonNull::new(slice_from_raw_parts_mut(
                unsafe { addr_of_mut!((*HEAP.get()).0) }.cast(),
                HEAP_SIZE,
            ))
            .unwrap(),
        );
        let l1 = Layout::new::<u64>();
        let l2 = Layout::new::<u64>();
        let l3 = Layout::new::<u64>();
        unsafe {
            let p1 = alloc.alloc(l1).unwrap();
            let p2 = alloc.alloc(l2).unwrap();
            assert!(alloc.alloc(l3).is_none());
            alloc.dealloc(p1.as_mut_ptr(), l1);
            alloc.dealloc(p2.as_mut_ptr(), l2);
            alloc.alloc(l3).unwrap();
        }
    }
}
