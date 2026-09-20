#![allow(unused_imports)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(warnings)]

pub mod common_constants;
pub mod config;
pub mod env_type;
pub mod packable;

pub use common_constants::CommonConstants;
pub use config::Config;
pub use env_type::EnvType;
pub use packable::Packable;

pub mod crypto;
pub mod utils;

/// 提供一个全局的Build Trait
/// 通过 build 方法返回任意对象
/// 这里的定义模仿了 org.apache.commons.lang3.builder.Builder
/// public interface Builder<T> {
///      T build();
/// }
pub trait Builder<T> {
    fn build(self) -> T;
}
