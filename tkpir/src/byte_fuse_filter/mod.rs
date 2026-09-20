pub mod abs_arity3_byte_fuse_instances;
pub mod abs_arity3_byte_fuse_position;
pub mod airty3_byte_fuse_instances;
pub mod airty3_byte_fuse_position;
pub use airty3_byte_fuse_position::*;
pub mod arity3_byte_fuse_filter;
pub use arity3_byte_fuse_filter::*;
pub mod byte_fuse_utils;

pub mod airty3_byte_fuse_filter_test;
pub use airty3_byte_fuse_filter_test::*;

use common_tool::utils::{ObjectToByteArr, ObjectUtils};

use std::vec::Vec;

pub const BLOCK_BYTE_LEN: usize = 16;

use bytebuffer::ByteBuffer;

pub trait ByteFuseFilter<T: ObjectToByteArr> {
    fn storage() -> Vec<Vec<u8>>;
    fn decode(x: T) -> Vec<u8>;
}

pub trait ByteFuseInstance {
    fn airty(&self) -> usize;

    fn value_byte_len(&self) -> usize;

    fn filter_len(&self) -> usize;
}

pub trait ByteFusePosition<T: ObjectToByteArr> {
    /// Gets seed that is used to compute the positions for the input x.
    fn seed(&self) -> Vec<u8>;

    /// Gets positions for the input x. The returned positions must be distinct.
    fn positions(&mut self, x: T) -> Vec<usize>;
}
