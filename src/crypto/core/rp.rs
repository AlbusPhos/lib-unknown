// SPDX-License-Identifier: MPL-2.0
// src/crypto/core/rp.rs
//! 不可逆随机映射（random permutation）函数族：
//! 以隐藏常量 + 半宽拆分 + 乘法链 mix 混合构造的单向折叠，无逆函数，
//! 常用于洗牌与混淆路径中的种子派生。
//!
//! 模板参数常量经 `hide_array!` 加掩码后于 `.rodata` 以密文形态存放。

/// 公共核心：返回 (mix 后的 l, mix 后的 h, o)
macro_rules! rp_core {
    (
        $x:expr, $ty:ty,
        $half_shift:expr, $mask:expr, $add_c:expr,
        $mul1:expr, $mul2:expr,
        $s1:expr, $s2:expr, $s3:expr
    ) => {{
        let x: $ty = $x;

        let [low_mask, add_c, mul_1, mul_2] =
            $crate::hide_array!($ty, [$mask, $add_c, $mul1, $mul2]);
        let [half_shift, s1, s2, s3] = $crate::hide_array!(u32, [$half_shift, $s1, $s2, $s3]);

        let low = x & low_mask;
        let high = x >> half_shift;

        let mut l = low
            .wrapping_add(add_c)
            .wrapping_mul(mul_1)
            .wrapping_mul(x | 1);
        let mut h = high
            .wrapping_add(add_c)
            .wrapping_mul(mul_1)
            .wrapping_mul(l | 1);

        let o = (x ^ l ^ h) ^ l.wrapping_mul(!h | 1);

        #[cfg_attr(coin96, cold)]
        #[cfg_attr(any(coin31, coin76), inline(never))]
        #[cfg_attr(not(any(coin31, coin76)), inline(always))]
        const fn mix(mut v: $ty, mul_1: $ty, mul_2: $ty, s1: u32, s2: u32, s3: u32) -> $ty {
            v ^= v >> s1;
            v = v.wrapping_mul(mul_1);
            v ^= v >> s2;
            v = v.wrapping_mul(mul_2);
            v ^= v >> s3;
            v
        }

        l = mix(l, mul_1, mul_2, s1, s2, s3);
        h = mix(h, mul_1, mul_2, s1, s2, s3);

        (l, h, o)
    }};
}
macro_rules! impl_rp {
    (@combine $lo:ident, $hi:ident, $ty:ty, [$elem:ty; 2]) => {
        [$lo, $hi]
    };
    (@combine $lo:ident, $hi:ident, $ty:ty, $out_ty:ty) => {
        (($hi as $out_ty) << <$ty>::BITS) | ($lo as $out_ty)
    };

    ($(
        $(#[$meta:meta])*
        $fn_name:ident $(, $rpp_name:ident -> $out_ty:tt)? : $ty:ty => (
            $half_shift:expr,
            $mask:expr,
            $add_c:expr,
            $mul1:expr,
            $mul2:expr,
            $s1:expr,
            $s2:expr,
            $s3:expr
        )
    );* $(;)?) => {
        $(
            $(#[$meta])*
            #[doc = concat!(
                "`", stringify!($fn_name), "` 的一倍宽度不可逆混合映射，输出与输入同宽。"
            )]
            #[cfg_attr(any(coin3,coin63), cold)]
            #[cfg_attr(coin17, inline(never))]
            #[cfg_attr(not(coin17), inline(always))]
            pub const fn $fn_name(x: $ty) -> $ty {
                let (l, h, o) = rp_core!(
                    x, $ty, $half_shift, $mask, $add_c,
                    $mul1, $mul2, $s1, $s2, $s3
                );
                !l ^ h ^ o
            }

            $(
                #[doc = concat!(
                    "两倍宽度版本，不折叠：原生整数返回 `(hi << ", stringify!($ty), "::BITS) | lo`，",
                    "数组类型返回 `[lo, hi]`；其中 `lo = !l ^ o`，`hi = h ^ o`。逻辑与 [`", stringify!($fn_name), "`] 相同。"
                )]
                #[cfg_attr(any(coin110,coin86), cold)]
                #[cfg_attr(all(coin42,coin117), inline(never))]
                #[cfg_attr(not(all(coin42,coin117)), inline(always))]
                pub const fn $rpp_name(x: $ty) -> $out_ty {
                    let (l, h, o) = rp_core!(
                        x, $ty, $half_shift, $mask, $add_c,
                        $mul1, $mul2, $s1, $s2, $s3
                    );
                    let lo = !l ^ o;
                    let hi = h ^ o;
                    impl_rp!(@combine lo, hi, $ty, $out_ty)
                }
            )?
        )*
    };
}

impl_rp! {
    rp8, rpp8 -> u16 : u8 => (
        4, 0x0f,
        0x11, 0xbf, 0x95,
        4, 3, 4
    );
    rp16, rpp16 -> u32 : u16 => (
        8, 0x00ff,
        0x116a, 0xbf59, 0x94d1,
        7, 6, 8
    );
    rp32, rpp32 -> u64 : u32 => (
        16, 0x0000_ffff,
        0x116a_3b35, 0xbf58_476d, 0x94d0_49bb,
        15, 13, 16
    );
    rp64, rpp64 -> u128 : u64 => (
        32, 0x0000_0000_ffff_ffff,
        0x116a_3b35_fc84_5a8e, 0xbf58_476d_1ce4_e5b9, 0x94d0_49bb_1331_11eb,
        30, 27, 31
    );
    rp128, rpp128 -> [u128; 2] : u128 => (
        64, 0x0000_0000_0000_0000_ffff_ffff_ffff_ffff,
        0x116a_3b35_fc84_5a8e_cd7f_2116_a3b3_5fc8,
        0x2360_ed05_1fc6_5da4_4385_df64_9fcc_f645,
        0x5851_f42d_4c95_7f2d_1405_7b7e_f767_814f,
        60, 57, 61
    );
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "std")]
    fn test_mix64() {
        use super::*;

        let t = std::time::Instant::now();
        let l = 10000;
        for i in 0..std::hint::black_box(l) {
            std::hint::black_box(rp64(i));
        }
        println!("avg {:?} ns", t.elapsed().as_nanos() / l as u128);

        println!("rp64(0) {}", rp64(0));
        println!("rpp64(0) {:#034x}", rpp64(0));
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_mix64_and_128() {
        use super::*;

        println!("rp64(0)  {}", rp64(0));
        println!("rpp64(0) {:#034x}", rpp64(0));

        let [lo, hi] = rpp128(0);
        println!("rp128(0)  {:#034x}", rp128(0));
        println!("rpp128(0) lo={:#034x}, hi={:#034x}", lo, hi);
    }
}
