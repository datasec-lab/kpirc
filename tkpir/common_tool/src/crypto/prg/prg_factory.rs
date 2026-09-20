use super::aes_ctr_prg::AesCtrPrg;
use super::prg::Prg;
use crate::EnvType;
use log;
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PrgType {
    // AES_ECB,
    AES_CTR_PRG,
}

pub struct PrgFactory;

impl PrgFactory {
    /// 合法的返回类型是 实现了 Prg trait的所有类型
    /// 动态分派
    fn create_instance_dynamic(prg_type: &PrgType, output_byte_len: usize) -> Box<dyn Prg> {
        match prg_type {
            PrgType::AES_CTR_PRG => Box::new(AesCtrPrg::new(output_byte_len)),
        }
    }

    /// 合法的返回类型是 实现了 Prg trait的所有类型
    /// 静态分派
    fn create_instance_static(prg_type: &PrgType, output_byte_len: usize) -> impl Prg {
        match prg_type {
            PrgType::AES_CTR_PRG => AesCtrPrg::new(output_byte_len),
        }
    }
    // pub fn create_instance(prg_type: &PrgType, output_byte_len: u32) -> impl Prg {
    //     match prg_type {
    //         PrgType::AES_CTR_PRG => AesCtrPrg::new(output_byte_len),
    //     }
    // }
    pub fn create_instance(prg_type: &PrgType, output_byte_len: usize) -> Box<dyn Prg> {
        match prg_type {
            PrgType::AES_CTR_PRG => Box::new(AesCtrPrg::new(output_byte_len)),
        }
    }

    pub fn create_instance_with_env_type(
        env_type: EnvType,
        output_byte_len: usize,
    ) -> Box<dyn Prg> {
        match env_type {
            EnvType::DEFAULT => Box::new(AesCtrPrg::new(output_byte_len)),
            _ => panic!("Not supported env type"),
        }
    }
    /// PrgType::AES_CTR_PRG
    pub fn create_default(output_byte_len: usize) -> Box<dyn Prg> {
        Box::new(AesCtrPrg::new(output_byte_len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn prg_factory_test() {
        // init_logger();

        // 0. create a instance of Prg
        let prg_type = PrgType::AES_CTR_PRG;
        let out_len = 32;
        let prg = PrgFactory::create_instance(&prg_type, out_len);
        // 1. 调用prg的方法
        let seed = vec![0u8; 16];
        let out = prg.extend_to_bytes(&seed[..]);
        log::info!("out: {:?}", out); // RUST_LOG=info cargo test prg_factory_test
    }
}
