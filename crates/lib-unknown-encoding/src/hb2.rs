// SPDX-License-Identifier: MPL-2.0
//! HB2 编码：把每个输入字节按**大端序**展开为 8 个二值符号，典型用于空白字符隐写。

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

use lib_unknown_crypto::core::mix2::{inv_mix8_2, mix8_2, mix128_2};
use lib_unknown_polymorphism::poly;

/// 默认符号表：`\0` 表示比特 0，`\n` 表示比特 1。
pub const DEFAULT_TABLE: &[u8; 2] = b"\0\n";

/// 基于逻辑字节索引 `idx` 生成扰动参数：`(异或白化掩码, 循环移位位数)`。
#[poly]
const fn index_perturb(idx: usize) -> (u8, u32) {
    let mut h = mix128_2(idx as u128);
    let mut m = 0;
    let mut r: u32 = 0;

    let mut i = 0;
    while i < 16 {
        let b = h as u8;
        m ^= b;
        r ^= r.wrapping_mul((m | 3) as u32);
        h >>= 8;
        i += 1;
    }
    (m, r)
}

/// 对第 `idx` 个逻辑字节施加正向扰动（编码前调用）。
///
/// 先结合索引参数做移位与异或白化，再经 `mix8_2` 做非线性双射置换。
#[poly]
const fn perturb_byte(val: u8, idx: usize) -> u8 {
    let (mask, rot) = index_perturb(idx);
    mix8_2(val.rotate_left(rot) ^ mask)
}

/// 对第 `idx` 个逻辑字节施加逆向解扰（解码后调用）。
///
/// 先经 `inv_mix8_2` 还原非线性双射，再逆向还原异或白化与移位。
#[poly]
const fn unperturb_byte(val: u8, idx: usize) -> u8 {
    let (mask, rot) = index_perturb(idx);
    (inv_mix8_2(val) ^ mask).rotate_right(rot)
}

/// 将 `src` 逐字节编码为带索引扰动的 HB2 符号串，写入 `dest`。
///
/// 每读入 1 个输入字节需要 8 字节输出空间；当 `dest` 剩余空间不足 8 字节时停止。
///
/// # Examples
///
/// ```rust
/// use lib_unknown_encoding::hb2::{hb2_encode, hb2_decode, DEFAULT_TABLE};
///
/// let mut encoded = [0u8; 16];
/// let (read, written) = hb2_encode(b"AB", &mut encoded, DEFAULT_TABLE);
/// assert_eq!((read, written), (2, 16));
///
/// let mut decoded = [0u8; 2];
/// hb2_decode(&encoded, &mut decoded, DEFAULT_TABLE);
/// assert_eq!(&decoded, b"AB");
/// ```
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
#[poly]
pub const fn hb2_encode(src: &[u8], dest: &mut [u8], table: &[u8; 2]) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j + 8 <= dest.len() {
        // 引入基于逻辑字节索引 i 的扰动
        let val = perturb_byte(src[i], i);

        dest[j] = table[((val >> 7) & 1) as usize];
        dest[j + 1] = table[((val >> 6) & 1) as usize];
        dest[j + 2] = table[((val >> 5) & 1) as usize];
        dest[j + 3] = table[((val >> 4) & 1) as usize];
        dest[j + 4] = table[((val >> 3) & 1) as usize];
        dest[j + 5] = table[((val >> 2) & 1) as usize];
        dest[j + 6] = table[((val >> 1) & 1) as usize];
        dest[j + 7] = table[(val & 1) as usize];

        i += 1;
        j += 8;
    }

    (i, j) // (已读字节数, 已写字节数)
}

