#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(strict_provenance)]
#![feature(slice_ptr_len, slice_ptr_get)]
#![feature(pointer_byte_offsets)]
#![feature(pointer_is_aligned)]
#![feature(ptr_sub_ptr)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::as_conversions)]

use core::{alloc::Layout, ptr::NonNull};

pub mod bump;
pub mod linked_list;

unsafe trait Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Option<NonNull<[u8]>>;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}
