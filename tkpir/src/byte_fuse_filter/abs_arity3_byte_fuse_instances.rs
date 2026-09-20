use crate::byte_fuse_filter::byte_fuse_utils::ByteFuseUtils;

use super::ByteFuseInstance;

pub struct AbsArity3ByteFuseInstance {
    /// segment count.
    pub segment_count: usize,

    /// segment count length.
    pub segment_count_len: usize,

    /// segment length, must be a power-of-2.
    pub segment_len: usize,

    /// segment length mask, used for fast module.
    pub segment_len_mask: usize,

    /// filter length, i.e., the length of the filter.
    pub filter_len: usize,

    /// value byte len
    pub value_byte_len: usize,
}

fn is_power_of_two(x: usize) -> bool {
    x != 0 && (x & (x - 1)) == 0
}

impl AbsArity3ByteFuseInstance {
    /// number of positions.
    pub const AIRTY: usize = 3;

    pub fn new(size: usize, value_byte_len: usize) -> Self {
        assert!(size > 0);
        // segment length must be a power-of-2
        let mut segment_len = ByteFuseUtils::calc_arity3_segment_len(size);
        // the current implementation hardcodes an 18-bit limit to the segment length.
        if segment_len > 1usize << 18 {
            segment_len = 1usize << 18;
        }

        let size_factor = ByteFuseUtils::calc_arity3_size_factor(size);
        // println!("size_factor: {}", size_factor);

        // calculate capacity: (1) N = c * m; (2) round N to divide segment length; (3) calculate number of segments
        let capacity = (size as f64 * size_factor) as usize;
        // println!("capacity: {}", capacity);
        let mut segment_count = (capacity + segment_len - 1) / segment_len - (Self::AIRTY - 1);
        // println!("first segment_count: {}", segment_count);

        let array_len = (segment_count + Self::AIRTY - 1) * segment_len;

        segment_count = (array_len + segment_len - 1) / segment_len;
        // make sure that segment count must be a positive value
        if segment_count <= Self::AIRTY - 1 {
            segment_count = 1;
        } else {
            segment_count = segment_count - (Self::AIRTY - 1);
        }

        // println!("final segment_count: {}", segment_count);

        assert!(is_power_of_two(segment_len));
        assert!(value_byte_len > 0);

        return Self {
            segment_len,
            segment_count,
            segment_len_mask: segment_len - 1,
            segment_count_len: segment_count * segment_len,
            filter_len: (segment_count + Self::AIRTY - 1) * segment_len,
            value_byte_len,
        };

        // set parameters
    }
}

impl ByteFuseInstance for AbsArity3ByteFuseInstance {
    fn airty(&self) -> usize {
        return Self::AIRTY;
    }

    fn value_byte_len(&self) -> usize {
        return self.value_byte_len;
    }

    fn filter_len(&self) -> usize {
        return self.filter_len;
    }
}
