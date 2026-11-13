// src/encoding/hb2.rs
//! HB2 编码：把每个输入字节按**大端序**展开为 8 个二值符号，典型用于空白字符隐写。
//!
//! 默认符号表为 `\0` / `\n`；调用方也可传入任意两个互不相同的字节（如 `b"01"`）。
//! 解码为宽松模式：表外字节会被自动跳过，因此可直接处理混入空格、换行或任意脏字符的输入。
//!
//! 需要启用 `"encoding-hb2"` 特性；`*_to_vec` 系列还需 `"alloc"`。

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

/// 默认符号表：`\0` 表示比特 0，`\n` 表示比特 1。
pub const DEFAULT_TABLE: &[u8; 2] = b"\0\n";

/// 将 `src` 逐字节编码为 HB2 符号串，写入 `dest`。
///
/// 每读入 1 个输入字节需要 8 字节输出空间；当 `dest` 剩余空间不足 8 字节时停止。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::encoding::hb2::{hb2_encode, DEFAULT_TABLE};
///
/// let mut out = [0u8; 16];
/// let (read, written) = hb2_encode(b"AB", &mut out, DEFAULT_TABLE);
/// assert_eq!((read, written), (2, 16));
/// ```
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
pub const fn hb2_encode(src: &[u8], dest: &mut [u8], table: &[u8; 2]) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j + 8 <= dest.len() {
        let val = src[i];

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

/// HB2 解码（大端序，宽松模式：自动跳过非表内字节）。
///
/// 每收集到 8 个表内符号即还原 1 个字节；`dest` 写满或尾部符号不足 8 个时停止。
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
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
            dest[j] = acc;
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
#[cfg(feature = "alloc")]
pub fn hb2_decode_to_vec(src: &[u8], table: &[u8; 2]) -> Vec<u8> {
    let mut dest = vec![0u8; src.len() / 8];
    let (_, written) = hb2_decode(src, &mut dest, table);
    dest.truncate(written);
    dest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "std")]
    fn test_hb2_roundtrip() {
        let custom_table = b"01"; // 使用直观的 01 打印测试

        let test_cases = [
            "A", // 'A' 的 ASCII 为 0x41 = 0b01000001
            "hello world",
            "\0\0\0",
            "🦀 Rust 隐写测试",
        ];

        for text in test_cases {
            let encoded = hb2_encode_to_vec(text.as_bytes(), custom_table);

            if text == "A" {
                assert_eq!(&encoded, b"01000001");
            }

            let decoded = hb2_decode_to_vec(&encoded, custom_table);
            assert_eq!(text.as_bytes(), &decoded[..]);
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_hb2_loose_mode() {
        let table = b"01";
        // 混入了脏字符 '@'、换行符和空格
        let dirty_input = b"01@00\n 0001"; // 过滤后应为 "01000001" ('A')
        let decoded = hb2_decode_to_vec(dirty_input, table);
        assert_eq!(decoded, b"A");
    }
}
