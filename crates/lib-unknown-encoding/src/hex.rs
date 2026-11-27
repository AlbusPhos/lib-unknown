// src/encoding/hex.rs

#[allow(unused_imports)]
use lib_unknown_polymorphism::poly;

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

#[poly(inline = 0.7, cold = 0.05)]
const fn nibble_to_hex(n: u8, upper: bool) -> u8 {
    if n < 10 {
        b'0' + n
    } else if upper {
        b'A' - 10 + n
    } else {
        b'a' - 10 + n
    }
}

#[poly]
const fn hex_val(c: u8) -> i8 {
    match c {
        b'0'..=b'9' => (c - b'0') as i8,
        b'a'..=b'f' => (c - b'a' + 10) as i8,
        b'A'..=b'F' => (c - b'A' + 10) as i8,
        _ => -1,
    }
}

/// 计算编码指定长度字节所需的输出缓冲区容量大小（每个字节转为 2 个十六进制字符）
#[poly(inline = 0.9)]
pub const fn hex_encode_len(src_len: usize) -> usize {
    src_len * 2
}

/// 计算解码指定长度十六进制字符所需的最大缓冲区容量（上界）
///
/// 注意：由于 `hex_decode` 具有容错机制（自动跳过非十六进制字符），实际解码输出的字节数可能小于此值。
#[poly]
pub const fn hex_decode_len(src_len: usize) -> usize {
    src_len / 2
}

#[poly]
/// 将 `src` 编码为十六进制（`upper` 控制大小写），返回 `(已读字节数, 已写字符数)`。
pub const fn hex_encode(src: &[u8], dest: &mut [u8], upper: bool) -> (usize, usize) {
    let src_len = src.len();
    let dest_len = dest.len();
    let src_ptr = src.as_ptr();
    let dest_ptr = dest.as_mut_ptr();

    let mut i = 0;
    let mut j = 0;

    while i < src_len && j + 1 < dest_len {
        // SAFETY: 循环条件保证了 i < src_len，且 j + 1 < dest_len，
        // 因此访问不会越界。
        unsafe {
            let b = *src_ptr.add(i);
            let hi = b >> 4;
            let lo = b & 0x0F;
            *dest_ptr.add(j) = nibble_to_hex(hi, upper);
            *dest_ptr.add(j + 1) = nibble_to_hex(lo, upper);
        }
        i += 1;
        j += 2;
    }
    (i, j)
}

#[poly(inline = 0.2)]
/// 将十六进制文本解码到 `dest`，返回 `(已读字符数, 已写字节数)`；非十六进制字符会被跳过。
pub const fn hex_decode(src: &[u8], dest: &mut [u8]) -> (usize, usize) {
    let src_len = src.len();
    let dest_len = dest.len();
    #[allow(unused)]
    let src_ptr = src.as_ptr();
    let dest_ptr = dest.as_mut_ptr();

    let mut i = 0;
    let mut j = 0;

    while i < src_len && j < dest_len {
        let mut hi: i8 = -1;
        while i < src_len {
            let v = hex_val(lib_unknown_polymorphism::poly_select!(
                {
                    // SAFETY: 循环条件已严格限制 i < src_len
                    unsafe { *src_ptr.add(i) }
                },
                src[i],
            ));
            i += 1;
            if v >= 0 {
                hi = v;
                break;
            }
        }
        if hi < 0 {
            break;
        }

        let mut lo: i8 = -1;
        while i < src_len {
            // SAFETY: 循环条件已严格限制 i < src_len
            let v = hex_val(lib_unknown_polymorphism::poly_select!(
                {
                    // SAFETY: 循环条件已严格限制 i < src_len
                    unsafe { *src_ptr.add(i) }
                },
                src[i],
            ));
            i += 1;
            if v >= 0 {
                lo = v;
                break;
            }
        }
        if lo < 0 {
            break;
        }

        // SAFETY: 外层循环保证了 j < dest_len
        unsafe {
            *dest_ptr.add(j) = ((hi as u8) << 4) | (lo as u8);
        }
        j += 1;
    }
    (i, j)
}

/// 将字节切片编码为十六进制，返回一个 `Vec<u8>`
#[cfg(feature = "alloc")]
#[poly(inline = 0.85)]
pub fn hex_encode_vec(src: &[u8], upper: bool) -> Vec<u8> {
    let out_len = hex_encode_len(src.len());
    let mut dest = vec![0; out_len];
    hex_encode(src, &mut dest, upper);
    dest
}

/// 将十六进制字节切片解码，返回一个 `Vec<u8>`
///
/// 自动处理并截除跳过非法字符后的多余容量。
#[cfg(feature = "alloc")]
#[poly(inline = 0.85)]
pub fn hex_decode_vec(src: &[u8]) -> Vec<u8> {
    let max_len = hex_decode_len(src.len());
    let mut dest = vec![0; max_len];
    let (_, written) = hex_decode(src, &mut dest);
    dest.truncate(written);
    dest
}
