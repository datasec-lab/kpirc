use blake2::{Blake2s256, Digest};
use common_tool::crypto::prf::*;
use common_tool::utils::LongUtils;
use hex_literal::hex;

use super::{
    abs_arity3_byte_fuse_instances::*, byte_fuse_utils::ByteFuseUtils, ByteFuseInstance,
    ByteFusePosition, ObjectToByteArr, ObjectUtils, BLOCK_BYTE_LEN,
};

pub struct AbsArity3ByteFusePosition {
    pub abs_arity3_byte_fuse_instance: AbsArity3ByteFuseInstance,

    pub seed: Vec<u8>,

    pub hash: Box<dyn Prf>,
}

impl AbsArity3ByteFusePosition {
    pub fn new(size: usize, value_byte_len: usize) -> Self {
        let abs_arity3_byte_fuse_instance = AbsArity3ByteFuseInstance::new(size, value_byte_len);

        let hash = PrfFactory::create_instance(&PrfType::AES_CBC_PRF, 8);
        let seed = vec![0u8; BLOCK_BYTE_LEN];

        Self {
            abs_arity3_byte_fuse_instance,
            seed,
            hash,
        }
    }

    pub fn hash<T: ObjectToByteArr>(&mut self, x: T) -> u64 {
        let bytes = ObjectUtils::object_to_byte_arr(x);
        u64::from_be_bytes(self.hash.get_bytes(bytes).as_slice().try_into().unwrap())
    }
    
    /// Computes the filter position for a given `index` (0, 1, or 2) and a 64-bit `hash`.
    ///
    /// Each position consists of two parts:
    /// 
    /// - **Segment index `h`**: In the range `[0, segment_count_len)`. For the same hash input,
    ///   the segment indices for index 0, 1, and 2 will be in consecutive regions:
    ///   `h`, `h + segment_len`, and `h + 2 * segment_len`.
    ///
    /// - **Segment-local offset `hh`**: In the range `[0, segment_len)`, where `segment_len` is
    ///   a power of two. For the same hash input, `hh` is chosen as:
    ///   - index 0: offset is 0 (base position),
    ///   - index 1: a random value derived from the middle 18 bits of the hash,
    ///   - index 2: a random value derived from the lower 18 bits of the hash.
    ///
    /// The final position is computed by combining the segment index with the randomized local offset.
    pub fn get_hash_from_hash(&self, hash: u64, index: usize) -> u32 {
        let mut h = ByteFuseUtils::reduce(
            (hash >> 32) as u32,
            self.abs_arity3_byte_fuse_instance.segment_count_len,
        ) as u64;

        h += index as u64 * self.abs_arity3_byte_fuse_instance.segment_len as u64;

        // Extracts the lowest 36 bits of the hash 
        let hh = hash & ((1u64 << 36) - 1);
        // index 0: right shift by 36; index 1: right shift by 18; index 2: no shift
        h ^= (hh >> (36 - 18 * index)) & self.abs_arity3_byte_fuse_instance.segment_len_mask as u64;

        h as u32
    }

    pub fn arity(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.airty()
    }

    pub fn filter_len(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.filter_len
    }

    pub fn segment_count(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.segment_count
    }

    pub fn segment_count_len(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.segment_count_len
    }

    pub fn segment_len(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.segment_len
    }

    pub fn segment_len_mask(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.segment_len_mask
    }
    pub fn value_byte_len(&self) -> usize {
        self.abs_arity3_byte_fuse_instance.value_byte_len
    }
}

impl<T: ObjectToByteArr> ByteFusePosition<T> for AbsArity3ByteFusePosition {
    fn seed(&self) -> Vec<u8> {
        self.seed.clone()
    }

    fn positions(&mut self, x: T) -> Vec<usize> {
        // first , hash(x)
        let hash = self.hash(x);
        let h0 = ByteFuseUtils::reduce(
            (hash >> 32) as u32,
            self.abs_arity3_byte_fuse_instance.segment_count_len,
        );
        let mut h1 = h0 + self.abs_arity3_byte_fuse_instance.segment_len as u32;

        let mut h2 = h1 + self.abs_arity3_byte_fuse_instance.segment_len as u32;

        h1 ^= ((hash >> 18) & self.abs_arity3_byte_fuse_instance.segment_len_mask as u64) as u32;
        h2 ^= (hash & self.abs_arity3_byte_fuse_instance.segment_len_mask as u64) as u32;

        return vec![h0 as usize, h1 as usize, h2 as usize];
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn blake2_hash() {
        let mut hasher = Blake2s256::new();

        hasher.update(b"hello world");
        let res = hasher.finalize();
        assert_eq!(
            res[..],
            hex!("9aec6806794561107e594b1f6a8a6b0c92a0cba9acf5e5e93cca06f781813b0b")[..]
        );
    }
}
