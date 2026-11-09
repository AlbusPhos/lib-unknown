/// RFC 4648 base64（标准字母表与 URL-safe 变体），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-base64")]
pub mod base64;
/// 小写十六进制编解码，定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hex")]
pub mod hex;
