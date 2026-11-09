// SPDX-License-Identifier: MPL-2.0
// src/crypto/core/boxs.rs

use crate::crypto::core::shuffle_with;
use crate::errors::{ErrType, Result, bail};

// 注意S1，S2不能够跨编译轮次使用

#[cfg_attr(
    all(coin86, any(windows, target_os = "linux")),
    unsafe(link_section = ".data")
)]
/// 内置初始字节级 S 盒（由 `gen_s` 生成的基准形态）。
pub static S1: [u8; 256] = [
    248, 232, 208, 109, 8, 26, 79, 75, 118, 93, 59, 184, 171, 116, 70, 111, 67, 153, 169, 105, 0,
    46, 52, 197, 203, 220, 117, 227, 142, 72, 87, 106, 82, 98, 17, 223, 102, 200, 71, 170, 143,
    230, 206, 40, 14, 145, 39, 58, 193, 166, 4, 136, 89, 236, 41, 107, 182, 240, 228, 175, 37, 159,
    163, 241, 134, 64, 7, 25, 125, 217, 219, 12, 121, 42, 61, 140, 243, 29, 77, 28, 120, 32, 95,
    242, 36, 51, 110, 73, 137, 6, 92, 112, 155, 212, 150, 47, 152, 3, 149, 10, 13, 191, 189, 22,
    133, 50, 101, 147, 9, 91, 157, 33, 160, 16, 151, 88, 65, 113, 1, 74, 210, 135, 146, 207, 195,
    201, 178, 218, 154, 238, 86, 249, 34, 127, 215, 108, 2, 129, 180, 104, 138, 179, 181, 31, 128,
    66, 85, 114, 130, 216, 19, 100, 205, 15, 78, 122, 234, 167, 235, 148, 251, 202, 119, 24, 55,
    62, 174, 115, 231, 246, 83, 158, 48, 250, 38, 177, 209, 196, 68, 244, 245, 247, 124, 5, 45,
    103, 199, 204, 97, 168, 123, 172, 27, 139, 226, 233, 164, 165, 211, 213, 188, 49, 54, 176, 194,
    183, 132, 23, 192, 81, 141, 162, 69, 131, 84, 11, 80, 63, 198, 237, 187, 44, 57, 190, 53, 126,
    18, 99, 161, 76, 224, 221, 254, 156, 96, 20, 60, 222, 21, 56, 173, 252, 144, 30, 43, 186, 255,
    225, 35, 94, 214, 185, 229, 90, 253, 239,
];
#[cfg_attr(
    all(coin16, any(windows, target_os = "linux")),
    unsafe(link_section = ".data.1")
)]
/// 内置第二张初始字节级 S 盒，与 `S1` 一起供构建期硬币条件选择。
pub static S2: [u8; 256] = [
    20, 118, 136, 97, 50, 183, 89, 66, 4, 108, 99, 215, 71, 100, 44, 153, 113, 34, 226, 150, 235,
    238, 103, 207, 163, 67, 5, 192, 79, 77, 243, 143, 81, 111, 132, 248, 84, 60, 174, 46, 43, 54,
    73, 244, 221, 184, 21, 95, 172, 201, 105, 85, 22, 224, 202, 164, 239, 222, 47, 10, 236, 74,
    165, 217, 65, 116, 145, 16, 178, 212, 14, 38, 29, 87, 119, 7, 229, 78, 154, 6, 216, 209, 32,
    170, 214, 146, 130, 30, 115, 52, 253, 109, 90, 9, 249, 82, 234, 188, 33, 227, 151, 106, 36,
    185, 139, 19, 31, 55, 135, 3, 86, 15, 91, 117, 147, 167, 13, 26, 8, 162, 80, 72, 155, 190, 182,
    68, 225, 133, 144, 137, 148, 213, 206, 104, 64, 121, 51, 88, 140, 193, 75, 210, 28, 40, 242,
    45, 122, 107, 159, 98, 94, 114, 96, 17, 128, 92, 233, 110, 171, 61, 112, 228, 211, 62, 196,
    197, 49, 157, 189, 18, 39, 12, 191, 240, 166, 59, 203, 175, 126, 141, 138, 142, 56, 205, 11,
    251, 245, 220, 200, 102, 223, 101, 208, 48, 204, 124, 177, 23, 218, 186, 37, 125, 161, 24, 187,
    152, 42, 123, 2, 176, 120, 198, 93, 199, 250, 134, 149, 69, 127, 70, 25, 231, 237, 35, 230,
    247, 194, 27, 58, 252, 41, 168, 1, 195, 156, 158, 53, 219, 129, 255, 57, 63, 83, 76, 179, 180,
    169, 181, 0, 131, 173, 160, 241, 254, 232, 246,
];

