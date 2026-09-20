use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

use crate::crypto::prg::Prg;
use crate::crypto::prg::PrgFactory;
use crate::crypto::prg::PrgType;

use crate::crypto::prp::Prp;
use crate::crypto::prp::PrpFactory;
use crate::crypto::prp::PrpType;

use crate::common_constants::CommonConstants;
use crate::utils::BytesUtils;
use crate::utils::CommonUtils;

use super::Prf;
use super::PrfType;
/// 使用JDK的ECB-AES实现的PRF。方案构造来自于论文：
/// Chase M, Miao P. Private Set Intersection in the Internet Setting from Lightweight Oblivious PRF. CRYPTO 2020.
/// 第4.2节：Instantiation of Cryptographic Primitives。
pub struct AesCbcPrf {
    // 伪随机数生成器
    prg: Box<dyn Prg>,
    // 伪随机置换
    prp: Box<dyn Prp>,
    // 输出字节长度
    output_byte_len: usize,
    // 密钥
    key: Vec<u8>,
}

impl AesCbcPrf {
    pub fn new(output_byte_len: usize) -> Self {
        Self {
            output_byte_len,
            prg: PrgFactory::create_instance(&PrgType::AES_CTR_PRG, output_byte_len),
            prp: PrpFactory::create_instance(&PrpType::AES_ECB_PRP),
            key: vec![],
        }
    }
}

impl Prf for AesCbcPrf {
    /// core method
    /// 这显然不是一个 Getter方法，前缀使用 get 是符合规范的
    fn get_bytes(&self, mut message: Vec<u8>) -> Vec<u8> {
        assert!(message.len() > 0);
        let mut mac: Vec<u8> = vec![0u8; CommonConstants::BLOCK_BYTE_LENGTH];

        if message.len() == CommonConstants::BLOCK_BYTE_LENGTH {
            mac = self.prp.prp(&mut message);
        } else if message.len() < CommonConstants::BLOCK_BYTE_LENGTH {
            // let mut message = message;
            // let padd = vec![0u8; CommonConstants::BLOCK_BYTE_LENGTH - message.len()];
            // message.extend_from_slice(&padd[..]);
            message.append(&mut vec![
                0u8;
                CommonConstants::BLOCK_BYTE_LENGTH - message.len()
            ]);

            assert!(message.len() == CommonConstants::BLOCK_BYTE_LENGTH);
            // 再加密
            mac = self.prp.prp(&mut message);
        } else {
            // 如果输入长度大于了 一个BLOCK
            // 也就是以 block 的长度为单位，计算需要多少个block 才能容纳 message
            let block_num =
                CommonUtils::get_unit_num(message.len(), CommonConstants::BLOCK_BYTE_LENGTH);
            // 填充
            let mut padd_message =
                vec![0u8; block_num as usize * CommonConstants::BLOCK_BYTE_LENGTH];

            // println!("message len={}", message.len());
            // 拷贝 message 到 padd_message 的后端
            let dst_idx = padd_message.len() - message.len();
            // println!("block_num={}, padd_message.len()={}, dst_idx={}", block_num, padd_message.len(), dst_idx);
            padd_message[dst_idx..].copy_from_slice(&message[..]);

            let mut x = vec![0u8; CommonConstants::BLOCK_BYTE_LENGTH];
            for block_idx in 0..block_num as usize {
                // 按block大小拷贝到 x
                let idx_start = block_idx as usize * CommonConstants::BLOCK_BYTE_LENGTH;
                let idx_end = idx_start as usize + CommonConstants::BLOCK_BYTE_LENGTH;
                x[..].copy_from_slice(&padd_message[idx_start..idx_end]);
                //
                BytesUtils::xor_mut(&mut mac, &x);
                //
                mac = self.prp.prp(&mut mac);
            }
        }
        // 输出结果
        if self.output_byte_len as usize == CommonConstants::BLOCK_BYTE_LENGTH {
            return mac;
        } else if (self.output_byte_len as usize) < CommonConstants::BLOCK_BYTE_LENGTH {
            // 截断到用户指定长度
            // 保留前面的
            return mac[..self.output_byte_len as usize].to_vec();
        } else {
            return self.prg.extend_to_bytes(&mac);
        }
    }

    fn output_byte_len(&self) -> usize {
        self.output_byte_len
    }

    fn set_key(&mut self, key: Vec<u8>) {
        // Vec<u8> to [u8; 16]
        self.prp.set_key(key.clone());
        self.key = key;
    }
    fn key(&self) -> Vec<u8> {
        self.key.clone()
    }

    fn prf_type(&self) -> PrfType {
        PrfType::AES_CBC_PRF
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::RngCore;

    #[test] // cargo test aes_cbc_prf_test -- --show-output
    fn aes_cbc_prf_test() {
        let a = rand::thread_rng().next_u64();

        println!("a: {}", a);

        // 0.
        let output_byte_len = 16;
        let mut aes_prf = AesCbcPrf::new(output_byte_len);
        //1. 测试基础函数
        assert_eq!(aes_prf.prf_type(), PrfType::AES_CBC_PRF);
        // 2.
        assert_eq!(aes_prf.output_byte_len(), output_byte_len);
        // 3. 随机密钥
        let mut key = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut key);
        println!("key: {:?}", key);
        // 4.
        aes_prf.set_key(key.to_vec());
        // 5.
        assert_eq!(key[..], aes_prf.key());
        // 6.
        let mut message = vec![0u8; 16];
        rand::thread_rng().fill_bytes(&mut message);
        // let output = aes_prf.get_bytes(message.clone());
        // println!("output_bytes: {:?}", output);

        // 7. get integer
        let upper_bound = u32::MAX;
        let lower_bound = u32::MIN;
        println!(
            "get_integer: {:?}",
            aes_prf.get_integer(message.clone(), upper_bound)
        );
        println!(
            "get_integer_with_index: {:?}",
            aes_prf.get_integer_with_index(1, message.clone(), upper_bound)
        );
        println!(
            "get_integer_with_lower_bound: {}",
            aes_prf.get_integer_with_lower_bound(message.clone(), lower_bound, upper_bound)
        );

        println!("get_double: {}", aes_prf.get_double(message.clone()));
    }
}
