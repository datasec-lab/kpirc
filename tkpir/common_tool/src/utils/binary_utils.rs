use super::CommonUtils;

use super::BYTE_SIZE;

const BYTE_BOOLEAN_TRUE_TABLE: [u8; 8] = [
    0b10000000, // 128
    0b01000000, // 64
    0b00100000, // 32
    0b00010000, // 16
    0b00001000, 0b00000100, 0b00000010, 0b00000001,
];

const BYTE_BOOLEAN_FALSE_TABLE: [u8; 8] = [
    0b01111111, // 就是对上面的取反
    0b10111111, 0b11011111, 0b11101111, 0b11110111, 0b11111011, 0b11111101, 0b11111110,
];

pub struct BinaryUtils {}

impl BinaryUtils {
    /// 给定字节数组，返回指定位置(bit位置)所对应的布尔值，大端表示。
    /// 把字节数组看作比特数组的一种压缩存储方式， 每一个元素包含了8个比特
    pub fn get_bool(byte_array: &[u8], bit_pos: usize) -> bool {
        // pos is bit-position
        // u32 has ensured that pos >= 0
        assert!(bit_pos < (byte_array.len() * BYTE_SIZE));
        // bit-position to byte-position
        let byte_index = bit_pos >> 3;
        let bit_index = bit_pos & 0x07;

        return ((byte_array[byte_index]) & BYTE_BOOLEAN_TRUE_TABLE[bit_index]) != 0;
    }

    /// 比较2个字节数组是否相等
    pub fn equals(a: &Vec<u8>, b: &Vec<u8>) -> bool {
        a == b
    }

    // pub fn byte_vec_to_binary_double_loop(x: &Vec<u8>) -> Vec<bool> {
    //     if x.len() == 0 {
    //         return Vec::new();
    //     }
    //     // 双循环来热个身
    //     let mut res: Vec<bool> = vec![false; x.len() * BYTE_SIZE];
    //     // let mut res: Vec<bool> = Vec::with_capacity(x.len() * BYTE_SIZE)
    //     for i in 0..x.len() {
    //         let start = i * BYTE_SIZE;
    //         for j in 0..BYTE_SIZE {
    //             res[start + j] = (x[i] & BYTE_BOOLEAN_TRUE_TABLE[j]) != 0;
    //         }
    //     }
    //     return res;
    // }

    pub fn byte_vec_to_binary(byte_vec: &Vec<u8>) -> Vec<bool> {
        if byte_vec.len() == 0 {
            return Vec::new();
        }

        let mut res: Vec<bool> = Vec::with_capacity(byte_vec.len() * BYTE_SIZE);
        for x in byte_vec.iter() {
            res.append(&mut BinaryUtils::convert_u8_to_binary_with_right_shift(*x));
        }
        res
    }

    /// 实测性能最快
    fn byte_vec_to_binary_double_loop(byte_vec: &Vec<u8>) -> Vec<bool> {
        if byte_vec.len() == 0 {
            return Vec::new();
        }

        let mut res: Vec<bool> = Vec::with_capacity(byte_vec.len() * BYTE_SIZE);
        for x in byte_vec.iter() {
            res.append(&mut BinaryUtils::convert_u8_to_binary_with_right_shift(*x));
        }
        res
    }

    fn byte_vec_to_binary_iter(byte_vec: &Vec<u8>) -> Vec<bool> {
        if byte_vec.len() == 0 {
            return Vec::new();
        }
        // 注意这里要用 flat_map
        // x --> Vec<bool>  , 再把多个 Vec<bool> flat化
        let res: Vec<bool> = byte_vec
            .iter()
            .flat_map(|x| BinaryUtils::convert_u8_to_binary_with_right_shift(*x))
            .collect();

        res
    }

    fn convert_u8_to_binary_with_matrix(x: u8) -> Vec<bool> {
        let mut res: Vec<bool> = vec![false; 8];
        for j in 0..BYTE_SIZE {
            res[j] = (x & BYTE_BOOLEAN_TRUE_TABLE[j]) != 0;
        }
        return res;
    }

    pub fn convert_u8_to_binary_with_right_shift(mut x: u8) -> Vec<bool> {
        let mut res: Vec<bool> = vec![false; 8];
        // 从低位开始计算
        for i in (0..BYTE_SIZE).rev() {
            // 取低位
            res[i] = (x & 1) != 0;
            // 然后右移
            x >>= 1;
        }
        return res;
    }

    // pub fn byte_vec_to_binary_iter(byte_vec: &Vec<u8>) -> Vec<bool> {
    //     if byte_vec.len() == 0 {
    //         return Vec::new();
    //     }

