// SPDX-License-Identifier: MPL-2.0

#![cfg(feature = "crypto-core")]

/// 密码核心原语：置换盒、PKCS#7/ISO 7816 填充、浮点替身运算与不可逆随机映射。
pub mod core;

/// RFC 8439 ChaCha20 流加密（标准形态 + 编译期多态替身形态）。
#[cfg(feature = "crypto-chacha20")]
pub mod chacha20;

/// MSE 第一代：字节级链式混淆（无盒 / 无动态盒 / 完整版及其逆）。
#[cfg(feature = "crypto-mse")]
pub mod mse;
/// MSE2：64 字节块、32 字节密钥、64 位 nonce 的分组混淆（PKCS#7 对齐）。
#[cfg(feature = "crypto-mse2")]
pub mod mse2;
