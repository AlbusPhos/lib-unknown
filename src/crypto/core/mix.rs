// SPDX-License-Identifier: MPL-2.0
//! 可逆雪崩映射

use crate::hide_array_lazy;

macro_rules! impl_mix {
    ($(
        $(#[$meta:meta])*
        $fn_name:ident, $ty:ty => ($s1:expr, $mul1:expr, $s2:expr, $mul2:expr, $s3:expr)
    );* $(;)?) => {
        $(
            $(#[$meta])*
/// 乘法混合函数（由宏实例化为各宽度版本）：将输入经两次乘法扩散后折叠。
            #[cfg_attr(all(coin52,coin104), cold)]
            #[cfg_attr(any(coin23,coin86), inline(never))]
            #[cfg_attr(not(any(coin23,coin86)), inline(always))]
            pub fn $fn_name(mut x: $ty) -> $ty {
                hide_array_lazy!(static MULS: $ty = [$mul1, $mul2]);
                hide_array_lazy!(static SHIFTS: u32 = [$s1, $s2, $s3]);

                let [mul_1, mul_2] = *MULS;
                let [s1, s2, s3] = *SHIFTS;

                x ^= x >> s1;
                x = x.wrapping_mul(mul_1);
                x ^= x >> s2;
                x = x.wrapping_mul(mul_2);
                x ^= x >> s3;
                x
            }
        )*
    };
}

impl_mix! {
    mix8, u8 => (4, 0xbf, 3, 0x95, 4);
    mix16, u16 => (7, 0xbf59, 6, 0x94d1, 8);
    mix32, u32 => (15, 0xbf58476d, 13, 0x94d049bb, 16);
    mix64, u64 => (30, 0xbf58476d1ce4e5b9, 27, 0x94d049bb133111eb, 31);
    mix128, u128 => (
        60,
        0x2360ed051fc65da44385df649fccf645,
        57,
        0x5851f42d4c957f2d14057b7ef767814f,
        61
    );
}