#[cfg_attr(
    all(coin6, any(windows, target_os = "linux")),
    unsafe(link_section = ".data.2")
)]
/// 内置初始 8 位位置置换表。
pub static P1: [u8; 8] = [2, 5, 6, 3, 1, 7, 0, 4];
#[cfg_attr(
    all(coin36, any(windows, target_os = "linux")),
    unsafe(link_section = ".data")
)]
/// 内置第二张 8 位位置置换表。
pub static P2: [u8; 8] = [6, 4, 0, 3, 7, 1, 2, 5];

/// 双向查表置换结构：同时保存正向表 `fwd`（输入→输出）与逆向表 `inv`（输出→输入）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DYB<const N: usize> {
    fwd: [u8; N], // 正向置换表: fwd[input] = output
    inv: [u8; N], // 逆向置换表: inv[output] = input
}

impl<const N: usize> DYB<N> {
    /// 校验并构造双表置换结构（自动生成反向表）。
    ///
    /// `fwd` 必须是 `0..N` 的合法排列：每个元素都小于 `N`，且 `0..N` 中的每个值
    /// 恰好出现一次。
    ///
    /// # Errors
    ///
    /// - 当 `N` 既不是 8 也不是 256 时返回 [`ErrType::InvalidArg`]；
    /// - 当 `fwd` 含越界值或重复值时返回 [`ErrType::InvalidArg`]。
    #[cfg_attr(all(coin49, coin11), cold)]
    #[cfg_attr(any(coin74, coin122), inline(never))]
    #[cfg_attr(not(any(coin74, coin122)), inline(always))]
    pub fn try_new(fwd: [u8; N]) -> Result<Self> {
        if N != 8 && N != 256 {
            bail!(
                ErrType::InvalidArg,
                "DYB only supports N = 8 or 256, got {}",
                N
            );
        }

        let mut seen = [false; N];
        for &v in fwd.iter() {
            let idx = v as usize;
            if idx >= N || seen[idx] {
                bail!(
                    ErrType::InvalidArg,
                    "DYB fwd must be a permutation of 0..{}",
                    N
                );
            }
            seen[idx] = true;
        }

        let mut inv = [0u8; N];
        for (i, &v) in fwd.iter().enumerate() {
            inv[v as usize] = i as u8;
        }
        Ok(Self { fwd, inv })
    }

    /// 由正向置换表构造双表置换结构（自动生成反向表）。
    ///
    /// 这是 [`DYB::try_new`] 的 panic 版本，适用于表由常量或受信生成器提供的场景。
    ///
    /// # Panics
    ///
    /// - 当 `N` 既不是 8 也不是 256 时 panic（仅支持 8 位与字节级两种表宽）；
    /// - 当 `fwd` 不是 `0..N` 的合法排列（越界或重复）时 panic。
    #[cfg_attr(all(coin49, coin11), cold)]
    #[cfg_attr(any(coin74, coin122), inline(never))]
    #[cfg_attr(not(any(coin74, coin122)), inline(always))]
    pub fn new(fwd: [u8; N]) -> Self {
        match Self::try_new(fwd) {
            Ok(dyb) => dyb,
            Err(_) => panic!("invalid DYB permutation table"),
        }
    }

    /// 交换两个索引处的映射，并同步维护逆向表。
    ///
    /// # Panics
    ///
    /// - 当 `i >= N` 或 `j >= N` 时 panic。
    #[cfg_attr(all(coin64, coin108), cold)]
    #[cfg_attr(all(coin89, coin104), inline(never))]
    #[cfg_attr(not(all(coin89, coin104)), inline(always))]
    pub fn swap_by_idx(&mut self, i: usize, j: usize) {
        if i == j {
            return;
        }
        let v1 = self.fwd[i] as usize;
        let v2 = self.fwd[j] as usize;
        self.fwd.swap(i, j);
        self.inv.swap(v1, v2);
    }

