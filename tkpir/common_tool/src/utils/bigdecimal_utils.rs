use crate::CommonConstants;

use rug::float;
use rug::float::Round;
use rug::ops::{AssignRound, Pow};
use rug::Float; // https://docs.rs/rug/1.19.1/rug/struct.Float.html

/// 基于 rug 库的 Float 来实现 BigDecimalRug
pub type BigDecimalRug = Float;
// pub const BigDecimalZero: BigDecimalRug =
pub const DEFAULT_PRECISION: u32 = 69;

/**
 * 对标 java 中的 RoundingMode
 * 注意参考不同模式的具体表现 https://docs.oracle.com/javase/7/docs/api/java/math/RoundingMode.html
 */
pub type RoundMode = Round; // 经过研究，这里的Round 和 java.math.RoundingMode 存在不小区别
                            // java中的是 Round to integer，而这里是 Round to next bit

pub struct BigDecimalUtils;

pub mod BigDecimalUtilsConstants {
    use super::*;
    use rug::Float;

    /**
     * @description: 无法用常数来设置，直接用函数计算吧
     * @return {*}
     */
    pub fn stat_neg_probability() -> Float {
        let ONE_POINT_FIVE = Float::with_val(DEFAULT_PRECISION, 0.5);

        ONE_POINT_FIVE.pow(CommonConstants::STATS_BIT_LENGTH)
    }

    // pub const A: Float = Float::with_val(DEFAULT_PRECISION, 0.5).pow(CommonConstants::STATS_BIT_LENGTH);
    // let STATS_NEG_PROBABILITY: Float = BigDecimalRug::with_val_round(DEFAULT_PRECISION, 0.5.pow(CommonConstants::SATAS_));
}

#[cfg(test)]
mod BigDecimalUtilsTest {

    use rug::Assign;

    use super::*;

    /**
     * @description: 测试等价于 mpc4j 中, RoundingMode::HALF_UP
     * @return {*}
     */
    #[test]
    fn round_test() {
        let mut f4 = Float::new(4);

        f4.assign(0.00000000000000001); // 能否通过
        println!("{}", f4);

        // f4.assign_round(10.5, Round::Up);
        // assert_eq!(f4, 11);
        // println!("max: {},min: {}", f4::max, f4::min);

        let mut f4 = Float::new(4);
        let a = 5.5;
        f4.assign_round(a, Round::Up);
        // assert_eq!(f4, 6);
        println!("Round::UP({})={:?}", a, f4);

        let a = 2.5;
        f4.assign_round(a, Round::Up);
        // assert_eq!(f4, 3);
        println!("Round::UP({})={}", a, f4);

        let a = 1.6;
        f4.assign_round(a, Round::Up);
        // assert_eq!(f4, 2);
        println!("Round::UP({})={}", a, f4);

        let a = 1.1;
        f4.assign_round(a, Round::Up);
        // assert_eq!(f4, 1);
        println!("Round::UP({})={}", a, f4);

        let a = 1.0;
        f4.assign_round(a, Round::Up);
        println!("Round::UP({})={}", a, f4);
    }

    /**
     * @description: 目的是为了 找到 float precision 和 小数点后多少位数的联系
     * 因为mpc4j中 的 java.math.BigDecimal 可以直接 setScale 表示小数点后的位数
     * 但是此处使用的 rug::Float 暂时只支持 precision 的概念
     * Float 文档中提到：(The primitive `f32` has a precision of 24 and
     * `f64` has a precision of 53.)
     * 这里主要和 IEEE754 这个浮点数表示标准有关系，可以查看：https://en.wikipedia.org/wiki/Double-precision_floating-point_format
     * 这个测试用例主要用来测试 precision 和 digits after point 的关系
     *      precision     digits after point
     *         53                 16
     *         
     *  
     * @return {*}
     */
    #[test]
    fn prec_test() {
        // 突然回过头来
        // rug::Float 和 java.big.BigDecimal 根本不是同一个东西
        // Float 本质上还是浮点数类型，只是 rug 对其做了扩展，让其支持了任意的 precision
        // 而 BigDecimal 本质上是 value * 10 ^scale 这样一个东西，本质上不是浮点数
        let mut f1 = Float::new(60);

        // f1.assign(1.1111111111111238170); // 19
        f1.assign(1.12345678911131517191); // 20
                                           // f1.assign(1.11111111111112381782); // 21
        println!("f1: {:?}", f1);

        // let mut f2 = Float::new(60);
        let mut f2 = Float::with_val_round(DEFAULT_PRECISION, 1.123, Round::Up);
        println!("f2: {:?}", f2);

        // println!("f1: {:?}", f1.to_string_radix(10, None));

        // // 验证 f64 能否表示 小数点后 20位
        // let a : f64 = 1.10000000000000000002;
        // println!("f64 {}", a);

        //   // 验证 f64 能否表示 小数点后 17位
        //   let b : f64 = 1.10000000000000002;
        //   println!("f64 {}", b);

        //   let c: f64 = 1.1111111111111237;//16
        //   let d: f64 = 1.0000000000000001;//16
        //   println!("f64 {}", c + d);

        // println!("Minimum precision is {}", float::prec_min());
        // println!("Minimum precision is {}", float::prec_max());
    }

    // precision 和 digits 的换算
    /*
       precision 60: 18-digits
       precision 61: 18-digits
       precision 62: 18-digits
       precision 63: 18-digits
       precision 64: 19-digits
       precision 65: 19-digits
       precision 66: 19-digits
       precision 67: 20-digits
       precision 68: 20-digits
       precision 69: 20-digits // 设置为默认的 digits
       precision 70: 21-digits
       precision 71: 21-digits
       precision 72: 21-digits
       precision 73: 21-digits
       precision 74: 22-digits
       precision 75: 22-digits
       precision 76: 22-digits
       precision 77: 23-digits
       precision 78: 23-digits
       precision 79: 23-digits
    */
    #[test]
    fn precision_digits_test() {
        let a = Integer::from(10);
        for precision in 60..80 {
            let (b, _) = Float::with_val_round(precision, &a, Round::Up);
            // println!("{:?}", b);

            println!(
                "precision {}: {}-digits",
                precision,
                b.to_string().len() - 3
            );
        }
    }

    use rug::ops::{AddAssignRound, CompleteRound, Pow};
    use rug::Integer;
    #[test]
    fn basic_op() {
        let a = Integer::from(10);
        let (b, _) = Float::with_val_round(DEFAULT_PRECISION, a, Round::Up);

        // pow 计算
        let c: Float = b.clone().pow(2);
        println!("c: {}", c);

        println!("{}", b);

        // 乘法计算
        let mut d = b.clone() * c;
        println!("{}", d);

        //
        d.add_assign_round(b.clone(), RoundMode::Up);
        println!("d: {}", d);
    }
}
