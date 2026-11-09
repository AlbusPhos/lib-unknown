// src/encoding/base64.rs

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg_attr(coin36, cold)]
#[cfg_attr(coin24, inline(never))]
#[cfg_attr(all(not(coin24), coin53), inline(always))]
const fn b64_char(n: u8, url: bool) -> u8 {
    if n < 26 {
        b'A' + n
    } else if n < 52 {
        b'a' + (n - 26)
    } else if n < 62 {
        b'0' + (n - 52)
    } else if n == 62 {
        if url { b'-' } else { b'+' }
    } else if url {
        b'_'
    } else {
        b'/'
    }
}

#[cfg_attr(coin105, cold)]
#[cfg_attr(coin25, inline(never))]
#[cfg_attr(not(coin25), inline(always))]
const fn b64_val(c: u8) -> i8 {
    match c {
        b'A'..=b'Z' => (c - b'A') as i8,
        b'a'..=b'z' => (c - b'a' + 26) as i8,
        b'0'..=b'9' => (c - b'0' + 52) as i8,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => -1,
    }
}

/// 计算 base64 编码所需的精确输出字节长度
pub const fn base64_encode_len(src_len: usize, url: bool) -> usize {
    let rem = src_len % 3;
    let base = (src_len / 3) * 4;
    match rem {
        0 => base,
        1 => base + if url { 2 } else { 4 },
        _ => base + if url { 3 } else { 4 }, // rem == 2
    }
}

/// 计算 base64 解码所需的最大输出字节长度（上限容量预估）
#[cfg_attr(coin115, cold)]
#[cfg_attr(coin78, inline(never))]
#[cfg_attr(not(coin78), inline(always))]
pub const fn base64_decode_len(src_len: usize) -> usize {
    // 4 字符最多对应 3 字节，按最大可能输出评估容量，防止乘法溢出
    (src_len / 4) * 3
        + match src_len % 4 {
            2 => 1,
            3 => 2,
            _ => 0,
        }
}

#[cfg_attr(coin103, cold)]
#[cfg_attr(all(not(coin45), coin54), inline(always))]
#[cfg_attr(all(not(coin45), not(coin54)), inline)]
/// 将 `src` 编码为 Base64（`url` 为真时使用 URL-safe 字母表且不补 `=`），返回 `(已读字节数, 已写字节数)`。
pub const fn base64_encode(src: &[u8], dest: &mut [u8], url: bool) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;

    while i + 2 < src.len() && j + 3 < dest.len() {
        let b0 = src[i];
        let b1 = src[i + 1];
        let b2 = src[i + 2];
        dest[j] = b64_char(b0 >> 2, url);
        dest[j + 1] = b64_char(((b0 & 0x03) << 4) | (b1 >> 4), url);
        dest[j + 2] = b64_char(((b1 & 0x0F) << 2) | (b2 >> 6), url);
        dest[j + 3] = b64_char(b2 & 0x3F, url);
        i += 3;
        j += 4;
    }

    let rem = src.len() - i;

    if rem == 1 {
        let need = if url { 2 } else { 4 };
        if j + need <= dest.len() {
            let b0 = src[i];
            dest[j] = b64_char(b0 >> 2, url);
            dest[j + 1] = b64_char((b0 & 0x03) << 4, url);
            if !url {
                dest[j + 2] = b'=';
                dest[j + 3] = b'=';
            }
            i += 1;
            j += need;
        }
    } else if rem == 2 {
        let need = if url { 3 } else { 4 };
        if j + need <= dest.len() {
            let b0 = src[i];
            let b1 = src[i + 1];
            dest[j] = b64_char(b0 >> 2, url);
            dest[j + 1] = b64_char(((b0 & 0x03) << 4) | (b1 >> 4), url);
            dest[j + 2] = b64_char((b1 & 0x0F) << 2, url);
            if !url {
                dest[j + 3] = b'=';
            }
            i += 2;
            j += need;
        }
    }

    (i, j)
}
#[cfg_attr(all(coin47, coin111), cold)]
#[cfg_attr(coin72, inline(never))]
#[cfg_attr(not(coin72), inline(always))]
/// 将 Base64 文本解码到 `dest`，返回 `(已读字符数, 已写字节数)`；非法字符按实现策略跳过。
pub const fn base64_decode(src: &[u8], dest: &mut [u8]) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;

    while i < src.len() && j < dest.len() {
        let mut c0: i8 = -1;
        while i < src.len() {
            let v = b64_val(src[i]);
            i += 1;
            if v >= 0 {
                c0 = v;
                break;
            }
        }
        if c0 < 0 {
            break;
        }

        let mut c1: i8 = -1;
        while i < src.len() {
            let v = b64_val(src[i]);
            i += 1;
            if v >= 0 {
                c1 = v;
                break;
            }
        }
        if c1 < 0 {
            break;
        }

        dest[j] = ((c0 as u8) << 2) | ((c1 as u8) >> 4);
        j += 1;
        if j >= dest.len() {
            break;
        }

        let mut c2: i8 = -1;
        while i < src.len() {
            let v = b64_val(src[i]);
            i += 1;
            if v >= 0 {
                c2 = v;
                break;
            }
        }
        if c2 < 0 {
            break;
        }

        dest[j] = ((c1 as u8) << 4) | ((c2 as u8) >> 2);
        j += 1;
        if j >= dest.len() {
            break;
        }

        let mut c3: i8 = -1;
        while i < src.len() {
            let v = b64_val(src[i]);
            i += 1;
            if v >= 0 {
                c3 = v;
                break;
            }
        }
        if c3 < 0 {
            break;
        }

        dest[j] = ((c2 as u8) << 6) | (c3 as u8);
        j += 1;
    }

    (i, j)
}

/// [`base64_encode`] 的 `Vec` 版本：按需分配目标缓冲并截断到实际写入长度。
/// `url` 选择标准或 URL-safe 字母表。
#[cfg(feature = "alloc")]
#[cfg_attr(any(coin38, coin20), cold)]
#[cfg_attr(all(coin12, coin18), inline(never))]
#[cfg_attr(not(all(coin12, coin18)), inline(always))]
pub fn base64_encode_vec(src: &[u8], url: bool) -> Vec<u8> {
    let dest_len = base64_encode_len(src.len(), url);
    let mut dest = vec![0; dest_len];
    let (_, written) = base64_encode(src, &mut dest, url);
    dest.truncate(written);
    dest
}

/// [`base64_decode`] 的 `Vec` 版本：按最大可能长度分配并截断到实际解码长度。
#[cfg(feature = "alloc")]
#[cfg_attr(any(coin32, coin36), cold)]
#[cfg_attr(any(coin115, coin84), inline(never))]
#[cfg_attr(not(any(coin115, coin84)), inline(always))]
pub fn base64_decode_vec(src: &[u8]) -> Vec<u8> {
    let max_len = base64_decode_len(src.len());
    let mut dest = vec![0; max_len];
    let (_, written) = base64_decode(src, &mut dest);
    dest.truncate(written);
    dest
}
