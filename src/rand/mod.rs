/// 微架构探针熵源与运行时种子派生（`probe`/`seed`/`seed128`）。
#[cfg(feature = "rand-seed")]
pub mod seed;

/// `Seed128`/`RngCore`/`Rng` trait、全局便捷接口与按 u64/u128 填充的工具函数。
#[cfg(feature = "rand-core")]
pub mod core;

#[cfg(any(
    feature = "rand-cwg",
    feature = "rand-chacha",
    feature = "rand-xoshiro",
    feature = "rand-splitmix64"
))]
/// 具体生成器实现族（ChaCha / Cwg / SplitMix64 / Xoshiro），均实现 [`RngCore`](crate::rand::core::RngCore)。
pub mod rngs;
