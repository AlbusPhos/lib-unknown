// SPDX-License-Identifier: MPL-2.0
// src/crypto/mse.rs
/// 本模块实现的 MSE（及其核心变换 r8/ir8）是一种面向软件保护与载荷混淆设计的CFB模式变体的字节级分组密码。
/// 该算法工作在 8 位数据块粒度，采用密文反馈链式结构保证扩散性。算法在轮函数中引入了基于常数逼近的定点密钥调度、
/// 时变动态 S/P 盒置换，以及模乘逆元混合以及基于数学常数（π、1/e）构造的非线性密钥派生机制。
/// 其底层位运算算子均经过 IEEE-754 浮点代数重构。针对符号执行求解器进行了高强度防御，但不保证侧信道安全。
use crate::crypto::core::boxs::{DYB, gen_p, gen_s};
use crate::crypto::core::rotate::{f_rotate_left_u8, f_rotate_left_u64, f_rotate_right_u8};
use crate::crypto::core::rp::rp64;
use crate::crypto::core::{Key, inv_mul8};
use crate::errors::*;

#[cfg_attr(coin66, cold)]
#[cfg_attr(not(any(coin37, coin93)), inline(never))]
fn derive_r8_keys(k: u128, l: u8, i: usize) -> (u8, u64) {
    let i = i as u64;
    let k1 = f_rotate_left_u64(k as u64, i as u32);
    let k2 = f_rotate_left_u64((k >> 64) as u64, l as u32);
    let k = k1.wrapping_mul(k2);

    let v_i = (k1 & 0x0000_0000_FFFF_FFFF) | 0x0000_0001_0000_0000;
    let v_l = (k2 & 0x0000_0000_FFFF_FFFF) | 0x0000_0002_0000_0000;

    static PI_Q32: u128 = 13_493_037_704; // PI * 2^32
    static INV_E_Q32: u128 = 1_580_030_169; // (1/E) * 2^32

    let num = (v_i as u128) * PI_Q32 + (v_l as u128) * INV_E_Q32;

    let den = (v_l - (v_i >> 1)) as u128;

    let ratio = (num / den) as u64;

    let r = rp64(k ^ !ratio);

    (ratio as u8, r)
}

/// 单字节加密
#[cfg_attr(any(coin87, coin53), cold)]
#[cfg_attr(any(coin47, coin53), inline(never))]
fn r8(mut x: u8, l: u8, i: usize, k: u128, p: Option<&mut DYB<8>>, s: Option<&mut DYB<256>>) -> u8 {
    let k = k.wrapping_add(0x116a3b35fc845a8ecd7f2116a3b35fc8);
    x = x.wrapping_add(0x11);

    let (ki, r) = derive_r8_keys(k, l, i);

    x ^= l ^ !ki ^ r as u8;

    if let Some(p_box) = p {
        x = p_box.apply_p8(x);
        let val1 = (r & 0b111) as u8;
        let val2 = ((r >> 3) & 0b111) as u8;
        p_box.swap_by_val(val1, val2);
    }

    x = x.wrapping_mul(inv_mul8(ki | 1));
    x = x.wrapping_mul(inv_mul8(rp64(r) as u8 | 1));

    if let Some(s_box) = s {
        x = s_box.apply_s8(x);
        let idx1 = ((r >> 8) & 0xFF) as usize;
        let idx2 = ((r >> 16) & 0xFF) as usize;
        s_box.swap_by_idx(idx1, idx2);
    }

    x = f_rotate_left_u8(x, !(ki as u32));

    x
}

/// 单字节解密
#[cfg_attr(coin31, cold)]
#[cfg_attr(any(coin47, coin53), inline(never))]
#[cfg_attr(not(any(coin47, coin53)), inline(always))]
fn ir8(
    mut y: u8,
    l: u8,
    i: usize,
    k: u128,
    p: Option<&mut DYB<8>>,
    s: Option<&mut DYB<256>>,
) -> u8 {
    let k = k.wrapping_add(0x116a3b35fc845a8ecd7f2116a3b35fc8);

    let (ki, r) = derive_r8_keys(k, l, i);

    y = f_rotate_right_u8(y, !(ki as u32));

    if let Some(s_box) = s {
        y = s_box.apply_inv_s8(y);
        let idx1 = ((r >> 8) & 0xFF) as usize;
        let idx2 = ((r >> 16) & 0xFF) as usize;
        s_box.swap_by_idx(idx1, idx2);
    }

    y = y.wrapping_mul(rp64(r) as u8 | 1);
    y = y.wrapping_mul(ki | 1);

    if let Some(p_box) = p {
        y = p_box.apply_inv_p8(y);
        let val1 = (r & 0b111) as u8;
        let val2 = ((r >> 3) & 0b111) as u8;
        p_box.swap_by_val(val1, val2);
    }

    y ^= l ^ !ki ^ r as u8;

    y = y.wrapping_sub(0x11);

    y
}

// =========================================================================
// 外部调用接口与分组流模式
// =========================================================================
/// MSE（无 P/S 盒版本）：按字节对 `data` 做链式混淆（初始 IV 为 `iv`），原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换为 `Key<16>` 失败时返回错误；本实现以宽松模式转换，实际不会触发。
#[cfg_attr(coin11, cold)]
#[cfg_attr(coin93, inline(never))]
#[cfg_attr(not(coin93), inline(always))]
pub fn mse_no_s<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;
    let mut p = DYB::new(gen_p((key ^ key >> 64) as u64));

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = r8(*pt, prev, i, key, Some(&mut p), None);
        prev = ct;
        *pt = ct;
    }

    Ok(())
}

