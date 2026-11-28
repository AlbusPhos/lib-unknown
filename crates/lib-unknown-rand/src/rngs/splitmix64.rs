// src/rand/splitmix64.rs
#[allow(unused_imports)]
use lib_unknown_polymorphism::poly;

#[cfg(feature = "rand-core")]
use super::super::core::{RngCore, Seed128};
#[cfg(target_has_atomic = "64")]
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_has_atomic = "64"))]
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
use lib_unknown_crypto::core::mix::mix64; // 隐藏了常数但非const
#[cfg(feature = "rand-seed")]
use lib_unknown_seed::seed;

static GOLDEN_GAMMA: u64 = 0x9E3779B97F4A7C15;

/// 共享状态的 SplitMix64：以 `&self` 即可多线程取数；
/// 支持 64 位原子的平台使用无锁 `AtomicU64`，其余 32 位目标自动降级为 `AtomicBool` 自旋锁。
pub struct SplitMix64Atomic {
    #[cfg(target_has_atomic = "64")]
    state: AtomicU64,
    #[cfg(not(target_has_atomic = "64"))]
    locked: AtomicBool,
    #[cfg(not(target_has_atomic = "64"))]
    state: UnsafeCell<SplitMix64>,
}

// SAFETY: `not(target_has_atomic = "64")` 分支下所有对 `UnsafeCell<SplitMix64>` 的访问
// 均受 `AtomicBool` 自旋锁（Acquire/Release 内存序）严格互斥保护。
#[cfg(not(target_has_atomic = "64"))]
unsafe impl Sync for SplitMix64Atomic {}

impl SplitMix64Atomic {
    /// 由 64 位种子构造（state 直接取种子值）。
    #[poly(inline = 0.9, cold = 0.05)]
    pub const fn from_seed(seed: u64) -> Self {
        #[cfg(target_has_atomic = "64")]
        {
            Self {
                state: AtomicU64::new(seed),
            }
        }
        #[cfg(not(target_has_atomic = "64"))]
        {
            Self {
                locked: AtomicBool::new(false),
                state: UnsafeCell::new(SplitMix64::from_seed(seed)),
            }
        }
    }

    #[cfg(not(target_has_atomic = "64"))]
    #[inline]
    fn with_lock<R>(&self, f: impl FnOnce(&mut SplitMix64) -> R) -> R {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        // SAFETY: 已通过 `Acquire` 屏障独占锁，生命周期内不存在其他可变或不可变借用。
        let res = unsafe { f(&mut *self.state.get()) };
        self.locked.store(false, Ordering::Release);
        res
    }

    /// 以 128 位种子的低 64 位重置状态（`Relaxed` 序；高 64 位被丢弃）。
    #[poly(inline = 0.9)]
    pub fn reseed_from(&self, seed: u128) {
        #[cfg(target_has_atomic = "64")]
        self.state.store(seed as u64, Ordering::Relaxed);

        #[cfg(not(target_has_atomic = "64"))]
        self.with_lock(|rng| rng.state = seed as u64);
    }

    /// 以任意 [`Seed128`] 源重置状态（[`reseed_from`](Self::reseed_from) 的便捷封装）。
    #[cfg(feature = "rand-core")]
    #[poly(inline = 0.9)]
    pub fn reseed<S: Seed128>(&self, seed: S) {
        self.reseed_from(seed.into_seed128());
    }

    /// 前向 golden-gamma 并返回 `mix64` 混合输出；`order` 控制递增操作的原子内存序。
    #[poly(inline = 0.85)]
    pub fn next(&self, order: Ordering) -> u64 {
        #[cfg(target_has_atomic = "64")]
        {
            mix64(
                self.state
                    .fetch_add(GOLDEN_GAMMA, order)
                    .wrapping_add(GOLDEN_GAMMA),
            )
        }
        #[cfg(not(target_has_atomic = "64"))]
        {
            let _ = order;
            self.with_lock(|rng| rng.next())
        }
    }

    /// [`next`](Self::next) 的 `Relaxed` 快捷版本（进程日常取数用这个）。
    #[poly(inline = 0.9)]
    pub fn next_relaxed(&self) -> u64 {
        self.next(Ordering::Relaxed)
    }

    /// 一次性原子预留 2 个步进槽位并产出 `u128`（仅 1 次原子操作，吞吐翻倍）。
    #[poly(inline = 0.85)]
    pub fn next_u128_relaxed(&self) -> u128 {
        #[cfg(target_has_atomic = "64")]
        {
            let base = self
                .state
                .fetch_add(GOLDEN_GAMMA.wrapping_mul(2), Ordering::Relaxed);
            SplitMix64::from_seed(base).next_u128()
        }
        #[cfg(not(target_has_atomic = "64"))]
        {
            self.with_lock(|rng| rng.next_u128())
        }
    }

