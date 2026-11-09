// src/rand/xoshiro.rs
#[cfg(feature = "rand-core")]
use super::super::core::RngCore;
use crate::crypto::core::rp::rpp128;
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed128;

#[cfg_attr(any(coin12, coin107), cold)]
#[cfg_attr(all(coin37, coin16), inline(never))]
#[cfg_attr(not(all(coin37, coin16)), inline(always))]
const fn seed_to_state4(seed: u128) -> [u64; 4] {
    let [a, b] = rpp128(seed);
    [
        a as u64 | 1,
        (a >> 64) as u64 | 2,
        b as u64 | 1,
        (b >> 64) as u64 | 2,
    ]
}

#[cfg_attr(all(coin13, coin101), cold)]
#[cfg_attr(coin53, inline(never))]
#[cfg_attr(not(coin53), inline(always))]
const fn seed_to_state2(seed: u128) -> [u64; 2] {
    let [a, b] = rpp128(seed);
    [a as u64 | 1, b as u64 | 2]
}

#[cfg_attr(all(coin44, coin97), cold)]
#[cfg_attr(all(coin38, coin102), inline(never))]
#[cfg_attr(not(all(coin38, coin102)), inline(always))]
const fn step_xoshiro256(s: &mut [u64; 4]) {
    let t = s[1] << 17;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = s[3].rotate_left(45);
}

// ============================================================================
// Xoshiro256PlusPlus
// ============================================================================

/// 256 位状态 xoshiro`++`：`next` 输出 `((s0 + s3).rotl(23) + s0)`。
pub struct Xoshiro256PlusPlus {
    s: [u64; 4],
}

impl Xoshiro256PlusPlus {
    #[cfg_attr(any(coin114, coin47), cold)]
    #[cfg_attr(coin73, inline(never))]
    #[cfg_attr(not(coin73), inline(always))]
    /// 以 128 位种子展开为 256 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state4(seed),
        }
    }

    /// 推进一步（`step_xoshiro256`）并输出混合结果。`const` 可用。
    #[cfg_attr(all(coin35, coin117), cold)]
    #[cfg_attr(coin97, inline(never))]
    #[cfg_attr(all(not(coin97), coin57), inline(always))]
    #[cfg_attr(all(not(coin97), not(coin57)), inline)]
    pub const fn next(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        step_xoshiro256(&mut self.s);
        result
    }
}

impl From<u128> for Xoshiro256PlusPlus {
    #[cfg_attr(all(coin78, coin112), cold)]
    #[cfg_attr(all(coin30, coin124), inline(never))]
    #[cfg_attr(not(all(coin30, coin124)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoshiro256PlusPlus {
    #[cfg_attr(coin82, cold)]
    #[cfg_attr(all(coin79, coin102), inline(never))]
    #[cfg_attr(not(all(coin79, coin102)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoshiro256PlusPlus {
    #[cfg_attr(any(coin19, coin14), cold)]
    #[cfg_attr(all(coin74, coin6), inline(never))]
    #[cfg_attr(not(all(coin74, coin6)), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

// ============================================================================
// Xoshiro256StarStar
// ============================================================================

/// 256 位状态 xoshiro`**`：`next` 输出 `(s1×5).rotl(7)×9`。
pub struct Xoshiro256StarStar {
    s: [u64; 4],
}

impl Xoshiro256StarStar {
    #[cfg_attr(all(coin32, coin102), cold)]
    #[cfg_attr(coin101, inline(never))]
    #[cfg_attr(not(coin101), inline(always))]
    /// 以 128 位种子展开为 256 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state4(seed),
        }
    }

    #[cfg_attr(coin71, cold)]
    #[cfg_attr(any(coin110, coin24), inline(never))]
    #[cfg_attr(not(any(coin110, coin24)), inline(always))]
    /// 推进一步 xoshiro256** 状态并输出 64 位结果。
    pub const fn next(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        step_xoshiro256(&mut self.s);
        result
    }
}

