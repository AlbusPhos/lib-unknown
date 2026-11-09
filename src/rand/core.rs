// src/rand/core.rs
use crate::crypto::core::shuffle_with;
use crate::lazy::LazyLock;
use crate::rand::rngs::splitmix64::SplitMix64Atomic;
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed;
use core::sync::atomic::{AtomicUsize, Ordering};

/// 可无损转换成 128 位种子的类型集合（`u128`、`(u64, u64)`、`[u64; 2]` 等）；
/// 由 [`RngCore::new`] 作为统一种子入口。
pub trait Seed128 {
    /// 转换为统一的 128 位种子；无符号整数零扩展、有符号整数符号扩展，
    /// 元组/数组按“高 64 位 | 低 64 位”组合。
    fn into_seed128(self) -> u128;
}

macro_rules! impl_seed128_int {
    ($($t:ty),*) => {
        $(
            impl Seed128 for $t {
                fn into_seed128(self) -> u128 {
                    self as u128
                }
            }
        )*
    };
}

impl_seed128_int!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);
impl Seed128 for [u8; 16] {
    fn into_seed128(self) -> u128 {
        u128::from_le_bytes(self)
    }
}
impl Seed128 for (u64, u64) {
    fn into_seed128(self) -> u128 {
        ((self.0 as u128) << 64) | (self.1 as u128)
    }
}
impl Seed128 for [u64; 2] {
    fn into_seed128(self) -> u128 {
        ((self[0] as u128) << 64) | (self[1] as u128)
    }
}

/// PRNG 核心 trait：`next_u64` 为唯一必实现源，其余接口提供默认实现。
///
/// PRNG 核心 trait：只需实现 `next_u64`，即可获得 `next_u32` / `next_u128` /
/// `fill_bytes` / `reseed` 等默认接口；`From<u128>` 超约束要求每个生成器
/// 都能由 128 位种子构造，以便 `new<S: Seed128>` / `reseed` 统一重建状态。
pub trait RngCore: From<u128> {
    /// 以任意 [`Seed128`] 构造生成器（等价于 `seed.into_seed128().into()`）。
    fn new<S: Seed128>(seed: S) -> Self
    where
        Self: Sized,
    {
        seed.into_seed128().into()
    }

    /// 生成下一个 64 位随机数；这是唯一必须实现的核心方法。
    fn next_u64(&mut self) -> u64;

    /// 以 128 位种子重建内部状态（默认实现为 `*self = seed.into()`）。
    #[cfg_attr(coin19, inline(never))]
    fn reseed_from(&mut self, seed: u128) {
        *self = seed.into();
    }

    /// 以任意 [`Seed128`] 重建内部状态。
    #[cfg_attr(all(coin101, coin54), cold)]
    #[cfg_attr(all(not(coin83), not(coin106)), inline)]
    fn reseed<S: Seed128>(&mut self, seed: S) {
        self.reseed_from(seed.into_seed128());
    }

    /// 生成下一个 32 位随机数（默认截取 [`next_u64`](Self::next_u64) 的低 32 位）。
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    /// 生成下一个 128 位随机数（高 64 位来自第一次 [`next_u64`](Self::next_u64)）。
    fn next_u128(&mut self) -> u128 {
        ((self.next_u64() as u128) << 64) | (self.next_u64() as u128)
    }

    /// 用 [`next_u64`](Self::next_u64) 按小端顺序填充目标字节缓冲区。
    #[cfg_attr(all(coin3, coin22), cold)]
    #[cfg_attr(any(coin11, coin47), inline(never))]
    #[cfg_attr(not(any(coin11, coin47)), inline(always))]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes_via_u64(self, dest)
    }
}

