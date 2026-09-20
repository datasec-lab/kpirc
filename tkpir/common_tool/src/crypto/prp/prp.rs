use super::PrpType;

///伪随机置换（Pseudo-Random Permutation，PRP）接口。
/// PRP使用{0,1}^κ的密钥进行初始化，以{0,1}^κ为输入，返回{0,1}^κ的输出。
pub trait Prp {
    /// 设置密钥。密钥将被拷贝，防止后续可能的篡改。所以这里需要把所有权
    /// 篡改了会怎么样啊？
    fn set_key(&mut self, key: Vec<u8>);

    /// 对明文伪随机置换

    fn prp(&self, plaintext: &[u8]) -> Vec<u8>;

    /// 对密文逆伪随机置换
    fn inv_prp(&self, ciphertext: &[u8]) -> Vec<u8>;

    /// 返回伪随机置换类型
    fn prp_type(&self) -> PrpType;
}
