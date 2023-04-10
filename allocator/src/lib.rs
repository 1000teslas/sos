#![no_std]
#![feature(sync_unsafe_cell)]
#![feature(strict_provenance)]
#![feature(slice_ptr_len, slice_ptr_get)]
#![feature(pointer_byte_offsets)]
#![feature(pointer_is_aligned)]
#![feature(ptr_sub_ptr)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::as_conversions)]

pub mod linked_list;
