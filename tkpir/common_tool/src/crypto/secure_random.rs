use rand::rngs::OsRng;
use rand::Rng;
use rand_core::{CryptoRng, RngCore};

/**
 * @description: 在 OsRng 上包装了一层，包装了一层自己的 SecureRandom
 * 提供 RngCore 的功能，对标 mpc4j
 * @return {*}
 */
#[derive(Debug)]
pub struct SecureRandom;

impl SecureRandom {
    pub fn new() -> Self {
        Self {}
    }

    pub fn next_u32(&mut self) -> u32 {
        OsRng.next_u32()
    }

    pub fn next_u64(&mut self) -> u64 {
        OsRng.next_u64()
    }

    pub fn fill_bytes(&mut self, dest: &mut [u8]) {
        OsRng.fill_bytes(dest)
    }

    pub fn gen_bool(&mut self) -> bool {
        OsRng.gen_bool(1.0 / 2.0)
    }

    // pub fn gen_range(&mut self, range: Range)
}
