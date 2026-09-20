use aes::cipher::block_padding::Pkcs7;
// GenericArray doc：
// https://docs.rs/generic-array/latest/generic_array/struct.GenericArray.html#method.from_slice
use aes::cipher::{
    generic_array::{typenum::U16, GenericArray},
    BlockCipher, BlockDecrypt, BlockEncrypt, KeyInit,
};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut};
use aes::{Aes128, Aes128Dec, Aes128Enc};

use log;

use super::prp::Prp;
use super::prp_factory::PrpType;
use crate::common_constants::CommonConstants;
/// AES伪随机置换
/// 主要是基于rust下的 aes,cipher这些库来构造, 模仿mpc4j中的 JdkAesPrp.java

pub struct AesEcbPrp {
    // encrypt_cipher: Option<Aes128Enc>,
    // decrypt_cipher: Option<Aes128Dec>,
    cipher: Option<Aes128>,
}

impl AesEcbPrp {
    pub fn new() -> Self {
        // Self { encrypt_cipher: None, decrypt_cipher: None }
        Self { cipher: None }
    }
}

impl Prp for AesEcbPrp {
    fn set_key(&mut self, key: Vec<u8>) {
        assert_eq!(key.len(), CommonConstants::BLOCK_BYTE_LENGTH);

        // 初始化 encrypt_cipher and  decrypt_cipher
        // convert [u8; 16] to GenericArray,
        // self.encrypt_cipher = Some(Aes128Enc::new(&key.into()));
        // self.decrypt_cipher = Some(Aes128Dec::new(&key.into()));
        // let len = key.len();
        // let a: GenericArray<u8, len> = GenericArray::clone_from_slice(&key);

        //
        // let key_arr:GenericArray<_, U16> = GenericArray::clone_from_slice(&key[..]);
        // self.encrypt_cipher = Some(Aes128Enc::new(GenericArray::from_slice(&key)));
        // self.decrypt_cipher = Some(Aes128Dec::new(GenericArray::from_slice(&key)));
        self.cipher = Some(Aes128::new(GenericArray::from_slice(&key)));
    }

    fn prp(&self, plaintext: &[u8]) -> Vec<u8> {
        // 保证不为none
        // assert!(self.encrypt_cipher.is_some());
        assert!(self.cipher.is_some());
        assert!(plaintext.len() == CommonConstants::BLOCK_BYTE_LENGTH);
        // 1. 直接加密
        // 注意,encrypt_block 接收的合法参数类型是 GenericArray,所以这里需要转换一下
        // 直接 unwrap() 会转移 encrypt_cipher的所有权
        // self.encrypt_cipher.as_ref().unwrap().encrypt_block(&mut plaintext.into());
        // return plaintext; // 这样直接返回得到的不是密文！

        // 下面才是加密的正确打开方式
        // 注意必须要使用 clone_from_slice ,传入的 plaintext 是引用，否则该引用背后的值会发生变化
        let mut buffer_array = GenericArray::clone_from_slice(plaintext);

        // self.encrypt_cipher.as_ref().unwrap().encrypt_block(&mut buffer_array);
        self.cipher
            .as_ref()
            .unwrap()
            .encrypt_block(&mut buffer_array);

        return buffer_array
            .as_slice()
            .try_into()
            .expect("Convert slice to array failed.");
    }

    fn inv_prp(&self, ciphertext: &[u8]) -> Vec<u8> {
        // 保证不为none
        //  assert!(self.decrypt_cipher.is_some());
        assert!(self.cipher.is_some());
        assert!(ciphertext.len() == CommonConstants::BLOCK_BYTE_LENGTH);
        //  // 1. 直接解密
        //  // 直接 unwrap() 会转移 encrypt_cipher的所有权

        //  self.decrypt_cipher.as_ref().unwrap().decrypt_block(&mut ciphertext.into());
        //  return ciphertext;

        // 下面才是解密的正确打开方式
        let mut buffer_array = GenericArray::clone_from_slice(ciphertext);
        // self.decrypt_cipher.as_ref().unwrap().decrypt_block(&mut buffer_array);
        self.cipher
            .as_ref()
            .unwrap()
            .decrypt_block(&mut buffer_array);
        return buffer_array
            .as_slice()
            .try_into()
            .expect("Convert slice to array failed.");
    }
    fn prp_type(&self) -> PrpType {
        PrpType::AES_ECB_PRP
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand;
    use rand::RngCore;

    use env_logger::{Builder, Target};
    use std::env;
    // use log::info;
    fn init_logger() {
        let mut builder = Builder::from_default_env();
        builder.target(Target::Stdout);
        builder.init();
        // let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn aes_prp_test() {
        // 0.
        let mut prp = AesEcbPrp::new();
        // 1. 随机Key,随机明文
        for _ in 0..10 {
            let mut key = vec![0u8; 16];
            rand::thread_rng().fill_bytes(&mut key);
            prp.set_key(key);
            for _ in 0..100 {
                // 2.
                let mut plaintext = [0u8; 16];
                // print!("p: {:?}", plaintext);
                // rand::thread_rng().fill_bytes(&mut plaintext);
                // 3.
                let mut ct = prp.prp(&mut plaintext);
                // print!("after enc p: {:?}", plaintext);
                // println!("ct: {:?}", ct);
                // // 4.
                let dec_ct = prp.inv_prp(&mut ct);
                // print!("after enc p: {:?}", plaintext);
                // // 5.
                assert!(plaintext.to_vec() == dec_ct);
            }
        }
    }
}
