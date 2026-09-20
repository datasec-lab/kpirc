use super::PrfType;
/// 伪随机函数（Pseudo-random Function）trait
use crate::utils::BinaryUtils;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

pub trait Prf {
    // 返回输出字节长度
    fn output_byte_len(&self) -> usize;

    /// 设置密钥。密钥将被拷贝，以放置后续可能的修改。
    fn set_key(&mut self, key: Vec<u8>);

    /// 返回密钥
    fn key(&self) -> Vec<u8>;

    /// 返回给定消息所对应的随机结果
    fn get_bytes(&self, message: Vec<u8>) -> Vec<u8>;

    /// 返回给定消息所对应的布尔值
    fn get_bool(&self, message: Vec<u8>) -> bool {
        let output_byte_array = self.get_bytes(message);
        return BinaryUtils::get_bool(&output_byte_array, output_byte_array.len() - 1);
    }

    /// 返回给定消息对应[0, bound)的整数值。
    fn get_integer(&self, message: Vec<u8>, upper_bound: u32) -> u32 {
        assert!(upper_bound > 0);
        assert!(
            self.output_byte_len() as usize >= U32_BYTES,
            "left: {}, right: {}",
            self.output_byte_len(),
            U32_BYTES
        );
        // 上界为1，自然返回值为 0
        if upper_bound == 1 {
            return 0;
        }
        let byte_array = self.get_bytes(message);
        // return Math.abs(ByteBuffer.wrap(byteArray).getInt(byteArray.length - Integer.BYTES) % upperBound);
        let idx = byte_array.len() - U32_BYTES as usize;
        return u32::from_be_bytes(byte_array[idx..].try_into().unwrap()) % upper_bound;
    }

    fn get_integer_with_index(&self, index: u32, message: Vec<u8>, upper_bound: u32) -> u32 {
        assert!(index > 0);
        // 0. 新建足够长度的buffers, 4 = u32::BITS/8
        let mut buffer = vec![0u8; message.len() + U32_BYTES];
        // 1. index -> byte array
        let index_byte = index.to_be_bytes();
        // 2. 拼接
        buffer[..U32_BYTES].copy_from_slice(&index_byte[..]);
        buffer[U32_BYTES..].copy_from_slice(&message[..]);

        return self.get_integer(buffer, upper_bound);
    }
    /// 返回给定消息对应[lowerBound, upperBound)的整数值。
    fn get_integer_with_lower_bound(
        &self,
        message: Vec<u8>,
        lower_bound: u32,
        upper_bound: u32,
    ) -> u32 {
        assert!(lower_bound < upper_bound);

        return self.get_integer(message, upper_bound - lower_bound) + lower_bound;
    }

    /// 返回给定消息对应[0, bound)的整数值。
    fn get_long(&self, message: Vec<u8>, upper_bound: u64) -> u64 {
        assert!(upper_bound > 0);
        assert!(self.output_byte_len() as usize >= U64_BYTES);
        // 上界为1，自然返回值为 0
        if upper_bound == 1 {
            return 0;
        }
        let byte_array = self.get_bytes(message);
        // return Math.abs(ByteBuffer.wrap(byteArray).getInt(byteArray.length - Integer.BYTES) % upperBound);
        let idx = byte_array.len() - U64_BYTES as usize;
        return u64::from_be_bytes(byte_array[idx..].try_into().unwrap()) % upper_bound;
    }

    fn get_long_with_index(&self, index: u32, message: Vec<u8>, upper_bound: u64) -> u64 {
        // assert!(index > 0);
        // 0. 新建足够长度的buffers, 4 = u32::BITS/8
        let mut buffer = vec![0u8; message.len() + U32_BYTES];
        // 1. index -> byte array
        let index_byte = index.to_be_bytes();
        // 2. 拼接
        buffer[..U32_BYTES].copy_from_slice(&index_byte[..]);
        buffer[U32_BYTES..].copy_from_slice(&message[..]);

        return self.get_long(buffer, upper_bound);
    }
    /// 返回给定消息对应[0, 1)的浮点数。
    fn get_double(&self, message: Vec<u8>) -> f32 {
        //  注意获得[0,1) 之间浮点数的正确方法
        //  (self.get_integer(message, u32::MAX)  / u32::MAX) as f32; 只能得到0
        return self.get_integer(message, u32::MAX) as f32 / u32::MAX as f32;
    }

    ///返回伪随机函数类型
    fn prf_type(&self) -> PrfType;
}
