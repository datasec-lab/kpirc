use super::*;

#[cfg(test)]
mod tests {

    use super::*;

    use airty3_byte_fuse_position::Arity3ByteFusePosition;
    use byte_fuse_utils::ByteFuseUtils;
    use bytebuffer::ByteBuffer;
    use common_tool::utils::BytesUtils;
    use common_tool::utils::ObjectToByteArr;
    use rand::thread_rng;
    use rand::Rng;
    use rand_core::CryptoRng;
    use rand_core::OsRng;
    use rand_core::RngCore;
    use serde::de::value;
    use std::collections::HashMap;

    const DEFAULT_VALUE_BYTE_LEN: usize = 16;

    const DEFAULT_SIZE: usize = 1 << 10 + 1;

    #[test]
    fn bytebuffer_to_bytearr() {
        let data = vec![0u8; 16];

        let bytebuffer = ByteBuffer::from_vec(data);

        let byte_arr = bytebuffer.to_byte_arr();

        println!("byte arr: {:?}", byte_arr);
    }

    // must run in release
    // cargo test test_small_size --release -- --nocapture
    #[test]
    fn test_small_size() {
        for size in 1..40usize {
            test_byte_fuse_filter(size, DEFAULT_VALUE_BYTE_LEN);
        }
    }

    // must run in release
    // cargo test test_large_size --release -- --nocapture
    #[test]
    fn test_large_size() {
        let size = 10000;
        test_byte_fuse_filter(size, DEFAULT_VALUE_BYTE_LEN);
    }

    // must run in release
    // cargo test test_small_value_byte_len --release -- --nocapture
    #[test]
    fn test_small_value_byte_len() {
        for value_byte_len in 1..DEFAULT_VALUE_BYTE_LEN {
            test_byte_fuse_filter(DEFAULT_SIZE, value_byte_len);
        }
    }

    // must run in release
    // cargo test test_log_set_size --release -- --nocapture
    #[test]
    fn test_log_set_size() {
        for log_size in 0..22usize {
            test_byte_fuse_filter(1 << log_size, DEFAULT_VALUE_BYTE_LEN);
        }
    }

    fn random_byte_arr(len: usize, rng: &mut impl RngCore) -> Vec<u8> {
        let mut data = vec![0u8; len];
        rng.fill_bytes(&mut data);
        return data;
    }

    fn random_key_value_map(size: usize, value_byte_len: usize) -> HashMap<ByteBuffer, Vec<u8>> {
        let mut map: HashMap<ByteBuffer, Vec<u8>> = HashMap::new();
        let mut rng = thread_rng();
        for _ in 0..size {
            let key = random_byte_arr(BLOCK_BYTE_LEN, &mut rng);
            let value = random_byte_arr(value_byte_len, &mut rng);
            map.insert(ByteBuffer::from_vec(key), value);
        }
        map
    }

    fn test_byte_fuse_filter(size: usize, value_byte_len: usize) {
        let key_value_map = random_key_value_map(size, value_byte_len);

        // init a ByteFuseFilter
        let mut byte_fuse_filter = Arity3ByteFuseFilter::new(key_value_map.clone(), value_byte_len);

        let seed = byte_fuse_filter.seed();
        let storage = byte_fuse_filter.storage();

        let mut byte_fuse_position = Arity3ByteFusePosition::new(size, value_byte_len, seed);

        assert_eq!(
            byte_fuse_filter.filter_len(),
            byte_fuse_position.filter_len(),
            "byte_fuse_filter filter len: {}, byte_fuse_position filter len: {}",
            byte_fuse_filter.filter_len(),
            byte_fuse_position.filter_len()
        );
        println!("fuse filter_len: {}", byte_fuse_filter.filter_len());
        println!("pos filter_len: {}", byte_fuse_position.filter_len());

        // verify
        for (key, value) in key_value_map.iter() {
            let filter_decode = byte_fuse_filter.decode(key.clone());
            assert_eq!(filter_decode, *value);

            let positions = byte_fuse_position
                .abs_arity3_byte_fuse_pos
                .positions(key.clone());
            let mut position_dedcode = vec![0u8; value_byte_len];

            for pos in positions {
                ByteFuseUtils::addi(&mut position_dedcode, &storage[pos], value_byte_len);
            }
            assert_eq!(position_dedcode, *value);
        }
    }
}
