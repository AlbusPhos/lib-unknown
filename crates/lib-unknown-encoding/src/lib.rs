// SPDX-License-Identifier: MPL-2.0

//! hex / base64 / hb2 编码辅助。

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

/// RFC 4648 base64（标准字母表与 URL-safe 变体），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-base64")]
pub mod base64;
/// HB2：每字节展开为 8 个二值符号（默认表 `\0\n`），定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hb2")]
pub mod hb2;
/// 小写十六进制编解码，定容缓冲接口 + `alloc` 向量封装。
#[cfg(feature = "encoding-hex")]
pub mod hex;

#[cfg(any(
    feature = "encoding-hex",
    feature = "encoding-base64",
    feature = "encoding-hb2"
))]
lib_unknown_polymorphism::poly_track_env!();
