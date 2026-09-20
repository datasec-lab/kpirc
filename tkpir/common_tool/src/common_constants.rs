pub mod CommonConstants {

    use encoding_rs::Encoding;
    use encoding_rs::UTF_8; //const

    /// 字符串编码名称
    pub const DEFAULT_CHARSET_NAME: &str = "utf-8";

    /// 字符串编码字符集
    pub static DEFAULT_CHARSET: &Encoding = UTF_8;

    /// 本地工具库名称
    pub const MPC4J_NATIVE_TOOL_NAME: &str = "mpc4j-rust-native-tool";

    /// 本地全同态库名称
    pub const MPC4J_NATIVE_FHE_NAME: &str = "mpc4j-native-fhe";

    /// 这几个长度经常作为 数组的长度，所以用 usize
    /// 分组比特长度，等价于密钥比特长度
    pub const BLOCK_BIT_LENGTH: usize = 128;

    /// 分组字节长度，等价于密钥字节长度
    pub const BLOCK_BYTE_LENGTH: usize = 16;

    /// 分组长整数长度，等价于密钥字节长度 , 没太懂
    pub const BLOCK_LONG_LENGTH: usize = 2;

    /// 统计安全性比特长度
    pub const STATS_BIT_LENGTH: usize = 40;
    /// 统计安全性字节长度
    pub const STATS_BYTE_LENGTH: usize = 5;
}
