// src/rand/core.rs
use crate::crypto::core::shuffle_with;
use crate::lazy::LazyLock;
use crate::rand::rngs::splitmix64::SplitMix64Atomic;
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed;

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
                #[inline(always)]
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
    #[inline(always)]
    fn into_seed128(self) -> u128 {
        u128::from_le_bytes(self)
    }
}
impl Seed128 for (u64, u64) {
    #[inline(always)]
    fn into_seed128(self) -> u128 {
        ((self.0 as u128) << 64) | (self.1 as u128)
    }
}
impl Seed128 for [u64; 2] {
    #[inline(always)]
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
    #[inline]
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
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    /// 生成下一个 128 位随机数（高 64 位来自第一次 [`next_u64`](Self::next_u64)）。
    #[inline(always)]
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
    ///
    /// # Panics
    ///
    /// - 当整数区间为空（`Range` 满足 `start >= end`，`RangeInclusive` 满足
    ///   `start > end`），或浮点区间不满足 `start < end`（`RangeInclusive` 为
    ///   `start <= end`）时 panic。
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

    /// 直接生成定长随机字节数组（底层按 [`RngCore::fill_bytes`] 整块填充）。
    ///
    /// 同样产出 `[u8; N]` 时，本方法比 [`Random`] 为 `[T; N]` 提供的实现更高效：
    /// 后者逐元素调用 `next_u32`，前者按 8/16 字节块拷贝。
    ///
    /// # Examples
    ///
    /// ```
    /// use lib_unknown::rand::core::{GlobalRng, Rng};
    ///
    /// let mut rng = GlobalRng;
    /// let key: [u8; 16] = rng.random_bytes();
    /// assert_eq!(key.len(), 16);
    /// ```
    #[inline(always)]
    fn random_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut buf = [0u8; N];
        self.fill_bytes(&mut buf);
        buf
    }

    /// 分配并生成指定长度 `len` 的随机字节向量（底层按 [`RngCore::fill_bytes`] 填充）。
    ///
    /// # Feature flags
    ///
    /// 需要启用 `"alloc"` 特性（`"std"` 会隐式启用它）。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "alloc")]
    /// # {
    /// use lib_unknown::rand::core::{GlobalRng, Rng};
    ///
    /// let mut rng = GlobalRng;
    /// let buf = rng.random_vec(32);
    /// assert_eq!(buf.len(), 32);
    /// # }
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    fn random_vec(&mut self, len: usize) -> alloc::vec::Vec<u8> {
        let mut buf = alloc::vec![0u8; len];
        self.fill_bytes(&mut buf);
        buf
    }

    /// 以 `p`（`0.0..=1.0`）的概率返回 `true`。
    ///
    /// `p <= 0.0` 恒返回 `false`，`p >= 1.0` 恒返回 `true`；`p` 为 `NaN` 时返回 `false`。
    ///
    /// # Examples
    ///
    /// ```
    /// use lib_unknown::rand::core::{GlobalRng, Rng};
    ///
    /// let mut rng = GlobalRng;
    /// assert!(!rng.random_bool(0.0));
    /// assert!(rng.random_bool(1.0));
    /// ```
    #[inline]
    fn random_bool(&mut self, p: f64) -> bool {
        if p <= 0.0 {
            return false;
        }
        if p >= 1.0 {
            return true;
        }
        self.random::<f64>() < p
    }

    /// 从切片中无偏随机选择一个元素的引用；切片为空时返回 [`None`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use lib_unknown::rand::core::{GlobalRng, Rng};
    ///
    /// let mut rng = GlobalRng;
    /// let picked = rng.choose(&[10u8, 20, 30]);
    /// assert!(picked.is_some());
    /// assert!(rng.choose(&[] as &[u8]).is_none());
    /// ```
    #[inline]
    fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            Some(&slice[self.random_range(0..slice.len())])
        }
    }

    /// 从可变切片中无偏随机选择一个元素的可变引用；切片为空时返回 [`None`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use lib_unknown::rand::core::{GlobalRng, Rng};
    ///
    /// let mut rng = GlobalRng;
    /// let mut slots = [0u8; 4];
    /// *rng.choose_mut(&mut slots).unwrap() = 1;
    /// assert_eq!(slots.iter().filter(|&&v| v == 1).count(), 1);
    /// ```
    #[inline]
    fn choose_mut<'a, T>(&mut self, slice: &'a mut [T]) -> Option<&'a mut T> {
        if slice.is_empty() {
            None
        } else {
            let idx = self.random_range(0..slice.len());
            Some(&mut slice[idx])
        }
    }
}

