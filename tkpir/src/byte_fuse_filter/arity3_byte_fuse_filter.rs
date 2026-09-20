use super::abs_arity3_byte_fuse_position::*;
use super::byte_fuse_utils::ByteFuseUtils;
use super::ByteFuseFilter;
use super::ByteFusePosition;
use super::BLOCK_BYTE_LEN;
use common_tool::utils::ObjectToByteArr;

use rand_core::CryptoRng; // trait , impl Crypto 表示该伪随机生成器 是 密码学安全的伪随机生成器
use rand_core::OsRng;
use rand_core::RngCore;
use serde::de::value;
use std::hint;
use std::{collections::HashMap, hash::Hash}; // trait 提供 伪随机生成器的常用方法
pub struct Arity3ByteFuseFilter {
    pub abs_arity3_byte_fuse_pos: AbsArity3ByteFusePosition,

    storage: Vec<Vec<u8>>,
}

fn mod3(mut x: u8) -> u8 {
    if x > 2 {
        x -= 3;
    }
    return x as u8;
}

impl Arity3ByteFuseFilter {
    pub fn new<T: ObjectToByteArr + Clone>(
        key_value_map: HashMap<T, Vec<u8>>,
        value_byte_len: usize,
    ) -> Self {
        Self::new_with_rng(key_value_map, value_byte_len, OsRng)
    }

    pub fn new_with_rng<T: ObjectToByteArr + Clone>(
        key_value_map: HashMap<T, Vec<u8>>,
        value_byte_len: usize,
        mut rng: impl CryptoRng + RngCore,
    ) -> Self {
        let mut abs_arity3_byte_fuse_pos =
            AbsArity3ByteFusePosition::new(key_value_map.len(), value_byte_len);
        // check value byte len
        assert!(key_value_map.iter().all(|(_, v)| v.len() == value_byte_len));

        let mut storage = vec![
            vec![0u8; value_byte_len];
            abs_arity3_byte_fuse_pos
                .abs_arity3_byte_fuse_instance
                .filter_len
        ];
        Self::add_all(
            &mut abs_arity3_byte_fuse_pos,
            &mut storage,
            value_byte_len,
            key_value_map,
            &mut rng,
        );

        Self {
            abs_arity3_byte_fuse_pos,
            storage,
        }
    }

    pub fn seed(&self) -> Vec<u8> {
        self.abs_arity3_byte_fuse_pos.seed.clone()
    }

    pub fn filter_len(&self) -> usize {
        self.abs_arity3_byte_fuse_pos.filter_len()
    }

    pub fn storage(&self) -> Vec<Vec<u8>> {
        return self.storage.clone();
    }

    pub fn decode<T: ObjectToByteArr + Clone>(&mut self, x: T) -> Vec<u8> {
        let pos = self.abs_arity3_byte_fuse_pos.positions(x);
        let mut value = vec![0u8; self.abs_arity3_byte_fuse_pos.value_byte_len()];

        ByteFuseUtils::addi(
            &mut value,
            &self.storage[pos[0]],
            self.abs_arity3_byte_fuse_pos.value_byte_len(),
        );

        ByteFuseUtils::addi(
            &mut value,
            &self.storage[pos[1]],
            self.abs_arity3_byte_fuse_pos.value_byte_len(),
        );

        ByteFuseUtils::addi(
            &mut value,
            &self.storage[pos[2]],
            self.abs_arity3_byte_fuse_pos.value_byte_len(),
        );

        value
    }

