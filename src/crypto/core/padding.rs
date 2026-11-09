// src/crypto/core/padding.rs
// SPDX-License-Identifier: MPL-2.0

//! 常数时间填充与去填充原语：PKCS#7（RFC 5652 §6.3）与 ISO/IEC 7816-4（类型 1 填充）。
//!
//! 去填充的校验循环不依赖秘密分支提前返回，所有条件以位掩码累加后统一判定，
//! 避免填充错误信息的时间侧信道。
use crate::errors::{ErrType, Result, bail, ensure};

/// 以 PKCS#7 方式将 `src` 补齐到 `align` 的整数倍并写入 `dest` 的开头。
/// 恰好对齐时追加一整个填充块。返回填充后的总长度。
///
/// # Errors
///
/// - 当 `align` 不在 `1..=255` 时返回 [`ErrType::InvalidArg`]；
/// - 当填充后总长度溢出时返回 [`ErrType::InvalidArg`]；
/// - 当 `dest` 容量不足以容纳填充结果时返回 [`ErrType::BufferTooSmall`]。
pub fn pkcs7_pad(src: &[u8], dest: &mut [u8], align: usize) -> Result<usize> {
    ensure!(
        (1..=255).contains(&align),
        ErrType::InvalidArg,
        "PKCS#7 align must be in 1..=255, got {}",
        align
    );

    let pad_len = align - (src.len() % align);
    let total_len = src
        .len()
        .checked_add(pad_len)
        .ok_or_else(|| crate::errors::err!(ErrType::InvalidArg, "padded length overflow"))?;

    ensure!(
        dest.len() >= total_len,
        ErrType::BufferTooSmall {
            required: total_len,
            available: dest.len(),
        }
    );

    dest[..src.len()].copy_from_slice(src);
    dest[src.len()..total_len].fill(pad_len as u8);

    Ok(total_len)
}

/// 与 [`pkcs7_pad`] 逆过程：按常数时间一致性校验去除 PKCS#7 填充，
/// 把去掉填充的数据写入 `dest` 开头并返回其长度。
/// 校验失败时不区分具体失败原因（可能同样的耗时与常量形态）。
///
/// # Errors
///
/// - 当 `align` 不在 `1..=255` 时返回 [`ErrType::InvalidArg`]；
/// - 当 `src` 为空或长度不是 `align` 的倍数时分别返回
///   [`ErrType::InvalidLength`] /
///   [`ErrType::UnalignedByteLength`]；
/// - 当填充字节形式非法（0 填充、越界、不一致）时返回 [`ErrType::InvalidEncoding`]；
/// - 当 `dest` 容量不足时返回 [`ErrType::BufferTooSmall`]。
pub fn pkcs7_unpad(src: &[u8], dest: &mut [u8], align: usize) -> Result<usize> {
    ensure!(
        (1..=255).contains(&align),
        ErrType::InvalidArg,
        "PKCS#7 align must be in 1..=255, got {}",
        align
    );
    ensure!(
        !src.is_empty(),
        ErrType::InvalidLength {
            expected: align,
            actual: 0,
        }
    );
    ensure!(
        src.len().is_multiple_of(align),
        ErrType::UnalignedByteLength {
            byte_len: src.len(),
            elem_size: align,
        }
    );

    let last_block = &src[src.len() - align..];
    let pad_byte = last_block[align - 1];
    let pad_len = pad_byte as usize;

    let mut diff = 0usize;
    diff |= (pad_len == 0) as usize;
    diff |= (pad_len > align) as usize;

    for (i, &blk) in last_block.iter().enumerate() {
        let dist_from_end = align - i;
        let mask = ((dist_from_end <= pad_len) as u8).wrapping_neg();
        diff |= ((blk ^ pad_byte) & mask) as usize;
    }

    if diff != 0 {
        bail!(ErrType::InvalidEncoding, "invalid PKCS#7 padding");
    }

    let out_len = src.len() - pad_len;
    ensure!(
        dest.len() >= out_len,
        ErrType::BufferTooSmall {
            required: out_len,
            available: dest.len(),
        }
    );

    dest[..out_len].copy_from_slice(&src[..out_len]);
    Ok(out_len)
}

/// 以 ISO/IEC 7816-4 方式填充：在数据后写入单个 `0x80` 标记，再补 `0x00` 至 `align` 整数倍，
/// 恰好对齐时（不满块）追加一整块 `0x80`。返回填充后的总长度。
///
/// # Errors
///
/// - 当 `align` 为 0 时返回 [`ErrType::InvalidArg`]；
/// - 当填充后总长度溢出时返回 [`ErrType::InvalidArg`]；
/// - 当 `dest` 容量不足时返回 [`ErrType::BufferTooSmall`]。
pub fn iso7816_pad(src: &[u8], dest: &mut [u8], align: usize) -> Result<usize> {
    ensure!(
        align >= 1,
        ErrType::InvalidArg,
        "ISO/IEC 7816-4 align must be >= 1"
    );

    let pad_len = align - (src.len() % align);
    let total_len = src
        .len()
        .checked_add(pad_len)
        .ok_or_else(|| crate::errors::err!(ErrType::InvalidArg, "padded length overflow"))?;

    ensure!(
        dest.len() >= total_len,
        ErrType::BufferTooSmall {
            required: total_len,
            available: dest.len(),
        }
    );

    dest[..src.len()].copy_from_slice(src);
    dest[src.len()] = 0x80;
    dest[src.len() + 1..total_len].fill(0x00);
    Ok(total_len)
}

