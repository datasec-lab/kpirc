use rug::integer::Order;
use rug::ops::Pow;
use rug::{Assign, Complete, Integer};

use rand_core::CryptoRng;
use rand_core::RngCore;

/// 基于 rug库 来实现 BigInteger
pub type BigIntegerRug = Integer;
/// Wrap a util Struct on rug
pub struct BigIntegerRugUtils {}

impl BigIntegerRugUtils {
    /// return a random value in [1, n)
    /// rng needs to be a CSPRNG
    pub fn random_positive<T: RngCore + CryptoRng>(n: BigIntegerRug, rng: &mut T) -> BigIntegerRug {
        // 0. 获取字节长度
        let bytes_len = n.to_digits::<u8>(Order::Lsf).len();
        // 1. 缓存数组
        let mut buf = vec![0; bytes_len];
        // 2. 产生满足条件的随机数
        loop {
            rng.fill_bytes(&mut buf);
            // 3. 转换为BigInteger
            let r = BigIntegerRug::from_digits(&mut buf, Order::Lsf);
            // 4. 判断是否满足条件
            if r >= 1 && r < n {
                return r;
            }
        }
    }

    pub fn to_32_bytes(v: Integer) -> [u8; 32] {
        let v_vec = v.to_digits::<u8>(Order::Lsf); // 左低又高
                                                   // 长度小于64 还是比较好解决的，关键是大于呢？
        if v_vec.len() > 32 {
            panic!("Now can not handle the bytes len > 64");
        }
        let mut output = [0u8; 32];
        for (i, val) in v_vec.into_iter().enumerate() {
            output[i] = val;
        }
        return output;
    }

    pub fn to_64_bytes(v: Integer) -> [u8; 64] {
        let v_vec = v.to_digits::<u8>(Order::Lsf); // 左低又高
                                                   // 长度小于64 还是比较好解决的，关键是大于呢？
        if v_vec.len() > 64 {
            panic!("Now can not handle the bytes len > 64");
        }
        let mut output = [0u8; 64];
        for (i, val) in v_vec.into_iter().enumerate() {
            output[i] = val;
        }
        return output;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use curve25519_dalek::constants;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    #[test]
    fn bigint_random_positive() {
        let order = constants::BASEPOINT_ORDER;
        let order_integer = Integer::from_digits(order.as_bytes(), Order::Lsf);

        // 0. CSPRNG
        let mut rng = ChaCha20Rng::from_entropy();

        // 1.
        let r = BigIntegerRugUtils::random_positive(order_integer.clone(), &mut rng);

        println!("{:?}", r);

        assert!(r >= 1 && r < order_integer);
    }
}
