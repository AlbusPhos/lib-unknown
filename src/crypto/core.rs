// SPDX-License-Identifier: MPL-2.0
// src/crypto/core.rs

use crate::errors::{ErrType, Result, ensure};
/// S/P 置换盒：`DYB` 双向查表结构、静态盒常量与 `gen_p`/`gen_s` 随机盒生成。
pub mod boxs;
/// 乘法混合函数族（编译期多态实现选择）。
pub mod mix;
/// 常数时间填充与去填充（PKCS#7 / ISO 7816-4）。
pub mod padding;
/// 循环左移/右移的替身实现族（基于 IEEE-754 浮点代数，规避典型 `rol`/`ror` 指令模式）。
pub mod rotate;
/// 伪随机置换函数族（`rp64` / `rp128` / `rpp128` 等）。
pub mod rp;
/// 异或运算的浮点代数替身实现族。
pub mod xor;

/// 以 `s` 为种子做无偏洗牌（Lemire 无偏取模 + `rp128` 伪随机置换）。
/// 只能由[`unshuffle_with`]以同一种子还原。
pub fn shuffle_with<T, U>(mut table: U, mut s: u128) -> U
where
    U: AsMut<[T]>,
{
    let slice = table.as_mut();
    let len = slice.len();
    if len <= 1 {
        return table;
    }
    s = s.wrapping_add(0x15754638f880658ac9242c12a7b762cc);

    #[cfg(coin13)]
    const ADD: u128 = 0x67348a87a10ad1111b8856013e3d71u128;
    #[cfg(not(coin13))]
    static ADD: u128 = 0x67348a87a10ad1111b8856013e3d71u128;

    let ptr = slice.as_mut_ptr();

    for i in (1..len).rev() {
        let bound = (i + 1) as u64;

        let mut local_s = s.wrapping_add((i as u128).wrapping_mul(core::hint::black_box(ADD)));

        let mut r = (rp::rp128(local_s) as u64) as u128;
        let mut m = r * (bound as u128);
        let mut l = m as u64;

        if l < bound {
            let t = bound.wrapping_neg() % bound;
            while l < t {
                local_s = local_s.wrapping_add(core::hint::black_box(ADD));
                r = (rp::rp128(local_s) as u64) as u128;
                m = r * (bound as u128);
                l = m as u64;
            }
        }

        let j = (m >> 64) as usize;
        // SAFETY: `ptr` 指向长度为 `len` 的切片底层内存；`i < len`，且
        // `j = (r × bound) >> 64 < bound = i + 1 ≤ len`（Lemire 无偏取模范围），
        // 两个交换地址均不会越界。
        unsafe {
            core::ptr::swap(ptr.add(i), ptr.add(j));
        }
    }
    table
}

/// 对应的逆函数：循环体代码与正向 100% 相同，仅仅把 (1..len).rev() 改成 1..len
pub fn unshuffle_with<T, U>(mut table: U, mut s: u128) -> U
where
    U: AsMut<[T]>,
{
    let slice = table.as_mut();
    let len = slice.len();
    if len <= 1 {
        return table;
    }
    s = s.wrapping_add(0x15754638f880658ac9242c12a7b762cc);

    #[cfg(coin11)]
    const ADD: u128 = 0x67348a87a10ad1111b8856013e3d71u128;
    #[cfg(not(coin11))]
    static ADD: u128 = 0x67348a87a10ad1111b8856013e3d71u128;

    let ptr = slice.as_mut_ptr();

    for i in 1..len {
        let bound = (i + 1) as u64;

        let mut local_s = s.wrapping_add((i as u128).wrapping_mul(core::hint::black_box(ADD)));

        let mut r = (rp::rp128(local_s) as u64) as u128;
        let mut m = r * (bound as u128);
        let mut l = m as u64;

        if l < bound {
            let t = bound.wrapping_neg() % bound;
            while l < t {
                local_s = local_s.wrapping_add(core::hint::black_box(ADD));
                r = (rp::rp128(local_s) as u64) as u128;
                m = r * (bound as u128);
                l = m as u64;
            }
        }

        let j = (m >> 64) as usize;
        // SAFETY: `ptr` 指向长度为 `len` 的切片底层内存；`i < len`，且
        // `j = (r × bound) >> 64 < bound = i + 1 ≤ len`（Lemire 无偏取模范围），
        // 两个交换地址均不会越界。
        unsafe {
            core::ptr::swap(ptr.add(i), ptr.add(j));
        }
    }
    table
}

/// 计算 8 位模乘逆元（`m` 为奇数；零/偶数输入结果未定义，由调用方保证）。
pub fn inv_mul8(m: u8) -> u8 {
    let mut inv = m;
    inv = inv.wrapping_mul(2u8.wrapping_sub(m.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u8.wrapping_sub(m.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u8.wrapping_sub(m.wrapping_mul(inv)));
    inv
}

/// 长度为 `N` 字节的密钥构造 trait：让整数、数组、切片等源类型能统一进入密码原语。
pub trait Key<const N: usize>: Sized {
    /// 将输入转换为长度为 `N` 字节的密钥。
    ///
    /// - `lenient = false`（严格模式）：要求源类型的字节宽度必须严格等于 `N`，否则返回 [`ErrType::InvalidKeyLength`]。
    /// - `lenient = true`（宽松模式）：允许字节宽度不匹配；不足 `N` 字节时在高位补 `0`，超出 `N` 字节时截断。
    ///
    /// # Errors
    ///
    /// - 当 `lenient` 为 `false` 且源字节宽度不等于 `N` 时返回
    ///   [`ErrType::InvalidKeyLength`]。
    fn into_key(self, lenient: bool) -> Result<[u8; N]>;
}

#[inline]
fn bytes_to_key<const N: usize>(src: &[u8], lenient: bool) -> Result<[u8; N]> {
    if !lenient {
        ensure!(
            src.len() == N,
            ErrType::InvalidKeyLength {
                expected: N,
                actual: src.len(),
            }
        );
    }

    let mut key = [0u8; N];
    let len = N.min(src.len());
    key[..len].copy_from_slice(&src[..len]);
    Ok(key)
}

impl<const N: usize, const M: usize> Key<N> for [u8; M] {
    #[inline]
    fn into_key(self, lenient: bool) -> Result<[u8; N]> {
        bytes_to_key(&self, lenient)
    }
}
impl<const N: usize, const M: usize> Key<N> for &[u8; M] {
    #[inline]
    fn into_key(self, lenient: bool) -> Result<[u8; N]> {
        bytes_to_key(self.as_slice(), lenient)
    }
}
impl<const N: usize> Key<N> for &[u8] {
    #[inline]
    fn into_key(self, lenient: bool) -> Result<[u8; N]> {
        bytes_to_key(self, lenient)
    }
}
macro_rules! impl_key_int {
    ($($t:ty),*) => {
        $(
            impl<const N: usize> Key<N> for $t {
                #[inline]
                fn into_key(self, lenient: bool) -> Result<[u8; N]> {
                    bytes_to_key(&self.to_le_bytes(), lenient)
                }
            }
        )*
    };
}

impl_key_int!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_unshuffle_with() {
        let t1 = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48,
        ];

        let mut t2 = t1;

        for i in 0..100 {
            t2 = shuffle_with(t2, i);
            assert_ne!(t1, t2);
            t2 = unshuffle_with(t2, i);
            assert_eq!(t1, t2);
        }
    }
}
