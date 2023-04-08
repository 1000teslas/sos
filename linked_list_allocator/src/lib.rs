#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(strict_provenance)]
#![feature(slice_ptr_len, slice_ptr_get)]
#![feature(pointer_byte_offsets)]
#![feature(pointer_is_aligned)]
#![feature(ptr_sub_ptr)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::as_conversions)]

use core::{
    alloc::Layout,
    mem,
    ptr::{self, addr_of_mut, NonNull},
};
// based off https://os.phil-opp.com/allocator-designs/#linked-list-allocator

pub struct LinkedListAllocator {
    head: Node,
}

impl LinkedListAllocator {
    /// Creates an empty LinkedListAllocator.
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
    /// heap bounds are valid and that the heap is unused.
    pub unsafe fn add_free_region(&mut self, range: *mut [u8]) {
        assert!(range.as_mut_ptr().is_aligned_to(mem::align_of::<Node>()));
        assert!(range.len() >= mem::size_of::<Node>());

        let node = Node {
            size: range.len(),
            next: self.head.next.take(),
        };
        let node_ptr = NonNull::new(range.as_mut_ptr().cast::<Node>()).unwrap();
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

    pub unsafe fn alloc(&mut self, layout: Layout) -> Option<NonNull<[u8]>> {
        let layout = LinkedListAllocator::adjust(layout);
        self.find_region(layout).map(|(region, alloc)| {
            let alloc_end = alloc.as_ptr().as_mut_ptr().wrapping_add(alloc.len());
            let excess_size = Node::end(region.as_ptr()).addr() - alloc_end.addr();
            if excess_size > 0 {
                unsafe {
                    // SAFETY: alloc has provenance for entire memory region pointed to by region
                    self.add_free_region(ptr::slice_from_raw_parts_mut(alloc_end, excess_size));
                }
            }
            alloc
        })
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let layout = LinkedListAllocator::adjust(layout);
        unsafe {
            self.add_free_region(ptr::slice_from_raw_parts_mut(ptr, layout.size()));
        }
    }

    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `ListNode`.
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

// node: Node is the header of a memory region of size node.size >=
// size_of::<Node>() bytes, except for the dummy node at the start of
// LinkedListAllocator
struct Node {
    size: usize,
    next: Option<NonNull<Node>>,
}

impl Node {
    fn start(this: *mut Node) -> *mut u8 {
        this.cast()
    }
    fn end(this: *mut Node) -> *mut u8 {
        this.cast::<u8>().wrapping_add(unsafe { (*this).size })
    }
    fn alloc_from_region(this: *mut Self, layout: Layout) -> Option<NonNull<[u8]>> {
        let alloc_start = align_up(Node::start(this), layout.align());
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

/// Align the given pointer `ptr` upwards to alignment `align`.
fn align_up(ptr: *mut u8, align: usize) -> *mut u8 {
    assert!(align.is_power_of_two());
    ptr.map_addr(|addr| (addr + align - 1) & !(align - 1))
}

#[cfg(test)]
mod tests {
    use core::{
        alloc::Layout,
        cell::SyncUnsafeCell,
        mem,
        ptr::{addr_of_mut, slice_from_raw_parts_mut},
    };

    use static_assertions::const_assert_eq;

    use super::{LinkedListAllocator, Node};

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
        let mut alloc = LinkedListAllocator::new();
        unsafe {
            alloc.add_free_region(slice_from_raw_parts_mut(
                addr_of_mut!((*HEAP1.get()).0).cast(),
                HEAP_SIZE,
            ));
            alloc.add_free_region(slice_from_raw_parts_mut(
                addr_of_mut!((*HEAP2.get()).0).cast(),
                HEAP_SIZE,
            ));
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
