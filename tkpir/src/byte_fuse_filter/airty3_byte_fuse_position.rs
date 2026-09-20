use common_tool::CommonConstants::BLOCK_BIT_LENGTH;

use crate::byte_fuse_filter::BLOCK_BYTE_LEN;

use super::abs_arity3_byte_fuse_position::AbsArity3ByteFusePosition;

pub struct Arity3ByteFusePosition {
    pub abs_arity3_byte_fuse_pos: AbsArity3ByteFusePosition,
}

impl Arity3ByteFusePosition {
    pub fn new(size: usize, value_byte_len: usize, seed: Vec<u8>) -> Self {
        let mut abs_arity3_byte_fuse_pos = AbsArity3ByteFusePosition::new(size, value_byte_len);

        assert_eq!(seed.len(), BLOCK_BYTE_LEN);

        abs_arity3_byte_fuse_pos
            .seed
            .as_mut_slice()
            .copy_from_slice(seed.as_slice());

        abs_arity3_byte_fuse_pos
            .hash
            .set_key(abs_arity3_byte_fuse_pos.seed.clone());

        Self {
            abs_arity3_byte_fuse_pos,
        }
    }

    pub fn filter_len(&self) -> usize {
        self.abs_arity3_byte_fuse_pos.filter_len()
    }
}