/// 以 `next_u64` 为源按小端逐块填充 `dest`（用于 `RngCore::fill_bytes` 的默认实现）。
#[cfg_attr(all(coin58, coin48), cold)]
#[cfg_attr(all(coin57, coin32), inline(never))]
#[cfg_attr(not(all(coin57, coin32)), inline(always))]
pub fn fill_bytes_via_u64<R: RngCore>(rng: &mut R, dest: &mut [u8]) {
    let (chunks, remainder) = dest.as_chunks_mut::<8>();
    for chunk in chunks {
        *chunk = rng.next_u64().to_le_bytes();
    }
    if !remainder.is_empty() {
        let bytes = rng.next_u64().to_le_bytes();
        remainder.copy_from_slice(&bytes[..remainder.len()]);
    }
}

/// 以 `next_u128` 为源按小端逐块填充 `dest`（供自有 PRNG 复用的填充工具）。
#[cfg_attr(coin110, cold)]
#[cfg_attr(any(coin83, coin99), inline(never))]
#[cfg_attr(not(any(coin83, coin99)), inline(always))]
pub fn fill_bytes_via_u128<R: RngCore>(rng: &mut R, dest: &mut [u8]) {
    let (chunks, remainder) = dest.as_chunks_mut::<16>();
    for chunk in chunks {
        *chunk = rng.next_u128().to_le_bytes();
    }
    if !remainder.is_empty() {
        let bytes = rng.next_u128().to_le_bytes();
        remainder.copy_from_slice(&bytes[..remainder.len()]);
    }
}

/// 便捷扩展 trait：为任意 [`RngCore`] 提供类型推断随机值、区间采样与洗牌。
pub trait Rng: RngCore {
    #[cfg_attr(coin93, cold)]
    #[cfg_attr(coin40, inline(never))]
    #[cfg_attr(all(not(coin40), coin41), inline(always))]
    #[cfg_attr(all(not(coin40), not(coin41)), inline)]
    /// 从当前 PRNG 抽取一个实现了 [`Random`] 的值。
    fn random<T: Random>(&mut self) -> T {
        T::random(self)
    }

    #[cfg_attr(coin112, cold)]
    #[cfg_attr(all(coin120, coin123), inline(never))]
    #[cfg_attr(not(all(coin120, coin123)), inline(always))]
    /// 从给定区间无偏采样一个值。
    fn random_range<R: SampleRange>(&mut self, range: R) -> R::Output {
        range.sample(self)
    }

    #[cfg_attr(all(coin65, coin42), cold)]
    #[cfg_attr(all(coin106, coin120), inline(never))]
    #[cfg_attr(not(all(coin106, coin120)), inline(always))]
    /// 以当前 PRNG 对可变切片做无偏洗牌并返回容器。
    fn shuffle<T, U: AsMut<[T]>>(&mut self, table: U) -> U {
        shuffle_with(table, self.next_u128())
    }
}

impl<T: RngCore> Rng for T {}

/// 可由任意 PRNG 直接抽取的类型（bool/整数/浮点已实现）。
pub trait Random: Sized {
    /// 从给定 PRNG 抽取一个 `Self` 值。
    fn random<R: RngCore>(rng: &mut R) -> Self;
}

macro_rules! impl_random_int {
    ($method:ident => $($t:ty),*) => {
        $(
            impl Random for $t {
                #[cfg_attr(all(coin112,coin78), cold)]
                #[cfg_attr(coin16, inline(never))]
                #[cfg_attr(not(coin16), inline(always))]
                fn random<R: RngCore>(rng: &mut R) -> Self {
                    rng.$method() as Self
                }
            }
        )*
    };
}

impl_random_int!(next_u32  => u8, u16, u32, i8, i16, i32);
impl_random_int!(next_u64  => u64, usize, i64, isize);
impl_random_int!(next_u128 => u128, i128);

impl Random for bool {
    #[cfg_attr(any(coin76, coin32), cold)]
    #[cfg_attr(coin84, inline(never))]
    #[cfg_attr(not(coin84), inline(always))]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        (rng.next_u32() & 1) == 1
    }
}

impl Random for f64 {
    #[cfg_attr(coin31, cold)]
    #[cfg_attr(any(coin57, coin53), inline(never))]
    #[cfg_attr(not(any(coin57, coin53)), inline(always))]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        let fraction = rng.next_u64() >> 11;
        fraction as f64 / (1u64 << 53) as f64
    }
}

