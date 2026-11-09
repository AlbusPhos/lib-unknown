// SPDX-License-Identifier: MPL-2.0

use super::core::Key;
use crate::crypto::core::xor::f_xor_u8;
use crate::errors::*;
use crate::hide_array_lazy;
use crate::lazy::LazyLock;

static CONSTANTS: LazyLock<[u32; 4]> =
    hide_array_lazy!(u32, [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);

/// 四分之一轮运算 - ChaCha20的核心操作单元
#[cfg_attr(all(coin84, coin60), cold)]
#[cfg_attr(coin20, inline(never))]
#[cfg_attr(not(coin20), inline(always))]
fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(16);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(12);

    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(8);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(7);
}

/// ChaCha20 块函数 - 生成一个512bit(64字节)的密钥流块
///
/// # 参数
/// - key: 256位密钥,表示为8个u32
/// - counter: 32位块计数器
/// - nonce: 96位随机数,表示为3个u32
#[cfg_attr(all(coin14, coin16), cold)]
#[cfg_attr(coin26, inline(never))]
#[cfg_attr(not(coin26), inline(always))]
fn chacha20_block(key: &[u32; 8], counter: u32, nonce: &[u32; 3]) -> [u32; 16] {
    let mut state: [u32; 16] = [
        CONSTANTS[0],
        CONSTANTS[1],
        CONSTANTS[2],
        CONSTANTS[3],
        key[0],
        key[1],
        key[2],
        key[3],
        key[4],
        key[5],
        key[6],
        key[7],
        counter,
        nonce[0],
        nonce[1],
        nonce[2],
    ];

    let initial_state = state;

    for _ in 0..10 {
        quarter_round(&mut state, 0, 4, 8, 12);
        quarter_round(&mut state, 1, 5, 9, 13);
        quarter_round(&mut state, 2, 6, 10, 14);
        quarter_round(&mut state, 3, 7, 11, 15);

        quarter_round(&mut state, 0, 5, 10, 15);
        quarter_round(&mut state, 1, 6, 11, 12);
        quarter_round(&mut state, 2, 7, 8, 13);
        quarter_round(&mut state, 3, 4, 9, 14);
    }

    for i in 0..16 {
        state[i] = state[i].wrapping_add(initial_state[i]);
    }

    state
}

#[cfg_attr(all(coin54, coin56), cold)]
#[cfg_attr(coin76, inline(never))]
#[cfg_attr(not(coin76), inline(always))]
fn serialize_block(state: &[u32; 16]) -> [u8; 64] {
    let mut output = [0u8; 64];
    #[cfg(coin4)]
    let n = core::hint::black_box(16);
    #[cfg(not(coin4))]
    let n = 16;

    for i in 0..n {
        output[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_le_bytes());
    }
    output
}

#[cfg_attr(coin36, inline(never))]
#[cfg_attr(not(coin36), inline(always))]
fn bytes_to_u32_array_8(bytes: &[u8; 32]) -> [u32; 8] {
    let mut result = [0u32; 8];
    for i in 0..8 {
        result[i] = u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]);
    }
    result
}

#[cfg_attr(coin6, inline(never))]
#[cfg_attr(not(coin6), inline(always))]
fn bytes_to_u32_array_3(bytes: &[u8; 12]) -> [u32; 3] {
    let mut result = [0u32; 3];
    for i in 0..3 {
        result[i] = u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]);
    }
    result
}

/// 通用 ChaCha20：对 `in_place` 中的数据以 `initial_counter` 为起始块计数做流加密/解密，
/// 由调用方传入具体异或实现 `cipher`（用于编译期多态轮转选择）。
///
/// # Errors
///
/// - 仅当 `key`/`nonce` 转换为 `Key<32>` / `Key<12>` 失败时返回
///   [`ErrType::InvalidKeyLength`]；
///   本实现统一以宽松模式转换，实际不会触发。
#[cfg_attr(coin67, inline(never))]
#[cfg_attr(not(coin67), inline(always))]
pub fn chacha20<K: Key<32>, N: Key<12>>(
    input: &mut [u8],
    key: K,
    nonce: N,
    initial_counter: u32,
    cipher: fn(&mut [u8], &[u8; 64]),
) -> Result<()> {
    let k = key.into_key(true)?;
    let n = nonce.into_key(true)?;
    let key_words = bytes_to_u32_array_8(&k);
    let nonce_words = bytes_to_u32_array_3(&n);

    let mut block_counter = initial_counter;

    for chunk in input.chunks_mut(64) {
        let block_state = chacha20_block(&key_words, block_counter, &nonce_words);
        let key_stream = serialize_block(&block_state);
        cipher(chunk, &key_stream);
        block_counter = block_counter.wrapping_add(1);
    }

    Ok(())
}

