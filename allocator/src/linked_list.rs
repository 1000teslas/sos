use core::{
    alloc::Layout,
    mem,
    ptr::{self, addr_of_mut, NonNull},
};

use ptr_ext::PtrExt;

// based off https://os.phil-opp.com/allocator-designs/#linked-list-allocator

pub struct Allocator {
    head: Node,
}

impl Allocator {
    /// Creates an empty Allocator.
    pub const fn new() -> Self {
        Self {
            head: Node {
                size: 0,
                next: None,
            },
        }
    }

    /// Adds the given memory region to the front of the list.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// memory region is valid and unused.
    pub unsafe fn add_free_region(&mut self, region: NonNull<[u8]>) {
        assert!(region.as_mut_ptr().is_aligned_to(mem::align_of::<Node>()));
        assert!(region.len() >= mem::size_of::<Node>());

        let node = Node {
            size: region.len(),
            next: self.head.next.take(),
        };
        let node_ptr = region.cast::<Node>();
        unsafe {
            node_ptr.as_ptr().write(node);
        }
        self.head.next = Some(node_ptr);
    }

    /// Looks for a free region with the given size and alignment and removes
    /// it from the list.
    ///
    /// Returns a tuple of the list node and a slice pointing to the allocation
    fn find_region(&mut self, layout: Layout) -> Option<(NonNull<Node>, NonNull<[u8]>)> {
        let mut curr = addr_of_mut!(self.head);
        while let Some(region) = unsafe { (*curr).next } {
            let region = region.as_ptr();
            if let Some(alloc) = Node::alloc_from_region(region, layout) {
                let next = unsafe { (*region).next.take() };
                let node = mem::replace(unsafe { &mut (*curr).next }, next).unwrap();
                assert_eq!(node.as_ptr(), region);
                return Some((node, alloc));
            } else {
                curr = region;
            }
        }
        None
    }

    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `Node`.
    fn adjust(layout: Layout) -> Layout {
        let layout = layout
            .align_to(mem::align_of::<Node>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        Layout::from_size_align(
            Ord::max(layout.size(), mem::size_of::<Node>()),
            layout.align(),
        )
        .unwrap()
    }
}

unsafe impl super::Allocator for Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Option<NonNull<[u8]>> {
        let layout = Allocator::adjust(layout);
        self.find_region(layout).map(|(region, alloc)| {
            let alloc_end = alloc
                .as_ptr()
                .as_mut_ptr()
                .map_addr(|addr| addr + alloc.len());
            let excess_size = Node::end(region.as_ptr()).addr() - alloc_end.addr();
            if excess_size > 0 {
                unsafe {
                    // SAFETY: alloc has provenance for entire memory region pointed to by region
                    self.add_free_region(
                        NonNull::new(ptr::slice_from_raw_parts_mut(alloc_end, excess_size))
                            .unwrap(),
                    );
                }
            }
            alloc
        })
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let layout = Allocator::adjust(layout);
        unsafe {
            self.add_free_region(
                NonNull::new(ptr::slice_from_raw_parts_mut(ptr, layout.size())).unwrap(),
            );
        }
    }
}

// node: Node is the header of a memory region of size node.size >=
// size_of::<Node>() bytes, except for the dummy node at the start of
// Allocator
struct Node {
    size: usize,
    next: Option<NonNull<Node>>,
}

impl Node {
    fn end(this: *mut Node) -> *mut u8 {
        this.cast::<u8>()
            .map_addr(|addr| addr + unsafe { (*this).size })
    }
    fn alloc_from_region(this: *mut Self, layout: Layout) -> Option<NonNull<[u8]>> {
        let alloc_start = this.cast::<u8>().try_align_up(layout.align())?;
        let alloc_end = alloc_start.with_addr(alloc_start.addr().checked_add(layout.size())?);

        if alloc_end > Node::end(this) {
            return None;
        }

        let excess_size = Node::end(this).addr() - alloc_end.addr();
        if 0 < excess_size && excess_size < mem::size_of::<Node>() {
            return None;
        }

        NonNull::new(ptr::slice_from_raw_parts_mut(alloc_start, layout.size()))
    }
}

#[cfg(test)]
mod tests {
    use core::{
        alloc::Layout,
        cell::SyncUnsafeCell,
        mem,
        ptr::{addr_of_mut, slice_from_raw_parts_mut, NonNull},
    };

    use static_assertions::const_assert_eq;

    use super::{Allocator, Node};
    use crate::Allocator as _;

    #[repr(align(8))]
    struct MemPool<const N: usize>([u8; N]);
    const_assert_eq!(mem::align_of::<MemPool<1>>(), mem::align_of::<Node>());

    #[test]
    fn test() {
        const HEAP_SIZE: usize = 1 << 12;
        static HEAP1: SyncUnsafeCell<MemPool<HEAP_SIZE>> =
            SyncUnsafeCell::new(MemPool([0; HEAP_SIZE]));
        static HEAP2: SyncUnsafeCell<MemPool<HEAP_SIZE>> =
            SyncUnsafeCell::new(MemPool([0; HEAP_SIZE]));
        let mut alloc = Allocator::new();
        unsafe {
            alloc.add_free_region(
                NonNull::new(slice_from_raw_parts_mut(
                    addr_of_mut!((*HEAP1.get()).0).cast(),
                    HEAP_SIZE,
                ))
                .unwrap(),
            );
            alloc.add_free_region(
                NonNull::new(slice_from_raw_parts_mut(
                    addr_of_mut!((*HEAP2.get()).0).cast(),
                    HEAP_SIZE,
                ))
                .unwrap(),
            );
        }
        let l1 = Layout::new::<[u8; HEAP_SIZE]>();
        let l2 = Layout::new::<u64>();
        let l3 = Layout::new::<u64>();
        unsafe {
            let p1 = alloc.alloc(l1).unwrap();
            let p2 = alloc.alloc(l2).unwrap();
            let p3 = alloc.alloc(l3).unwrap();
            alloc.dealloc(p1.as_mut_ptr(), l1);
            alloc.dealloc(p3.as_mut_ptr(), l2);
            alloc.dealloc(p2.as_mut_ptr(), l2);
        }
    }
}