    //     let res: Vec<bool> = byte_vec.iter().map(|x|

    //     )

    //     // 双循环来热个身
    //     let mut res: Vec<bool> = vec![false; x.len() * BYTE_SIZE];
    //     for i in 0..x.len() {
    //         let start = i * BYTE_SIZE;
    //         for j in 0..BYTE_SIZE {
    //             res[start + j] = (x[i] & BYTE_BOOLEAN_TRUE_TABLE[j]) != 0;
    //         }
    //     }
    //     return res;
    // }

    /// 将 bool 数组 转换为 byte 数组
    ///  0100,0011 <-> 0x43
    ///  0011,1010,0100,0011 <-> 0x3A,0x43
    /// 注意大端表示
    ///
    pub fn binary_to_round_byte_vec(binary: &[bool]) -> Vec<u8> {
        if binary.len() == 0 {
            return Vec::new();
        }
        // 根据比特长度 计算字节长度， 例如 8个比特 = 1-byte
        let byte_len = CommonUtils::get_byte_len(binary.len());
        // 因为上面是向上取整，转换后存在一个 offset
        let offset = byte_len * BYTE_SIZE - binary.len();
        // 转换逻辑是这样：0100,0011 <-> 0x43
        //             0011,1010,0100,0011 <-> 0x3A,0x43
        //   每8个 bit 为单位，将8个bit 转换为一个 byte，也就是 2位16进制
        //   8个比特 转换为2位16进制的时候， 采取的是大端表示，即左高右低
        let mut round_byte_vec = vec![0u8; byte_len];
        // 遍历每一个 real bit 进行转换
        // offset 在高位补0， 例如 只有5个比特  10111， 这个时候为了转换为 1-byte， 在高位补3个0: 0001 0111
        for i in 0..binary.len() {
            if binary[i] {
                // 就是把 bit 转换为 byte 对应的值
                BinaryUtils::set_boolean(&mut round_byte_vec, offset + i, true);
            }
        }
        return round_byte_vec;
    }
    /// 将 bool 数组 转换为 byte 数组
    /// 此处必须要求 输入的bianry 可以整除 8
    pub fn binary_to_byte_vec(binary: &[bool]) -> Vec<u8> {
        assert!(binary.len() % BYTE_SIZE == 0);

        if binary.len() == 0 {
            return vec![0];
        }
        let byte_len = binary.len() >> 3;
        let mut byte_vec = vec![0u8; byte_len];
        for byte_idx in 0..byte_len {
            let binary_idx_offset = byte_idx << 3;
            for idx in 0..BYTE_SIZE {
                if binary[binary_idx_offset + idx] {
                    byte_vec[byte_idx] |= BYTE_BOOLEAN_TRUE_TABLE[idx];
                }
            }
        }
        return byte_vec;
    }

