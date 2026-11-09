// SPDX-License-Identifier: MPL-2.0
// src/crypto/mse2.rs

use core::hint::black_box;

use super::core::boxs::{DYB, P1, P2, S1, S2};
use super::core::mix::mix64;
use super::core::padding::{pkcs7_pad, pkcs7_unpad};
use super::core::rotate::{f_rotate_left_u64, f_rotate_right_u64};
use super::core::rp::{rp64, rpp64, rpp128};
use super::core::{Key, shuffle_with};

use crate::errors::*;

#[cfg_attr(coin19, cold)]
#[cfg_attr(coin71, inline(never))]
#[cfg_attr(not(coin71), inline(always))]
fn inv_mul64(m: u64) -> u64 {
    let m = m | 1;
    let mut inv = m.wrapping_mul(3) ^ 2; // 5 bits
    inv = inv.wrapping_mul(2u64.wrapping_sub(m.wrapping_mul(inv))); // 10 bits
    inv = inv.wrapping_mul(2u64.wrapping_sub(m.wrapping_mul(inv))); // 20 bits
    inv = inv.wrapping_mul(2u64.wrapping_sub(m.wrapping_mul(inv))); // 40 bits
    inv = inv.wrapping_mul(2u64.wrapping_sub(m.wrapping_mul(inv))); // 80 >= 64 bits
    inv
}

/// 从 `buf` 的 `offset` 处按小端读取一个 `u64`。
///
/// # Panics
///
/// - 当 `offset + 8 > buf.len()` 时 panic。本模块所有调用点均在
///   已按块尺寸（64 字节）切片的数据上调用，长度条件由切片保证。
#[inline(always)]
fn read_u64_le(buf: &[u8], offset: usize) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(arr)
}

