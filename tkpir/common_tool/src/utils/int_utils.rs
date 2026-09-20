use std::vec;

use bytebuffer::ByteBuffer;
use cipher::typenum::Integer;

use crate::utils::{BYTE_SIZE, INTEGER_BYTES};

pub struct IntUtils;

impl IntUtils {
    /// 大端表示
    pub fn u32_to_byte_vec(value: u32) -> [u8; 4] {
        value.to_be_bytes()
    }
    pub fn byte_vec_to_u32(value: [u8; 4]) -> u32 {
        // assert!(value.len() == 4);

        u32::from_be_bytes(value)
    }

    pub fn byte_vec_to_u64(value: [u8; 8]) -> u64 {
        u64::from_be_bytes(value)
    }

    /// 大端表示
    pub fn int_to_byte_vec(value: i32) -> [u8; 4] {
        value.to_be_bytes()
    }
    /// 大端表示
    /// 因为数据类型一定是i32 所以直接在参数部分把 长度限制死
    /// 这样非法的输入之间无法通过编译
    pub fn byte_vec_to_int(value: [u8; 4]) -> i32 {
        i32::from_be_bytes(value)
    }

    /// 将指定长度的value 转换为int，大端表示。转换结果一定为正整数。
    /// 如果 value 的长度 小于 4-bytes, 则在前面补0 后转换
    /// 如果 values 的长度  大于 4-byte，则只取最后 4-bytes，并转换
    pub fn fixed_byte_array_to_non_neg_int(value: &[u8]) -> i32 {
        assert!(value.len() > 0);
        if value.len() >= 4 {
            // 超过了表示范围，只取剩下的 4-byte
            let value_last_byte: [u8; 4] = value[(value.len() - 4)..].try_into().unwrap();
            // 对标 ByteBuffer.wrap(paddingValue).getInt();
            // 按 Big-endian 解读 bytes
            let output = i32::from_be_bytes(value_last_byte);
            assert!(output >= 0);
            return output;
        } else {
            // 如果不够 4-bytes，则填充到4-bytes
            // 例如 value 是 1-byte [0x14]
            // 新建 [0, 0, 0, 0]
            // 简单的 copy [0, 0, 0, 0x14]
            let mut padding = [0u8; 4];
            // 拷贝的起点是 4 - value.len() , 终点是 4
            // 利用 slice 的拷贝方法

            let _ = &padding[4 - value.len()..].copy_from_slice(&value[..]);
            // println!("value: {:?}", value);
            // println!("padding: {:?}", padding);

            let output = i32::from_be_bytes(padding);
            // println!("output: {}", output);
            assert!(output >= 0);
            return output;
        }
    }

    /// 把非负整数 转换为指定长度的 bytes
    pub fn non_neg_int_to_fixed_byte_vec(value: i32, byte_len: usize) -> Vec<u8> {
        assert!(value >= 0);
        assert!(byte_len >= 0);

        if byte_len > 4 {
            let mut output = vec![0u8; byte_len];
            let value_byte = value.to_be_bytes();
            // 拷贝
            // 起点是 byte_len - value_byte.len()
            let _ = &output[byte_len - value_byte.len()..].copy_from_slice(&value_byte[..]);
            return output;
        } else {
            // 验证给定的 bytes 能够装得下 values
            // 这里比较的时候 要给出足够大的 数据类型，不然会报 overflow 的panic，因为用户给的 byte_len 可能很大
            assert!(value as i128 <= (1 << (byte_len * BYTE_SIZE)));
            // 先转换，再复制
            let value_byte = value.to_be_bytes();
            let mut output = vec![0u8; byte_len];
            // byte_len 是小于4的, 而value_byte 则一定是 4的
            let start = value_byte.len() - output.len();
            let _ = &output[..].copy_from_slice(&value_byte[start..]);
            return output;
        }
    }
    /// 默认 byte_array 是 big-endian
    pub fn byte_array_to_u32_array(byte_array: &[u8]) -> Vec<u32> {
        assert!(byte_array.len() > 0 && byte_array.len() % 4 == 0);

        let u32_len = byte_array.len() / INTEGER_BYTES;
        let mut u32_array = Vec::with_capacity(u32_len);

        for idx in 0..u32_len {
            // convert &[u8] to [u8; _] , using try_into().unwrap()
            u32_array.push(u32::from_be_bytes(
                byte_array[idx * 4..(idx + 1) * 4].try_into().unwrap(),
            ));
        }

        return u32_array;
    }
    /// 默认 byte_array 是 big-endian
    pub fn u32_array_to_byte_array(u32_array: &[u32]) -> Vec<u8> {
        assert!(u32_array.len() > 0);

        let byte_len = u32_array.len() * INTEGER_BYTES;
        let mut byte_array = vec![0u8; byte_len];

        for idx in 0..u32_array.len() {
            //
            byte_array[idx * 4..(idx + 1) * 4]
                .copy_from_slice(u32_array[idx].to_be_bytes().as_slice());
        }

        return byte_array;
    }

    /// Convert bytebuffer::ByteBuffer to Vec<u32> using read_u32 api
    pub fn byte_buffer_to_u32_array(byter_buffer: &mut ByteBuffer) -> Vec<u32> {
        // 长度必须是整数个 bytes
        assert!(byter_buffer.len() > 0 && byter_buffer.len() % 8 == 0);

        let u32_num = byter_buffer.len() / 8;

        let mut res = vec![];
        for _ in 0..u32_num {
            res.push(byter_buffer.read_u32().unwrap());
        }
        res
    }

