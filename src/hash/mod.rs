// SPDX-License-Identifier: MPL-2.0

#![cfg(feature = "hash-base")]

/// 无 std 定容开放寻址哈希表（线性探测）。
#[cfg(feature = "hash-hashmap")]
pub mod hashmap;

/// 32 字节分块的多泳道乘法链哈希器（wyz 式流式，兼容 `core::hash::Hasher`）。
pub struct LightHasher {
    h: [u64; 4],
    buf: [u8; 32],
    buf_len: u8,
    size: u64,
}

const C: u64 = 0x9e3779b97f4a7c15;

impl LightHasher {
    /// 以显式种子构造哈希器；`new0()` 变体使用构建期随机常量。
    #[cfg_attr(all(coin88, coin93), cold)]
    #[cfg_attr(any(coin100, coin31), inline(never))]
    #[cfg_attr(not(any(coin100, coin31)), inline(always))]
    pub fn new(seed: u64) -> Self {
        Self {
            h: [
                seed,
                seed.wrapping_add(0x85ebca6b),
                seed.wrapping_add(0xc2b2ae35),
                seed.wrapping_add(0x27d4eb2f),
            ],
            buf: [0; 32],
            buf_len: 0,
            size: 0,
        }
    }
    #[cfg_attr(any(coin27, coin39), cold)]
    #[cfg_attr(any(coin100, coin78), inline(never))]
    #[cfg_attr(not(any(coin100, coin78)), inline(always))]
    fn read_u64_x4(slice: &[u8; 32]) -> [u64; 4] {
        let ptr = slice.as_ptr() as *const u64;
        // SAFETY: `slice` 是 32 字节定长数组引用；`ptr.add(0..=3)` 的 4 组 8 字节
        // 未对齐读取始终落在该数组范围内，且 `u64` 满足 `Copy`，无别名问题。
        unsafe {
            [
                u64::from_le(ptr.read_unaligned()),
                u64::from_le(ptr.add(1).read_unaligned()),
                u64::from_le(ptr.add(2).read_unaligned()),
                u64::from_le(ptr.add(3).read_unaligned()),
            ]
        }
    }

    #[cfg_attr(all(coin52, coin79), cold)]
    #[cfg_attr(coin4, inline(never))]
    #[cfg_attr(not(coin4), inline(always))]
    fn round(h: &mut [u64; 4], v: [u64; 4]) {
        h[0] = h[0].wrapping_add(v[0]).wrapping_mul(C);
        h[1] = h[1].wrapping_add(v[1]).wrapping_mul(C);
        h[2] = h[2].wrapping_add(v[2]).wrapping_mul(C);
        h[3] = h[3].wrapping_add(v[3]).wrapping_mul(C);
    }

    #[cfg_attr(all(coin104, coin97), cold)]
    #[cfg_attr(all(not(coin82), coin87), inline(always))]
    #[cfg_attr(all(not(coin82), not(coin87)), inline)]
    fn write_bytes(&mut self, mut bytes: &[u8]) {
        self.size = self.size.wrapping_add(bytes.len() as u64);

        let buf_len = self.buf_len as usize;
        if buf_len > 0 {
            let take = (32 - buf_len).min(bytes.len());
            self.buf[buf_len..buf_len + take].copy_from_slice(&bytes[..take]);
            self.buf_len = (buf_len + take) as u8;
            bytes = &bytes[take..];

            if self.buf_len == 32 {
                let v = Self::read_u64_x4(&self.buf);
                Self::round(&mut self.h, v);
                self.buf_len = 0;
            } else {
                return;
            }
        }

        let (chunks, remainder) = bytes.as_chunks::<32>();
        for chunk in chunks {
            let v = Self::read_u64_x4(chunk);
            Self::round(&mut self.h, v);
        }

        let remainder = if remainder.is_empty() {
            None
        } else {
            Some(remainder)
        };
        if let Some(remainder) = remainder {
            self.buf[..remainder.len()].copy_from_slice(remainder);
            self.buf_len = remainder.len() as u8;
        }
    }
}

impl core::hash::Hasher for LightHasher {
    #[cfg_attr(any(coin36, coin87), cold)]
    #[cfg_attr(any(coin121, coin114), inline(never))]
    #[cfg_attr(not(any(coin121, coin114)), inline(always))]
    fn finish(&self) -> u64 {
        let mut h = self.h;

        if self.buf_len > 0 {
            let mut remaining = [0_u8; 32];
            let len = self.buf_len as usize;
            remaining[..len].copy_from_slice(&self.buf[..len]);
            let v = Self::read_u64_x4(&remaining);
            Self::round(&mut h, v);
        }

        let combined = h[0] ^ h[1].rotate_left(13) ^ h[2].rotate_left(27) ^ h[3].rotate_left(41);
        combined.wrapping_add(self.size).wrapping_mul(C)
    }

    #[cfg_attr(any(coin84, coin24), cold)]
    #[cfg_attr(any(coin112, coin12), inline(never))]
    #[cfg_attr(not(any(coin112, coin12)), inline(always))]
    fn write(&mut self, bytes: &[u8]) {
        self.write_bytes(bytes);
    }
}

#[cfg(test)]
mod tests {

    #[test]
    #[cfg(feature = "std")]
    fn light_hasher() {
        use super::*;
        use core::hash::Hasher;
        use std::time::Instant;
        let data = vec![128; 1024 * 1024];
        let iters = 1000u64;

        for _ in 0..10 {
            let mut h = LightHasher::new(0);
            h.write(core::hint::black_box(&data));
            core::hint::black_box(h.finish());
        }

        let start = Instant::now();
        for _ in 0..iters {
            let mut h = LightHasher::new(0);
            h.write(core::hint::black_box(&data));
            core::hint::black_box(h.finish());
        }
        let elapsed = start.elapsed();

        let gb = data.len() as f64 * iters as f64 / 1024.0 / 1024.0 / 1024.0;
        let gbps = gb / elapsed.as_secs_f64();
        println!("{gbps:.3} GB/s");
    }
}
