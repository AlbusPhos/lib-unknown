// SPDX-License-Identifier: MPL-2.0

#![no_std]

//! 编译期常量隐藏宏族：`const_random!` / `hide_array!` / `hide_array_lazy!`。

/// 在编译期生成一个由内部种子、调用位置与调用序号共同派生的伪随机整数。
///
/// 支持 `const_random!()`（默认 `u64`）、`const_random!(Type)` 与
/// `const_random!(Type, "salt")` 三种形式；结果可保存到 `const`/`static`。
///
/// 过程宏实现位于 `lib-unknown-polymorphism`，本 crate 转发以保持
/// `lib_unknown_hide::const_random!` 与 `hide_array!` 内部路径兼容。
pub use lib_unknown_polymorphism::const_random;

#[cfg(feature = "lazy")]
/// 惰性容器，供 `hide_array_lazy!` 展开使用。
pub use lib_unknown_lazy as lazy;

/// 将一个常量数组以 XOR 掩码形式存入 `.rodata`，并在运行时经 `black_box` 还原。
///
/// 适用于整数数组；返回值类型为 `[$t; N]`；const fn可用。
#[macro_export]
macro_rules! hide_array {
    ($t:ty, [$($val:expr),+ $(,)?]) => {{
        const __LEN: usize = $crate::hide_array!(@count $($val),+);
        static __MASK: $t = $crate::const_random!($t);

        static __ENC: [$t; __LEN] = const {
            let __orig: [$t; __LEN] = [ $( $val ),+ ];
            let mut __out = [0 as $t; __LEN];
            let mut __i = 0;
            while __i < __LEN {
                let __k = __MASK.wrapping_add((__i as $t).wrapping_mul(3));
                __out[__i] = __orig[__i] ^ __k;
                __i += 1;
            }
            __out
        };

        ::core::hint::black_box({
            let mut __out = ::core::hint::black_box(__ENC);
            let __mask = ::core::hint::black_box(__MASK);
            let mut __i = ::core::hint::black_box(0);
            while __i < ::core::hint::black_box(__LEN) {
                // SAFETY: `__out` 是本宏展开时声明的栈上 [$t; __LEN] 数组，
                // `__i` 由循环保证小于 `__LEN`，写入地址不越界，元素为 `Copy` 类型。
                unsafe {
                    *__out.as_mut_ptr().add(__i) ^= __mask.wrapping_add((__i as $t).wrapping_mul(3));
                }
                __i += 1;
            }
            __out
        })
    }};

    (@count $($val:expr),+) => {
        <[()]>::len(&[ $( $crate::hide_array!(@unit $val) ),+ ])
    };
    (@unit $val:expr) => { () };
}

/// 声明一个以 [`hide_array!`] 初始化、首次访问才还原的 `static LazyLock<[T; N]>`。
///
/// 需要启用 `"lazy"` 特性。
#[cfg(feature = "lazy")]
#[macro_export]
macro_rules! hide_array_lazy {
    (
        $(#[$meta:meta])*
        $vis:vis static $name:ident : $t:ty = [ $($val:expr),+ $(,)? ] $(;)?
    ) => {
        $(#[$meta])*
        $vis static $name: $crate::lazy::LazyLock<[$t; $crate::hide_array!(@count $($val),+)]> =
            $crate::hide_array_lazy!($t, [$($val),+]);
    };
    ($t:ty, [$($val:expr),+ $(,)?]) => {
        $crate::lazy::LazyLock::new(|| $crate::hide_array!($t, [$($val),+]))
    };
}
