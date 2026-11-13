// src/rand/cwg.rs
#[cfg(feature = "rand-core")]
use super::super::core::{RngCore, fill_bytes_via_u128};
use crate::crypto::core::rp::rpp128;
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed128;

#[cfg_attr(all(coin99, coin114), inline(never))]
#[cfg_attr(not(all(coin99, coin114)), inline(always))]
const fn seed_to_state_cwg64(seed: u128) -> [u64; 4] {
    let [x1, x2] = rpp128(seed);
    [
        x1 as u64,     // x
        0,             // a
        0,             // weyl
        x2 as u64 | 1, // s
    ]
}

#[cfg_attr(not(any(coin41, coin21)), inline(always))]
const fn step_cwg64(s: &mut [u64; 4]) {
    s[1] = s[1].wrapping_add(s[0]);
    let x = (s[0] >> 1).wrapping_mul(s[1] | 1);
    s[2] = s[2].wrapping_add(s[3]);
    s[0] = x ^ s[2];
}

/// 4×u64 状态的 Cwg 生成器：非线性 Weyl 步进，48 次暖启动，`next` 输出 `u64`。
pub struct Cwg64 {
    s: [u64; 4],
}

impl Cwg64 {
    const WARMUP: usize = 48;

    /// 由 128 位种子构造：`rpp128` 展开初始状态后执行暖启动步进（冷却噪声残留）。
    /// 由 128 位种子构造：`rpp128` 展开初始状态后执行 48 次暖启动。
    #[cfg_attr(coin42, cold)]
    #[cfg_attr(all(coin123, coin70), inline(never))]
    #[cfg_attr(not(all(coin123, coin70)), inline(always))]
    pub const fn from_seed(seed: u128) -> Self {
        let mut state = seed_to_state_cwg64(seed);
        let mut i = 0;
        while i < Self::WARMUP {
            step_cwg64(&mut state);
            i += 1;
        }
        Self { s: state }
    }

    /// 推进一步并输出 ((s1 >> 48) ^ s0)。`const` 可用。
    #[cfg_attr(not(any(coin20, coin75)), inline(always))]
    pub const fn next(&mut self) -> u64 {
        step_cwg64(&mut self.s);
        (self.s[1] >> 48) ^ self.s[0]
    }
}

impl From<u128> for Cwg64 {
    #[cfg_attr(any(coin33, coin113), cold)]
    #[cfg_attr(any(coin21, coin27), inline(never))]
    #[cfg_attr(not(any(coin21, coin27)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Cwg64 {
    #[cfg_attr(all(not(coin83), coin92), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Cwg64 {
    #[cfg_attr(coin4, cold)]
    #[cfg_attr(coin10, inline(never))]
    #[cfg_attr(all(not(coin10), coin37), inline(always))]
    #[cfg_attr(all(not(coin10), not(coin37)), inline)]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

#[cfg_attr(not(coin17), inline(always))]
const fn step_cwg128x64(x: &mut u128, a: &mut u64, weyl: &mut u64, s: u64) {
    *a = a.wrapping_add(*x as u64);
    let factor = (*a >> 1) as u128;
    *weyl = weyl.wrapping_add(s);
    *x = (*x | 1).wrapping_mul(factor) ^ (*weyl as u128);
}

/// 混合位宽 Cwg 变体（128 位主状态 + u64 辅助步进）：48 次暖启动，`next` 输出 128 位。
pub struct Cwg128x64 {
    x: u128,
    a: u64,
    weyl: u64,
    s: u64,
}

impl Cwg128x64 {
    const WARMUP: usize = 48;

    /// 由 128 位种子构造（`rpp128` 展开状态后 48 次暖启动）。
    #[cfg_attr(any(coin120, coin47), inline(never))]
    pub const fn from_seed(seed: u128) -> Self {
        let [x1, x2] = rpp128(seed);

        let mut g = Self {
            x: x1,
            a: 0,
            weyl: 0,
            s: x2 as u64 | 1,
        };
        let mut i = 0;
        while i < Self::WARMUP {
            step_cwg128x64(&mut g.x, &mut g.a, &mut g.weyl, g.s);
            i += 1;
        }
        g
    }

