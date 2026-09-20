pub mod cwutils;
pub mod tfhe_utils;

use std::{num, u8};

use bincode::serialize;
use blake2::{Blake2s256, Digest};
use bytebuffer::ByteBuffer;
use num_traits::pow;
use rayon::ThreadPoolBuilder;
use serde::Serialize;

#[macro_export]
macro_rules! time {
    ($label: expr, $block: expr) => {{
        let start = std::time::Instant::now();

        let result = $block;

        let elapsed = start.elapsed();

        println!("{} took: {:?}", $label, elapsed);

        (result, elapsed)
    }};
}

pub fn calc_total_comm_cost<T>(data: &[T], total: &mut f64)
where
    T: Serialize,
{
    let data_serialized = data
        .iter()
        .map(|enc| serialize(enc).unwrap())
        .collect::<Vec<_>>();

    let bytes: usize = data_serialized.iter().map(|q| q.len()).sum();

    let kbytes: f64 = bytes as f64 / 1024.0;

    *total += kbytes;
}

pub fn set_num_threads(num_threads: Option<usize>) {
    match num_threads {
        Some(num_threads) => {
            ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build_global()
                .expect("Failed to build thread pool");
        }
        None => {}
    }
}

pub fn partition_u8(value: u8, value_bitlen: usize, partition_bitlen: usize) -> Vec<u8> {
    assert!(partition_bitlen <= value_bitlen);
    assert!(value_bitlen % partition_bitlen == 0);

    let partition_num = value_bitlen / partition_bitlen;
    let mut res = Vec::with_capacity(partition_num);

    let mut num_bits_handled = 0usize;

    let partition_mask = (1 << partition_bitlen) - 1 as u8;

    while num_bits_handled < value_bitlen {
        if res.len() < partition_num {
            let partition = (value >> num_bits_handled) & partition_mask;
            res.push(partition);
        }
        num_bits_handled += partition_bitlen;
    }

    res
}

pub fn partition_u64(value: u64, value_bitlen: usize, partition_bitlen: usize) -> Vec<u8> {
    assert!(partition_bitlen <= value_bitlen);

    let num_partitions = (value_bitlen as f64 / partition_bitlen as f64).ceil() as usize;
    let mut res = Vec::with_capacity(num_partitions);
    let partition_mask = (1 << partition_bitlen) - 1 as u64;

    for i in 0..num_partitions {
        let partition = ((value >> (i * partition_bitlen)) & partition_mask) as u8;
        res.push(partition);
    }

    res
}

pub fn from_decomposition(radix: u64, digits: &[u8]) -> u64 {
    let mut result: u64 = 0;
    let mut power = 1;

    for &digit in digits {
        result += (digit as u64) * power;
        power *= radix;
    }

    result
}

pub fn decompose_hashed(value: u64, digest_byte_len: usize, partition_bitlen: usize) -> Vec<u8> {
    let mut hasher = Blake2s256::new();

    let key_buffer: ByteBuffer = ByteBuffer::from_vec(value.to_be_bytes().to_vec());
    hasher.update(key_buffer.as_bytes());
    let digest: Vec<u8> = hasher.finalize_reset()[..digest_byte_len]
        .try_into()
        .unwrap();

    let mut digest_decomposed = Vec::new();
    digest.iter().for_each(|&d| {
        digest_decomposed.extend_from_slice(&partition_u8(d, 8usize, partition_bitlen))
    });

    digest_decomposed
}

pub fn within_bit_range(x: u64, bitlen: usize) -> bool {
    x < 1 << bitlen
}

pub fn cal_num_partitions(value_bitlen: usize, partition_bitlen: usize) -> usize {
    (value_bitlen + partition_bitlen - 1) / partition_bitlen
}