    /// 给定字节数组, 将指定位置的值  设置为 指定 值， 大端表示
    /// 例如： [0x00, 0x10]   我现在将第 position = 0 的值设为 true， 那么值等于多少呢？
    /// 本质上把它展开： 0000 0000 0001 0000
    ///               1000 0000 0001 0000
    ///               0x80 0x10
    pub fn set_boolean(byte_vec: &mut [u8], position: usize, bool_val: bool) {
        // uszie 已经保证 position >= 0
        assert!(position < byte_vec.len() * BYTE_SIZE);
        // 输入的 position 是在 bit 中的位置
        // 这里要先寻找在byte 中的位置 ， 本质上就是 mod 8
        let byte_idx = position >> 3; // 在哪一个 byte， 就是整除 // 8
        let binary_idx = position & 0x07; // 在当前这个byte里的哪一个 bit
                                          // 例如 potition = 11 , 那么显然应该在 第1个byte里
                                          //  0x07 = 0000 0111
                                          //    11 = 0000 1011
                                          //    恰好等于   0000 0011，那么这里就是在当前byte 的第三个位置, 0-7 是第一个byte, 8 9 10 11 12 13 14 15
                                          //  那么 11 恰好是 0 1 2 3, 即idx=3
        if bool_val {
            byte_vec[byte_idx] |= BYTE_BOOLEAN_TRUE_TABLE[binary_idx];
        } else {
            byte_vec[byte_idx] &= BYTE_BOOLEAN_FALSE_TABLE[binary_idx];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 0100,0011 <-> 0x43
    // 0011,1010,0100,0011 <-> 0x3A,0x43
    #[test]
    fn binary_to_round_byte_vec_test() {
        let b1 = vec![false, true, false, false, false, false, true, true];
        let a1 = vec![0x43];
        assert_eq!(a1, BinaryUtils::binary_to_round_byte_vec(&b1));

        // 不足 8-bit，高位自动补0
        let b2 = vec![true, false, false, false, false, true, true];
        // let a1 = vec![0x43];
        assert_eq!(a1, BinaryUtils::binary_to_round_byte_vec(&b2));

        let b3 = vec![
            false, false, true, true, true, false, true, false, false, true, false, false, false,
            false, true, true,
        ];
        let a3 = vec![0x3A, 0x43];
        assert_eq!(a3, BinaryUtils::binary_to_round_byte_vec(&b3));
    }

    #[test]
    fn set_bool_test() {
        let mut byte_vec = vec![0x00_u8];
        let pos = 0;
        let bool_val = true;
        let expected = vec![0x80];
        BinaryUtils::set_boolean(&mut byte_vec, pos, bool_val);
        assert_eq!(expected, byte_vec);
    }

    #[test]
    fn byte_equal_test() {
        //0. 长度不等
        let a = vec![1, 2];
        let b = vec![1];
        assert_eq!(false, BinaryUtils::equals(&a, &b));
        //1. 长度相等，但是元素不等
        let a = vec![1, 2];
        let b = vec![1, 1];
        assert_eq!(false, BinaryUtils::equals(&a, &b));
        //2. 长度相等，元素相等
        let a = vec![1, 2];
        let b = vec![1, 2];
        assert_eq!(true, BinaryUtils::equals(&a, &b));
    }

    #[test]
    fn byte_vec_to_binary_right_test() {
        let x = vec![0_u8, 2, 255];
        let res = vec![
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, true, false, true, true, true, true, true, true, true, true,
        ];
        assert_eq!(res, BinaryUtils::byte_vec_to_binary_double_loop(&x));

        assert_eq!(res, BinaryUtils::byte_vec_to_binary_iter(&x));
    }

    use rand::rngs::OsRng;
    use rand::Rng;
    // #[test] // 非必要不做
    /**
     * double loop: 8905 ms
        iter: 11414 ms
    */
    fn byte_vec_to_binary_perforamnce_test() {
        let num = 128;
        let byte_vec: Vec<u8> = (0..num).into_iter().map(|_| OsRng.gen::<u8>()).collect();

        let loop_num = 100 * 100 * 100;
        let now = Instant::now();
        for _ in 0..loop_num {
            BinaryUtils::byte_vec_to_binary_double_loop(&byte_vec);
        }
        println!("double loop: {:?} ms", now.elapsed().as_millis());

        let now = Instant::now();
        for _ in 0..loop_num {
            BinaryUtils::byte_vec_to_binary_iter(&byte_vec);
        }
        println!("     iter: {:?} ms", now.elapsed().as_millis());
        // 结果很明显了，用double loop 吧
    }

    #[test]
    fn convert_u8_to_binary_right_test() {
        let x = 255_u8;
        let res = [true, true, true, true, true, true, true, true];

        assert_eq!(
            res.to_vec(),
            BinaryUtils::convert_u8_to_binary_with_right_shift(x)
        );
        assert_eq!(
            res.to_vec(),
            BinaryUtils::convert_u8_to_binary_with_matrix(x)
        );
    }

    use std::time::{Duration, Instant};
    // #[test] // 非必要不做性能测试，会比较慢
    fn convert_u8_to_binary_performance_test() {
        let x = 255_u8;
        let res = [true, true, true, true, true, true, true, true];

        // assert_eq!(res.to_vec(), BinaryUtils::convert_u8_to_binary_with_right_shift(x));
        // assert_eq!(res.to_vec(), BinaryUtils::convert_u8_to_binary_with_matrix(x));

        // cargo test convert_u8_to_binary_test --release -- --show-output
        // 性能测试, 太牛逼了，在 release 版本下，耗时都在 ns下了
        /**
        * ---- utils::binary_utils::tests::convert_u8_to_binary_test stdout ----
               matrix: 29ns
               right shift: 24ns
        */
        // 以后直接用 right shift 版本的吧
        let now = Instant::now();
        let max_time = 100 * 100 * 100 as usize;
        for _ in 0..max_time {
            BinaryUtils::convert_u8_to_binary_with_matrix(x);
        }
        println!("matrix: {:?}ns", now.elapsed().as_nanos());

        let now = Instant::now();
        for _ in 0..max_time {
            BinaryUtils::convert_u8_to_binary_with_right_shift(x);
        }
        println!("right shift: {:?}ns", now.elapsed().as_nanos());
    }
}
