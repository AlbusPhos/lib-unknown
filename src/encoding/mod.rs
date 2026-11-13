/// RFC 4648 base64（标准字母表与 URL-safe 变体），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-base64")]
pub mod base64;
/// HB2：每字节展开为 8 个二值符号（默认表 `\0\n`），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hb2")]
pub mod hb2;
/// HB3：每字节展开为 6 个三进制符号（默认表 `\0\n `），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hb3")]
pub mod hb3;
/// 小写十六进制编解码，定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hex")]
pub mod hex;
