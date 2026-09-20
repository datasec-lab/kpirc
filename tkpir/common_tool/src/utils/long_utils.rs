/// 长整数工具类
pub struct LongUtils;

impl LongUtils {
    pub const LONG_BYTES: usize = 8;

    /// y = log2(x), if y < min , return min
    pub fn ceil_log2_min(x: u64, min: u32) -> u32 {
        assert!(min > 0);

        u32::max(LongUtils::ceil_log2(x), min)
    }

    /// old verison before mpc4j-1.0.6
    /// 使用u64 替代 java中的Long
    /// u32 代替 int
    /// 计算 Math.min(1, log_2(x))， 向上取整
    // pub fn ceil_log2(x: u64) -> u32 {
    //     assert!(x > 0);
    //     // 特殊处理1
    //     if x == 1{
    //         return 1;
    //     }

    //     // 尝试直接用 rust 原生库支持
    //     // 经过验证 直接使用 f64.log2() 会导致精度不够，例如：
    //     // log(2^47 + 1) = 47.00000000000001
    //     // log(2^48 + 1) = 48.00000000000001
    //     // log(2^49 + 1) = 49.0
    //     // log(2^50 + 1) = 50.0
    //     // .....
    //     // 从 2^49 开始就会报错
    //     // (x as f64).log2().ceil() as u32
    //     // 但是为什么会有这个错误呢？因为精度不够吗？只到了小数点后10位？也就是 log(2^49 + 1) = 49.0 小数点后面的值太小，直接变成了0

    //     // 模仿mpc4j中的实现
    //     // 因为我这里使用的是 u64，不需要单独处理 63/62 比特
    //     // 感觉这里可以用二分搜索来提速，但是数据范围就是在[0,64),所以感觉提升不会很明显
    //     let mut k: u32 = 0;
    //     let mut pow_k: u64 = 1;
    //     while pow_k < x {
    //         k += 1;
    //         // pow_k = pow_k << 1; //
    //         // pow_k <<= 1;
    //         // 如果逻辑是 pow_k <<= 1; 这里有个bug
    //         // 即 此时 k = 64,那么 pow_k = 2^64 ， 因为pow_k的类型是u64, 这就会导致 pow_k 溢出
    //         // pow_k 就等于0，那么 后续 k 就会一直增长，直到k溢出！所以，当 k = 64时，应该单独的逻辑
    //         // 下面是修复后的版本
    //         if k == 64 {
    //             pow_k = u64::MAX;
    //         }else {
    //             pow_k <<= 1;
    //         }
    //     }
    //     return k;

    // }

    /// mpc4j 1.0.6 的新实现
    /// // See https://github.com/google/guava/blob/master/guava/src/com/google/common/math/LongMath.java for details.
    pub fn ceil_log2(x: u64) -> u32 {
        assert!(x > 0);
        // 牛逼啊，真快这个方法
        // 为什要-1
        u64::BITS - (x - 1).leading_zeros()
    }
}

#[cfg(test)]
mod LongUtilsTest {

    fn celi_log2_106(x: u64) -> u32 {
        u64::BITS - (x - 1).leading_zeros()
    }

    fn celi_log2_104(x: u64) -> u32 {
        assert!(x > 0);
        // 特殊处理1
        if x == 1 {
            return 1;
        }

        // 尝试直接用 rust 原生库支持
        // 经过验证 直接使用 f64.log2() 会导致精度不够，例如：
        // log(2^47 + 1) = 47.00000000000001
        // log(2^48 + 1) = 48.00000000000001
        // log(2^49 + 1) = 49.0
        // log(2^50 + 1) = 50.0
        // .....
        // 从 2^49 开始就会报错
        // (x as f64).log2().ceil() as u32
        // 但是为什么会有这个错误呢？因为精度不够吗？只到了小数点后10位？也就是 log(2^49 + 1) = 49.0 小数点后面的值太小，直接变成了0

        // 模仿mpc4j中的实现
        // 因为我这里使用的是 u64，不需要单独处理 63/62 比特
        // 感觉这里可以用二分搜索来提速，但是数据范围就是在[0,64),所以感觉提升不会很明显
        let mut k: u32 = 0;
        let mut pow_k: u64 = 1;
        while pow_k < x {
            k += 1;

            // pow_k <<= 1;
            // 如果逻辑是 pow_k <<= 1; 这里有个bug
            // 即 此时 k = 64,那么 pow_k = 2^64 ， 因为pow_k的类型是u64, 这就会导致 pow_k 溢出
            // pow_k 就等于0，那么 后续 k 就会一直增长，直到k溢出！所以，当 k = 64时，应该单独的逻辑
            // 下面是修复后的版本
            if k == 64 {
                pow_k = u64::MAX;
            } else {
                pow_k <<= 1;
            }
            // println!("k: {}, pow_k: {}", k, pow_k);
        }
        return k;
    }

    use rand::rngs::OsRng;
    use rand_core::RngCore;
    use rug::ops::Pow;
    const RANDOM_NUMBER: usize = 100000;
    #[test]
    fn ceil_log2_test() {
        for _ in 0..RANDOM_NUMBER {
            // let x = 1 << 50 + 1;
            // let x = OsRng.next_u32() as u64;
            let x = OsRng.next_u64();
            // let x = 9233177101484439451;
            // println!("u64 max {}", u64::MAX); // 18446744073709551615

            // println!("x: {}", x);
            let left = celi_log2_106(x);
            let right = celi_log2_104(x);
            assert_eq!(left, right, "left(106): {}, right(104): {}", left, right,)
        }
    }

    use std::time::Instant;
    #[test]
    fn ceil_log2_efficiency_test() {
        let t = Instant::now();
        let x = OsRng.next_u64();
        for _ in 0..RANDOM_NUMBER {
            // println!("x: {}", x);
            let right = celi_log2_104(x);
        }
        println!("104 time: {:?}-ms", t.elapsed().as_millis());

        let t = Instant::now();
        // let x = OsRng.next_u64();
        for _ in 0..RANDOM_NUMBER {
            // println!("x: {}", x);
            let left = celi_log2_106(x);
        }
        println!("106 time: {:?}-ms", t.elapsed().as_millis());
    }
}
