// SPDX-License-Identifier: MPL-2.0

//! 轻量多泳道乘法链哈希与无 std 定容哈希表。

#![no_std]
#![cfg(feature = "hash-core")]

// 单元测试与性能基准需要 std 的时间、vec 与 println 宏；库本身仍保持 no_std。
#[cfg(test)]
#[macro_use]
extern crate std;

#[allow(unused_imports)]
use lib_unknown_polymorphism::poly;

use lib_unknown_hide::hide_array_lazy;

/// 无 std 定容开放寻址哈希表（线性探测）。
#[cfg(feature = "hash-hashmap")]
pub mod hashmap;

hide_array_lazy!(static C : u64 = [0x9e3779b97f4a7c15, 0xbf58476d1ce4e5b9, 31, 13, 27, 41]);

/// 32 字节分块的多泳道乘法链哈希器（兼容 `core::hash::Hasher`）。
pub struct LightHasher {
    h: [u64; 4],
    buf: [u8; 32],
    buf_len: u8,
    size: u64,
}

impl LightHasher {
    /// 以显式种子构造哈希器；
    #[poly(inline = 0.7, cold = 0.05)]
    pub fn new(seed: u64) -> Self {
        hide_array_lazy!(static C : u64 = [0x85ebca6b, 0xc2b2ae35, 0x27d4eb2f]);
        let [c1, c2, c3] = *C;
        Self {
            h: [
                seed,
                seed.wrapping_add(c1),
                seed.wrapping_add(c2),
                seed.wrapping_add(c3),
            ],
            buf: [0; 32],
            buf_len: 0,
            size: 0,
        }
    }
    #[poly(inline = 0.85)]
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

    #[poly(inline = 0.85)]
    fn round(h: &mut [u64; 4], v: [u64; 4]) {
        let [c1, c2, c3, _, _, _] = *C;
        for i in 0..4 {
            h[i] = h[i]
                .wrapping_add(v[i].wrapping_add(c2))
                .rotate_left(c3 as u32)
                .wrapping_mul(c2)
                .rotate_left(c3 as u32)
                .wrapping_mul(c1)
                .wrapping_mul(h[i] | 3);
        }
    }
    #[poly]
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

    #[poly(inline = 0.85)]
    pub fn hash_with_seed<T: core::hash::Hash + ?Sized>(val: &T, seed: u64) -> u64 {
        use core::hash::Hasher;
        let mut hasher = Self::new(seed);
        val.hash(&mut hasher);
        hasher.finish()
    }
    #[poly(inline = 0.85)]
    pub fn hash<T: core::hash::Hash + ?Sized>(val: &T) -> u64 {
        use core::hash::Hasher;
        let mut hasher = Self::new(0);
        val.hash(&mut hasher);
        hasher.finish()
    }
}

impl core::hash::Hasher for LightHasher {
    #[poly(inline = 0.6)]
    fn finish(&self) -> u64 {
        let mut h = self.h;
        let [c1, c2, c3, c4, c5, c6] = *C;

        if self.buf_len > 0 {
            let mut remaining = [0_u8; 32];
            let len = self.buf_len as usize;
            remaining[..len].copy_from_slice(&self.buf[..len]);
            let v = Self::read_u64_x4(&remaining);
            Self::round(&mut h, v);
        }

        let combined = h[0]
            ^ h[1].rotate_left(c4 as u32)
            ^ h[2].rotate_left(c5 as u32)
            ^ h[3].rotate_left(c6 as u32)
            ^ !h[0].wrapping_add(h[1].wrapping_add(h[2].wrapping_add(h[3])));
        combined
            .wrapping_add(self.size)
            .wrapping_mul(c1)
            .rotate_left(c3 as u32)
            .wrapping_mul(c2)
    }

    #[poly(inline = 0.9)]
    fn write(&mut self, bytes: &[u8]) {
        self.write_bytes(bytes);
    }
}

#[derive(Clone, Copy, Default)]
pub struct LightBuildHasher {
    pub seed: u64,
}

impl LightBuildHasher {
    #[inline(always)]
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }
}

impl core::hash::BuildHasher for LightBuildHasher {
    type Hasher = LightHasher;
    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        LightHasher::new(self.seed)
    }
}
#[cfg(test)]
mod tests {
    use crate::{LightBuildHasher, LightHasher};

    #[test]
    fn test_hash() {
        #[derive(core::hash::Hash)]
        struct User {
            id: u32,
            name: &'static str,
        }
        let _h1: u64 = LightHasher::hash(&42_u64);
        let _h2: u64 = LightHasher::hash("hello world");
        let _h3: u64 = LightHasher::hash(&[1_u8, 2, 3, 4]);

        let user = User {
            id: 1,
            name: "Alice",
        };
        let _h4: u64 = LightHasher::hash_with_seed(&user, 0x1234_5678);

        use core::hash::BuildHasher;
        let builder = LightBuildHasher::new(0);
        let _h5: u64 = builder.hash_one(&user);
        let _h6: u64 = builder.hash_one("hello world");

        assert_eq!(
            LightHasher::hash("hello world"),
            builder.hash_one("hello world")
        );
        assert_ne!(LightHasher::hash(&0), 0);
    }

    #[test]
    #[ignore = "performance benchmark"]
    fn light_hasher() {
        use super::*;
        use core::hash::Hasher;
        use std::time::Instant;
        let data = std::vec![128; 1024 * 1024];
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

lib_unknown_polymorphism::poly_track_env!();