impl<T: RngCore> Rng for T {}

/// 可由任意 PRNG 直接抽取的类型（bool/整数/浮点/char/数组已实现）。
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
        const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
        let fraction = rng.next_u64() >> 11;
        (fraction as f64) * SCALE
    }
}

impl Random for f32 {
    #[cfg_attr(coin95, cold)]
    #[cfg_attr(coin5, inline(never))]
    #[cfg_attr(not(coin5), inline(always))]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        const SCALE: f32 = 1.0 / ((1u32 << 24) as f32);
        let fraction = rng.next_u32() >> 8;
        (fraction as f32) * SCALE
    }
}

/// 定长数组采样：逐元素对 `T` 独立调用 [`Random::random`]。
///
/// 若目标只是 `[u8; N]` 随机字节，优先使用 [`Rng::random_bytes`]（按块填充，开销更低）。
impl<T: Random, const N: usize> Random for [T; N] {
    #[inline]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        core::array::from_fn(|_| T::random(rng))
    }
}

/// `char` 采样：在全部 Unicode 标量值上等概率取值，自动跳过 `0xD800..=0xDFFF` 代理区。
impl Random for char {
    #[inline]
    fn random<R: RngCore>(rng: &mut R) -> Self {
        const SURROGATE_START: u32 = 0xD800;
        const SURROGATE_LEN: u32 = 0x800;
        const MAX_CHAR: u32 = 0x10FFFF;
        let n = (0..=(MAX_CHAR - SURROGATE_LEN)).sample(rng);
        let cp = if n >= SURROGATE_START {
            n + SURROGATE_LEN
        } else {
            n
        };
        // SAFETY: cp 保证落在 [0, 0xD7FF] ∪ [0xE000, 0x10FFFF]
        unsafe { char::from_u32_unchecked(cp) }
    }
}

/// 采样范围抽象：`Range` / `RangeInclusive`（整数与浮点）可通过 [`Rng::random_range`] 直接取数。
pub trait SampleRange {
    /// 采样输出类型。
    type Output;
    /// 从给定 PRNG 按范围采样一个值。
    ///
    /// # Panics
    ///
    /// - 当区间为空（整数 `Range` 的 `start >= end`、`RangeInclusive` 的
    ///   `start > end`；浮点区间不满足 `start < end`，浮点 `RangeInclusive`
    ///   不满足 `start <= end`）时 panic。
    fn sample<R: RngCore>(self, rng: &mut R) -> Self::Output;
}

/// Lemire 无除法无偏区间采样宏（适用于 u8..=u64 / usize，快路径 0 除法指令）。
macro_rules! impl_sample_range_int_lemire {
    ($($t:ty, $u:ty, $wide:ty),*) => {
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

                    let r = <$u as Random>::random(rng);
                    let mut mul = (r as $wide).wrapping_mul(span as $wide);
                    let mut low = mul as $u;

                    if low < span {
                        let threshold = span.wrapping_neg() % span;
                        while low < threshold {
                            let r = <$u as Random>::random(rng);
                            mul = (r as $wide).wrapping_mul(span as $wide);
                            low = mul as $u;
                        }
                    }

                    let offset = (mul >> <$u>::BITS) as $u;
                    start.wrapping_add(offset) as $t
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

                    let r = <$u as Random>::random(rng);
                    let mut mul = (r as $wide).wrapping_mul(span as $wide);
                    let mut low = mul as $u;

                    if low < span {
                        let threshold = span.wrapping_neg() % span;
                        while low < threshold {
                            let r = <$u as Random>::random(rng);
                            mul = (r as $wide).wrapping_mul(span as $wide);
                            low = mul as $u;
                        }
                    }

                    let offset = (mul >> <$u>::BITS) as $u;
                    start.wrapping_add(offset) as $t
                }
            }
        )*
    };
}

