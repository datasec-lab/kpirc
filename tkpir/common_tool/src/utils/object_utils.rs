pub struct ObjectUtils;

impl ObjectUtils {
    /**
     * @description: 这里有个很大的挑战，需要把 任意类型转换为一个 byte arr, 在mpc4j 中，基于Java语言比较好解决
     * Java 中有一个能够表示所有类型的 Object， 所以入参的类型就很容易解决了。
     * 此外，java 还提供了 instanceof，很容易判断不同的变量 的具体的数据类型。
     * 但是在 rust里，这些都没有。经过一些调研，找到了一个简单的解决方案。
     * 定义一个trait, 让这里的范型满足这个约束
     * 这个trait 定义了什么行为呢？ 它的行为就是把当前的 对象 转换为 byte arr
     * 主要参考了这个链接：https://stackoverflow.com/questions/41596628/how-to-match-on-data-type-in-rust
     * 特别是注意到了这一句话：
     * This is principled generic programming, where you declare exactly the difference
     * of behavior of the possible generic parameters, so that there is no surprise.
     *
     * declare exactly the difference 这句话点醒了我，可以在trait 里定义我想要的任何的行为
     *
     * 这里定义的范型必须满足这个约束，因为就是要调用对应的方法
     * @return {*}
     */
    pub fn object_to_byte_arr<U: ObjectToByteArr>(x: U) -> Vec<u8> {
        x.to_byte_arr()
    }
}

/**
 * @description: 提供一个方法，把任意类型转换为 字节数组
 *               为具体类型实现该trait，该类型就具有了 转换到 字节数组的能力
 * @return {*}
 */
pub trait ObjectToByteArr {
    fn to_byte_arr(self) -> Vec<u8>;
}

use bytebuffer::ByteBuffer;

impl ObjectToByteArr for ByteBuffer {
    fn to_byte_arr(self) -> Vec<u8> {
        self.into_vec()
    }
}

impl ObjectToByteArr for u8 {
    /**
     * @description: 把当前u8 转换为 Vec<u8>，这里byte order 是 BigEndian，和mpc4j保持一致
     * @param {*} self
     * @return {*}
     */
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

impl ObjectToByteArr for Vec<u8> {
    fn to_byte_arr(self) -> Vec<u8> {
        self
    }
}

// 针对常见的 primitives 都先实现了
impl ObjectToByteArr for u16 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for u32 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for u64 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for u128 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for usize {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}

// 针对常见的 primitives 都先实现了

impl ObjectToByteArr for i8 {
    /**
     * @description: 把当前u8 转换为 Vec<u8>，这里byte order 是 BigEndian，和mpc4j保持一致
     * @param {*} self
     * @return {*}
     */
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for i16 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for i32 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for i64 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for i128 {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
impl ObjectToByteArr for isize {
    fn to_byte_arr(self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }
}
// 未完待续....

#[cfg(test)]
mod ObjectUtilsTest {

    use super::*;

    #[test]
    fn ObjectToByteArrTest() {
        let x = 6u8;
        println!("{:?}", x.to_byte_arr());
    }

    #[test]
    fn test_u8() {
        println!("{:?}", ObjectUtils::object_to_byte_arr(16u8));
    }
}
