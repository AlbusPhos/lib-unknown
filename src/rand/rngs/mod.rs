/// ChaCha 系（RFC 8439 核心，8/12/14/20 轮类型别名）。
#[cfg(feature = "rand-chacha")]
pub mod chacha;
/// Cwg 系（64/128 位状态非线性步进，暖启动后输出）。
#[cfg(feature = "rand-cwg")]
pub mod cwg;
/// SplitMix64 单线程与原子共享两种变体。
#[cfg(feature = "rand-splitmix64")]
pub mod splitmix64;
/// xoshiro256/xoroshiro128 的 `++` 与 `**` 四种变体。
#[cfg(feature = "rand-xoshiro")]
pub mod xoshiro;