/// 与 [`iso7816_pad`] 逆过程：从尾部向前以常数时间方式定位 `0x80` 标记，
/// 去除填充并把裸数据写入 `dest` 开头，返回其长度。
///
/// # Errors
///
/// - 当 `align` 为 0 时返回 [`ErrType::InvalidArg`]；
/// - 当 `src` 为空或长度不是 `align` 的倍数时分别返回
///   [`ErrType::InvalidLength`] /
///   [`ErrType::UnalignedByteLength`]；
/// - 当末尾不存在合法 `0x80` 标记时返回 [`ErrType::InvalidEncoding`]；
/// - 当 `dest` 容量不足时返回 [`ErrType::BufferTooSmall`]。
pub fn iso7816_unpad(src: &[u8], dest: &mut [u8], align: usize) -> Result<usize> {
    ensure!(
        align >= 1,
        ErrType::InvalidArg,
        "ISO/IEC 7816-4 align must be >= 1"
    );
    ensure!(
        !src.is_empty(),
        ErrType::InvalidLength {
            expected: align,
            actual: 0,
        }
    );
    ensure!(
        src.len().is_multiple_of(align),
        ErrType::UnalignedByteLength {
            byte_len: src.len(),
            elem_size: align,
        }
    );

    let last_block = &src[src.len() - align..];

    let mut seen_non_zero = 0u8;
    let mut valid_marker = 0u8;
    let mut pad_len = 0usize;

    for i in (0..align).rev() {
        let b = last_block[i];
        let is_non_zero = (b != 0) as u8;
        let is_first_non_zero = is_non_zero & (seen_non_zero ^ 1);
        let is_0x80 = (b == 0x80) as u8;

        valid_marker |= is_first_non_zero & is_0x80;
        seen_non_zero |= is_non_zero;
        pad_len += ((seen_non_zero ^ 1) | is_first_non_zero) as usize;
    }

    if valid_marker != 1 {
        bail!(ErrType::InvalidEncoding, "invalid ISO/IEC 7816-4 padding");
    }

    let out_len = src.len() - pad_len;
    ensure!(
        dest.len() >= out_len,
        ErrType::BufferTooSmall {
            required: out_len,
            available: dest.len(),
        }
    );

    dest[..out_len].copy_from_slice(&src[..out_len]);
    Ok(out_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkcs7_roundtrip() {
        let mut padded = [0u8; 32];
        let mut unpadded = [0u8; 32];

        // 1. 非对齐长度 (5 bytes -> 8 bytes)
        let msg = b"hello";
        let n = pkcs7_pad(msg, &mut padded, 8).unwrap();
        assert_eq!(&padded[..n], b"hello\x03\x03\x03");

        let m = pkcs7_unpad(&padded[..n], &mut unpadded, 8).unwrap();
        assert_eq!(&unpadded[..m], msg);

        // 2. 恰好对齐长度 (8 bytes -> 16 bytes，必须多补一整块)
        let msg_aligned = b"12345678";
        let n = pkcs7_pad(msg_aligned, &mut padded, 8).unwrap();
        assert_eq!(n, 16);
        assert_eq!(&padded[8..16], &[8u8; 8]);

        let m = pkcs7_unpad(&padded[..n], &mut unpadded, 8).unwrap();
        assert_eq!(&unpadded[..m], msg_aligned);
    }

    #[test]
    fn test_pkcs7_invalid_padding() {
        let mut out = [0u8; 16];

        // 填充字节不一致
        let bad_bytes = b"hello\x03\x02\x03";
        assert_eq!(
            pkcs7_unpad(bad_bytes, &mut out, 8).unwrap_err().err_type(),
            ErrType::InvalidEncoding
        );

        // 填充长度为 0
        let zero_pad = b"hello12\x00";
        assert_eq!(
            pkcs7_unpad(zero_pad, &mut out, 8).unwrap_err().err_type(),
            ErrType::InvalidEncoding
        );

        // 填充长度超过块大小
        let over_pad = b"hello12\x09";
        assert_eq!(
            pkcs7_unpad(over_pad, &mut out, 8).unwrap_err().err_type(),
            ErrType::InvalidEncoding
        );

        // 输入长度未对齐
        assert_eq!(
            pkcs7_unpad(b"12345", &mut out, 8).unwrap_err().err_type(),
            ErrType::UnalignedByteLength {
                byte_len: 5,
                elem_size: 8
            }
        );
    }

    #[test]
    fn test_iso7816_roundtrip_and_errors() {
        let mut padded = [0u8; 32];
        let mut unpadded = [0u8; 32];

        let msg = b"hello";
        let n = iso7816_pad(msg, &mut padded, 8).unwrap();
        assert_eq!(&padded[..n], b"hello\x80\x00\x00");

        let m = iso7816_unpad(&padded[..n], &mut unpadded, 8).unwrap();
        assert_eq!(&unpadded[..m], msg);

        // 篡改末尾 0x00 为非零字节
        let bad = b"hello\x80\x00\x01";
        assert_eq!(
            iso7816_unpad(bad, &mut unpadded, 8).unwrap_err().err_type(),
            ErrType::InvalidEncoding
        );

        // 缺少 0x80 标记
        let no_marker = b"\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            iso7816_unpad(no_marker, &mut unpadded, 8)
                .unwrap_err()
                .err_type(),
            ErrType::InvalidEncoding
        );
    }
}
