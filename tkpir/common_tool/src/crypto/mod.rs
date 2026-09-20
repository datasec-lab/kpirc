pub mod prf;
pub mod prg;
pub mod prp; // 抗关联哈希函数, Correlation Robustness Hash Function，CRHF
mod secure_random;
pub use secure_random::*;