impl Random for f32 {
    #[cfg_attr(coin95, cold)]
    #[cfg_attr(coin5, inline(never))]
    #[cfg_attr(not(coin5), inline(always))]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        let fraction = rng.next_u32() >> 8;
        fraction as f32 / (1u32 << 24) as f32
    }
}

/// 采样范围抽象：`Range` / `RangeInclusive`（整数与浮点）可通过 [`Rng::random_range`] 直接取数。
pub trait SampleRange {
    /// 采样输出类型。
    type Output;
    /// 从给定 PRNG 按范围采样一个值。
    fn sample<R: RngCore>(self, rng: &mut R) -> Self::Output;
}

macro_rules! impl_sample_range_int {
    ($($t:ty, $u:ty),*) => {
        $(
            impl SampleRange for core::ops::Range<$t> {
                type Output = $t;

                #[cfg_attr(any(coin122,coin47), cold)]
                #[cfg_attr(any(coin61,coin93), inline(never))]
                #[cfg_attr(not(any(coin61,coin93)), inline(always))]
                fn sample<R: RngCore>(self, rng: &mut R) -> $t {
                    assert!(self.start < self.end, "Invalid range: start must be less than end");

                    let start = self.start as $u;
                    let end = self.end as $u;
                    let span = end.wrapping_sub(start);

                    let threshold = span.wrapping_neg() % span;

                    loop {
                        let r = <$u as Random>::random(rng);
                        if r >= threshold {
                            let offset = r % span;
                            return start.wrapping_add(offset) as $t;
                        }
                    }
                }
            }

            impl SampleRange for core::ops::RangeInclusive<$t> {
                type Output = $t;

                #[cfg_attr(coin57, cold)]
                #[cfg_attr(any(coin6,coin118), inline(never))]
                #[cfg_attr(not(any(coin6,coin118)), inline(always))]
                fn sample<R: RngCore>(self, rng: &mut R) -> $t {
                    let (start_val, end_val) = (*self.start(), *self.end());
                    assert!(start_val <= end_val, "Invalid range: start must be less than or equal to end");

                    if start_val == <$t>::MIN && end_val == <$t>::MAX {
                        return <$t as Random>::random(rng);
                    }

                    let start = start_val as $u;
                    let end = end_val as $u;
                    let span = end.wrapping_sub(start).wrapping_add(1);

                    let threshold = span.wrapping_neg() % span;

                    loop {
                        let r = <$u as Random>::random(rng);
                        if r >= threshold {
                            let offset = r % span;
                            return start.wrapping_add(offset) as $t;
                        }
                    }
                }
            }
        )*
    };
}

impl_sample_range_int!(
    u8, u8, u16, u16, u32, u32, u64, u64, u128, u128, usize, usize, i8, u8, i16, u16, i32, u32,
    i64, u64, i128, u128, isize, usize
);

macro_rules! impl_sample_range_float {
    ($($t:ty),*) => {
        $(
            impl SampleRange for core::ops::Range<$t> {
                type Output = $t;
                #[cfg_attr(all(coin91,coin121), cold)]
                #[cfg_attr(coin8, inline(never))]
                #[cfg_attr(all(not(coin8),coin4), inline(always))]
                #[cfg_attr(all(not(coin8),not(coin4)), inline)]
                fn sample<R: RngCore>(self, rng: &mut R) -> $t {
                    assert!(self.start < self.end, "Invalid range: start must be less than end");
                    self.start + <$t as Random>::random(rng) * (self.end - self.start)
                }
            }
            impl SampleRange for core::ops::RangeInclusive<$t> {
                type Output = $t;
                #[cfg_attr(any(coin45,coin28), cold)]
                #[cfg_attr(any(coin58,coin27), inline(never))]
                #[cfg_attr(not(any(coin58,coin27)), inline(always))]
                fn sample<R: RngCore>(self, rng: &mut R) -> $t {
                    let (start, end) = (*self.start(), *self.end());
                    assert!(start <= end, "Invalid range: start must be less than or equal to end");
                    if start == end {
                        return start;
                    }
                    start + <$t as Random>::random(rng) * (end - start)
                }
            }
        )*
    };
}