    fn add_all<T: ObjectToByteArr + Clone>(
        abs_arity3_byte_fuse_pos: &mut AbsArity3ByteFusePosition,
        storage: &mut Vec<Vec<u8>>,
        value_byte_len: usize,
        key_value_map: HashMap<T, Vec<u8>>,
        rng: &mut (impl CryptoRng + RngCore),
    ) {
        // #keys that we are inserting.
        let size = key_value_map.len();

        let mut hash_value_map = HashMap::<u64, Vec<u8>>::new();
        // stores the hash of each key.
        let mut reverse_order = vec![0u64; size + 1];
        // stores which of the 3 hash slots each key was assigned to.
        let mut reverse_h = vec![0u8; size];

        // tracks #keys that were successfully peeled
        let mut reverse_order_pos: u32 = 0;

        // tracks #keys mapping to each slot, encoded as `count << 2 | h_index`
        let mut t2_count = vec![0u8; abs_arity3_byte_fuse_pos.filter_len()];
        // stores the XOR of all hashes assigned to each slot
        let mut t2_hash = vec![0u64; abs_arity3_byte_fuse_pos.filter_len()];
        // stores "singleton" positions
        let mut alone = vec![0u32; abs_arity3_byte_fuse_pos.filter_len()];

        // counts #times we re-try with a new seed.
        let mut repeat = 0u32;
        // (h_0(x), h_1(x), h_2(x), h_0(x), h_1(x))
        let mut h01201 = vec![0u32; 5];

        // find the smallest power of 2 >= segment_count.
        let mut block_bits = 1u32;
        while (1 << block_bits) < abs_arity3_byte_fuse_pos.segment_count() as u32 {
            block_bits += 1;
        }
        let block = 1 << block_bits;

        // Try to build the filter using a random seed.
        //  -> Exit if successful.
        //  -> Repeat if the peeling process fails.
        while true {
            rng.fill_bytes(&mut abs_arity3_byte_fuse_pos.seed);
            abs_arity3_byte_fuse_pos
                .hash
                .set_key(abs_arity3_byte_fuse_pos.seed.clone());

            // avoids out-of-bounds access
            reverse_order[size] = 1;

            // start position for each segment
            let mut start_pos = vec![0u32; block];
            for i in 0..start_pos.len() {
                start_pos[i] = ((i as u64 * size as u64) / block as u64) as u32;
            }

            for (key, value) in key_value_map.iter() {
                let hash = abs_arity3_byte_fuse_pos.hash(key.clone());
                hash_value_map.insert(hash, value.clone());

                // assigns each hash to a segment (i.e, top `block_bits` of the hash).
                let mut segment_index = (hash >> (64 - block_bits)) as usize;

                // We hit the condition when:
                //  -> We try to insert more keys than the segment’s share of slots,
                //  -> And that the next slot is already occupied — e.g., the next segment is also inserting there.
                // ---
                // Note:
                // -> We only overwrite when the hash was zero. Zero hash values may be misplaced (unlikely).
                // ---
                while reverse_order[start_pos[segment_index] as usize] != 0 {
                    segment_index += 1;
                    segment_index &= block - 1;
                }

                reverse_order[start_pos[segment_index] as usize] = hash;
                start_pos[segment_index] += 1;
            }

            // count mask is used to label if there is a bin with count overflow (greater than 2^5).
            let mut count_mask = 0i8;
            for i in 0..size {
                let hash = reverse_order[i];
                for hi in 0..abs_arity3_byte_fuse_pos.arity() {
                    let idx = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, hi) as usize;

                    // top 6 bits counts #keys mapping to the same position.
                    t2_count[idx] += 4;
                    // lowest 2 bits encodes hi.
                    t2_count[idx] ^= hi as u8;
                    // XORs the hashes of keys mapping to the same position.
                    t2_hash[idx] ^= hash;
                    count_mask |= t2_count[idx] as i8;
                }
            }

            if count_mask < 0 {
                // we have a possible counter overflow
                continue;
            }

            reverse_order_pos = 0;
            // #singletons
            let mut alone_pos = 0usize;

            // scan through the bins (e.g., the stack C)
            for i in 0..abs_arity3_byte_fuse_pos.filter_len() {
                // if `i` is a singleton, add it to the stack Q (i.e., `alone`)
                alone[alone_pos] = i as u32;
                let inc = match t2_count[i] >> 2 {
                    1 => 1usize,
                    _ => 0usize,
                };
                alone_pos += inc;
            }

            // while Q is not empty:
            while alone_pos > 0 {
                alone_pos -= 1;

                let idx = alone[alone_pos] as usize;

                // if `idx` is a singleton:
                if t2_count[idx] >> 2 == 1 {
                    let hash = t2_hash[idx];
                    let found = t2_count[idx] & 3;

                    reverse_h[reverse_order_pos as usize] = found;
                    // append the key x mapping to `idx` to the stack P (i.e, `reverse_order`)
                    reverse_order[reverse_order_pos as usize] = hash;

                    // remove x from the other two indexes it mapped to.
                    h01201[0] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 0);
                    h01201[1] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 1);
                    h01201[2] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 2);
                    // used to avoid mod opretaion.
                    h01201[3] = h01201[0];
                    h01201[4] = h01201[1];