    /// 交换两个"值"的输入位置（经逆向表定位），并同步维护两张表。
    ///
    /// # Panics
    ///
    /// - 当 `val1 >= N` 或 `val2 >= N` 时 panic。
    #[cfg_attr(coin81, cold)]
    #[cfg_attr(any(coin127, coin50), inline(never))]
    #[cfg_attr(not(any(coin127, coin50)), inline(always))]
    pub fn swap_by_val(&mut self, val1: u8, val2: u8) {
        if val1 == val2 {
            return;
        }
        let p1 = self.inv[val1 as usize] as usize;
        let p2 = self.inv[val2 as usize] as usize;
        self.fwd.swap(p1, p2);
        self.inv.swap(val1 as usize, val2 as usize);
    }
}

impl DYB<8> {
    /// 应用 8 位位置置换：`x` 的第 `fwd[dst]` 位送到第 `dst` 位。
    #[cfg_attr(all(coin91, coin62), cold)]
    #[cfg_attr(coin3, inline(never))]
    #[cfg_attr(not(coin3), inline(always))]
    pub fn apply_p8(&self, x: u8) -> u8 {
        let mut out = 0u8;
        for dst in 0..8 {
            let src = self.fwd[dst];
            let bit = (x >> src) & 1;
            out |= bit << dst;
        }
        out
    }

    /// `apply_p8` 的逆置换。
    #[cfg_attr(all(coin5, coin45), cold)]
    #[cfg_attr(all(coin48, coin102), inline(never))]
    #[cfg_attr(not(all(coin48, coin102)), inline(always))]
    pub fn apply_inv_p8(&self, y: u8) -> u8 {
        let mut out = 0u8;
        for dst in 0..8 {
            let src = self.inv[dst];
            let bit = (y >> src) & 1;
            out |= bit << dst;
        }
        out
    }

    /// 应用 16 位位置置换（逐半字节/位重排）。
    #[inline(always)]
    pub fn apply_p16(&self, x: u16) -> u16 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_p8(*b);
        }
        u16::from_le_bytes(bytes)
    }

    /// `apply_p16` 的逆置换。
    #[inline(always)]
    pub fn apply_inv_p16(&self, y: u16) -> u16 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_p8(*b);
        }
        u16::from_le_bytes(bytes)
    }

    /// 应用 32 位位置置换。
    #[inline(always)]
    pub fn apply_p32(&self, x: u32) -> u32 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_p8(*b);
        }
        u32::from_le_bytes(bytes)
    }

    /// `apply_p32` 的逆置换。
    #[inline(always)]
    pub fn apply_inv_p32(&self, y: u32) -> u32 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_p8(*b);
        }
        u32::from_le_bytes(bytes)
    }

    /// 应用 64 位位置置换。
    #[inline(always)]
    pub fn apply_p64(&self, x: u64) -> u64 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_p8(*b);
        }
        u64::from_le_bytes(bytes)
    }

    /// `apply_p64` 的逆置换。
    #[inline(always)]
    pub fn apply_inv_p64(&self, y: u64) -> u64 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_p8(*b);
        }
        u64::from_le_bytes(bytes)
    }

    /// 应用 128 位位置置换。
    #[inline(always)]
    pub fn apply_p128(&self, x: u128) -> u128 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_p8(*b);
        }
        u128::from_le_bytes(bytes)
    }

    /// `apply_p128` 的逆置换。
    #[inline(always)]
    pub fn apply_inv_p128(&self, y: u128) -> u128 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_p8(*b);
        }
        u128::from_le_bytes(bytes)
    }
}

impl DYB<256> {
    /// 应用 8 位 S 盒替换。
    #[cfg_attr(coin25, cold)]
    #[cfg_attr(coin73, inline(never))]
    #[cfg_attr(all(not(coin73), coin77), inline(always))]
    #[cfg_attr(all(not(coin73), not(coin77)), inline)]
    pub fn apply_s8(&self, x: u8) -> u8 {
        self.fwd[x as usize]
    }

    /// `apply_s8` 的逆替换。
    #[cfg_attr(coin14, cold)]
    #[cfg_attr(coin70, inline(never))]
    #[cfg_attr(not(coin70), inline(always))]
    pub fn apply_inv_s8(&self, y: u8) -> u8 {
        self.inv[y as usize]
    }