/// 128 位整数无偏区间采样（惰性计算 threshold，省去快路径下的多余 128 位取模）。
macro_rules! impl_sample_range_int_128 {
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

                    let mut r = <$u as Random>::random(rng);
                    if r < span {
                        let threshold = span.wrapping_neg() % span;
                        while r < threshold {
                            r = <$u as Random>::random(rng);
                        }
                    }
                    start.wrapping_add(r % span) as $t
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

                    let mut r = <$u as Random>::random(rng);
                    if r < span {
                        let threshold = span.wrapping_neg() % span;
                        while r < threshold {
                            r = <$u as Random>::random(rng);
                        }
                    }
                    start.wrapping_add(r % span) as $t
                }
            }
        )*
    };
}

impl_sample_range_int_lemire!(
    u8, u8, u16, u16, u16, u32, u32, u32, u64, u64, u64, u128, usize, usize, u128, i8, u8, u16,
    i16, u16, u32, i32, u32, u64, i64, u64, u128, isize, usize, u128
);

impl_sample_range_int_128!(u128, u128, i128, u128);

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
                    let r = <$t as Random>::random(rng);
                    let val = (1.0 - r) * self.start + r * self.end;
                    // 防止浮点舍入导致结果等于右开边界 self.end
                    if val >= self.end {
                        self.start
                    } else {
                        val
                    }
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
                    let r = <$t as Random>::random(rng);
                    (1.0 - r) * start + r * end
                }
            }
        )*
    };
}

impl_sample_range_float!(f32, f64);

#[inline(always)]
fn next_atomic() -> u64 {
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

    let n = RNG.next_relaxed();

    if (n & 0x3ff) == 0x29a {
        #[cold]
        #[inline(never)]
        fn reseed_slow(rng: &SplitMix64Atomic, _n: u64) {
            rng.reseed({
                #[cfg(feature = "rand-seed")]
                {
                    seed()
                }
                #[cfg(not(feature = "rand-seed"))]
                {
                    _n.wrapping_mul(crate::const_random!() | 1)
                        ^ crate::const_random!()
                        ^ _n.wrapping_add(crate::const_random!())
                }
            });
        }
        reseed_slow(&RNG, n);
    }
    n
}

#[cfg(all(feature = "std", feature = "rand-cwg"))]
mod global_imp {
    use super::next_atomic;
    use crate::rand::core::RngCore;
    use crate::rand::rngs::cwg::Cwg128x64;
    use core::cell::UnsafeCell;

    #[inline(never)]
    fn make_thread_seed() -> u128 {
        let lo = next_atomic() as u128;
        let hi = next_atomic() as u128;
        let atomic_mix = (hi << 64) | lo;
        #[cfg(feature = "rand-seed")]
        {
            atomic_mix ^ crate::rand::seed::seed128()
        }
        #[cfg(not(feature = "rand-seed"))]
        {
            atomic_mix ^ crate::const_random!(u128)
        }
    }

    struct TlsCwg {
        rng: Cwg128x64,
        cached_hi: u64,
        /// 奇偶位 (bit 0) 标记 `cached_hi` 是否有效；满 2^24 次 (~128 MiB) 触发一次低频重播种
        step: u32,
    }

    impl TlsCwg {
        #[inline(never)]
        fn new() -> Self {
            Self {
                rng: Cwg128x64::from_seed(make_thread_seed()),
                cached_hi: 0,
                step: 0,
            }
        }

        #[cold]
        #[inline(never)]
        fn reseed_slow(&mut self) {
            self.rng = Cwg128x64::from_seed(make_thread_seed());
        }

