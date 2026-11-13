// src/rand/chacha.rs
#[cfg(feature = "rand-core")]
use super::super::core::RngCore;
use crate::crypto::core::rp::{rp128, rpp128};
use crate::hide_array;
#[cfg(feature = "rand-seed")]
use crate::rand::seed::seed128;

#[cfg_attr(coin72, cold)]
#[cfg_attr(coin86, inline(never))]
#[cfg_attr(not(coin86), inline(always))]
const fn seed_to_state_chacha(seed: u128) -> [u32; 16] {
    let [k0, k1] = rpp128(seed);
    let nonce = rp128(seed.wrapping_add(3));

    let constants = hide_array!(u32, [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);

    [
        constants[0],
        constants[1],
        constants[2],
        constants[3],
        k0 as u32,
        (k0 >> 32) as u32,
        (k0 >> 64) as u32,
        (k0 >> 96) as u32,
        k1 as u32,
        (k1 >> 32) as u32,
        (k1 >> 64) as u32,
        (k1 >> 96) as u32,
        0,
        0,
        nonce as u32,
        (nonce >> 32) as u32,
    ]
}

#[cfg_attr(coin91, cold)]
#[cfg_attr(any(coin114, coin62), inline(never))]
#[cfg_attr(not(any(coin114, coin62)), inline(always))]
const fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);

    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);

    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);

    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

#[cfg_attr(any(coin41, coin60), cold)]
#[cfg_attr(coin100, inline(never))]
#[cfg_attr(all(not(coin100), coin14), inline(always))]
#[cfg_attr(all(not(coin100), not(coin14)), inline)]
const fn double_round(s: &mut [u32; 16]) {
    // 列变换 (Column Round)
    quarter_round(s, 0, 4, 8, 12);
    quarter_round(s, 1, 5, 9, 13);
    quarter_round(s, 2, 6, 10, 14);
    quarter_round(s, 3, 7, 11, 15);

    // 对角变换 (Diagonal Round)
    quarter_round(s, 0, 5, 10, 15);
    quarter_round(s, 1, 6, 11, 12);
    quarter_round(s, 2, 7, 8, 13);
    quarter_round(s, 3, 4, 9, 14);
}

#[cfg_attr(coin46, cold)]
#[cfg_attr(coin67, inline(never))]
#[cfg_attr(all(not(coin67), coin10), inline(always))]
#[cfg_attr(all(not(coin67), not(coin10)), inline)]
const fn increment_counter(s: &mut [u32; 16]) {
    let (next_lo, carry) = s[12].overflowing_add(1);
    s[12] = next_lo;
    if carry {
        s[13] = s[13].wrapping_add(1);
    }
}

#[cfg_attr(coin50, cold)]
#[cfg_attr(all(coin114, coin77), inline(never))]
#[cfg_attr(not(all(coin114, coin77)), inline(always))]
const fn chacha_core_block<const ROUNDS: usize>(input: &[u32; 16]) -> [u64; 8] {
    let mut s = *input;
    let mut r = 0;
    while r < ROUNDS {
        double_round(&mut s);
        r += 2;
    }

    let mut output = [0u64; 8];
    let mut i = 0;
    while i < 8 {
        let lo = s[i * 2].wrapping_add(input[i * 2]) as u64;
        let hi = s[i * 2 + 1].wrapping_add(input[i * 2 + 1]) as u64;
        output[i] = lo | (hi << 32);
        i += 1;
    }
    output
}

/// 参数化轮数的 ChaCha 生成器：16×u32 状态 + 预生成块缓冲，8×u64 交错输出可兼作 256 位块。
pub struct ChaCha<const ROUNDS: usize> {
    state: [u32; 16],
    buffer: [u64; 8],
    index: usize,
}

impl<const ROUNDS: usize> ChaCha<ROUNDS> {
    /// 由 128 位种子构造：种子展开为状态后先执行一个核心块并递增计数器（预热）。
    ///
    /// # Panics
    ///
    /// `ROUNDS` 不受限制编译期校验，仅受调用方所选类型别名约束。
    #[cfg_attr(any(coin52, coin31), cold)]
    #[cfg_attr(coin74, inline(never))]
    #[cfg_attr(all(not(coin74), coin19), inline(always))]
    #[cfg_attr(all(not(coin74), not(coin19)), inline)]
    pub const fn from_seed(seed: u128) -> Self {
        let mut state = seed_to_state_chacha(seed);
        let buffer = chacha_core_block::<ROUNDS>(&state);
        increment_counter(&mut state);
        Self {
            state,
            buffer,
            index: 0,
        }
    }

    /// 产出一个 64 位输出；预生成缓冲耗尽时重新执行核心块并递增计数器。`const` 可用。
    #[cfg_attr(not(all(coin31, coin81)), inline(always))]
    pub const fn next(&mut self) -> u64 {
        if self.index >= 8 {
            self.buffer = chacha_core_block::<ROUNDS>(&self.state);
            increment_counter(&mut self.state);
            self.index = 0;
        }
        let val = self.buffer[self.index];
        self.index += 1;
        val
    }
}

impl<const ROUNDS: usize> From<u128> for ChaCha<ROUNDS> {
    #[cfg_attr(any(coin50, coin1), cold)]
    #[cfg_attr(any(coin99, coin6), inline(never))]
    #[cfg_attr(not(any(coin99, coin6)), inline(always))]
    fn from(value: u128) -> Self {
        Self::from_seed(value)
    }
}

#[cfg(feature = "rand-core")]
impl<const ROUNDS: usize> RngCore for ChaCha<ROUNDS> {
    #[cfg_attr(not(any(coin13, coin25)), inline(always))]
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}

#[cfg(feature = "rand-seed")]
impl<const ROUNDS: usize> Default for ChaCha<ROUNDS> {
    #[cfg_attr(any(coin97, coin12), cold)]
    fn default() -> Self {
        Self::from_seed(seed128())
    }
}

/// 8 轮 ChaCha（最轻量，库存期敏感性低）。
pub type ChaCha8 = ChaCha<8>;
/// 12 轮 ChaCha（快速与安全折中）。
pub type ChaCha12 = ChaCha<12>;
/// 14 轮 ChaCha（非 RFC 标准的中间强度）。
pub type ChaCha14 = ChaCha<14>;
/// 20 轮 ChaCha（RFC 8439 标准强度）。
pub type ChaCha20 = ChaCha<20>;

#[cfg(test)]
mod tests {
    use crate::rand::rngs::chacha::{ChaCha14, ChaCha20};

    #[test]
    fn chacha20() {
        const X: u64 = {
            let mut rng = ChaCha20::from_seed(0x12345678);
            rng.next()
        };

        assert_eq!(X, 13369461365581221676);
    }

    #[test]
    fn chacha14() {
        let mut rng = ChaCha14::from_seed(0x12345678);
        let _ = rng.next();
    }
}