    /// 一次性原子预留全部所需槽位并在本地寄存器中完成高速填充（Wait-Free，~6.9 GiB/s）。
    #[poly(inline = 0.85)]
    pub fn fill_bytes_relaxed(&self, dest: &mut [u8]) {
        #[cfg(target_has_atomic = "64")]
        {
            let steps = dest.len().div_ceil(8);
            if steps == 0 {
                return;
            }
            let base = self
                .state
                .fetch_add(GOLDEN_GAMMA.wrapping_mul(steps as u64), Ordering::Relaxed);
            SplitMix64::from_seed(base).fill_bytes(dest);
        }
        #[cfg(not(target_has_atomic = "64"))]
        {
            // 先在锁内推进状态并拷贝起始种子，大缓冲区填充在锁外完成，避免长时间持锁
            let steps = dest.len().div_ceil(8);
            if steps == 0 {
                return;
            }
            let base = self.with_lock(|rng| {
                let b = rng.state;
                rng.state = b.wrapping_add(GOLDEN_GAMMA.wrapping_mul(steps as u64));
                b
            });
            SplitMix64::from_seed(base).fill_bytes(dest);
        }
    }
}

impl From<u128> for SplitMix64Atomic {
    #[poly(inline = 0.9)]
    fn from(value: u128) -> Self {
        Self::from_seed(value as u64)
    }
}

impl From<u64> for SplitMix64Atomic {
    #[poly(inline = 0.9)]
    fn from(value: u64) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for SplitMix64Atomic {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next_relaxed()
    }

    #[poly(inline = 0.9)]
    fn reseed_from(&mut self, seed: u128) {
        SplitMix64Atomic::reseed_from(self, seed);
    }

    #[poly(inline = 0.9)]
    fn next_u128(&mut self) -> u128 {
        self.next_u128_relaxed()
    }

    #[poly(inline = 0.9)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.fill_bytes_relaxed(dest);
    }
}

#[cfg(feature = "rand-seed")]
impl Default for SplitMix64Atomic {
    #[poly(inline = 0.9)]
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
    #[poly(inline = 0.9, cold = 0.05)]
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// 标准 SplitMix64 步进（golden gamma 前向 + 两次固定乘法折叠）产出 `u64`。
    ///
    /// 方法名沿用 PRNG 取数的通用叫法（`next`），并非 `Iterator::next` 的语义；
    /// 该类型是无限取数器，不实现 `Iterator`。
    #[allow(clippy::should_implement_trait)]
    #[poly(inline = 0.9)]
    pub fn next(&mut self) -> u64 {
        // 牺牲const确保常量安全
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        mix64(self.state)
    }

    /// 生成下一个 128 位随机数（高 64 位来自第一次 [`next`](Self::next)）。
    ///
    /// 固有版本不依赖 `rand-core`，供 [`SplitMix64Atomic`] 的 relaxed 批量取数
    /// 在仅启用 `rand-splitmix64` 时复用；语义与 `RngCore::next_u128` 默认实现一致。
    #[poly(inline = 0.9)]
    pub fn next_u128(&mut self) -> u128 {
        ((self.next() as u128) << 64) | (self.next() as u128)
    }

    /// 以 [`next`](Self::next) 为源按小端顺序填充目标字节缓冲区。
    ///
    /// 语义与 `RngCore::fill_bytes` 默认实现（`fill_bytes_via_u64`）一致。
    #[poly(inline = 0.85)]
    pub fn fill_bytes(&mut self, dest: &mut [u8]) {
        let (chunks, remainder) = dest.as_chunks_mut::<8>();
        for chunk in chunks {
            *chunk = self.next().to_le_bytes();
        }
        if !remainder.is_empty() {
            let bytes = self.next().to_le_bytes();
            remainder.copy_from_slice(&bytes[..remainder.len()]);
        }
    }
}

impl From<u128> for SplitMix64 {
    #[poly(inline = 0.9)]
    fn from(value: u128) -> Self {
        Self::from_seed(value as u64)
    }
}

impl From<u64> for SplitMix64 {
    #[poly(inline = 0.9)]
    fn from(value: u64) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for SplitMix64 {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }

    #[poly(inline = 0.9)]
    fn reseed_from(&mut self, seed: u128) {
        self.state = seed as u64;
    }
}

#[cfg(feature = "rand-seed")]
impl Default for SplitMix64 {
    #[poly(inline = 0.9)]
    fn default() -> Self {
        Self::from_seed(seed())
    }
}