                    let mut index3 = h01201[(found + 1) as usize] as usize;
                    // let mut index3 = h01201[mod3(found + 1) as usize] as usize;

                    // if `index3` becomes singleton (i.e., if it has two keys before removal),
                    // add it to the stack Q
                    if t2_count[index3 as usize] >> 2 == 2 {
                        alone[alone_pos] = index3 as u32;
                        alone_pos += 1;
                    }

                    t2_count[index3] -= 4;
                    t2_count[index3] ^= mod3(found + 1);
                    t2_hash[index3] ^= hash;

                    index3 = h01201[mod3(found + 2) as usize] as usize;

                    // if `index3` becomes singleton add it to the stack Q
                    if t2_count[index3 as usize] >> 2 == 2 {
                        alone[alone_pos] = index3 as u32;
                        alone_pos += 1;
                    }

                    t2_count[index3] -= 4;
                    t2_count[index3] ^= mod3(found + 2);
                    t2_hash[index3] ^= hash;

                    reverse_order_pos += 1;
                }
            }
            // if we peeled all the keys, we are done;
            if reverse_order_pos == size as u32 {
                break;
            }

            // otherwise repeat (i.e., change the seed)
            repeat += 1;

            t2_count.fill(0);
            t2_hash.fill(0);
            reverse_order.fill(0);
            hash_value_map.clear();

            // if construction doesn't succeed eventually, then there is likely a problem with the hash function
            if repeat > 100 {
                for entry in storage.iter_mut() {
                    entry.fill(0xFF);
                }
                return;
            }
        } // while(true)

        // while stack P is not empty do:
        for i in (0..=reverse_order_pos - 1).rev() {
            let hash = reverse_order[i as usize];
            let found = reverse_h[i as usize];

            // Set H[i] ← V(x) xor H[i+1] xor H[i+2]

            let value = hash_value_map.get(&hash).unwrap();

            h01201[0] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 0);
            h01201[1] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 1);
            h01201[2] = abs_arity3_byte_fuse_pos.get_hash_from_hash(hash, 2);
            h01201[3] = h01201[0];
            h01201[4] = h01201[1];

            storage[h01201[found as usize] as usize].fill(0x00);

            let mut tmp1 = storage[h01201[found as usize] as usize].clone();
            let tmp2 = &storage[h01201[(found + 1) as usize] as usize];
            ByteFuseUtils::subi(&mut tmp1, tmp2, value_byte_len);
            storage[h01201[found as usize] as usize]
                .as_mut_slice()
                .copy_from_slice(&tmp1);

            let mut tmp1 = storage[h01201[found as usize] as usize].clone();
            let tmp2 = &storage[h01201[(found + 2) as usize] as usize];
            ByteFuseUtils::subi(&mut tmp1, tmp2, value_byte_len);
            storage[h01201[found as usize] as usize]
                .as_mut_slice()
                .copy_from_slice(&tmp1);

            let mut tmp1 = storage[h01201[found as usize] as usize].clone();

            let mut tmp1 = storage[h01201[found as usize] as usize].clone();
            ByteFuseUtils::addi(&mut tmp1, value, value_byte_len);
            storage[h01201[found as usize] as usize]
                .as_mut_slice()
                .copy_from_slice(&tmp1);
        }
    }
}
