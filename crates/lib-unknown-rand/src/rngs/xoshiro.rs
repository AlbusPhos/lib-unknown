// src/rand/xoshiro.rs
#[allow(unused_imports)]
use lib_unknown_polymorphism::poly;

#[cfg(feature = "rand-core")]
use super::super::core::RngCore;
use lib_unknown_crypto::core::rp::rpp128;
#[cfg(feature = "rand-seed")]
use lib_unknown_seed::seed128;

#[poly(inline = 0.7, cold = 0.05)]
const fn seed_to_state4(seed: u128) -> [u64; 4] {
    let [a, b] = rpp128(seed);
    [
        a as u64 | 1,
        (a >> 64) as u64 | 2,
        b as u64 | 1,
        (b >> 64) as u64 | 2,
    ]
}

#[poly(inline = 0.9, cold = 0.05)]
const fn seed_to_state2(seed: u128) -> [u64; 2] {
    let [a, b] = rpp128(seed);
    [a as u64 | 1, b as u64 | 2]
}

#[poly(inline = 0.85)]
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
    #[poly(inline = 0.9, cold = 0.05)]
    /// 以 128 位种子展开为 256 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state4(seed),
        }
    }

    /// 推进一步（`step_xoshiro256`）并输出混合结果。`const` 可用。
    #[poly(inline = 0.85)]
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
    #[poly(inline = 0.9)]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoshiro256PlusPlus {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoshiro256PlusPlus {
    #[poly(inline = 0.9)]
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
    #[poly(inline = 0.9, cold = 0.05)]
    /// 以 128 位种子展开为 256 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state4(seed),
        }
    }

    #[poly(inline = 0.9)]
    /// 推进一步 xoshiro256** 状态并输出 64 位结果。
    pub const fn next(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        step_xoshiro256(&mut self.s);
        result
    }
}

impl From<u128> for Xoshiro256StarStar {
    #[poly(inline = 0.9)]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoshiro256StarStar {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoshiro256StarStar {
    #[poly]
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
    #[poly(inline = 0.9, cold = 0.05)]
    /// 以 128 位种子展开为 128 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state2(seed),
        }
    }

    #[poly(inline = 0.85)]
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
    #[poly(inline = 0.9)]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoroshiro128PlusPlus {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoroshiro128PlusPlus {
    #[poly(inline = 0.9)]
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
    #[poly(inline = 0.9, cold = 0.05)]
    /// 以 128 位种子展开为 128 位状态。
    pub const fn from_seed(seed: u128) -> Self {
        Self {
            s: seed_to_state2(seed),
        }
    }

    #[poly(inline = 0.85)]
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
    #[poly]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl RngCore for Xoroshiro128StarStar {
    #[poly(inline = 0.9)]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl Default for Xoroshiro128StarStar {
    #[poly(inline = 0.9)]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}
