use crate::common_constants::CommonConstants;

pub struct DoubleUtils;

pub mod DoubleUtilsConstant {
    use crate::CommonConstants;

    pub const PRECISION: f64 = 1e-7;
    // 1 / 2^n
    // 统计安全性对应的概率值
    // pub const STATS_NEG_PROBABILITY: f64 = 1.0 / (2f64).powf(CommonConstants::STATS_BIT_LENGTH as f64);

    pub fn stats_neg_probability() -> f64 {
        (2f64)
            .powf(CommonConstants::STATS_BIT_LENGTH as f64)
            .recip()
    }
    pub const STATS_NEG_PROBABILITY: f64 =
        1.0 / ((2 as u64).pow(CommonConstants::STATS_BIT_LENGTH as u32) as f64);
    // 计算安全性对应的概率值 1/2^{-128}
    // 使用 primive type，根本无法表示 2^128 这个值，使用第三方库肯定又无法声明位 const
    // 这里暂时做一些变通吧 取 u128::MAX = 2^128 - 1
    // pub const COMP_NEG_PROBABILITY: f64 = 1.0 / ((2 as u128).pow(CommonConstants::BLOCK_BIT_LENGTH as u32) as f64);
    pub const COMP_NEG_PROBABILITY: f64 = 1.0 / (u128::MAX as f64);
}

impl DoubleUtils {
    /// 计算组合数 C(n, m), m <= n, 从 n 个不同的数 任取 m个数 的所有可能情况
    /// 返回 C(n, m) 的近似值
    pub fn estimate_combinatorial(n: u32, m: u32) -> f64 {
        assert!(m <= n);

        let mut combinatorial = 1.0;
        // C(n-m) = C(n, n -m)，选择更小的 m
        // C(n,m) = n! /m! * (n-m)!
        let mut min_m = 0 as u64;
        if m > n / 2 {
            min_m = (n - m) as u64;
        } else {
            min_m = m as u64;
        }
        for i in 1..=min_m {
            combinatorial = combinatorial * (n as u64 + 1 - i) as f64;
            combinatorial = combinatorial / i as f64;
        }

        // 一定能除尽，不需要保留小数（但是为何返回值还是 f64呢？）
        return combinatorial;
    }

    pub fn log2(x: f64) -> f64 {
        x.log2()
    }
}

#[cfg(test)]
mod double_utils_test {

    use super::*;

    #[test]
    #[should_panic]
    fn double_utils_panic_1() {
        DoubleUtils::estimate_combinatorial(10, 11);
    }

    #[test]
    fn double_utils_estimate_combinatorial() {
        test_estimate_combinatorial(1, 0, 1);

        test_estimate_combinatorial(1, 1, 1);

        test_estimate_combinatorial(10, 0, 1);

        test_estimate_combinatorial(10, 1, 10);

        test_estimate_combinatorial(10, 9, 10);

        test_estimate_combinatorial(10, 10, 1);

        test_estimate_combinatorial(10, 5, 252);

        test_estimate_combinatorial(10, 3, 120);

        test_estimate_combinatorial(10, 6, 210);
    }

    fn test_estimate_combinatorial(n: u32, m: u32, truth: u64) {
        let combinatorial = DoubleUtils::estimate_combinatorial(n, m);

        assert_eq!(combinatorial, truth as f64);
        assert!(combinatorial - truth as f64 <= DoubleUtilsConstant::PRECISION);
    }
}
