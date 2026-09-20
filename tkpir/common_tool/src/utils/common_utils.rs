use rand_core::{CryptoRng, RngCore};

// const BYTE_SIZE: usize = 8;
use super::BYTE_SIZE;
use crate::CommonConstants;
/// 公共工具类
pub struct CommonUtils;

use crate::crypto::SecureRandom;

impl CommonUtils {
    /// 在给定的单位长度下，至少需要多少单位长度才能容纳输入的长度。
    pub fn get_unit_num(len: usize, unit_len: usize) -> usize {
        // 就是向上取整
        (len + unit_len - 1) / unit_len
    }

    /// 比特长度 转换为 字节长度，本质上是以 8 为base 的向上取整
    /// <=8 -> 1 , 9-16 -> 2 ....
    pub fn get_byte_len(bit_len: usize) -> usize {
        return CommonUtils::get_unit_num(bit_len, BYTE_SIZE);
    }

    /**
     * @description: 生成随机密钥
     * @param {*} rng
     * @return {*}
     */
    pub fn generate_random_key(rng: &mut SecureRandom) -> Vec<u8> {
        let mut key = vec![0u8; CommonConstants::BLOCK_BYTE_LENGTH as usize];
        rng.fill_bytes(&mut key);
        key
    }

    /**
     * @description: 生成随机密钥数组
     * @param {*} rng
     * @return {*}
     */
    pub fn generate_random_keys(key_num: usize, rng: &mut SecureRandom) -> Vec<Vec<u8>> {
        (0..key_num)
            .into_iter()
            .map(|_| CommonUtils::generate_random_key(rng))
            .collect()
    }
}