/// 标准异或实现的 ChaCha20（counter=0 起）。
///
/// # Errors
///
/// - 仅当 `key`/`nonce` 转换失败时返回
///   [`ErrType::InvalidKeyLength`]；宽松模式下不会触发。
#[cfg_attr(coin64, cold)]
#[cfg_attr(coin0, inline(never))]
#[cfg_attr(not(coin0), inline(always))]
pub fn chacha20_0<K: Key<32>, N: Key<12>>(input: &mut [u8], key: K, nonce: N) -> Result<()> {
    chacha20(input, key, nonce, 0, |c, k| {
        for (byte, k) in c.iter_mut().zip(k.iter()) {
            *byte ^= k;
        }
    })
}

/// 等价于 [`chacha20_0`]，异或以替身实现 `f_xor_u8` 完成以规避典型 `xor` 指令模式。
///
/// # Errors
///
/// - 仅当 `key`/`nonce` 转换失败时返回
///   [`ErrType::InvalidKeyLength`]；宽松模式下不会触发。
#[cfg_attr(coin49, cold)]
#[cfg_attr(coin125, inline(never))]
#[cfg_attr(not(coin125), inline(always))]
pub fn chacha20_1<K: Key<32>, N: Key<12>>(input: &mut [u8], key: K, nonce: N) -> Result<()> {
    chacha20(input, key, nonce, 0, |c, k| {
        for (byte, k) in c.iter_mut().zip(k.iter()) {
            *byte = f_xor_u8(*byte, *k);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc8439_block_function() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];

        let nonce: [u8; 12] = [
            0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00,
        ];

        let counter: u32 = 1;

        let key_words = bytes_to_u32_array_8(&key);
        let nonce_words = bytes_to_u32_array_3(&nonce);
        let block = chacha20_block(&key_words, counter, &nonce_words);
        let output = serialize_block(&block);

        let expected: [u8; 64] = [
            0x10, 0xf1, 0xe7, 0xe4, 0xd1, 0x3b, 0x59, 0x15, 0x50, 0x0f, 0xdd, 0x1f, 0xa3, 0x20,
            0x71, 0xc4, 0xc7, 0xd1, 0xf4, 0xc7, 0x33, 0xc0, 0x68, 0x03, 0x04, 0x22, 0xaa, 0x9a,
            0xc3, 0xd4, 0x6c, 0x4e, 0xd2, 0x82, 0x64, 0x46, 0x07, 0x9f, 0xaa, 0x09, 0x14, 0xc2,
            0xd7, 0x05, 0xd9, 0x8b, 0x02, 0xa2, 0xb5, 0x12, 0x9c, 0xd1, 0xde, 0x16, 0x4e, 0xb9,
            0xcb, 0xd0, 0x83, 0xe8, 0xa2, 0x50, 0x3c, 0x4e,
        ];

        assert_eq!(output, expected, "ChaCha20 block function 测试失败!");
    }

    #[test]
    fn test_chacha20_xor_roundtrip() {
        let key = [0x5au8; 32];
        let nonce = [0xa5u8; 12];
        let original_text = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

        let mut buffer = original_text.to_vec();

        chacha20_0(&mut buffer, key, nonce).unwrap();
        assert_ne!(&buffer[..], &original_text[..]);

        chacha20_1(&mut buffer, key, nonce).unwrap();
        assert_eq!(&buffer[..], &original_text[..]);
    }

    #[test]
    #[cfg(all(feature = "std", feature = "rand"))]
    fn test_bench() {
        use core::hint::black_box;
        const SIZE: usize = 1024 * 111;
        const I: u64 = 30;
        let mb = SIZE as f64 / 1024.0 / 1024.0 * I as f64;
        let mut buf = vec![0u8; SIZE];
        let mut key = [0u8; 32];
        crate::rand::core::fill_bytes(buf.as_mut_slice());
        crate::rand::core::fill_bytes(key.as_mut_slice());

        let mut buf2 = buf.clone();

        let s = std::time::Instant::now();

        for _ in 0..black_box(I) {
            chacha20_1(&mut buf2, key, [0u8; 12]).unwrap();
        }

        println!("chacha20_1: {:.3} mb/s", mb / s.elapsed().as_secs_f64());
        let s = std::time::Instant::now();

        for _ in (0..black_box(I)).rev() {
            chacha20_1(&mut buf2, key, [0u8; 12]).unwrap();
        }

        println!("chacha20_1: {:.3} mb/s", mb / s.elapsed().as_secs_f64());

        assert_eq!(buf, buf2);
    }
}