        #[inline(always)]
        fn next_u64(&mut self) -> u64 {
            let s = self.step;
            self.step = s.wrapping_add(1);

            if (s & 1) != 0 {
                return self.cached_hi;
            }

            if (s & 0xff_fffe) == 0xff_fffe {
                self.reseed_slow();
            }

            let val = self.rng.next();
            self.cached_hi = (val >> 64) as u64;
            val as u64
        }

        #[inline(always)]
        fn next_u128(&mut self) -> u128 {
            let s = self.step;
            self.step = s.wrapping_add(2);
            if (s & 0xff_fffe) == 0xff_fffe {
                self.reseed_slow();
            }
            self.rng.next()
        }

        #[inline(always)]
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            self.rng.fill_bytes(dest);
        }
    }

    std::thread_local! {
        static TLS_RNG: UnsafeCell<TlsCwg> = UnsafeCell::new(TlsCwg::new());
    }

    #[inline(always)]
    pub fn next_u64() -> u64 {
        // SAFETY: `TLS_RNG` 是线程局部存储，`with` 保证当前线程独占访问；
        // 返回引用不会逃逸出闭包，因此同一线程内不存在别名 `&mut`。
        TLS_RNG.with(|cell| unsafe { (*cell.get()).next_u64() })
    }

    #[inline(always)]
    pub fn next_u128() -> u128 {
        // SAFETY: 同 `next_u64`，线程局部 `UnsafeCell` 在闭包内独占访问且引用不外泄。
        TLS_RNG.with(|cell| unsafe { (*cell.get()).next_u128() })
    }

    #[inline(always)]
    pub fn fill_bytes(dest: &mut [u8]) {
        // SAFETY: 同 `next_u64`，线程局部 `UnsafeCell` 在闭包内独占访问且引用不外泄。
        TLS_RNG.with(|cell| unsafe { (*cell.get()).fill_bytes(dest) })
    }
}

#[cfg(not(all(feature = "std", feature = "rand-cwg")))]
mod global_imp {
    use super::next_atomic;
    use crate::rand::core::RngCore;
    use crate::rand::rngs::splitmix64::SplitMix64;

    #[inline(always)]
    pub fn next_u64() -> u64 {
        next_atomic()
    }

    #[inline(always)]
    pub fn next_u128() -> u128 {
        let lo = next_atomic() as u128;
        let hi = next_atomic() as u128;
        (hi << 64) | lo
    }

    #[inline(always)]
    pub fn fill_bytes(dest: &mut [u8]) {
        if dest.is_empty() {
            return;
        }
        if dest.len() <= 8 {
            let bytes = next_atomic().to_le_bytes();
            dest.copy_from_slice(&bytes[..dest.len()]);
        } else {
            let mut local_rng = SplitMix64::from_seed(next_atomic());
            local_rng.fill_bytes(dest);
        }
    }
}

/// 进程级全局 PRNG 取数入口：启用 `std` + `rand-cwg` 时走每线程 `Cwg128x64`，
/// 否则退回原子 `SplitMix64`；两种路径都会低概率混入微架构熵/编译期种子重播种。
#[cfg_attr(all(not(coin56), coin48), inline(always))]
pub fn next() -> u64 {
    global_imp::next_u64()
}

/// 零大小全局 PRNG 句柄（实现 [`RngCore`] 与 [`Rng`]）。
///
/// 每次取数都委托给进程级全局随机源（见 [`next`]）；该源是按线程/进程共享的，
/// 因此 `GlobalRng` 自身不保存任何状态：
///
/// - `From<u128>` 忽略传入种子，始终返回同一个零大小句柄；
/// - [`RngCore::reseed_from`] 为无操作，无法通过该句柄
///   替换全局源的状态。
///
/// 需要可复现、可独立重播种的序列时，请直接使用具名生成器（如
/// `SplitMix64` / `Cwg128x64`）而非 `GlobalRng`。
#[derive(Clone, Copy, Default, Debug)]
pub struct GlobalRng;

