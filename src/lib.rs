// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[doc(hidden)]
/// 内部模块：暴露构建期种子常量，供 `hide!` 等宏在 const 上下文中使用，不属于公开 API。
pub mod __private {
    /// 构建期 128 位种子拼接串。
    pub const SEED: &str = env!("SEED");
    /// 构建期 64 位种子字符串。
    pub const SEED64: &str = env!("SEED64");
    /// 构建期 128 位种子字符串。
    pub const SEED128: &str = env!("SEED128");
}

#[cfg(any(
    feature = "rand",
    feature = "rand-seed",
    feature = "rand-core",
    feature = "rand-cwg",
    feature = "rand-xoshiro",
    feature = "rand-chacha",
    feature = "rand-splitmix64",
))]
/// 微架构熵源与运行时种子派生、多族 PRNG。详见 [`rand`]。
pub mod rand;
#[cfg(any(
    feature = "sys",
    feature = "sys-syscall",
    feature = "sys-win",
    feature = "sys-unix"
))]

/// 裸系统调用、各平台调用号常量与动态符号解析。详见 [`sys`]。
pub mod sys;

#[cfg(any(
    feature = "types",
    feature = "types-str",
    feature = "types-buffer",
    feature = "types-cstr"
))]
/// 擦除式安全内存容器（`StackBytes` / `StackStr` / `StackCStr` 等）。详见 [`types`]。
pub mod types;

#[cfg(any(
    feature = "crypto",
    feature = "crypto-core",
    feature = "crypto-mse",
    feature = "crypto-mse2",
    feature = "crypto-chacha20"
))]
/// 混淆密码（MSE 系列）、RFC 8439 ChaCha20 与填充/置换原语。详见 [`crypto`]。
pub mod crypto;
#[cfg(feature = "dyntest")]
/// 动态编译运行 harness（仅开发期，需同时启用 `"std"`）。
pub mod dyntest;
#[cfg(feature = "lazy")]
/// 无 std 惰性初始化容器（手写 `OnceLock` / `LazyLock`）。
pub mod lazy;

#[cfg(any(
    feature = "encoding",
    feature = "encoding-hex",
    feature = "encoding-base64"
))]
/// hex / base64 编码辅助。
pub mod encoding;
/// 全库统一错误类型 `Error` / `ErrType` 与 `ensure!` / `bail!` / `err!` 宏。
pub mod errors;
#[cfg(any(feature = "hash", feature = "hash-base", feature = "hash-hashmap",))]
/// 轻量多泳道乘法链哈希与无 std 定容哈希表。
pub mod hash;
/// 编译期数据与代码随机化宏族（`const_random!` / `hide!` / `hide_array!` 等）。
pub mod hide;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