#[inline(always)]
fn write_u64_le(buf: &mut [u8], offset: usize, val: u64) {
    buf[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
}

struct Mse2<const N: usize> {
    s: [u64; 4],
    prev: u64,
    sb: DYB<256>,
    pb: DYB<8>,
}

impl<const N: usize> Mse2<N> {
    #[cfg_attr(coin38, cold)]
    #[cfg_attr(coin82, inline(never))]
    #[cfg_attr(not(coin82), inline(always))]
    fn new(k: &[u8; 32], nonce: u64) -> Self {
        crate::hide_array_lazy! {
            static C64: u64 = [
                0x6A09_E667_F3BC_C908,
                0xBB67_AE85_84CA_A73B,
                0x9E37_79B9_7F4A_7C15,
            ];
        }
        let &[iv0, iv1, golden] = &*C64;

        let mut acc0 = nonce ^ iv0;
        let mut acc1 = f_rotate_left_u64(nonce, 29) ^ iv1;

        for i in 0..32usize {
            let w = read_u64_le(k, (i & 3) * 8);
            let box_mask = (S2[(i * 13 + (nonce as usize)) & 0xFF] as u64)
                | ((P2[i & 7] as u64) << 8)
                | ((i as u64) << 32);
            acc0 = rp64(acc0 ^ w.wrapping_add(box_mask));
            acc1 = acc1.wrapping_add(mix64(w ^ acc0 ^ box_mask));
        }

        let seed = ((acc1 as u128) << 64) | (acc0 as u128);
        let [x1, x2] = rpp128(seed);

        let mut state = Self {
            s: [
                x1 as u64,               // x
                (x1 >> 64) as u64,       // a
                ((x2 >> 64) as u64) | 1, // weyl
                (x2 as u64) | 1,         // s
            ],
            prev: mix64(nonce.wrapping_add(golden)),
            sb: DYB::new(S1),
            pb: DYB::new(P1),
        };

        for _ in 0..4 {
            state.round();
        }
        state
    }

    #[inline(always)]
    fn mix1(&mut self) {
        self.s[0] = mix64(self.s[0]);
        self.s[1] = mix64(self.s[1]).wrapping_add(self.s[0]);
        let x = (self.s[0] >> 1).wrapping_mul(self.s[1] | 1);
        self.s[3] = mix64(self.s[3]) | 1;
        self.s[2] = mix64(self.s[2]).wrapping_add(self.s[3]);
        self.s[0] = x ^ self.s[2];
    }

    #[inline(always)]
    fn mix2(&mut self, a: usize, b: usize, c: usize, d: usize) {
        self.s[a] = self.s[a].wrapping_add(self.s[b]);
        self.s[d] ^= self.s[a];
        self.s[d] = self.s[d].rotate_left(32);

        self.s[c] = self.s[c].wrapping_add(self.s[d]);
        self.s[b] ^= self.s[c];
        self.s[b] = self.s[b].rotate_left(24);

        self.s[a] = self.s[a].wrapping_add(self.s[b]);
        self.s[d] ^= self.s[a];
        self.s[d] = self.s[d].rotate_left(16);

        self.s[c] = self.s[c].wrapping_add(self.s[d]);
        self.s[b] ^= self.s[c];
        self.s[b] = self.s[b].rotate_left(63);
    }

    #[inline(always)]
    fn mix3(&mut self) {
        let seed128 = rpp64(self.prev ^ self.s[0].wrapping_add(self.s[2]));
        self.s = shuffle_with(self.s, seed128);

        let dyn_seed = (seed128 as u64) ^ ((seed128 >> 32) as u64) ^ self.s[1];
        self.sb.swap_by_idx(
            (dyn_seed as usize) & 0xFF,
            ((dyn_seed >> 8) as usize) & 0xFF,
        );
        self.sb
            .swap_by_val((dyn_seed >> 16) as u8, (dyn_seed >> 24) as u8);
        self.pb.swap_by_idx(
            ((dyn_seed >> 32) as usize) & 7,
            ((dyn_seed >> 40) as usize) & 7,
        );

        let parity = self.s[0] ^ self.s[1] ^ self.s[2] ^ self.s[3];
        let d0 = self.s[0] ^ f_rotate_left_u64(parity, 11);
        let d1 = self.s[1] ^ f_rotate_left_u64(parity, 22);
        let d2 = self.s[2] ^ f_rotate_left_u64(parity, 33);
        let d3 = self.s[3] ^ f_rotate_left_u64(parity, 44);

        self.s[0] = self.pb.apply_p64(self.sb.apply_s64(d0));
        self.s[1] = self.sb.apply_s64(d1).rotate_left(17) ^ self.s[0];
        self.s[2] = self.pb.apply_p64(self.sb.apply_s64(d2));
        self.s[3] = (self.sb.apply_s64(d3).rotate_left(41) ^ self.s[2]) | 1;
    }

    #[cfg_attr(coin66, cold)]
    #[cfg_attr(coin37, inline(never))]
    #[cfg_attr(not(coin37), inline(always))]
    fn round(&mut self) {
        let expanded = rpp64(self.prev);
        let e0 = expanded as u64;
        let e1 = (expanded >> 64) as u64;

        self.s[0] ^= e0;
        self.s[1] = self.s[1].wrapping_add(e1);
        self.s[2] ^= f_rotate_left_u64(self.prev, 19);
        self.s[3] = self.s[3].wrapping_add(e0 ^ e1) | 1;

        let initial = self.s;

        self.mix1();
        self.mix2(0, 1, 2, 3);
        self.mix3();

        self.mix1();
        self.mix2(0, 2, 1, 3);
        self.mix3();

        for (s_val, ini) in self.s.iter_mut().zip(initial) {
            *s_val = s_val.wrapping_add(ini);
        }
        self.s[3] |= 1;

        self.prev = mix64(
            self.prev ^ self.s[0].wrapping_add(self.s[2]) ^ self.s[1].wrapping_mul(self.s[3]),
        );
    }

    #[cfg_attr(coin45, cold)]
    #[cfg_attr(coin61, inline(never))]
    #[cfg_attr(not(coin61), inline(always))]
    fn next_key_block(&mut self) -> [u64; 8] {
        let mut bk = [0u64; 8];

        for i in 0..black_box(N.saturating_add(1)) {
            self.round();
            let c = (i as u64) | 1;
            let c3 = (i as u64) | 3;

            bk[0] ^= self.s[0].wrapping_add(self.s[2]);
            bk[1] ^= self.s[2].wrapping_add(self.s[3]);
            bk[2] ^= mix64(self.s[0].wrapping_add(c));
            bk[3] ^= mix64(self.s[1].wrapping_add(c3));
            bk[4] ^= mix64(self.s[2].wrapping_add(c3));
            bk[5] ^= mix64(self.s[3].wrapping_add(c3));
            bk[6] ^= self.s[2].wrapping_mul(self.s[3]);
            bk[7] ^= self.s[1].wrapping_mul(self.s[0]);

            shuffle_with(
                &mut bk,
                (self.s[0] ^ self.s[1] ^ self.s[2] ^ self.s[3] ^ self.prev ^ (i as u64)) as u128,
            );
        }

        bk
    }

    #[inline(always)]
    fn fold_ciphertext_block(&mut self, ct: &[u64; 8]) {
        let a = ct[0].wrapping_add(ct[4]) ^ f_rotate_left_u64(ct[2], 17);
        let b = ct[1].wrapping_add(ct[5]) ^ f_rotate_left_u64(ct[3], 39);
        let d = ct[6].wrapping_mul(ct[7] | 1) ^ self.prev;
        self.prev = rp64(a ^ b.wrapping_mul(d | 1));
    }

    #[cfg_attr(coin52, cold)]
    #[cfg_attr(coin79, inline(never))]
    #[cfg_attr(not(coin79), inline(always))]
    fn encrypt_block_64(&mut self, mut w: [u64; 8]) -> [u64; 8] {
        let ks = self.next_key_block();

        for i in 0..4 {
            let e = i * 2;
            let o = e + 1;

            w[e] = w[e].wrapping_add(ks[e]) ^ ks[(e + 4) & 7];
            w[e] = w[e].wrapping_mul(inv_mul64(ks[o] | 1));

            w[o] = w[o].wrapping_add(ks[o]) ^ ks[(o + 4) & 7];
            w[o] = w[o].wrapping_mul(ks[(o + 1) & 7] | 1);
        }

        for i in 0..4 {
            let l = i * 2;
            let r = l + 1;
            w[l] = w[l].wrapping_add(w[r].rotate_left(23) ^ ks[r]);
            w[r] ^= w[l].rotate_left(37);
        }
        for i in 0..4 {
            let l = i;
            let r = ((i + 1) & 3) + 4;
            w[l] = w[l].wrapping_add(w[r].rotate_left(19) ^ ks[l]);
            w[r] ^= w[l].rotate_left(41);
        }

        for i in 0..4 {
            let l = i;
            let r = ((i + 3) & 3) + 4;
            w[l] = w[l].wrapping_add(w[r].rotate_left(11) ^ ks[(l + 4) & 7]);
            w[r] ^= w[l].rotate_left(47);
        }

        for (i, slot) in w.iter_mut().enumerate() {
            let s_val = self.sb.apply_s64(*slot);
            let shift = (i * 8) as u32;
            let b = ((s_val >> shift) & 0xFF) as u8;
            let pb = self.pb.apply_p8(b) as u64;
            *slot = (s_val & !(0xFFu64 << shift)) | (pb << shift);
        }

        w[0] = f_rotate_left_u64(w[0], (ks[7] as u32) & 63);
        w[4] = f_rotate_left_u64(w[4], ((ks[3] >> 32) as u32) & 63);

        for (i, slot) in w.iter_mut().enumerate() {
            *slot ^= ks[(i + 2) & 7].wrapping_add(self.prev);
        }

        self.fold_ciphertext_block(&w);

        w
    }

    #[cfg_attr(coin53, cold)]
    #[cfg_attr(coin80, inline(never))]
    #[cfg_attr(not(coin80), inline(always))]
    fn decrypt_block_64(&mut self, mut w: [u64; 8]) -> [u64; 8] {
        let ct_copy = w;
        let ks = self.next_key_block();

        for i in 0..8 {
            w[i] ^= ks[(i + 2) & 7].wrapping_add(self.prev);
        }

        w[4] = f_rotate_right_u64(w[4], ((ks[3] >> 32) as u32) & 63);
        w[0] = f_rotate_right_u64(w[0], (ks[7] as u32) & 63);

        for (i, slot) in w.iter_mut().enumerate() {
            let shift = (i * 8) as u32;
            let b = ((*slot >> shift) & 0xFF) as u8;
            let pb = self.pb.apply_inv_p8(b) as u64;
            let restored = (*slot & !(0xFFu64 << shift)) | (pb << shift);
            *slot = self.sb.apply_inv_s64(restored);
        }

        for i in (0..4).rev() {
            let l = i;
            let r = ((i + 3) & 3) + 4;
            w[r] ^= w[l].rotate_left(47);
            w[l] = w[l].wrapping_sub(w[r].rotate_left(11) ^ ks[(l + 4) & 7]);
        }

        for i in (0..4).rev() {
            let l = i;
            let r = ((i + 1) & 3) + 4;
            w[r] ^= w[l].rotate_left(41);
            w[l] = w[l].wrapping_sub(w[r].rotate_left(19) ^ ks[l]);
        }
        for i in (0..4).rev() {
            let l = i * 2;
            let r = l + 1;
            w[r] ^= w[l].rotate_left(37);
            w[l] = w[l].wrapping_sub(w[r].rotate_left(23) ^ ks[r]);
        }

        for i in 0..4 {
            let e = i * 2;
            let o = e + 1;

            w[e] = w[e].wrapping_mul(ks[o] | 1);
            w[e] ^= ks[(e + 4) & 7];
            w[e] = w[e].wrapping_sub(ks[e]);

            w[o] = w[o].wrapping_mul(inv_mul64(ks[(o + 1) & 7] | 1));
            w[o] ^= ks[(o + 4) & 7];
            w[o] = w[o].wrapping_sub(ks[o]);
        }

        self.fold_ciphertext_block(&ct_copy);

        w
    }
}

/// MSE2（64 字节块、32 字节密钥、64 位 nonce）：对 `src`（含尾部任意长度）
/// 做 PKCS#7 填充并逐块加密写入 `dest`，返回填充后的总长度。
///
/// # Errors
///
/// - 当 `pkcs7_pad` 的填充规则被违反或 `dest` 容量不足时，透传对应的错误变体；
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin60, cold)]
#[cfg_attr(coin37, inline(never))]
#[cfg_attr(not(coin37), inline(always))]
pub fn mse2<K: Key<32>>(src: &[u8], dest: &mut [u8], key: K, nonce: u64) -> Result<usize> {
    const BLK_SZ: usize = 64;

    let k = key.into_key(true)?;
    let total_len = pkcs7_pad(src, dest, BLK_SZ)?;

    let mut st: Mse2<4> = Mse2::new(&k, nonce);

    for chunk in dest[..total_len].as_chunks_mut::<BLK_SZ>().0 {
        let mut words = [0u64; 8];
        for (i, w_slot) in words.iter_mut().enumerate() {
            *w_slot = read_u64_le(chunk, i * 8);
        }

        let ct_words = st.encrypt_block_64(words);

        for (i, chunk_word) in ct_words.iter().enumerate() {
            write_u64_le(chunk, i * 8, *chunk_word);
        }
    }

    Ok(total_len)
}

