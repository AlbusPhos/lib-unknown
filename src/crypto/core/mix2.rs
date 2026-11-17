// SPDX-License-Identifier: MPL-2.0

use super::inv_mul::{inv_mul8, inv_mul16, inv_mul32, inv_mul64, inv_mul128};

macro_rules! impl_mix2 {
    ($(
        $(#[$meta:meta])*
        $fn_name:ident, $ty:ty => ($h:expr, $s1:expr, $s2:expr)
    );* $(;)?) => {
        $(
            $(#[$meta])*
            #[cfg_attr(all(coin52,coin104), cold)]
            #[cfg_attr(any(coin23,coin86), inline(never))]
            #[cfg_attr(not(any(coin23,coin86)), inline(always))]
            pub const fn $fn_name(mut x: $ty) -> $ty {
                let [h, s1, s2, c5] = [$h, $s1, $s2, 5];
                let five = c5 as $ty;

                x ^= x >> h;
                x = !(x.wrapping_add(x << 1));
                x = x.wrapping_add(x << h);

                x = x.wrapping_add(x.wrapping_mul(x) | five);
                x = x.rotate_left(s1);
                x ^= x >> h;

                x = x.wrapping_add(x.wrapping_mul(x) | five);
                x = x.rotate_left(s2);
                x ^= x >> h;
                x = x.wrapping_add(x << 1);
                x ^= x >> 1;

                x
            }
        )*
    };
}

impl_mix2! {
    mix8_2,   u8   => (4,  3, 5);
    mix16_2,  u16  => (8,  5, 11);
    mix32_2,  u32  => (16, 11, 21);
    mix64_2,  u64  => (32, 21, 43);
    mix128_2, u128 => (64, 43, 85);
}

macro_rules! impl_inv_mix2 {
    ($(
        $(#[$meta:meta])*
        $fn_name:ident, $ty:ty, $inv_mul:ident => ($h:expr, $s1:expr, $s2:expr)
    );* $(;)?) => {
        $(
            $(#[$meta])*
            #[cfg_attr(all(coin2,coin94), cold)]
            #[cfg_attr(any(coin27,coin0), inline(never))]
            #[cfg_attr(not(any(coin27,coin0)), inline(always))]
            pub const fn $fn_name(mut x: $ty) -> $ty {
                let [h, s1, s2, c5] = [$h, $s1, $s2, 5];
                let five = c5 as $ty;
                let inv3 = $inv_mul(3);

                #[inline(always)]
                const fn inv_t(y: $ty, five: $ty) -> $ty {
                    let mut x = y.wrapping_sub(five);
                    x = y.wrapping_sub(x.wrapping_mul(x) | five);

                    let mut bits = 4u32;
                    while bits < <$ty>::BITS {
                        let fx = x.wrapping_add(x.wrapping_mul(x) | five);
                        let df = x.wrapping_shl(1).wrapping_add(1);
                        let df_inv = $inv_mul(df);
                        x = x.wrapping_add(y.wrapping_sub(fx).wrapping_mul(df_inv));
                        bits *= 2;
                    }
                    x
                }

                let mut shift = 1u32;
                while shift < <$ty>::BITS {
                    x ^= x >> shift;
                    shift <<= 1;
                }
                x = x.wrapping_mul(inv3);
                x ^= x >> h;
                x = x.rotate_right(s2);
                x = inv_t(x, five);

                x ^= x >> h;
                x = x.rotate_right(s1);
                x = inv_t(x, five);

                x = x.wrapping_sub(x << h);
                x = (!x).wrapping_mul(inv3);
                x ^= x >> h;

                x
            }
        )*
    };
}

impl_inv_mix2! {
    inv_mix8_2,   u8,   inv_mul8   => (4,  3, 5);
    inv_mix16_2,  u16,  inv_mul16  => (8,  5, 11);
    inv_mix32_2,  u32,  inv_mul32  => (16, 11, 21);
    inv_mix64_2,  u64,  inv_mul64  => (32, 21, 43);
    inv_mix128_2, u128, inv_mul128 => (64, 43, 85);
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::ops::RangeInclusive;

    struct MixSpec {
        name: &'static str,
        bits: usize,
        mask: u128,
        fwd: fn(u128) -> u128,
        inv: fn(u128) -> u128,
        ranges: &'static [RangeInclusive<u128>],
    }

    macro_rules! mix_spec {
        ($name:literal, $bits:expr, $ty:ty, $fwd:ident, $inv:ident, full) => {
            MixSpec {
                name: $name,
                bits: $bits,
                mask: (<$ty>::MAX as u128),
                fwd: |x| $fwd(x as $ty) as u128,
                inv: |x| $inv(x as $ty) as u128,
                ranges: &[0..=(<$ty>::MAX as u128)],
            }
        };
        ($name:literal, $bits:expr, $ty:ty, $fwd:ident, $inv:ident, boundary) => {{
            const MAX: u128 = <$ty>::MAX as u128;
            const MID: u128 = 1u128 << ($bits - 1);
            const SPAN: u128 = 0x7FFF;
            MixSpec {
                name: $name,
                bits: $bits,
                mask: MAX,
                fwd: |x| $fwd(x as $ty) as u128,
                inv: |x| $inv(x as $ty) as u128,
                ranges: &[
                    0..=(SPAN * 2 + 1),
                    (MID - SPAN)..=(MID + SPAN),
                    (MAX - SPAN * 2 - 1)..=MAX,
                ],
            }
        }};
    }

    static ALL: &[MixSpec] = &[
        mix_spec!("mix8_2", 8, u8, mix8_2, inv_mix8_2, full),
        mix_spec!("mix16_2", 16, u16, mix16_2, inv_mix16_2, full),
        mix_spec!("mix32_2", 32, u32, mix32_2, inv_mix32_2, boundary),
        mix_spec!("mix64_2", 64, u64, mix64_2, inv_mix64_2, boundary),
        mix_spec!("mix128_2", 128, u128, mix128_2, inv_mix128_2, boundary),
    ];

    /// 根据位宽返回单次比特翻转允许的最低输出翻转位数
    #[inline]
    const fn min_avalanche_ones(bits: usize) -> u32 {
        match bits {
            8 => 1,    // 双射保证必然 >= 1
            16 => 1,   // B(16, 0.5) 下 P(X=1) = 2.4e-4
            32 => 3,   // B(32, 0.5) 下 P(X<=2) = 1.2e-7
            64 => 11,  // B(64, 0.5) 下 P(X<=10) = 2.0e-8
            128 => 32, // B(128, 0.5) 下 P(X<=31) = 3.1e-9
            _ => 1,
        }
    }

    #[inline]
    fn assert_bijection_at(spec: &MixSpec, x: u128) {
        let name = spec.name;
        let bits = spec.bits;
        let w = bits / 4 + 2;

        let y = (spec.fwd)(x);
        let x_back = (spec.inv)(y);
        let inv_x = (spec.inv)(x);
        let x_fwd_back = (spec.fwd)(inv_x);

        assert_eq!(
            y & !spec.mask,
            0,
            "[{name} ({bits}-bit)] 正向输出存在高位脏数据: x={x:#0w$x}, fwd(x)={y:#0w$x}"
        );
        assert_eq!(
            inv_x & !spec.mask,
            0,
            "[{name} ({bits}-bit)] 逆向输出存在高位脏数据: x={x:#0w$x}, inv(x)={inv_x:#0w$x}"
        );
        assert_eq!(
            x_back, x,
            "[{name} ({bits}-bit)] 左逆双射失败 inv(fwd(x)) != x:\n  输入 x        = {x:#0w$x}\n  正向 fwd(x)   = {y:#0w$x}\n  还原 inv(fwd) = {x_back:#0w$x}"
        );
        assert_eq!(
            x_fwd_back, x,
            "[{name} ({bits}-bit)] 右逆双射失败 fwd(inv(x)) != x:\n  输入 x        = {x:#0w$x}\n  逆向 inv(x)   = {inv_x:#0w$x}\n  还原 fwd(inv) = {x_fwd_back:#0w$x}"
        );

        // 短周期环与对合缺陷检测 (排查 fwd^k(x) == x, 2 <= k <= 8)
        if bits >= 32 {
            let mut cur = y;
            for step in 2..=8 {
                cur = (spec.fwd)(cur);
                assert_ne!(
                    cur, x,
                    "[{name} ({bits}-bit)] 触发 {step} 阶短周期退化环 (fwd^{step}(x) == x): x={x:#0w$x}"
                );
            }
        }
    }

    #[cfg(all(feature = "rand-core", feature = "rand-cwg"))]
    fn check_avalanche(spec: &MixSpec, func: fn(u128) -> u128, dir: &str) {
        use crate::rand::core::Rng;
        use crate::rand::rngs::cwg::Cwg128;

        let bits = spec.bits;
        let name = spec.name;
        let w = bits / 4 + 2;
        let samples: u32 = if bits == 8 { 256 } else { 4096 };
        let min_ones = min_avalanche_ones(bits);

        let allowed_sac_flips = if bits == 8 {
            88..=168 // 期望 128 (34.4% ~ 65.6%, ±5.0σ)
        } else {
            1843..=2253 // 期望 2048 (45.0% ~ 55.0%, ±6.4σ)
        };

        let mut rng = Cwg128::from_seed(0x1234_5678_9abc_def0);
        let mut flip_counts = [[0u32; 128]; 128];
        let mut in_bit_hamming = [0u64; 128];
        let mut out_bit_hamming = [0u64; 128];
        let mut total_hamming = 0u64;
        let mut total_hamming_sq = 0u64;

        for s in 0..samples {
            let x = if bits == 8 {
                s as u128
            } else {
                let r = rng.random_range(0..=spec.mask);
                if dir == "fwd" {
                    assert_bijection_at(spec, r);
                }
                r
            };
            let y = func(x);

            for in_bit in 0..bits {
                let x_flipped = x ^ (1u128 << in_bit);
                let y_flipped = func(x_flipped);
                let diff = y ^ y_flipped;

                let ones = diff.count_ones();
                assert!(
                    ones >= min_ones,
                    "[{name}::{dir} ({bits}-bit)] 单次翻转雪崩不足! in_bit={in_bit}, x={x:#0w$x}, \
                    diff={diff:#0w$x}, 仅翻转 {ones}/{bits} 位 (最低要求 {min_ones})"
                );

                let ones_u64 = ones as u64;
                total_hamming += ones_u64;
                total_hamming_sq += ones_u64 * ones_u64;
                in_bit_hamming[in_bit] += ones_u64;

                let mut d = diff;
                while d != 0 {
                    let out_bit = d.trailing_zeros() as usize;
                    flip_counts[in_bit][out_bit] += 1;
                    out_bit_hamming[out_bit] += 1;
                    d &= d - 1;
                }
            }
        }

        for (in_bit, row) in flip_counts.iter_mut().enumerate().take(bits) {
            for (out_bit, count) in row.iter_mut().enumerate().take(bits) {
                let count = *count;
                let rate = (count as f64) / (samples as f64) * 100.0;
                assert!(
                    allowed_sac_flips.contains(&count),
                    "[{name}::{dir} ({bits}-bit)] SAC 矩阵单点异常 (in_bit={in_bit}, out_bit={out_bit}): \
                    翻转 {count}/{samples} 次 ({rate:.2}%), 允许区间 {:?} (期望 50.00%)",
                    allowed_sac_flips
                );
            }
        }

        let bit_trials = (samples as u64) * (bits as u64);
        let expected_bit_sum = bit_trials / 2;
        let max_bit_dev = if bits == 8 {
            bit_trials / 20
        } else {
            bit_trials / 40
        };

        for b in 0..bits {
            let in_sum = in_bit_hamming[b];
            let in_rate = (in_sum as f64) / (bit_trials as f64) * 100.0;
            assert!(
                in_sum.abs_diff(expected_bit_sum) <= max_bit_dev,
                "[{name}::{dir} ({bits}-bit)] 输入位扩散均值偏离 (in_bit={b}): \
                实际翻转 {in_sum}/{bit_trials} ({in_rate:.2}%), 期望 {expected_bit_sum} (50.00%), 最大允许偏差 ±{max_bit_dev}"
            );

            let out_sum = out_bit_hamming[b];
            let out_rate = (out_sum as f64) / (bit_trials as f64) * 100.0;
            assert!(
                out_sum.abs_diff(expected_bit_sum) <= max_bit_dev,
                "[{name}::{dir} ({bits}-bit)] 输出位响应均值偏离 (out_bit={b}): \
                实际翻转 {out_sum}/{bit_trials} ({out_rate:.2}%), 期望 {expected_bit_sum} (50.00%), 最大允许偏差 ±{max_bit_dev}"
            );
        }

        let total_trials = (samples as u64) * (bits as u64) * (bits as u64);
        let expected_total = total_trials / 2;
        let max_global_dev = total_trials / 100; // 1% 容差 (>8σ)
        let global_rate = (total_hamming as f64) / (total_trials as f64) * 100.0;
        let n_flips = (samples as f64) * (bits as f64);
        let avg_bits_flipped = (total_hamming as f64) / n_flips;

        assert!(
            total_hamming.abs_diff(expected_total) <= max_global_dev,
            "[{name}::{dir} ({bits}-bit)] 全局雪崩均值偏离:\n  \
            平均每次翻转比特数: {avg_bits_flipped:.3} / {bits} (期望 {:.1})\n  \
            总翻转占比: {total_hamming}/{total_trials} ({global_rate:.3}%, 期望 50.000% ± 1.000%)",
            bits as f64 / 2.0
        );

        let variance = (total_hamming_sq as f64) / n_flips - (avg_bits_flipped * avg_bits_flipped);
        let expected_var = (bits as f64) * 0.25;
        let var_tol = if bits == 8 {
            expected_var * 0.20
        } else {
            expected_var * 0.08
        };

        assert!(
            (variance - expected_var).abs() <= var_tol,
            "[{name}::{dir} ({bits}-bit)] 汉明重量分布方差异常 (偏离标准二项分布 B({bits}, 0.5)):\n  \
            实测方差: {variance:.3}, 理论方差: {expected_var:.3} (允许偏差 ±{var_tol:.3})"
        );
    }

    #[test]
    fn test_bijection() {
        for spec in ALL {
            for range in spec.ranges {
                for x in range.clone() {
                    assert_bijection_at(spec, x);
                }
            }
        }
    }

    #[test]
    #[cfg(all(feature = "rand-core", feature = "rand-cwg"))]
    fn test_avalanche() {
        for spec in ALL {
            check_avalanche(spec, spec.fwd, "fwd");
            check_avalanche(spec, spec.inv, "inv");
        }
    }
}