impl From<u128> for GlobalRng {
    #[cfg_attr(not(any(coin54, coin40)), inline(always))]
    fn from(_: u128) -> Self {
        Self
    }
}

impl RngCore for GlobalRng {
    #[cfg_attr(not(any(coin56, coin49)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        global_imp::next_u64()
    }

    #[cfg_attr(not(all(coin45, coin23)), inline(always))]
    fn reseed_from(&mut self, _: u128) {}

    #[inline(always)]
    fn next_u128(&mut self) -> u128 {
        global_imp::next_u128()
    }

    #[inline(always)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        global_imp::fill_bytes(dest)
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

/// 由全局 PRNG 生成定长随机字节数组。
///
/// # Examples
///
/// ```
/// let nonce: [u8; 12] = lib_unknown::rand::core::random_bytes();
/// assert_eq!(nonce.len(), 12);
/// ```
#[inline(always)]
pub fn random_bytes<const N: usize>() -> [u8; N] {
    GlobalRng.random_bytes()
}

/// 由全局 PRNG 分配并生成指定长度 `len` 的随机字节向量。
///
/// # Feature flags
///
/// 需要启用 `"alloc"` 特性（`"std"` 会隐式启用它）。
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn random_vec(len: usize) -> alloc::vec::Vec<u8> {
    GlobalRng.random_vec(len)
}

/// 由全局 PRNG 以概率 `p`（`0.0..=1.0`）返回 `true`。
///
/// `p <= 0.0` 恒返回 `false`，`p >= 1.0` 恒返回 `true`，`p` 为 `NaN` 时返回 `false`。
///
/// # Examples
///
/// ```
/// use lib_unknown::rand::core::random_bool;
///
/// assert!(!random_bool(0.0));
/// assert!(random_bool(1.0));
/// ```
#[inline(always)]
pub fn random_bool(p: f64) -> bool {
    GlobalRng.random_bool(p)
}

/// 由全局 PRNG 从切片中无偏随机选择一个元素的引用；切片为空时返回 [`None`]。
///
/// # Examples
///
/// ```
/// use lib_unknown::rand::core::choose;
///
/// assert!(choose(&[1u8, 2, 3]).is_some());
/// assert!(choose(&[] as &[u8]).is_none());
/// ```
#[inline(always)]
pub fn choose<T>(slice: &[T]) -> Option<&T> {
    GlobalRng.choose(slice)
}

/// 由全局 PRNG 从可变切片中无偏随机选择一个元素的可变引用；切片为空时返回 [`None`]。
#[inline(always)]
pub fn choose_mut<T>(slice: &mut [T]) -> Option<&mut T> {
    GlobalRng.choose_mut(slice)
}

/// 由全局 PRNG 填充 `dest`。
#[cfg_attr(coin38, cold)]
#[cfg_attr(any(coin54, coin48), inline(never))]
#[cfg_attr(not(any(coin54, coin48)), inline(always))]
pub fn fill_bytes(dest: &mut [u8]) {
    GlobalRng.fill_bytes(dest)
}

