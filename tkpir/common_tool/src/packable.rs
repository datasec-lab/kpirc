/**
 * @description: 实现该trait的类型 可以把对象打包为 字节数组
 * @return {*}
 */
pub trait Packable {
    /// 对象打包为 Vec<Vec<u8>>
    fn to_byte_array_list(&self) -> Vec<Vec<u8>>;
}
