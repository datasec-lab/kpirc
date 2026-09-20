use super::*;

pub struct BytesUtils {}

impl BytesUtils {
    /// 2个Vec<u8> 逐位and
    /// 结果保存在第一个元素
    pub fn and_i(x1: &mut [u8], x2: &[u8]) {
        assert!(x1.len() == x2.len());

        let len = x2.len();
        for i in (0..len).rev() {
            x1[i] = x1[i] & x2[i];
        }
    }
    /// 2个Vec<u8> 逐位and
    pub fn and(x1: &[u8], x2: &[u8]) -> Vec<u8> {
        assert!(x1.len() == x2.len());

        let res = x1.iter().zip(x2.iter()).map(|(a, b)| a & b).collect();

        res
    }

    /// 结果放在第一个元素
    pub fn xor_mut(x1: &mut [u8], x2: &[u8]) {
        assert!(x1.len() == x2.len());

        let len = x2.len();
        for i in (0..len).rev() {
            x1[i] = x1[i] ^ x2[i];
        }
    }
    /// 2个Vec<u8> 逐位XOR
    pub fn xor(x1: &[u8], x2: &[u8]) -> Vec<u8> {
        assert!(x1.len() == x2.len());

        let res = x1.iter().zip(x2.iter()).map(|(a, b)| a ^ b).collect();

        res
    }
    /// 修正有效位
    /// 给定 byte_vec, 需要设置的有效位的位数，即比特位数
    /// 即把所有的非有效位设置为 0
    /// 例如 比特位数为9, 但是应该用 u8 来存储，需要2个u8
    /// 一共16个比特为，需要把高7位置为 0， 低9位保留不动
    pub fn reduce_byte_vec(byte_vec: &mut [u8], bit_len: usize) {
        assert!(bit_len <= byte_vec.len() * BYTE_SIZE);
        // 非有效位置为false
        for bin_idx in 0..byte_vec.len() * BYTE_SIZE - bit_len {
            BinaryUtils::set_boolean(byte_vec, bin_idx, false)
        }
    }

    /// 将比特矩阵的第y列 设置为 byte_vec
    /// 比特矩阵的第y列的比特行数 是 self.abs_trans_bit_matrix.rows
    /// byte_vec 是Vec<u8>， 存在补0的情况
    /// 例如，比特行数为 9，那么需要用2个u8来存储，一共 提供 2*8 = 16 个比特
    /// 但是我们只取其中 低9位，这里是在验证，给定的 byte_vec 的有效位数是否为预期的 self.abs_trans_bit_matrix.rows
    /// 验证方法也很简单，即 非有效位数是否全部为0
    pub fn is_reduce_byte_vec(byte_vec: &[u8], bit_len: usize) -> bool {
        // 这里的bitLength指的是要保留多少个比特位，因此可以取到[0, byteArray.length * Byte.SIZE]
        assert!(bit_len <= byte_vec.len() * BYTE_SIZE);

        // 这里是在遍历所有的非有效位 是否位0
        for bit_idx in 0..byte_vec.len() * BYTE_SIZE - bit_len {
            if BinaryUtils::get_bool(byte_vec, bit_idx) {
                return false;
            }
        }
        // 也就是 [0..byte_vec.len() * BYTE_SIZE - bit_len] 这个范围的 bit 需要全部为 0
        // |--a--|--bit-en--|，也就是 a 这部分需要全部为 0，即无效位数需要全部为0
        return true;
    }

    pub fn hamming_distance(a: &[u8], b: &[u8]) -> usize {
        assert!(a.len() == b.len());
        let xor = BinaryUtils::byte_vec_to_binary(&BytesUtils::xor(a, b));
        let mut hamming_distance = 0;
        for bit in xor.iter() {
            if *bit {
                hamming_distance += 1;
            }
        }
        return hamming_distance;
    }

    /// 填充给定 byte 到指定长度 length
    /// 即高位填充0
    pub fn padding_byte_array(byte_array: &[u8], length: usize) -> Vec<u8> {
        assert!(byte_array.len() <= length);
        if byte_array.len() == length {
            return byte_array.to_vec();
        }
        let mut padding = vec![0u8; length];

        let _ = &padding[(length - byte_array.len())..].copy_from_slice(&byte_array[..]);
        return padding;
    }
}
#[cfg(test)]
mod BytesUtilsTest {

    use super::*;

    #[test]
    fn xor_i_test() {
        let mut x1 = vec![1, 1, 1, 1];
        let x2 = vec![1, 1, 1, 1];
        BytesUtils::xor_mut(&mut x1, &x2);
        assert!(x1 == vec![0, 0, 0, 0]);
    }

    #[test]
    fn xor_all() {
        let x1 = vec![1, 2, 3, 4];
        let x2 = vec![1, 2, 3, 4];
        assert!(BytesUtils::xor(&x1, &x2) == vec![0, 0, 0, 0]);

        let x1 = vec![1, 2, 3, 4];
        let x2 = vec![0, 0, 0, 0];
        assert!(BytesUtils::xor(&x1, &x2) == vec![1, 2, 3, 4]);
    }

    #[test]
    fn padding() {
        let a = vec![0u8, 0, 0, 0, 1, 2, 3];
        let b = vec![1u8, 2, 3];

        assert_eq!(a, BytesUtils::padding_byte_array(&b, a.len()));
    }
}