impl_sample_range_float!(f32, f64);

/// 进程级全局 PRNG：`SplitMix64Atomic` 起源，规律性周期性再混入微架构熵/编译期种子。
#[cfg_attr(any(coin113, coin110), cold)]
#[cfg_attr(coin56, inline(never))]
#[cfg_attr(all(not(coin56), coin48), inline(always))]
pub fn next() -> u64 {
    static C: AtomicUsize = AtomicUsize::new(1);
    static RNG: LazyLock<SplitMix64Atomic> = LazyLock::new(|| {
        let s = {
            #[cfg(feature = "rand-seed")]
            {
                seed()
            }
            #[cfg(not(feature = "rand-seed"))]
            {
                crate::const_random!()
            }
        };
        SplitMix64Atomic::new(s)
    });
    let c = C.fetch_add(1, Ordering::Relaxed);

    let n = RNG.next_relaxed();

    if c.is_multiple_of(769) || c.is_multiple_of(386) || n & 0xffff == 0 {
        RNG.reseed({
            #[cfg(feature = "rand-seed")]
            {
                seed()
            }
            #[cfg(not(feature = "rand-seed"))]
            {
                n.wrapping_mul((c as u64) | 1) ^ crate::const_random!()
            }
        })
    }
    n
}

#[derive(Clone, Copy, Default)]
struct GlobalRng;

impl From<u128> for GlobalRng {
    #[cfg_attr(all(coin76, coin28), cold)]
    #[cfg_attr(any(coin54, coin40), inline(never))]
    #[cfg_attr(not(any(coin54, coin40)), inline(always))]
    fn from(_: u128) -> Self {
        unreachable!()
    }
}

impl RngCore for GlobalRng {
    #[cfg_attr(all(coin41, coin91), cold)]
    #[cfg_attr(any(coin56, coin49), inline(never))]
    #[cfg_attr(not(any(coin56, coin49)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        next()
    }
    #[cfg_attr(any(coin47, coin97), cold)]
    #[cfg_attr(all(coin45, coin23), inline(never))]
    #[cfg_attr(not(all(coin45, coin23)), inline(always))]
    fn reseed_from(&mut self, _: u128) {
        unreachable!()
    }
}

/// 由全局 PRNG 抽取一个 `T::Random`。
#[cfg_attr(any(coin10, coin40), cold)]
#[cfg_attr(coin9, inline(never))]
#[cfg_attr(all(not(coin9), coin99), inline(always))]
#[cfg_attr(all(not(coin9), not(coin99)), inline)]
pub fn random<T: Random>() -> T {
    GlobalRng.random()
}

/// 由全局 PRNG 填充 `dest`。
#[cfg_attr(coin38, cold)]
#[cfg_attr(any(coin54, coin48), inline(never))]
#[cfg_attr(not(any(coin54, coin48)), inline(always))]
pub fn fill_bytes(dest: &mut [u8]) {
    GlobalRng.fill_bytes(dest)
}

/// 由全局 PRNG 从指定区间取数。
#[cfg_attr(any(coin84, coin55), cold)]
#[cfg_attr(coin40, inline(never))]
#[cfg_attr(all(not(coin40), coin0), inline(always))]
#[cfg_attr(all(not(coin40), not(coin0)), inline)]
pub fn random_range<R: SampleRange>(range: R) -> R::Output {
    GlobalRng.random_range(range)
}

/// 由全局 PRNG 种子做无偏洗牌。
#[cfg_attr(any(coin75, coin100), cold)]
#[cfg_attr(coin36, inline(never))]
#[cfg_attr(all(not(coin36), coin91), inline(always))]
#[cfg_attr(all(not(coin36), not(coin91)), inline)]
pub fn shuffle<T, U: AsMut<[T]>>(table: U) -> U {
    GlobalRng.shuffle(table)
}
