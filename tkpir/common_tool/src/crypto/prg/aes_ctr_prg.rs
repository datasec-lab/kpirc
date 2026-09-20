use super::prg::Prg; //trait
use super::prg_factory::PrgType;
use crate::common_constants::CommonConstants;
use aes::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};

type Aes128Ctr64LE = ctr::Ctr64LE<aes::Aes128>;

pub struct AesCtrPrg {
    output_byte_len: usize,
}
// 初始向量为全0
const IV: [u8; 16] = [0u8; 16];

impl AesCtrPrg {
    pub fn new(output_byte_len: usize) -> Self {
        Self { output_byte_len }
    }
}

impl Prg for AesCtrPrg {
    fn output_byte_len(&self) -> usize {
        self.output_byte_len
    }
    fn prg_type(&self) -> PrgType {
        PrgType::AES_CTR_PRG
    }

    fn extend_to_bytes(&self, seed: &[u8]) -> Vec<u8> {
        // log::info!("extend_to_bytes in aes_ctr_prg");
        // log::info!("stuck on this line");
        // log::info!("seed.len(): {}", seed.len());
        // log::info!("CommonConstants::BLOCK_BYTE_LENGTH: {}", CommonConstants::BLOCK_BYTE_LENGTH);

        // 种子的长度一定需要等于 16-bytes
        assert_eq!(seed.len(), CommonConstants::BLOCK_BYTE_LENGTH);
        // assert!(seed.len() ==  CommonConstants::BLOCK_BYTE_LENGTH);
        // log::info!("after assert_eq");
        // seed作为key, 需要先转换为 [u8], 否则 后面的new 会出问题
        // let key: [u8; 16] = seed.try_into().unwrap();
        // log::info!("seed.try_into().unwrap()");
        let mut cipher = Aes128Ctr64LE::new(seed.into(), &IV.into());
        // prg是加密全0明文 , 这个全0的明文的长度是不限制的
        // 这个就实现了 Prg定义中的 将一个 short random number 变成了一个 long
        let mut plaintext = vec![0u8; self.output_byte_len];
        // in-place encrypt
        // log::info!("before apply_keystream");
        cipher.apply_keystream(&mut plaintext);
        // log::info!("after apply_keystream");
        return plaintext;
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::RngCore;

    #[test]
    fn aes_ctr_test() {
        let aes_ctr = AesCtrPrg::new(16);

        // 随机填充
        let mut seed = vec![0u8; 16];
        rand::thread_rng().fill_bytes(&mut seed);

        let random_vec = aes_ctr.extend_to_bytes(&seed);
        println!("{:?}", random_vec); // cargo test aes_ctr_test -- --show-output
        let random_bytes: [u8; 16] = random_vec.try_into().unwrap();
        // convert [u8; ] to integer, which is randomed.
        let random_num_be = u128::from_be_bytes(random_bytes);
        println!("random_num_be: {:?}", random_num_be);
        let random_num_le = u128::from_le_bytes(random_bytes);
        println!("random_num_le: {:?}", random_num_le);
    }
}
