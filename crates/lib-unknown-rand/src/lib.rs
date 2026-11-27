// SPDX-License-Identifier: MPL-2.0

//! 微架构熵源、种子派生与多族 PRNG。

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

/// 微架构探针熵源与运行时种子派生（`probe`/`seed`/`seed128`），转发自 `lib-unknown-seed`。
#[cfg(feature = "rand-seed")]
pub use lib_unknown_seed as seed;

/// `Seed128`/`RngCore`/`Rng` trait、全局便捷接口与按 u64/u128 填充的工具函数。
#[cfg(feature = "rand-core")]
pub mod core;

#[cfg(any(
    feature = "rand-cwg",
    feature = "rand-chacha",
    feature = "rand-xoshiro",
    feature = "rand-splitmix64"
))]
/// 具体生成器实现族（ChaCha / Cwg / SplitMix64 / Xoshiro），均实现 [`RngCore`](crate::core::RngCore)。
pub mod rngs;

#[cfg(any(
    feature = "rand-core",
    feature = "rand-splitmix64",
    feature = "rand-cwg",
    feature = "rand-chacha",
    feature = "rand-xoshiro"
))]
lib_unknown_polymorphism::poly_track_env!();