    /// 推进一步并输出 128 位折叠结果。`const` 可用。
    #[cfg_attr(all(not(coin53), coin101), inline(always))]
    pub const fn next(&mut self) -> u128 {
        step_cwg128x64(&mut self.x, &mut self.a, &mut self.weyl, self.s);
        ((self.a >> 48) as u128) ^ self.x
    }
}

impl From<u128> for Cwg128x64 {
    #[cfg_attr(not(any(coin62, coin57)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Cwg128x64 {
    #[cfg_attr(not(coin67), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next() as u64
    }

    #[cfg_attr(not(any(coin127, coin64)), inline(always))]
    fn next_u128(&mut self) -> u128 {
        self.next()
    }

    #[cfg_attr(coin2, inline(always))]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes_via_u128(self, dest)
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Cwg128x64 {
    #[cfg_attr(all(coin74, coin3), cold)]
    #[cfg_attr(all(coin55, coin40), inline(never))]
    #[cfg_attr(not(all(coin55, coin40)), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

#[cfg_attr(not(coin126), inline(always))]
const fn seed_to_state_cwg128(seed: u128) -> [u128; 4] {
    let [x1, x2] = rpp128(seed);
    [
        x1 | 1, // s
        x2,     // x
        0,      // a
        0,      // weyl
    ]
}

#[cfg_attr(coin5, cold)]
#[cfg_attr(not(any(coin75, coin57)), inline(always))]
const fn step_cwg128(c: &mut [u128; 4]) {
    c[2] = c[2].wrapping_add(c[1]);
    let x = (c[1] >> 1).wrapping_mul(c[2] | 1);
    c[3] = c[3].wrapping_add(c[0]);
    c[1] = x ^ c[3];
}

/// 4×u128 状态的 Cwg 生成器：96 次暖启动，`next` 输出 128 位。
pub struct Cwg128 {
    c: [u128; 4],
}

impl Cwg128 {
    const WARMUP: usize = 96;

    /// 由 128 位种子构造（`rpp128` 展开状态后 96 次暖启动）。
    #[cfg_attr(not(coin59), inline(always))]
    pub const fn from_seed(seed: u128) -> Self {
        let mut state = seed_to_state_cwg128(seed);
        let mut i = 0;
        while i < Self::WARMUP {
            step_cwg128(&mut state);
            i += 1;
        }
        Self { c: state }
    }

    #[cfg_attr(all(not(coin78), not(coin109)), inline)]
    /// 推进一步 CGW128 状态并输出 128 位结果。
    pub const fn next(&mut self) -> u128 {
        step_cwg128(&mut self.c);
        (self.c[2] >> 96) ^ self.c[1]
    }
}

impl From<u128> for Cwg128 {
    #[cfg_attr(not(coin43), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Cwg128 {
    #[cfg_attr(all(not(coin70), coin85), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next() as u64
    }

    #[cfg_attr(any(coin119, coin111), inline(always))]
    fn next_u128(&mut self) -> u128 {
        self.next()
    }

    #[cfg_attr(all(coin46, coin10), inline(always))]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        fill_bytes_via_u128(self, dest)
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Cwg128 {
    #[cfg_attr(not(all(coin80, coin2)), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

#[cfg(feature = "rand-core")]
#[cfg(feature = "rand-seed")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::core::{Rng, RngCore};

    #[test]
    fn test_cwg() {
        let mut rng64 = Cwg64::default();
        let _: u8 = rng64.random();

        let mut rng128 = Cwg128::new(42);
        rng128.reseed(-1);
        let _: u128 = rng128.random();
        let _: i128 = rng128.random_range(-1000i128..=1000i128);

        let mut buf = [0u8; 35];
        rng128.fill_bytes(&mut buf);
    }
}
