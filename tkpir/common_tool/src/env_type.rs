/// 提供 国产密码学算法/标准密码学算法的不同实现
/// 目前实际实现并未区分不同的type，而是用一个DEFAULT 统一所有的type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EnvType {
    // 增加一个默认字段，因为rust里暂时还不清楚如何区分国产密码学和标准密码学算法
    DEFAULT,
    /**
     * JDK下国产密码学算法
     */
    INLAND_JDK,
    /**
     * JDK下标准密码学算法
     */
    STANDARD_JDK,
    /**
     * 国产密码学算法
     */
    INLAND,
    /**
     * 标准密码学算法
     */
    STANDARD,
}

impl Default for EnvType {
    fn default() -> Self {
        EnvType::DEFAULT
    }
}
