use super::prg_factory::PrgType;

/// 伪随机数生成器接口
pub trait Prg {
    /// 返回输出字节长度
    fn output_byte_len(&self) -> usize;
    /// 输入种子，扩展为指定长度的随机数
    /// 伪随机数生成器的核心方法， 给定随机种子，生成一个随机数序列
    fn extend_to_bytes(&self, seed: &[u8]) -> Vec<u8>;

    /// 返回随机数生成器类型
    fn prg_type(&self) -> PrgType;
}
