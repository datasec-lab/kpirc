use super::aes_ecb_prp::AesEcbPrp;
use super::prp::Prp;

use crate::env_type::EnvType;

/// 伪随机置换工厂
///
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum PrpType {
    // 使用AES_ECB， 并且不做padding
    // prp中，AES的明文长度一定是 16-bytes的整数倍吗？否则为何可以不做padding？
    AES_ECB_PRP,
}

pub struct PrpFactory;

impl PrpFactory {
    /// 合法的返回类型是 实现了 Prg trait的所有类型
    /// 动态分派
    fn create_instance_dynamic(prg_type: &PrpType) -> Box<dyn Prp> {
        match prg_type {
            PrpType::AES_ECB_PRP => Box::new(AesEcbPrp::new()),
        }
    }

    /// 合法的返回类型是 实现了 Prg trait的所有类型
    /// 静态分派
    fn create_instance_static(prg_type: &PrpType) -> impl Prp {
        match prg_type {
            PrpType::AES_ECB_PRP => AesEcbPrp::new(),
        }
    }
    // pub fn create_instance(prp_type: &PrpType) -> impl Prp {
    //     match prp_type {
    //         PrpType::AES_ECB_PRP => AesEcbPrp::new(),
    //     }
    // }

    /// 合法的返回类型是 实现了 Prg trait的所有类型
    /// 动态分派
    pub fn create_instance(prp_type: &PrpType) -> Box<dyn Prp> {
        match prp_type {
            PrpType::AES_ECB_PRP => Box::new(AesEcbPrp::new()),
        }
    }
    /// EnvType::DEFAULT ---> AesEcbPrp
    pub fn create_instance_env_type(env_type: EnvType) -> Box<dyn Prp> {
        match env_type {
            EnvType::DEFAULT => Box::new(AesEcbPrp::new()),
            _ => panic!("other env_type have not been implemented"),
        }
    }
    /// EnvType::DEFAULT ---> AesEcbPrp,
    pub fn create_thread_saftey(env_type: EnvType) -> Box<dyn Prp + Sync> {
        match env_type {
            EnvType::DEFAULT => Box::new(AesEcbPrp::new()),
            _ => panic!(),
        }
    }

    /// PrpType::AES_ECB_PRP
    pub fn create_default() -> Box<dyn Prp> {
        Box::new(AesEcbPrp::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger::{Builder, Target};
    use rand::RngCore;
    use std::env;
    // use log::info;
    fn init_logger() {
        let mut builder = Builder::from_default_env();
        builder.target(Target::Stdout);
        builder.init();
        // let _ = env_logger::builder().is_test(true).try_init();
    }
    #[test]
    fn prp_factory_test() {
        init_logger();

        // 0. create a instance of Prg
        let prp_type = PrpType::AES_ECB_PRP;
        // let out_len = 32;
        let mut prp = PrpFactory::create_instance(&prp_type);
        // 1. 调用prg的方法
        // 1. 随机Key,随机明文
        for _ in 0..10 {
            let mut key = vec![0u8; 16];
            rand::thread_rng().fill_bytes(&mut key);
            prp.set_key(key);
            for _ in 0..100 {
                // 2.
                let mut plaintext = [0u8; 16];
                rand::thread_rng().fill_bytes(&mut plaintext);
                // 3.
                let mut ct = prp.prp(&mut plaintext);
                // 4.
                let dec_ct = prp.inv_prp(&mut ct);
                // 5.
                assert!(plaintext.to_vec() == dec_ct);
            }
        }
    }
}
