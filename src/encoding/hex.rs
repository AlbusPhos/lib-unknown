// src/encoding/hex.rs

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

#[cfg_attr(all(coin9, coin109), cold)]
#[cfg_attr(all(coin54, coin1), inline(never))]
#[cfg_attr(not(all(coin54, coin1)), inline(always))]
const fn nibble_to_hex(n: u8, upper: bool) -> u8 {
    if n < 10 {
        b'0' + n
    } else if upper {
        b'A' - 10 + n
    } else {
        b'a' - 10 + n
    }
}

#[cfg_attr(all(coin20, coin122), cold)]
#[cfg_attr(coin18, inline(never))]
#[cfg_attr(all(not(coin18), coin104), inline(always))]
#[cfg_attr(all(not(coin18), not(coin104)), inline)]
const fn hex_val(c: u8) -> i8 {
    match c {
        b'0'..=b'9' => (c - b'0') as i8,
        b'a'..=b'f' => (c - b'a' + 10) as i8,
        b'A'..=b'F' => (c - b'A' + 10) as i8,
        _ => -1,
    }
}

/// 计算编码指定长度字节所需的输出缓冲区容量大小（每个字节转为 2 个十六进制字符）
#[cfg_attr(all(coin20, coin41), cold)]
#[cfg_attr(any(coin106, coin49), inline(never))]
#[cfg_attr(not(any(coin106, coin49)), inline(always))]
pub const fn hex_encode_len(src_len: usize) -> usize {
    src_len * 2
}

/// 计算解码指定长度十六进制字符所需的最大缓冲区容量（上界）
///
/// 注意：由于 `hex_decode` 具有容错机制（自动跳过非十六进制字符），实际解码输出的字节数可能小于此值。
#[cfg_attr(all(coin32, coin33), cold)]
#[cfg_attr(coin97, inline(never))]
#[cfg_attr(all(not(coin97), coin122), inline(always))]
#[cfg_attr(all(not(coin97), not(coin122)), inline)]
pub const fn hex_decode_len(src_len: usize) -> usize {
    src_len / 2
}

#[cfg_attr(all(coin44, coin98), cold)]
#[cfg_attr(coin123, inline(never))]
#[cfg_attr(all(not(coin123), coin48), inline(always))]
#[cfg_attr(all(not(coin123), not(coin48)), inline)]
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

#[cfg_attr(any(coin29, coin37), cold)]
#[cfg_attr(all(coin54, coin52), inline(never))]
#[cfg_attr(not(all(coin54, coin52)), inline(always))]
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
            let v = hex_val({
                #[cfg(coin1)]
                {
                    // SAFETY: 循环条件已严格限制 i < src_len
                    unsafe { *src_ptr.add(i) }
                }
                #[cfg(not(coin1))]
                {
                    src[i]
                }
            });
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
            let v = hex_val({
                #[cfg(coin50)]
                {
                    // SAFETY: 循环条件已严格限制 i < src_len
                    unsafe { *src_ptr.add(i) }
                }
                #[cfg(not(coin50))]
                {
                    src[i]
                }
            });
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
#[cfg_attr(coin50, cold)]
#[cfg_attr(all(coin46, coin118), inline(never))]
#[cfg_attr(not(all(coin46, coin118)), inline(always))]
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
#[cfg_attr(any(coin16, coin6), cold)]
#[cfg_attr(any(coin115, coin40), inline(never))]
#[cfg_attr(not(any(coin115, coin40)), inline(always))]
pub fn hex_decode_vec(src: &[u8]) -> Vec<u8> {
    let max_len = hex_decode_len(src.len());
    let mut dest = vec![0; max_len];
    let (_, written) = hex_decode(src, &mut dest);
    dest.truncate(written);
    dest
}
