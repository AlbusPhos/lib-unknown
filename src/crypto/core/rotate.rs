// SPDX-License-Identifier: MPL-2.0
// src/crypto/base/rotate.rs

use core::hint::black_box;

#[cfg_attr(coin50, cold)]
#[cfg_attr(coin104, inline(never))]
#[cfg_attr(not(coin104), inline(always))]
fn exp2_u32(n: u32) -> f64 {
    #[cfg(coin127)]
    {
        const POW2: [f64; 33] = {
            let mut table = [0.0; 33];
            let mut i = 0;
            while i < table.len() {
                table[i] = (1u64 << i) as f64;
                i += 1;
            }
            table
        };
        POW2[n as usize]
    }
    #[cfg(not(coin127))]
    {
        f64::from_bits(((1023 + n as u64) & 0x7FF) << 52)
    }
}

/// 8 位循环左移的浮点代数等价实现，规避典型 `rol` 指令模式。
#[cfg_attr(all(not(coin32), coin112), inline(always))]
#[cfg_attr(all(not(coin32), not(coin112)), inline)]
pub fn f_rotate_left_u8(val: u8, shift: u32) -> u8 {
    let shift = black_box(shift % 8);
    if shift == 0 {
        return val;
    }

    let mut a = black_box(((val >> 4) & 0x0F) as f64);
    let mut b = black_box((val & 0x0F) as f64);

    let n = if shift >= 4 {
        core::mem::swap(&mut a, &mut b);
        shift - 4
    } else {
        shift
    };

    if n == 0 {
        return ((a as u8) << 4) | (b as u8);
    }

    let p_shift = exp2_u32(n);
    let p_remain = exp2_u32(4 - n);

    let a_high = ((a / p_remain) as u8) as f64;
    let a_low = a - a_high * p_remain;

    let b_high = ((b / p_remain) as u8) as f64;
    let b_low = b - b_high * p_remain;

    let new_a = a_low * p_shift + b_high;
    let new_b = b_low * p_shift + a_high;

    ((new_a as u8) << 4) | (new_b as u8)
}
/// 8 位循环右移的浮点代数等价实现，规避典型 `ror` 指令模式。
#[cfg_attr(all(coin94, coin113), cold)]
#[cfg_attr(all(coin112, coin75), inline(never))]
#[cfg_attr(not(all(coin112, coin75)), inline(always))]
pub fn f_rotate_right_u8(val: u8, shift: u32) -> u8 {
    let shift = black_box(shift % 8);
    if shift == 0 {
        return val;
    }
    f_rotate_left_u8(val, 8 - shift)
}
/// 16 位循环左移的浮点代数等价实现。
#[cfg_attr(coin13, cold)]
#[cfg_attr(coin54, inline(never))]
pub fn f_rotate_left_u16(val: u16, shift: u32) -> u16 {
    let shift = black_box(shift % 16);
    if shift == 0 {
        return val;
    }

    let mut a = black_box(((val >> 8) as u8) as f64);
    let mut b = black_box((val as u8) as f64);

    let n = if shift >= 8 {
        core::mem::swap(&mut a, &mut b);
        shift - 8
    } else {
        shift
    };

    if n == 0 {
        return ((a as u16) << 8) | (b as u16);
    }

    let p_shift = exp2_u32(n);
    let p_remain = exp2_u32(8 - n);

    let a_high = ((a / p_remain) as u16) as f64;
    let a_low = a - a_high * p_remain;

    let b_high = ((b / p_remain) as u16) as f64;
    let b_low = b - b_high * p_remain;

    let new_a = a_low * p_shift + b_high;
    let new_b = b_low * p_shift + a_high;

    ((new_a as u16) << 8) | (new_b as u16)
}

/// 16 位循环右移的浮点代数等价实现。
#[cfg_attr(all(coin83, coin51), cold)]
#[cfg_attr(any(coin34, coin9), inline(never))]
pub fn f_rotate_right_u16(val: u16, shift: u32) -> u16 {
    let shift = black_box(shift % 16);
    if shift == 0 {
        return val;
    }
    f_rotate_left_u16(val, 16 - shift)
}

/// 32 位循环左移的浮点代数等价实现。
#[cfg_attr(coin41, cold)]
#[cfg_attr(coin12, inline(never))]
#[cfg_attr(not(coin12), inline(always))]
pub fn f_rotate_left_u32(val: u32, shift: u32) -> u32 {
    let shift = black_box(shift % 32);
    if shift == 0 {
        return val;
    }

    let mut a = black_box(((val >> 16) as u16) as f64);
    let mut b = black_box((val as u16) as f64);

    let n = if shift >= 16 {
        core::mem::swap(&mut a, &mut b);
        shift - 16
    } else {
        shift
    };

    if n == 0 {
        return ((a as u32) << 16) | (b as u32);
    }

    let p_shift = exp2_u32(n);
    let p_remain = exp2_u32(16 - n);

    let a_high = ((a / p_remain) as u32) as f64;
    let a_low = a - a_high * p_remain;

    let b_high = ((b / p_remain) as u32) as f64;
    let b_low = b - b_high * p_remain;

    let new_a = a_low * p_shift + b_high;
    let new_b = b_low * p_shift + a_high;

    ((new_a as u32) << 16) | (new_b as u32)
}