/// [`mse2`] 的逆过程：`src` 必须为已填充密文（长度为 64 字节的整数倍且非空），
/// 解密最后一个块用于还原填充长度，前缀块逐块解密写入 `dest`，返回明文长度。
///
/// # Errors
///
/// - 当 `src` 为空或长度不是 64 的整数倍时分别返回
///   [`ErrType::InvalidLength`] /
///   [`ErrType::UnalignedByteLength`]；
/// - 当末块 PKCS#7 填充非法时返回 [`ErrType::InvalidEncoding`]；
/// - 当 `dest` 容量不足时返回 [`ErrType::BufferTooSmall`]；
/// - 仅当 `key` 转换失败时返回错误；宽松模式下不会触发。
#[cfg_attr(coin15, cold)]
#[cfg_attr(coin51, inline(never))]
#[cfg_attr(not(coin51), inline(always))]
pub fn imse2<K: Key<32>>(src: &[u8], dest: &mut [u8], key: K, nonce: u64) -> Result<usize> {
    const BLK_SZ: usize = 64;

    ensure!(
        !src.is_empty(),
        ErrType::InvalidLength {
            expected: BLK_SZ,
            actual: 0,
        }
    );
    ensure!(
        src.len().is_multiple_of(BLK_SZ),
        ErrType::UnalignedByteLength {
            byte_len: src.len(),
            elem_size: BLK_SZ,
        }
    );

    let full_blocks = src.len() / BLK_SZ;
    let prefix_blocks = full_blocks - 1;
    let prefix_len = prefix_blocks * BLK_SZ;

    ensure!(
        dest.len() >= prefix_len,
        ErrType::BufferTooSmall {
            required: prefix_len,
            available: dest.len(),
        }
    );

    let k = key.into_key(true)?;
    let mut st: Mse2<4> = Mse2::new(&k, nonce);

    for (src_chunk, dst_chunk) in src[..prefix_len]
        .as_chunks::<BLK_SZ>()
        .0
        .iter()
        .zip(dest[..prefix_len].as_chunks_mut::<BLK_SZ>().0)
    {
        let mut ct_words = [0u64; 8];
        for (i, slot) in ct_words.iter_mut().enumerate() {
            *slot = read_u64_le(src_chunk, i * 8);
        }

        let pt_words = st.decrypt_block_64(ct_words);

        for (i, slot) in pt_words.iter().enumerate() {
            write_u64_le(dst_chunk, i * 8, *slot);
        }
    }

    let last_src = &src[prefix_len..prefix_len + BLK_SZ];
    let mut ct_words = [0u64; 8];
    for (i, slot) in ct_words.iter_mut().enumerate() {
        *slot = read_u64_le(last_src, i * 8);
    }

    let pt_words = st.decrypt_block_64(ct_words);

    let mut last_blk = [0u8; 64];
    for (i, slot) in pt_words.iter().enumerate() {
        write_u64_le(&mut last_blk, i * 8, *slot);
    }

    let mut last_unpadded = [0u8; 64];
    let last_len = pkcs7_unpad(&last_blk, &mut last_unpadded, BLK_SZ)?;
    let out_len = prefix_len + last_len;

    ensure!(
        dest.len() >= out_len,
        ErrType::BufferTooSmall {
            required: out_len,
            available: dest.len(),
        }
    );

    dest[prefix_len..out_len].copy_from_slice(&last_unpadded[..last_len]);

    Ok(out_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inv_mul64() {
        let samples = [
            1u64,
            3,
            0x1234_5678_9ABC_DEF1,
            0xDEAD_BEEF_CAFE_BAB1,
            u64::MAX,
        ];
        for &m in &samples {
            let inv = inv_mul64(m);
            assert_eq!(m.wrapping_mul(inv), 1, "inv_mul64 failed for {:#x}", m);
        }
    }

    #[test]
    fn test_mse2_aligned_64b_blocks() {
        let key = 0xDEADBEEF_CAFEBABE_01234567_89ABCDEF_u128;
        let nonce = 0x1122_3344_5566_7788_u64;

        let mut pt = [0u8; 256]; // 4 个完整 64 字节块
        for (i, b) in pt.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(31).wrapping_add(7);
        }

        let mut ct = [0u8; 320]; // PKCS#7 填充后为 256 + 64 = 320 字节
        let mut dec = [0u8; 256];

        let ct_len = mse2(&pt, &mut ct, key, nonce).unwrap();
        assert_eq!(ct_len, 320);
        assert_ne!(&ct[..256], &pt[..]);

        let pt_len = imse2(&ct[..ct_len], &mut dec, key, nonce).unwrap();
        assert_eq!(pt_len, 256);
        assert_eq!(dec, pt);
    }

    #[test]
    fn test_mse2_unaligned() {
        let key = [0x5Au8; 64];
        let nonce = 0xA5A5_5A5A_F0F0_0F0F_u64;

        let mut pt = [0u8; 155];
        for (i, b) in pt.iter_mut().enumerate() {
            *b = (i as u8) ^ 0xCC;
        }

        let mut ct = [0u8; 192]; // 155 对齐到 64 字节后为 192 字节
        let mut dec = [0u8; 155];

        let ct_len = mse2(&pt, &mut ct, key, nonce).unwrap();
        assert_eq!(ct_len, 192);
        assert_ne!(&ct[..155], &pt[..]);

        let pt_len = imse2(&ct[..ct_len], &mut dec, key, nonce).unwrap();
        assert_eq!(pt_len, 155);
        assert_eq!(dec, pt);
    }

    #[test]
    fn test_mse2_intra_block_and_inter_block_avalanche() {
        let key = 0x99887766_55443322_1100FFEE_DDCCBBAA_u128;
        let nonce = 42u64;

        for flipped_word in 0..8 {
            let pt1 = [0xAAu8; 128];
            let mut pt2 = pt1;
            pt2[flipped_word * 8] ^= 1;

            let mut ct1 = [0u8; 192];
            let mut ct2 = [0u8; 192];

            mse2(&pt1, &mut ct1, key, nonce).unwrap();
            mse2(&pt2, &mut ct2, key, nonce).unwrap();

            for w_idx in 0..8 {
                let range = (w_idx * 8)..((w_idx + 1) * 8);
                assert_ne!(
                    &ct1[range.clone()],
                    &ct2[range],
                    "翻转字 {} 时，块内字 {} 未发生扩散！",
                    flipped_word,
                    w_idx
                );
            }

            assert_ne!(&ct1[64..128], &ct2[64..128], "64B 跨块 CFB 反馈未生效");
        }
    }

    #[test]
    #[cfg(all(feature = "std", feature = "rand"))]
    fn test_bench() {
        use core::hint::black_box;
        const SIZE: usize = 1024 * 111;
        const I: u64 = 30;
        let mb = SIZE as f64 / 1024.0 / 1024.0 * I as f64;
        let mut src = vec![0u8; SIZE];
        let mut key = [0u8; 64];
        crate::rand::core::fill_bytes(src.as_mut_slice());
        crate::rand::core::fill_bytes(key.as_mut_slice());

        let mut ct = vec![0u8; SIZE + 64];
        let mut dec = vec![0u8; SIZE];

        let s = std::time::Instant::now();
        let mut ct_len = 0;
        for i in 0..black_box(I) {
            ct_len = mse2(&src, &mut ct, key, i).unwrap();
        }
        println!("mse2: {:.3} mb/s", mb / s.elapsed().as_secs_f64());

        let s = std::time::Instant::now();
        for _ in 0..black_box(I) {
            imse2(&ct[..ct_len], &mut dec, key, I - 1).unwrap();
        }
        println!("imse2: {:.3} mb/s", mb / s.elapsed().as_secs_f64());

        assert_eq!(src, dec);
    }
}
