// src/crypto/core/inv_mul.rs
//! 定宽模乘逆元：对奇数 `m` 计算 `m⁻¹ mod 2^BITS`，供整数混合与分组密码轮函数使用。
//!
//! 各函数采用 Newton–Raphson 迭代，初始近似值保证低 5 位正确，随后每轮精度翻倍，
//! 因此迭代次数固定为 `ceil(log2(BITS / 5))` 量级。输入为偶数或零时内部先按 `m | 1`
//! 归一，结果语义由调用方保证。

macro_rules! impl_inv_mul {
    ($name:ident, $t:ty) => {
        /// 计算 `$t` 宽度的模乘逆元（`m` 为奇数；零/偶数输入结果未定义，由调用方保证）。
        ///
        /// 返回值满足 `m.wrapping_mul(result) == 1`（模 `2^BITS`）。
        #[cfg_attr(coin7, cold)]
        #[cfg_attr(all(coin59, coin37), inline(never))]
        #[cfg_attr(not(all(coin59, coin37)), inline(always))]
        pub fn $name(m: $t) -> $t {
            let m = m | 1;
            let mut inv = m.wrapping_mul(3) ^ 2;
            let mut bits = 5u32;
            while bits < <$t>::BITS {
                inv = inv.wrapping_mul((2 as $t).wrapping_sub(m.wrapping_mul(inv)));
                bits *= 2;
            }
            inv
        }
    };
}

impl_inv_mul!(inv_mul8, u8);
impl_inv_mul!(inv_mul16, u16);
impl_inv_mul!(inv_mul32, u32);
impl_inv_mul!(inv_mul64, u64);
impl_inv_mul!(inv_mul128, u128);