/// [`mse_no_s`] 的逆函数：按字节还原链式混淆，原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin55, cold)]
#[cfg_attr(coin44, inline(never))]
#[cfg_attr(not(coin44), inline(always))]
pub fn imse_no_s<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;
    let mut p = DYB::new(gen_p((key ^ key >> 64) as u64));

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = ir8(*pt, prev, i, key, Some(&mut p), None);
        prev = *pt;
        *pt = ct;
    }

    Ok(())
}

/// MSE（无 P 盒版本）：按字节对 `data` 做链式混淆，原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin22, cold)]
#[cfg_attr(coin126, inline(never))]
#[cfg_attr(not(coin126), inline(always))]
pub fn mse_no_ps<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = r8(*pt, prev, i, key, None, None);
        prev = ct;
        *pt = ct;
    }

    Ok(())
}

/// [`mse_no_ps`] 的逆函数：按字节还原链式混淆，原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin100, cold)]
#[cfg_attr(coin127, inline(never))]
#[cfg_attr(not(coin127), inline(always))]
pub fn imse_no_ps<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = ir8(*pt, prev, i, key, None, None);
        prev = *pt;
        *pt = ct;
    }

    Ok(())
}

/// MSE（完整版本，带动态生成的 P/S 盒）：按字节对 `data` 做链式混淆，原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin60, cold)]
#[cfg_attr(coin37, inline(never))]
#[cfg_attr(not(coin37), inline(always))]
pub fn mse<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;

    let seed_p = (key ^ (key >> 64)) as u64;
    let seed_s = rp64(seed_p);

    let mut p = DYB::new(gen_p(seed_p));
    let mut s = DYB::new(gen_s(seed_s));

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = r8(*pt, prev, i, key, Some(&mut p), Some(&mut s));
        prev = ct;
        *pt = ct;
    }

    Ok(())
}

/// [`mse`] 的逆函数：按字节还原带 P/S 盒的链式混淆，原地写回。
///
/// # Errors
///
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin15, cold)]
#[cfg_attr(coin51, inline(never))]
#[cfg_attr(not(coin51), inline(always))]
pub fn imse<K: Key<16>, D: AsMut<[u8]>>(mut data: D, key: K, iv: u8) -> Result<()> {
    let key = u128::from_le_bytes(key.into_key(true)?);
    let data = data.as_mut();
    let mut prev = iv;

    let seed_p = (key ^ (key >> 64)) as u64;
    let seed_s = rp64(seed_p);

    let mut p = DYB::new(gen_p(seed_p));
    let mut s = DYB::new(gen_s(seed_s));

    for (i, pt) in data.iter_mut().enumerate() {
        let ct = ir8(*pt, prev, i, key, Some(&mut p), Some(&mut s));
        prev = *pt;
        *pt = ct;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_byte_roundtrip() {
        let key = 0xA1B2C3D4E5F60718_2938475610293847_u128;
        let mut p_enc = DYB::new(gen_p(0x1234));
        let mut s_enc = DYB::new(gen_s(0x5678));
        let mut p_dec = DYB::new(gen_p(0x1234));
        let mut s_dec = DYB::new(gen_s(0x5678));

        for x in 0..=255u8 {
            let ct = r8(x, 0x55, 3, key, Some(&mut p_enc), Some(&mut s_enc));
            let pt = ir8(ct, 0x55, 3, key, Some(&mut p_dec), Some(&mut s_dec));
            assert_eq!(x, pt, "单字节加解密必须完全精确可逆");
        }
    }

    #[test]
    fn test_full_stream_roundtrip() {
        let key = 0xDEADBEEF_CAFEBABE_01234567_89ABCDEF_u128;
        let iv = 0xAA;
        let original = [0u8, 1, 2, 3, 4, 42, 127, 128, 254, 255, 100, 200];

        let mut buffer = original;
        mse(&mut buffer, key, iv).unwrap();
        assert_ne!(buffer, original);
        imse(&mut buffer, key, iv).unwrap();
        assert_eq!(buffer, original);

        let mut buffer_no_s = original;
        mse_no_s(&mut buffer_no_s, key, iv).unwrap();
        assert_ne!(buffer_no_s, original);
        imse_no_s(&mut buffer_no_s, key, iv).unwrap();
        assert_eq!(buffer_no_s, original);

        let mut buffer_no_ps = original;
        mse_no_ps(&mut buffer_no_ps, key, iv).unwrap();
        assert_ne!(buffer_no_ps, original);
        imse_no_ps(&mut buffer_no_ps, key, iv).unwrap();
        assert_eq!(buffer_no_ps, original);
    }

    #[test]
    #[cfg(all(feature = "std", feature = "rand"))]
    fn test_bench() {
        use core::hint::black_box;
        const SIZE: usize = 1024 * 111;
        const I: u64 = 30;
        let mb = SIZE as f64 / 1024.0 / 1024.0 * I as f64;
        let mut buf = vec![0u8; SIZE];
        let key = crate::rand::core::random::<u128>();
        crate::rand::core::fill_bytes(buf.as_mut_slice());

        let mut buf2 = buf.clone();

        let s = std::time::Instant::now();

        for i in 0..black_box(I) {
            mse(&mut buf2, key, i as u8).unwrap();
        }

        println!("mse: {:.3} mb/s", mb / s.elapsed().as_secs_f64());
        let s = std::time::Instant::now();

        for i in (0..black_box(I)).rev() {
            imse(&mut buf2, key, i as u8).unwrap();
        }

        println!("imse: {:.3} mb/s", mb / s.elapsed().as_secs_f64());

        assert_eq!(buf, buf2);
    }
}
