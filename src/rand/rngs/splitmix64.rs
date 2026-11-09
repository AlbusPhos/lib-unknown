// src/rand/splitmix64.rs
#[cfg(feature = "rand-core")]
use super::super::core::{RngCore, Seed128};
use crate::crypto::core::mix::mix64; // 隐藏了常数但非const
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed;
use core::sync::atomic::{AtomicU64, Ordering};
static GOLDEN_GAMMA: u64 = 0x9E3779B97F4A7C15;

/// 共享状态的 SplitMix64：内部为 `AtomicU64`，以 `&self` 即可多线程取数；
/// 输出 = `mix64(state += GOLDEN_GAMMA)`，内存序由 [`next`](Self::next) 的参数指定。
pub struct SplitMix64Atomic {
    state: AtomicU64,
}

impl SplitMix64Atomic {
    /// 由 64 位种子构造（state 直接取种子值）。
    #[cfg_attr(all(coin52, coin19), cold)]
    #[cfg_attr(coin9, inline(never))]
    #[cfg_attr(all(not(coin9), coin4), inline(always))]
    #[cfg_attr(all(not(coin9), not(coin4)), inline)]
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            state: AtomicU64::new(seed),
        }
    }

    /// 以 128 位种子的低 64 位重置状态（`Relaxed` 序；高 64 位被丢弃）。
    #[cfg_attr(any(coin87, coin124), cold)]
    #[cfg_attr(coin82, inline(never))]
    #[cfg_attr(not(coin82), inline(always))]
    pub fn reseed_from(&self, seed: u128) {
        self.state.store(seed as u64, Ordering::Relaxed);
    }

    /// 以任意 [`Seed128`] 源重置状态（[`reseed_from`](Self::reseed_from) 的便捷封装）。
    #[cfg(feature = "rand-core")]
    #[cfg_attr(coin108, cold)]
    #[cfg_attr(all(coin64, coin88), inline(never))]
    #[cfg_attr(not(all(coin64, coin88)), inline(always))]
    pub fn reseed<S: Seed128>(&self, seed: S) {
        self.state
            .store(seed.into_seed128() as u64, Ordering::Relaxed);
    }

    /// 前向 golden-gamma 并返回 `mix64` 混合输出；`order` 控制递增操作的原子内存序。
    #[cfg_attr(coin83, cold)]
    #[cfg_attr(all(coin63, coin40), inline(never))]
    #[cfg_attr(not(all(coin63, coin40)), inline(always))]
    pub fn next(&self, order: Ordering) -> u64 {
        mix64(
            self.state
                .fetch_add(GOLDEN_GAMMA, order)
                .wrapping_add(GOLDEN_GAMMA),
        )
    }

    /// [`next`](Self::next) 的 `Relaxed` 快捷版本（进程日常取数用这个）。
    #[cfg_attr(any(coin53, coin103), cold)]
    #[cfg_attr(any(coin77, coin109), inline(never))]
    #[cfg_attr(not(any(coin77, coin109)), inline(always))]
    pub fn next_relaxed(&self) -> u64 {
        self.next(Ordering::Relaxed)
    }
}

impl From<u128> for SplitMix64Atomic {
    #[cfg_attr(any(coin77, coin114), cold)]
    #[cfg_attr(all(coin84, coin41), inline(never))]
    #[cfg_attr(not(all(coin84, coin41)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value as u64)
    }
}

impl From<u64> for SplitMix64Atomic {
    #[cfg_attr(all(coin77, coin57), cold)]
    #[cfg_attr(coin8, inline(never))]
    #[cfg_attr(all(not(coin8), coin105), inline(always))]
    #[cfg_attr(all(not(coin8), not(coin105)), inline)]
    fn from(value: u64) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for SplitMix64Atomic {
    #[cfg_attr(all(coin21, coin47), cold)]
    #[cfg_attr(all(coin117, coin101), inline(never))]
    #[cfg_attr(not(all(coin117, coin101)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next_relaxed()
    }

    #[cfg_attr(all(coin59, coin31), cold)]
    #[cfg_attr(coin22, inline(never))]
    #[cfg_attr(all(not(coin22), coin94), inline(always))]
    #[cfg_attr(all(not(coin22), not(coin94)), inline)]
    fn reseed_from(&mut self, seed: u128) {
        self.state.store(seed as u64, Ordering::Relaxed);
    }
}

#[cfg(feature = "rand-seed")]
impl Default for SplitMix64Atomic {
    #[cfg_attr(any(coin75, coin31), cold)]
    #[cfg_attr(coin122, inline(never))]
    #[cfg_attr(not(coin122), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed())
    }
}

/// 单线程 SplitMix64：`&mut self` 独占状态，零原子开销。
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// 以 64 位种子直接初始化单线程状态。
    #[cfg_attr(coin22, cold)]
    #[cfg_attr(coin107, inline(never))]
    #[cfg_attr(all(not(coin107), coin117), inline(always))]
    #[cfg_attr(all(not(coin107), not(coin117)), inline)]
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// 标准 SplitMix64 步进（golden gamma 前向 + 两次固定乘法折叠）产出 `u64`。
    #[cfg_attr(any(coin42, coin99), cold)]
    #[cfg_attr(any(coin1, coin52), inline(never))]
    #[cfg_attr(not(any(coin1, coin52)), inline(always))]
    pub const fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.state;
        static MUL_1: u64 = 0xBF58476D1CE4E5B9;
        static MUL_2: u64 = 0x94D049BB133111EB;

        z = (z ^ (z >> 30)).wrapping_mul(MUL_1);
        z = (z ^ (z >> 27)).wrapping_mul(MUL_2);
        z ^ (z >> 31)
    }
}

impl From<u128> for SplitMix64 {
    #[cfg_attr(coin16, cold)]
    #[cfg_attr(coin77, inline(never))]
    #[cfg_attr(all(not(coin77), coin24), inline(always))]
    #[cfg_attr(all(not(coin77), not(coin24)), inline)]
    fn from(value: u128) -> Self {
        Self::from_seed(value as u64)
    }
}

impl From<u64> for SplitMix64 {
    #[cfg_attr(any(coin88, coin2), cold)]
    #[cfg_attr(all(coin91, coin34), inline(never))]
    #[cfg_attr(not(all(coin91, coin34)), inline(always))]
    fn from(value: u64) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for SplitMix64 {
    #[cfg_attr(all(coin87, coin1), cold)]
    #[cfg_attr(all(coin114, coin13), inline(never))]
    #[cfg_attr(not(all(coin114, coin13)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }

    #[cfg_attr(coin2, cold)]
    #[cfg_attr(coin101, inline(never))]
    #[cfg_attr(not(coin101), inline(always))]
    fn reseed_from(&mut self, seed: u128) {
        self.state = seed as u64;
    }
}

#[cfg(feature = "rand-seed")]
impl Default for SplitMix64 {
    #[cfg_attr(coin112, cold)]
    #[cfg_attr(coin81, inline(never))]
    #[cfg_attr(not(coin81), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed())
    }
}