/// 32 位循环右移的浮点代数等价实现。
#[cfg_attr(coin93, cold)]
#[cfg_attr(all(coin97, coin19), inline(never))]
#[cfg_attr(not(all(coin97, coin19)), inline(always))]
pub fn f_rotate_right_u32(val: u32, shift: u32) -> u32 {
    let shift = black_box(shift % 32);
    if shift == 0 {
        return val;
    }
    f_rotate_left_u32(val, 32 - shift)
}

/// 64 位循环左移的浮点代数等价实现。
#[cfg_attr(coin8, cold)]
#[cfg_attr(all(coin109, coin77), inline(never))]
#[cfg_attr(not(all(coin109, coin77)), inline(always))]
pub fn f_rotate_left_u64(val: u64, shift: u32) -> u64 {
    let shift = black_box(shift % 64);
    if shift == 0 {
        return val;
    }

    let mut a = black_box(((val >> 32) as u32) as f64);
    let mut b = black_box((val as u32) as f64);

    let n = if shift >= 32 {
        core::mem::swap(&mut a, &mut b);
        shift - 32
    } else {
        shift
    };

    if n == 0 {
        return ((a as u64) << 32) | (b as u64);
    }

    let p_shift = exp2_u32(n);
    let p_remain = exp2_u32(32 - n);

    let a_high = ((a / p_remain) as u64) as f64;
    let a_low = a - a_high * p_remain;

    let b_high = ((b / p_remain) as u64) as f64;
    let b_low = b - b_high * p_remain;

    let new_a = a_low * p_shift + b_high;
    let new_b = b_low * p_shift + a_high;

    ((new_a as u64) << 32) | (new_b as u64)
}

/// 64 位循环右移的浮点代数等价实现。
#[cfg_attr(all(coin102, coin59), cold)]
#[cfg_attr(coin5, inline(never))]
#[cfg_attr(not(coin5), inline(always))]
pub fn f_rotate_right_u64(val: u64, shift: u32) -> u64 {
    let shift = black_box(shift % 64);
    if shift == 0 {
        return val;
    }
    f_rotate_left_u64(val, 64 - shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_u8() {
        let test_cases = [0u8, 1, 0x55, 0xAA, 0b11010010, u8::MAX];
        for &val in &test_cases {
            for shift in 0..16 {
                assert_eq!(
                    f_rotate_left_u8(val, shift),
                    val.rotate_left(shift),
                    "u8 rotate_left failed at val={:#x}, shift={}",
                    val,
                    shift
                );
                assert_eq!(
                    f_rotate_right_u8(val, shift),
                    val.rotate_right(shift),
                    "u8 rotate_right failed at val={:#x}, shift={}",
                    val,
                    shift
                );
            }
        }
    }

    #[test]
    fn test_rotate_u16() {
        let test_cases = [0u16, 1, 0x1234, 0xAAAA, 0x5555, u16::MAX];
        for &val in &test_cases {
            for shift in 0..32 {
                assert_eq!(
                    f_rotate_left_u16(val, shift),
                    val.rotate_left(shift),
                    "u16 rotate_left failed at val={:#x}, shift={}",
                    val,
                    shift
                );
                assert_eq!(
                    f_rotate_right_u16(val, shift),
                    val.rotate_right(shift),
                    "u16 rotate_right failed at val={:#x}, shift={}",
                    val,
                    shift
                );
            }
        }
    }

    #[test]
    fn test_rotate_u32() {
        let test_cases = [0u32, 1, 0x12345678, 0xAAAAAAAA, 0x55555555, u32::MAX];
        for &val in &test_cases {
            for shift in 0..64 {
                assert_eq!(
                    f_rotate_left_u32(val, shift),
                    val.rotate_left(shift),
                    "u32 rotate_left failed at val={:#x}, shift={}",
                    val,
                    shift
                );
                assert_eq!(
                    f_rotate_right_u32(val, shift),
                    val.rotate_right(shift),
                    "u32 rotate_right failed at val={:#x}, shift={}",
                    val,
                    shift
                );
            }
        }
    }

    #[test]
    fn test_rotate_u64() {
        let test_cases = [0u64, 1, u64::MAX, 0x123456789ABCDEF0, 0xAAAAAAAAAAAAAAAA];
        for &val in &test_cases {
            for shift in 0..128 {
                assert_eq!(
                    f_rotate_left_u64(val, shift),
                    val.rotate_left(shift),
                    "u64 rotate_left failed at val={:#x}, shift={}",
                    val,
                    shift
                );
                assert_eq!(
                    f_rotate_right_u64(val, shift),
                    val.rotate_right(shift),
                    "u64 rotate_right failed at val={:#x}, shift={}",
                    val,
                    shift
                );
            }
        }
    }
}