    /// 默认 byte_array 是 big-endian
    pub fn byte_array_to_i32_array(byte_array: &[u8]) -> Vec<i32> {
        assert!(byte_array.len() > 0 && byte_array.len() % 4 == 0);

        let i32_len = byte_array.len() / INTEGER_BYTES;
        let mut i32_array = Vec::with_capacity(i32_len);

        for idx in 0..i32_len {
            // convert &[u8] to [u8; _] , using try_into().unwrap()
            i32_array.push(i32::from_be_bytes(
                byte_array[idx * 4..(idx + 1) * 4].try_into().unwrap(),
            ));
        }

        return i32_array;
    }
    /// 默认 byte_array 是 big-endian
    pub fn i32_array_to_byte_array(i32_array: &[i32]) -> Vec<u8> {
        assert!(i32_array.len() > 0);

        let byte_len = i32_array.len() * INTEGER_BYTES;
        let mut byte_array = vec![0u8; byte_len];

        for idx in 0..i32_array.len() {
            //
            byte_array[idx * 4..(idx + 1) * 4]
                .copy_from_slice(i32_array[idx].to_be_bytes().as_slice());
        }

        return byte_array;
    }
}

#[cfg(test)]
mod IntUtilsTest {
    use super::*;

    const MAX_ITERATIONS: usize = 100;
    use rand::rngs::OsRng;
    use rand::{Rng, RngCore};

    #[test]
    fn int_utils_int_byte_vec() {
        test_int_byte_vec(0, [0u8, 0, 0, 0]);
        test_int_byte_vec(1, [0u8, 0, 0, 0x01]);
        test_int_byte_vec(-1, [0xff, 0xff, 0xff, 0xff]);
        test_int_byte_vec(i32::MAX, [0x7f, 0xff, 0xff, 0xff]);
        test_int_byte_vec(i32::MIN, [0x80, 0x00, 0x00, 0x00]);
    }

    fn test_int_byte_vec(value: i32, byte_vec: [u8; 4]) {
        let convert_byte_vec = IntUtils::int_to_byte_vec(value);
        assert_eq!(convert_byte_vec, byte_vec);

        let convert_value = IntUtils::byte_vec_to_int(byte_vec);
        assert_eq!(value, convert_value);
    }

    #[test]
    fn int_fixed_byte_arr() {
        // 0
        test_int_fixed_byte_arr(0, vec![0x00, 0x00, 0x00, 0x00]);
        test_int_fixed_byte_arr(0, vec![0x00]);
        test_int_fixed_byte_arr(0, vec![0x00, 0x00, 0x00, 0x00, 0x00]);
        // 正数
        test_int_fixed_byte_arr(1, vec![0x00, 0x00, 0x00, 0x01]);
        test_int_fixed_byte_arr(1, vec![0x01]);
        test_int_fixed_byte_arr(1, vec![0x00, 0x00, 0x00, 0x00, 0x01]);
        // 最大值
        test_int_fixed_byte_arr((1 << BYTE_SIZE) - 1, vec![0xFF]);
        test_int_fixed_byte_arr((1 << 2 * BYTE_SIZE) - 1, vec![0xFF, 0xFF]);
        test_int_fixed_byte_arr((1 << 3 * BYTE_SIZE) - 1, vec![0xFF, 0xFF, 0xFF]);
        test_int_fixed_byte_arr(i32::MAX, vec![0x7F, 0xFF, 0xFF, 0xFF]);
        test_int_fixed_byte_arr(i32::MAX, vec![0x00, 0x7F, 0xFF, 0xFF, 0xFF]);
    }

    fn test_int_fixed_byte_arr(value: i32, byte_arr: Vec<u8>) {
        let convert_byte_arr = IntUtils::non_neg_int_to_fixed_byte_vec(value, byte_arr.len());
        assert_eq!(convert_byte_arr, byte_arr);

        let convert_value = IntUtils::fixed_byte_array_to_non_neg_int(&byte_arr);
        assert_eq!(convert_value, value);
    }

    #[test]
    fn u32_array_byte_array_test() {
        test_u32_array_byte_array(vec![0x00 as u32], vec![0x00 as u8, 0x00, 0x00, 0x00]);

        test_u32_array_byte_array(
            vec![u32::MIN, 1, u32::MAX],
            vec![
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF,
            ],
        )
    }

    fn test_u32_array_byte_array(u32_array: Vec<u32>, byte_array: Vec<u8>) {
        let convert_byte_array = IntUtils::u32_array_to_byte_array(&u32_array);
        assert_eq!(convert_byte_array, byte_array);

        let convert_u32_array = IntUtils::byte_array_to_u32_array(&byte_array);
        assert_eq!(convert_u32_array, u32_array);
    }

    #[test]
    fn i32_array_byte_array_test() {
        test_i32_array_byte_array(vec![0x00 as i32], vec![0x00 as u8, 0x00, 0x00, 0x00]);

        test_i32_array_byte_array(
            vec![i32::MIN, -1, 0, 1, i32::MAX],
            vec![
                0x80, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x01, 0x7F, 0xFF, 0xFF, 0xFF,
            ],
        )
    }

    fn test_i32_array_byte_array(i32_array: Vec<i32>, byte_array: Vec<u8>) {
        let convert_byte_array = IntUtils::i32_array_to_byte_array(&i32_array);
        assert_eq!(convert_byte_array, byte_array);

        let convert_u32_array = IntUtils::byte_array_to_i32_array(&byte_array);
        assert_eq!(convert_u32_array, i32_array);
    }
}