    /// 应用 S 盒对 16 位值逐字节替换。
    #[inline(always)]
    pub fn apply_s16(&self, x: u16) -> u16 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_s8(*b);
        }
        u16::from_le_bytes(bytes)
    }

    /// `apply_s16` 的逆替换（逐字节）。
    #[inline(always)]
    pub fn apply_inv_s16(&self, y: u16) -> u16 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_s8(*b);
        }
        u16::from_le_bytes(bytes)
    }

    /// 应用 S 盒对 32 位值逐字节替换。
    #[inline(always)]
    pub fn apply_s32(&self, x: u32) -> u32 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_s8(*b);
        }
        u32::from_le_bytes(bytes)
    }

    /// `apply_s32` 的逆替换（逐字节）。
    #[inline(always)]
    pub fn apply_inv_s32(&self, y: u32) -> u32 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_s8(*b);
        }
        u32::from_le_bytes(bytes)
    }

    /// 应用 S 盒对 64 位值逐字节替换。
    #[inline(always)]
    pub fn apply_s64(&self, x: u64) -> u64 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_s8(*b);
        }
        u64::from_le_bytes(bytes)
    }

    /// `apply_s64` 的逆替换（逐字节）。
    #[inline(always)]
    pub fn apply_inv_s64(&self, y: u64) -> u64 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_s8(*b);
        }
        u64::from_le_bytes(bytes)
    }

    /// 应用 S 盒对 128 位值逐字节替换。
    #[inline(always)]
    pub fn apply_s128(&self, x: u128) -> u128 {
        let mut bytes = x.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_s8(*b);
        }
        u128::from_le_bytes(bytes)
    }

    /// `apply_s128` 的逆替换（逐字节）。
    #[inline(always)]
    pub fn apply_inv_s128(&self, y: u128) -> u128 {
        let mut bytes = y.to_le_bytes();
        for b in &mut bytes {
            *b = self.apply_inv_s8(*b);
        }
        u128::from_le_bytes(bytes)
    }
}

/// 用显式 S 盒 `s` 做 8 位置换。
///
/// # Panics
///
/// - 当 `s.len() != 8` 时 panic；
/// - 当 `s` 中存在值 `>= 8`（非法位索引）时 panic。
#[cfg_attr(all(coin113, coin66), cold)]
#[cfg_attr(all(coin31, coin107), inline(never))]
#[cfg_attr(not(all(coin31, coin107)), inline(always))]
pub fn p8(x: u8, s: &[u8]) -> u8 {
    assert_eq!(s.len(), 8);
    let mut out = 0u8;
    for (dst, &src) in s.iter().enumerate().take(8) {
        assert!(src < 8);
        let bit = (x >> src) & 1;
        out |= bit << dst;
    }
    out
}

/// 用显式字节 S 盒 `s` 做 8→8 置换。
///
/// # Panics
///
/// - 当 `s.len() != 256` 时 panic。
#[cfg_attr(coin127, cold)]
#[cfg_attr(all(coin108, coin74), inline(never))]
#[cfg_attr(not(all(coin108, coin74)), inline(always))]
pub fn s8(x: u8, s: &[u8]) -> u8 {
    assert_eq!(s.len(), 256);
    s[x as usize]
}

/// 用显式字节 S 盒 `s` 对 `x` 的 8 个字节逐个做 8→8 置换。
///
/// # Panics
///
/// - 当 `s.len() != 256` 时 panic。
#[cfg_attr(all(coin8, coin7), cold)]
#[cfg_attr(coin51, inline(never))]
#[cfg_attr(all(not(coin51), coin100), inline(always))]
#[cfg_attr(all(not(coin51), not(coin100)), inline)]
pub fn s64(x: u64, s: &[u8]) -> u64 {
    assert_eq!(s.len(), 256);
    let mut bytes = x.to_le_bytes();
    for b in &mut bytes {
        *b = s[*b as usize];
    }
    u64::from_le_bytes(bytes)
}

/// 由给定 8 位位置置换表计算逆表；表非法（重复/越界）时返回 `None`。
#[cfg_attr(coin72, cold)]
#[cfg_attr(coin102, inline(never))]
#[cfg_attr(all(not(coin102), coin34), inline(always))]
#[cfg_attr(all(not(coin102), not(coin34)), inline)]
pub fn inv_p(s: &[u8]) -> Option<[u8; 8]> {
    if s.len() != 8 {
        return None;
    }
    let mut inv = [0u8; 8];
    let mut seen = [false; 8];
    for (dst, &src) in s.iter().enumerate() {
        if src >= 8 || seen[src as usize] {
            return None;
        }
        seen[src as usize] = true;
        inv[src as usize] = dst as u8;
    }
    Some(inv)
}

