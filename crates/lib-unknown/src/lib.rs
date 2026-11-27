// SPDX-License-Identifier: MPL-2.0

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::needless_doctest_main)]

/// 历史工具模块入口；`cfg` 子模块保留 `cfg_if!` / `cfg_match!` 的模块路径。
pub mod utils;

#[cfg(feature = "alloc")]
extern crate alloc;

pub use lib_unknown_errors as errors;
pub use lib_unknown_errors::{bail, ensure, err};
pub use lib_unknown_hide as hide;
pub use lib_unknown_hide::{const_random, hide_array};

#[cfg(feature = "lazy")]
pub use lib_unknown_hide::hide_array_lazy;
#[cfg(feature = "lazy")]
pub use lib_unknown_lazy as lazy;

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
pub use lib_unknown_rand as rand;

#[cfg(any(
    feature = "sys",
    feature = "sys-syscall",
    feature = "sys-win",
    feature = "sys-unix"
))]
/// 裸系统调用、各平台调用号常量与动态符号解析。详见 [`sys`]。
pub use lib_unknown_sys as sys;

#[cfg(any(
    feature = "types",
    feature = "types-str",
    feature = "types-buffer",
    feature = "types-cstr"
))]
/// 擦除式安全内存容器（`StackBytes` / `StackStr` / `StackCStr` 等）。详见 [`types`]。
pub use lib_unknown_types as types;

#[cfg(any(
    feature = "crypto",
    feature = "crypto-core",
    feature = "crypto-mse",
    feature = "crypto-mse2",
    feature = "crypto-chacha20"
))]
/// 混淆密码（MSE 系列）、RFC 8439 ChaCha20 与填充/置换原语。详见 [`crypto`]。
pub use lib_unknown_crypto as crypto;

#[cfg(any(feature = "hash", feature = "hash-core", feature = "hash-hashmap",))]
/// 轻量多泳道乘法链哈希与无 std 定容哈希表。详见 [`hash`]。
pub use lib_unknown_hash as hash;

#[cfg(any(
    feature = "encoding",
    feature = "encoding-hex",
    feature = "encoding-base64",
    feature = "encoding-hb2",
))]
/// hex / base64 / hb2 编码辅助。详见 [`encoding`]。
pub use lib_unknown_encoding as encoding;

#[cfg(feature = "harness")]
/// 动态编译运行 harness（仅开发期，需同时启用 `"std"`）。
pub use lib_unknown_harness as harness;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