/// 由全局 PRNG 从指定区间取数。
///
/// # Panics
///
/// - 当区间为空（整数 `Range` 的 `start >= end`、`RangeInclusive` 的
///   `start > end`；浮点区间不满足 `start < end`，浮点 `RangeInclusive`
///   不满足 `start <= end`）时 panic。
///
/// # Examples
///
/// ```
/// let v = lib_unknown::rand::core::random_range(0u8..=9);
/// assert!(v < 10);
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::rngs::splitmix64::SplitMix64;

    fn fixed(seed: u64) -> SplitMix64 {
        SplitMix64::new(seed)
    }

    #[test]
    fn int_range_uniform_and_in_bounds() {
        let mut rng = fixed(0x5EED_1234);
        let mut buckets = [0u32; 8];
        for _ in 0..80_000 {
            buckets[rng.random_range(0u8..8) as usize] += 1;
        }
        // 每桶期望 10000，固定种子下允许 ±5%。
        for &count in &buckets {
            assert!((9_500..=10_500).contains(&count), "bias: {count}");
        }

        for _ in 0..10_000 {
            let x = rng.random_range(1u64..u64::MAX);
            assert!((1..u64::MAX).contains(&x));
            let y = rng.random_range(-1_000_000i32..=1_000_000);
            assert!((-1_000_000..=1_000_000).contains(&y));
            let z = rng.random_range(u128::MIN..u128::MAX);
            assert!(z < u128::MAX);
            let w = rng.random_range(i128::MIN..=-1);
            assert!(w < 0);
        }
    }

    #[test]
    fn int_range_single_value_and_full_span() {
        let mut rng = fixed(7);
        assert_eq!(rng.random_range(5u64..=5), 5);
        assert_eq!(rng.random_range(0u128..=0), 0);
        // 全范围短路：不得 panic。
        let _: u8 = rng.random_range(0u8..=u8::MAX);
        let _: i128 = rng.random_range(i128::MIN..=i128::MAX);
    }

    #[test]
    fn int_range_pow2_span_is_balanced() {
        let mut rng = fixed(0xABCD);
        let mut buckets = [0u32; 256];
        for _ in 0..256 * 500 {
            buckets[rng.random_range(0u16..256) as usize] += 1;
        }
        for &count in &buckets {
            assert!((425..=575).contains(&count), "bias: {count}");
        }
    }

    #[test]
    fn char_random_skips_surrogates() {
        let mut rng = fixed(99);
        let mut astral = 0usize;
        for _ in 0..40_000 {
            let c: char = rng.random();
            let cp = c as u32;
            assert!(!(0xD800..=0xDFFF).contains(&cp));
            if cp > 0xFFFF {
                astral += 1;
            }
        }
        assert!(astral > 0, "辅助平面字符占比异常");
    }

    #[test]
    fn float_ranges_stay_in_bounds() {
        let mut rng = fixed(3);
        for _ in 0..20_000 {
            let a = rng.random_range(-f64::MAX..f64::MAX);
            assert!(a.is_finite() && a < f64::MAX);
            let b = rng.random_range(-f32::MAX..f32::MAX);
            assert!(b.is_finite() && b < f32::MAX);
            let c = rng.random_range(0.0f64..=1.0);
            assert!((0.0..=1.0).contains(&c));
        }
        // 相邻浮点值构成的窄区间不得越界。
        let start = 1.0f64;
        let end = f64::from_bits(start.to_bits() + 1);
        for _ in 0..20_000 {
            let v = rng.random_range(start..=end);
            assert!(v >= start && v <= end);
        }
    }

    #[test]
    fn choose_helpers() {
        let mut rng = fixed(11);
        assert!(rng.choose(&[] as &[u8]).is_none());
        assert!(rng.choose_mut::<u8>(&mut []).is_none());

        let values = [1u8, 2, 3, 4];
        for _ in 0..1_000 {
            let picked = rng.choose(&values).copied().expect("非空切片必返回 Some");
            assert!(values.contains(&picked));
        }

        let mut slots = [0u8; 4];
        *rng.choose_mut(&mut slots).expect("非空切片必返回 Some") = 9;
        assert_eq!(slots.iter().filter(|&&v| v == 9).count(), 1);
    }

    #[test]
    fn bytes_and_bool() {
        let mut rng = fixed(5);
        let empty: [u8; 0] = rng.random_bytes();
        assert_eq!(empty.len(), 0);
        let buf: [u8; 32] = rng.random_bytes();
        assert!(buf.iter().any(|&b| b != 0));

        assert!(!rng.random_bool(0.0));
        assert!(rng.random_bool(1.0));
        assert!(!rng.random_bool(-1.0));
        assert!(rng.random_bool(2.0));
        assert!(!rng.random_bool(f64::NAN));

        #[cfg(feature = "alloc")]
        {
            let v = rng.random_vec(64);
            assert_eq!(v.len(), 64);
            assert!(v.iter().any(|&b| b != 0));
        }
    }
}