/// 由给定字节 S 盒计算逆表；表非法（重复/未满）时返回 `None`。
#[cfg_attr(any(coin47, coin93), cold)]
#[cfg_attr(coin76, inline(never))]
pub fn inv_s(s: &[u8]) -> Option<[u8; 256]> {
    if s.len() != 256 {
        return None;
    }
    let mut inv = [0u8; 256];
    let mut seen = [false; 256];
    for (i, &val) in s.iter().enumerate() {
        if seen[val as usize] {
            return None;
        }
        seen[val as usize] = true;
        inv[val as usize] = i as u8;
    }
    Some(inv)
}

/// 由 64 位种子生成一张随机 8 位位置置换表（保证为合法双射）。
#[cfg_attr(coin67, cold)]
#[cfg_attr(all(coin63, coin72), inline(never))]
#[cfg_attr(not(all(coin63, coin72)), inline(always))]
pub fn gen_p(seed: u64) -> [u8; 8] {
    shuffle_with([0, 1, 2, 3, 4, 5, 6, 7], seed as u128)
}

/// 由 64 位种子生成一张随机字节 S 盒（保证为 0..=255 的排列）。
#[cfg_attr(all(coin93, coin31), cold)]
#[cfg_attr(all(coin82, coin24), inline(never))]
#[cfg_attr(not(all(coin82, coin24)), inline(always))]
pub fn gen_s(seed: u64) -> [u8; 256] {
    shuffle_with(core::array::from_fn(|i| i as u8), seed as u128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box() {
        assert_eq!(&S1, &inv_s(&S2).unwrap());
        assert_eq!(&P1, &inv_p(&P2).unwrap());
    }

    #[test]
    fn test_dyb_apply_widths() {
        let p_box = DYB::new(P1);
        let s_box = DYB::new(S1);

        let v16 = 0x1234u16;
        assert_eq!(p_box.apply_inv_p16(p_box.apply_p16(v16)), v16);
        assert_eq!(s_box.apply_inv_s16(s_box.apply_s16(v16)), v16);

        let v32 = 0x1234_5678u32;
        assert_eq!(p_box.apply_inv_p32(p_box.apply_p32(v32)), v32);
        assert_eq!(s_box.apply_inv_s32(s_box.apply_s32(v32)), v32);

        let v64 = 0x1234_5678_9ABC_DEF0u64;
        assert_eq!(p_box.apply_inv_p64(p_box.apply_p64(v64)), v64);
        assert_eq!(s_box.apply_inv_s64(s_box.apply_s64(v64)), v64);
        assert_eq!(s_box.apply_s64(v64), s64(v64, &S1));

        let v128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210u128;
        assert_eq!(p_box.apply_inv_p128(p_box.apply_p128(v128)), v128);
        assert_eq!(s_box.apply_inv_s128(s_box.apply_s128(v128)), v128);
    }

    #[test]
    fn test_dyb_try_new_validation() {
        assert!(DYB::<8>::try_new(P1).is_ok());
        assert!(DYB::<256>::try_new(S1).is_ok());
        assert!(DYB::<256>::try_new(S2).is_ok());

        // 不支持的 N
        assert!(DYB::<4>::try_new([0, 1, 2, 3]).is_err());

        // 越界值
        let err = DYB::<8>::try_new([8, 1, 2, 3, 4, 5, 6, 7]).unwrap_err();
        assert_eq!(err.err_type(), ErrType::InvalidArg);

        // 重复值
        let err = DYB::<8>::try_new([0, 0, 2, 3, 4, 5, 6, 7]).unwrap_err();
        assert_eq!(err.err_type(), ErrType::InvalidArg);

        // 重复值（256 宽）
        let mut bad_s = S1;
        bad_s[0] = bad_s[1];
        let err = DYB::<256>::try_new(bad_s).unwrap_err();
        assert_eq!(err.err_type(), ErrType::InvalidArg);
    }

    #[test]
    #[should_panic(expected = "invalid DYB permutation table")]
    fn test_dyb_new_panics_on_invalid_table() {
        let _ = DYB::<8>::new([8, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_dyb_swap_keeps_bijection() {
        let mut p_box = DYB::new(P1);
        p_box.swap_by_idx(0, 7);
        p_box.swap_by_val(1, 6);

        for x in 0..=255u8 {
            assert_eq!(p_box.apply_inv_p8(p_box.apply_p8(x)), x);
        }

        let mut s_box = DYB::new(S1);
        s_box.swap_by_idx(0, 255);
        s_box.swap_by_val(3, 200);

        for x in 0..=255u8 {
            assert_eq!(s_box.apply_inv_s8(s_box.apply_s8(x)), x);
        }
    }
}