/// HB2 解码（大端序，宽松模式：自动跳过非表内字节，并还原索引扰动）。
///
/// 每收集到 8 个表内符号即还原 1 个字节；`dest` 写满或尾部符号不足 8 个时停止。
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
#[poly]
pub const fn hb2_decode(src: &[u8], dest: &mut [u8], table: &[u8; 2]) -> (usize, usize) {
    let mut lut = [0xFFu8; 256];
    lut[table[0] as usize] = 0;
    lut[table[1] as usize] = 1;

    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j < dest.len() {
        let mut cur_i = i;
        let mut count = 0;
        let mut acc: u8 = 0;

        while cur_i < src.len() {
            let bit = lut[src[cur_i] as usize];
            cur_i += 1;

            if bit == 0xFF {
                if count == 0 {
                    i = cur_i;
                }
                continue;
            }

            acc = (acc << 1) | bit;
            count += 1;

            if count == 8 {
                break;
            }
        }

        if count == 8 {
            // 注意：此处必须使用逻辑输出索引 j 进行解扰，
            // 这样即使输入流混入了表外脏字符（导致 cur_i 偏移），解码依然完全同步。
            dest[j] = unperturb_byte(acc, j);
            j += 1;
            i = cur_i;
        } else {
            break;
        }
    }

    (i, j) // (已读字节数, 已写字节数)
}

/// 将 `src` 编码为 HB2 符号串并返回新分配的 `Vec<u8>`。
///
/// # Panics
///
/// - 当 `src.len() * 8` 超出 `usize` 可表示范围时 panic（容量溢出）。
#[poly]
#[cfg(feature = "alloc")]
pub fn hb2_encode_to_vec(src: &[u8], table: &[u8; 2]) -> Vec<u8> {
    let Some(cap) = src.len().checked_mul(8) else {
        panic!("HB2 encode capacity overflow");
    };
    let mut dest = vec![0u8; cap];
    let (_, written) = hb2_encode(src, &mut dest, table);
    dest.truncate(written);
    dest
}

/// 将 HB2 符号串解码为字节并返回新分配的 `Vec<u8>`（宽松模式，跳过表外字节）。
#[poly]
#[cfg(feature = "alloc")]
pub fn hb2_decode_to_vec(src: &[u8], table: &[u8; 2]) -> Vec<u8> {
    let mut dest = vec![0u8; src.len() / 8];
    let (_, written) = hb2_decode(src, &mut dest, table);
    dest.truncate(written);
    dest
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hb2_roundtrip() {
        let custom_table = b"01";

        let test_cases = [
            "A",
            "hello world",
            "\0\0\0",
            "AAAAAAAAAAAAAAAA",
            "🦀 Rust 隐写测试",
        ];

        for text in test_cases {
            let encoded = hb2_encode_to_vec(text.as_bytes(), custom_table);
            let decoded = hb2_decode_to_vec(&encoded, custom_table);
            assert_eq!(text.as_bytes(), &decoded[..]);
        }
    }

    #[test]
    fn test_hb2_feature_suppression() {
        let table = b"01";

        // 1. 验证单字节不会暴露原始二进制特征（'A' = 0b01000001）
        let encoded_a = hb2_encode_to_vec(b"A", table);
        assert_ne!(&encoded_a, b"01000001");

        // 2. 验证连续相同字节（如全零）编码后各块互不相同，消除周期与游程特征
        let encoded_zeros = hb2_encode_to_vec(b"\0\0\0\0", table);
        let chunks: Vec<&[u8]> = encoded_zeros.chunks(8).collect();
        for m in 0..chunks.len() {
            for n in (m + 1)..chunks.len() {
                assert_ne!(chunks[m], chunks[n], "相同字节在不同索引下编码结果不应相同");
            }
        }
    }

    #[test]
    fn test_hb2_loose_mode() {
        let table = b"01";
        let raw_encoded = hb2_encode_to_vec(b"AB", table);

        // 在编码流中人为混入脏字符 '@'、换行符和空格
        let mut dirty_input = Vec::new();
        dirty_input.extend_from_slice(&raw_encoded[0..2]);
        dirty_input.push(b'@');
        dirty_input.extend_from_slice(&raw_encoded[2..5]);
        dirty_input.extend_from_slice(b"\n ");
        dirty_input.extend_from_slice(&raw_encoded[5..]);

        let decoded = hb2_decode_to_vec(&dirty_input, table);
        assert_eq!(decoded, b"AB");
    }
}