impl From<u128> for Xoshiro256StarStar {
    #[cfg_attr(all(coin122, coin36), cold)]
    #[cfg_attr(coin33, inline(never))]
    #[cfg_attr(not(coin33), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoshiro256StarStar {
    #[cfg_attr(all(coin3, coin47), cold)]
    #[cfg_attr(all(coin101, coin67), inline(never))]
    #[cfg_attr(not(all(coin101, coin67)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoshiro256StarStar {
    #[cfg_attr(coin41, cold)]
    #[cfg_attr(coin16, inline(never))]
    #[cfg_attr(all(not(coin16), coin30), inline(always))]
    #[cfg_attr(all(not(coin16), not(coin30)), inline)]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

// ============================================================================
// Xoroshiro128PlusPlus
// ============================================================================

/// 128 位状态 xoroshiro`++`：`next` 输出 `((s0 + s1).rotl(17) + s0)`。
pub struct Xoroshiro128PlusPlus {
    s: [u64; 2],
}

impl Xoroshiro128PlusPlus {
    #[cfg_attr(any(coin59, coin110), cold)]
    #[cfg_attr(coin77, inline(never))]
    #[cfg_attr(not(coin77), inline(always))]
    /// 以 128 位种子展开为 128 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state2(seed),
        }
    }

    #[cfg_attr(all(coin94, coin42), cold)]
    #[cfg_attr(any(coin79, coin16), inline(never))]
    #[cfg_attr(not(any(coin79, coin16)), inline(always))]
    /// 推进一步 xoroshiro128++ 状态并输出 64 位结果。
    pub const fn next(&mut self) -> u64 {
        let [s0, mut s1] = self.s;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);

        s1 ^= s0;
        self.s[0] = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.s[1] = s1.rotate_left(28);

        result
    }
}

impl From<u128> for Xoroshiro128PlusPlus {
    #[cfg_attr(coin99, cold)]
    #[cfg_attr(all(coin113, coin58), inline(never))]
    #[cfg_attr(not(all(coin113, coin58)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoroshiro128PlusPlus {
    #[cfg_attr(any(coin101, coin5), cold)]
    #[cfg_attr(coin106, inline(never))]
    #[cfg_attr(not(coin106), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoroshiro128PlusPlus {
    #[cfg_attr(coin78, cold)]
    #[cfg_attr(all(coin55, coin15), inline(never))]
    #[cfg_attr(not(all(coin55, coin15)), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

// ============================================================================
// Xoroshiro128StarStar
// ============================================================================

/// 128 位状态 xoroshiro`**`：`next` 输出 `(s0×5).rotl(7)×9`。
pub struct Xoroshiro128StarStar {
    s: [u64; 2],
}

impl Xoroshiro128StarStar {
    #[cfg_attr(any(coin107, coin117), cold)]
    #[cfg_attr(any(coin88, coin64), inline(never))]
    #[cfg_attr(not(any(coin88, coin64)), inline(always))]
    /// 以 128 位种子展开为 128 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state2(seed),
        }
    }

    #[cfg_attr(coin0, cold)]
    #[cfg_attr(coin33, inline(never))]
    #[cfg_attr(not(coin33), inline(always))]
    /// 推进一步 xoroshiro128** 状态并输出 64 位结果。
    pub const fn next(&mut self) -> u64 {
        let [s0, mut s1] = self.s;
        let result = s0.wrapping_mul(5).rotate_left(7).wrapping_mul(9);

        s1 ^= s0;
        self.s[0] = s0.rotate_left(24) ^ s1 ^ (s1 << 16);
        self.s[1] = s1.rotate_left(37);

        result
    }
}

impl From<u128> for Xoroshiro128StarStar {
    #[cfg_attr(coin67, cold)]
    #[cfg_attr(coin29, inline(never))]
    #[cfg_attr(all(not(coin29), coin19), inline(always))]
    #[cfg_attr(all(not(coin29), not(coin19)), inline)]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoroshiro128StarStar {
    #[cfg_attr(all(coin124, coin69), cold)]
    #[cfg_attr(coin29, inline(never))]
    #[cfg_attr(all(not(coin29), coin121), inline(always))]
    #[cfg_attr(all(not(coin29), not(coin121)), inline)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoroshiro128StarStar {
    #[cfg_attr(coin77, cold)]
    #[cfg_attr(any(coin11, coin14), inline(never))]
    #[cfg_attr(not(any(coin11, coin14)), inline(always))]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}
