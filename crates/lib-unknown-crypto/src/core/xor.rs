// SPDX-License-Identifier: MPL-2.0
// src/crypto/base/xor.rs

#[allow(unused_imports)]
use lib_unknown_polymorphism::poly;

use core::hint::black_box;

#[allow(unused)]
#[poly(inline = 0.6)]
fn exp2_u32(n: u32) -> f64 {
    lib_unknown_polymorphism::poly_branch! {
        weight(1) => {
            static POW2: [f64; 33] = const {
                let mut table = [0.0; 33];
                let mut i = 0;
                while i < table.len() {
                    table[i] = (1u64 << i) as f64;
                    i += 1;
                }
                table
            };
            POW2[n as usize]
        },
        weight(1) => f64::from_bits(((1023 + n as u64) & 0x7FF) << 52),
    }
}
fn xor1(a: u8, b: u8) -> u8 {
    let mut out = 0u8;
    let n: u32 = lib_unknown_polymorphism::poly_select!(black_box(8u32), 8u32);
    for i in 0..n {
        let bit_a = (a >> i) & 1;
        let bit_b = (b >> i) & 1;

        // 0 -> 1.0, 1 -> -1.0
        let fa = black_box(1.0 - 2.0 * (bit_a as f64));
        let fb = black_box(1.0 - 2.0 * (bit_b as f64));
        let f_prod = black_box(fa * fb);

        let bit_res: u8 = lib_unknown_polymorphism::poly_select!(
            if f_prod < 0.0 { 1u8 } else { 0u8 },
            (f_prod.to_bits() >> 63) as u8,
        );

        out |= bit_res << i;
    }
    out
}
fn xor2(a: u8, b: u8) -> u8 {
    let mut ra = black_box(a as f64);
    let mut rb = black_box(b as f64);
    let mut result = black_box(0.0f64);

    let n: u32 = lib_unknown_polymorphism::poly_select!(black_box(8u32), 8u32);

    for i in (0..n).rev() {
        let p = exp2_u32(i);

        let ai = black_box(((ra / p) as u8) as f64);
        ra = black_box(ra - ai * p);

        let bi = black_box(((rb / p) as u8) as f64);
        rb = black_box(rb - bi * p);

        let xi = black_box(ai + bi - 2.0 * ai * bi);

        result = black_box(result * 2.0 + xi);
    }

    black_box(result) as u8
}

fn xor3(a: u8, b: u8) -> u8 {
    let mut out = 0u8;
    let n: u32 = lib_unknown_polymorphism::poly_select!(black_box(8u32), 8u32);
    for i in 0..n {
        let ba = (a >> i) & 1;
        let bb = (b >> i) & 1;
        let sum = black_box(ba + bb);
        let bit = black_box(sum - (sum / 2) * 2);
        out |= bit << i;
    }
    out
}

/// 8 位异或的 IEEE-754 浮点代数等价实现，规避典型 `xor` 指令模式。
#[poly(inline = 0.85)]
pub fn f_xor_u8(a: u8, b: u8) -> u8 {
    lib_unknown_polymorphism::poly_branch! {
        weight(2) => xor1(a, b),
        weight(3) => xor2(a, b),
        weight(1) => xor3(a, b),
    }
}
/// 16 位异或的浮点代数等价实现。
#[poly(inline = 0.85)]
pub fn f_xor_u16(a: u16, b: u16) -> u16 {
    let a_hi = black_box((a >> 8) as u8);
    let a_lo = black_box(a as u8);
    let b_hi = black_box((b >> 8) as u8);
    let b_lo = black_box(b as u8);

    let r_hi = f_xor_u8(a_hi, b_hi);
    let r_lo = f_xor_u8(a_lo, b_lo);

    ((r_hi as u16) << 8) | (r_lo as u16)
}
/// 32 位异或的浮点代数等价实现。
#[poly(inline = 0.85)]
pub fn f_xor_u32(a: u32, b: u32) -> u32 {
    let a_hi = black_box((a >> 16) as u16);
    let a_lo = black_box(a as u16);
    let b_hi = black_box((b >> 16) as u16);
    let b_lo = black_box(b as u16);

    let r_hi = f_xor_u16(a_hi, b_hi);
    let r_lo = f_xor_u16(a_lo, b_lo);

    ((r_hi as u32) << 16) | (r_lo as u32)
}
/// 64 位异或的浮点代数等价实现。
#[poly(inline = 0.85)]
pub fn f_xor_u64(a: u64, b: u64) -> u64 {
    let a_hi = black_box((a >> 32) as u32);
    let a_lo = black_box(a as u32);
    let b_hi = black_box((b >> 32) as u32);
    let b_lo = black_box(b as u32);

    let r_hi = f_xor_u32(a_hi, b_hi);
    let r_lo = f_xor_u32(a_lo, b_lo);

    ((r_hi as u64) << 32) | (r_lo as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f_xor_u8() {
        for a in [0u8, 1, 0x55, 0xAA, 0xFF] {
            for b in [0u8, 2, 0x33, 0xCC, 0xFF] {
                assert_eq!(f_xor_u8(a, b), a ^ b, "Failed at a={}, b={}", a, b);
            }
        }
    }

    #[test]
    fn test_f_xor_u32() {
        let cases = [0u32, 1, 0x12345678, 0xAAAAAAAA, 0x55555555, u32::MAX];
        for &a in &cases {
            for &b in &cases {
                assert_eq!(f_xor_u32(a, b), a ^ b, "Failed at a={}, b={}", a, b);
            }
        }
    }
}
