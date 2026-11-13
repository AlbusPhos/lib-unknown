// src/encoding/hb3.rs
//! HB3 编码：把每个输入字节展开为 6 个三进制（trit）符号，典型用于空白字符隐写。
//!
//! 默认符号表为 `\0` / `\n` / 空格（`3^6 = 729 > 256`，因此 6 个三进制位足以覆盖全部字节值）。
//! 解码为宽松模式：表外字节会被自动跳过。
//!
//! 需要启用 `"encoding-hb3"` 特性；`*_to_vec` 系列还需 `"alloc"`。

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

/// 默认符号表：`\0` 表示 trit 0，`\n` 表示 trit 1，空格表示 trit 2。
pub const DEFAULT_TABLE: &[u8; 3] = b"\0\n ";

/// 将 `src` 逐字节编码为 HB3 符号串（大端序），写入 `dest`。
///
/// 每读入 1 个输入字节需要 6 字节输出空间；当 `dest` 剩余空间不足 6 字节时停止。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::encoding::hb3::{hb3_encode, DEFAULT_TABLE};
///
/// let mut out = [0u8; 12];
/// let (read, written) = hb3_encode(b"AB", &mut out, DEFAULT_TABLE);
/// assert_eq!((read, written), (2, 12));
/// ```
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
pub const fn hb3_encode(src: &[u8], dest: &mut [u8], table: &[u8; 3]) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j + 6 <= dest.len() {
        let mut val = src[i];

        let t0 = table[(val % 3) as usize];
        val /= 3;
        let t1 = table[(val % 3) as usize];
        val /= 3;
        let t2 = table[(val % 3) as usize];
        val /= 3;
        let t3 = table[(val % 3) as usize];
        val /= 3;
        let t4 = table[(val % 3) as usize];
        val /= 3;
        let t5 = table[(val % 3) as usize];

        dest[j + 5] = t0;
        dest[j + 4] = t1;
        dest[j + 3] = t2;
        dest[j + 2] = t3;
        dest[j + 1] = t4;
        dest[j] = t5;

        i += 1;
        j += 6;
    }

    (i, j) // (已读字节数, 已写字节数)
}

/// HB3 解码（宽松模式：自动跳过非表内字节）。
///
/// 每收集到 6 个表内符号即还原 1 个字节；`dest` 写满或尾部符号不足 6 个时停止。
///
/// # Returns
///
/// `(已读取的输入字节数, 已写入的输出字节数)`。
pub const fn hb3_decode(src: &[u8], dest: &mut [u8], table: &[u8; 3]) -> (usize, usize) {
    let mut lut = [0xFFu8; 256];
    lut[table[0] as usize] = 0;
    lut[table[1] as usize] = 1;
    lut[table[2] as usize] = 2;

    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j < dest.len() {
        let mut cur_i = i;
        let mut count = 0;
        let mut acc: u16 = 0;

        while cur_i < src.len() {
            let trit = lut[src[cur_i] as usize];
            cur_i += 1;

            if trit == 0xFF {
                if count == 0 {
                    i = cur_i;
                }
                continue;
            }

            acc = acc * 3 + (trit as u16);
            count += 1;

            if count == 6 {
                break;
            }
        }

        if count == 6 {
            dest[j] = acc as u8;
            j += 1;
            i = cur_i;
        } else {
            break;
        }
    }

    (i, j) // (已读字节数, 已写字节数)
}

/// 将 `src` 编码为 HB3 符号串并返回新分配的 `Vec<u8>`。
///
/// # Panics
///
/// - 当 `src.len() * 6` 超出 `usize` 可表示范围时 panic（容量溢出）。
#[cfg(feature = "alloc")]
pub fn hb3_encode_to_vec(src: &[u8], table: &[u8; 3]) -> Vec<u8> {
    let Some(cap) = src.len().checked_mul(6) else {
        panic!("HB3 encode capacity overflow");
    };
    let mut dest = vec![0u8; cap];
    let (_, written) = hb3_encode(src, &mut dest, table);
    dest.truncate(written);
    dest
}

/// 将 HB3 符号串解码为字节并返回新分配的 `Vec<u8>`（宽松模式，跳过表外字节）。
#[cfg(feature = "alloc")]
pub fn hb3_decode_to_vec(src: &[u8], table: &[u8; 3]) -> Vec<u8> {
    let mut dest = vec![0u8; src.len() / 6];
    let (_, written) = hb3_decode(src, &mut dest, table);
    dest.truncate(written);
    dest
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(feature = "std")]
    fn test_hb3_encode() {
        let t = [
            "11111111111111",
            "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            "hello",
            "~呜嗷嗷嗷嗷呜喵嗷喵~呜嗷呜呜~嗷~嗷喵嗷喵呜嗷嗷呜~~嗷~呜呜嗷喵~嗷嗷嗷呜喵嗷嗷喵喵",
        ];

        for b in t {
            let x = hb3_encode_to_vec(b.as_ref(), DEFAULT_TABLE);
            println!("{b:<16}  {:?}", x);
            assert_eq!(
                b,
                String::from_utf8(hb3_decode_to_vec(&x[..], DEFAULT_TABLE)).unwrap()
            );
        }

        println!()
    }
}
