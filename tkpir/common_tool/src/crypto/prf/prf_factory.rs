use crate::EnvType;

use super::AesCbcPrf;
use super::Prf;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum PrfType {
    // AES-CBC-MAC伪随机函数
    AES_CBC_PRF,
}

impl Default for PrfType {
    fn default() -> Self {
        PrfType::AES_CBC_PRF
    }
}

pub struct PrfFactory;

impl PrfFactory {
    pub fn create_instance(prf_type: &PrfType, output_byte_len: usize) -> Box<dyn Prf> {
        match prf_type {
            PrfType::AES_CBC_PRF => Box::new(AesCbcPrf::new(output_byte_len)),
        }
    }
    /// EnvType::DEFAULT --> AES_CBC_PRF
    pub fn create_instance_with_env_type(
        env_type: EnvType,
        output_byte_len: usize,
    ) -> Box<dyn Prf> {
        match env_type {
            EnvType::DEFAULT => Box::new(AesCbcPrf::new(output_byte_len)),
            _ => panic!("Not supported env type."),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::RngCore;

    #[test]
    fn prf_factory_test() {
        // cargo test prf_factory_test -- --show-output
        // 1.
        let mut prf = PrfFactory::create_instance(&PrfType::AES_CBC_PRF, 16);
        //2.
        // 3. 随机密钥
        let mut key = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut key);
        println!("key: {:?}", key);
        prf.set_key(key.to_vec());
        // 4.
        // 5.
        assert_eq!(key[..], prf.key());
        // 6.
        let mut message = vec![0u8; 16];
        rand::thread_rng().fill_bytes(&mut message);
        // let output = prf.get_bytes(message.clone());
        // println!("output_bytes: {:?}", output);

        // 7. get integer
        let upper_bound = u32::MAX;
        let lower_bound = u32::MIN;
        println!(
            "get_integer: {:?}",
            prf.get_integer(message.clone(), upper_bound)
        );
        println!(
            "get_integer_with_index: {:?}",
            prf.get_integer_with_index(1, message.clone(), upper_bound)
        );
        println!(
            "get_integer_with_lower_bound: {}",
            prf.get_integer_with_lower_bound(message.clone(), lower_bound, upper_bound)
        );

        println!("get_double: {}", prf.get_double(message.clone()));
    }
}
