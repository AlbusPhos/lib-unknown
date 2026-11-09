//src/hide.rs

/// 在编译期生成一个由构建种子、调用位置与构建环境共同派生的伪随机整数。
///
/// 支持 `const_random!()`（默认 `u64`）、`const_random!(Type)` 与
/// `const_random!(Type, "salt")` 三种形式；结果可保存到 `const`/`static`。
#[macro_export]
macro_rules! const_random {
    () => {
        $crate::const_random!(u64, "const_random")
    };
    ($t:ty) => {
        $crate::const_random!($t, "const_random")
    };
    ($t:ty, $salt:expr) => {
        const {
            const fn mix(mut h: u64, s: &str) -> u64 {
                let bytes = s.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    h ^= !(bytes[i] as u64)
                        .wrapping_add(h)
                        .wrapping_mul(u64::MAX / 5 * 3)
                        .wrapping_add(i as u64);
                    i += 1;
                    if i >= 256 {
                        break;
                    }
                }
                h
            }

            const fn mix_u64(mut h: u64, n: u64) -> u64 {
                h ^= !n
                    .wrapping_add(h)
                    .wrapping_mul(u64::MAX / 5 * 3)
                    .wrapping_add(7);
                h
            }

            let mut h = mix(u64::MAX, $crate::__private::SEED);
            h = mix(h, $crate::__private::SEED64);
            h = mix(h, $crate::__private::SEED128);
            h = mix(h, module_path!());
            h = mix(h, file!());
            h = mix_u64(h, line!() as u64);
            h = mix_u64(h, column!() as u64);
            h = mix(h, stringify!($salt));
            const ENVS: &[Option<&str>] = &[
                option_env!("SCCACHE_CACHE_SIZE"),
                option_env!("RUSTC_WRAPPER"),
                option_env!("RUST_LOG"),
                option_env!("RUSTFLAGS"),
                option_env!("SHELL"),
                option_env!("HOSTNAME"),
            ];
            let mut idx = 0;
            while idx < ENVS.len() {
                if let Some(s) = ENVS[idx] {
                    h = mix(h, s);
                }
                idx += 1;
            }
            h as $t
        }
    };
}

/// 单个内容混淆在::core::hint::black_box下仍然容易明文出现，不推荐使用。优先使用数组版本
#[macro_export]
macro_rules! hide {
    ($t:ty, $val:expr) => {{
        static __MASK: $t = $crate::const_random!($t);
        static __ENC: $t = const { ($val) ^ __MASK };
        ::core::hint::black_box(::core::hint::black_box(__ENC) ^ ::core::hint::black_box(__MASK))
    }};
}

/// 将一个常量数组以 XOR 掩码形式存入 `.rodata`，并在运行时经 `black_box` 还原。
///
/// 适用于整数数组；返回值类型为 `[$t; N]`。
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

/// 声明一个以 [`hide!`] 初始化、首次访问才还原的 `static LazyLock<T>`。
///
/// 需要启用 `"lazy"` 特性。
#[cfg(feature = "lazy")]
#[macro_export]
macro_rules! hide_lazy {
    (
        $(#[$meta:meta])*
        $vis:vis static $name:ident : $t:ty = $val:expr $(;)?
    ) => {
        $(#[$meta])*
        $vis static $name: $crate::lazy::LazyLock<$t> = $crate::hide_lazy!($t, $val);
    };
    ($t:ty, $val:expr) => {
        $crate::lazy::LazyLock::new(|| $crate::hide!($t, $val))
    };
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
