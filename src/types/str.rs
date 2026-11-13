// SPDX-License-Identifier: MPL-2.0

//! 定容 UTF-8 字符串容器：栈上（`StackStr`）与堆上（`HeapStr`，需 `alloc`）实现，写入时校验编码，源缓冲按需擦除。
//!
//! 需要启用 `"types-str"` 特性。
#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use super::buffer::HeapBytes;
use super::buffer::{StackBytes, volatile_zero};
use crate::errors::{ErrType, Result, bail, err};
use core::convert::TryFrom;

#[cfg_attr(any(coin9, coin126), cold)]
#[cfg_attr(any(coin99, coin75), inline(never))]
#[cfg_attr(not(any(coin99, coin75)), inline(always))]
fn safe_parse_and_wipe<F>(buffer: &mut [u8], mut push_fn: F) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    let res = match ::core::str::from_utf8(buffer) {
        Ok(valid_str) => push_fn(valid_str),
        Err(e) => Err(err!(ErrType::InvalidUtf8(e))),
    };
    volatile_zero(buffer);
    res
}

/// 安全字符串的统一接口：支持只读视图、常量时间比较、原地追加、擦除与截断，内部恒为合法 UTF-8。
///
/// # Feature Requirement
///
/// 需要启用 `"types-str"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::str::{StackStr, Str};
///
/// let s: StackStr<8> = "hi".try_into().unwrap();
/// assert_eq!(s.as_str(), "hi");
/// assert_eq!(s.len(), 2);
/// ```
pub trait Str:
    ::core::ops::Deref<Target = str> + ::core::fmt::Display + ::core::fmt::Debug + Send + Sync
{
    /// 返回容器总容量（字节数）。
    fn capacity(&self) -> usize;

    /// 返回容器剩余可写入容量（字节数）。
    #[inline]
    fn remaining_capacity(&self) -> usize {
        self.capacity().saturating_sub(self.len())
    }

    /// 以 `&str` 形式查看内容。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    fn as_str(&self) -> &str;

    /// 以字节切片查看内容。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    #[cfg_attr(all(coin104, coin51), cold)]
    #[cfg_attr(any(coin68, coin47), inline(never))]
    #[cfg_attr(not(any(coin68, coin47)), inline(always))]
    fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    /// 以常量时间（Constant-Time）比较当前字符串与 `other` 字节序列是否相等，防止时序侧信道攻击。
    fn ct_eq(&self, other: &[u8]) -> bool;

    /// 类型擦除的堆克隆，用于 `Box<dyn Str>` 容器。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 与 `"alloc"` 特性。
    #[cfg(feature = "alloc")]
    fn dyn_clone(&self) -> alloc::boxed::Box<dyn Str>;

    /// 返回内容的字节长度。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    #[cfg_attr(coin111, cold)]
    #[cfg_attr(coin16, inline(never))]
    #[cfg_attr(all(not(coin16), coin15), inline(always))]
    #[cfg_attr(all(not(coin16), not(coin15)), inline)]
    fn len(&self) -> usize {
        self.as_str().len()
    }

    /// 内容是否为空。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    #[cfg_attr(coin63, cold)]
    #[cfg_attr(all(coin65, coin22), inline(never))]
    #[cfg_attr(not(all(coin65, coin22)), inline(always))]
    fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    /// 清零全部底层存储并将长度置 0。
    fn wipe_data(&mut self);
}

/// 栈上定容安全字符串，`Drop` 时自动清零，适用于短口令、令牌等小敏感文本。
///
/// # Feature Requirement
///
/// 需要启用 `"types-str"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::str::StackStr;
///
/// let s: StackStr<8> = "hi".try_into().unwrap();
/// assert_eq!(&*s, "hi");
/// ```
pub struct StackStr<const N: usize>(StackBytes<N>);

/// 堆上定容安全字符串，语义与 [`StackStr`] 一致，适用于较长的敏感文本。
///
/// # Feature Requirement
///
/// 需要启用 `"types-str"` 与 `"alloc"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::str::HeapStr;
///
/// let s: HeapStr<16> = "hi".try_into().unwrap();
/// assert_eq!(&*s, "hi");
/// ```
#[cfg(feature = "alloc")]
pub struct HeapStr<const N: usize>(HeapBytes<N>);

impl<const N: usize> StackStr<N> {
    /// 容器容量（字节）。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    pub const CAPACITY: usize = N;
    /// 创建空字符串。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 特性。
    #[cfg_attr(coin115, cold)]
    #[cfg_attr(any(coin14, coin122), inline(never))]
    #[cfg_attr(not(any(coin14, coin122)), inline(always))]
    pub const fn new() -> Self {
        Self(StackBytes::new())
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize> HeapStr<N> {
    /// 容器容量（字节）。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 与 `"alloc"` 特性。
    pub const CAPACITY: usize = N;
    /// 创建空字符串。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 与 `"alloc"` 特性。
    #[cfg_attr(coin58, cold)]
    #[cfg_attr(any(coin20, coin85), inline(never))]
    #[cfg_attr(not(any(coin20, coin85)), inline(always))]
    pub fn new() -> Self {
        Self(HeapBytes::new())
    }
}

macro_rules! impl_secure_str_base {
    ($name:ident) => {
        impl<const N: usize> $name<N> {
            /// 返回容器容量（字节数），恒为 `N`。
            #[inline]
            pub const fn capacity(&self) -> usize {
                N
            }

            /// 返回已写入字符串的字节长度。
            #[inline]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            /// 返回容器剩余可写入字节容量，等于 `N - len()`。
            #[inline]
            pub fn remaining_capacity(&self) -> usize {
                self.0.remaining_capacity()
            }

            /// 字符串是否为空。
            #[inline]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            /// 以 `&str` 形式查看内容（无需导入 [`Str`] trait 即可调用）。
            #[inline]
            pub fn as_str(&self) -> &str {
                // SAFETY: 类型不变式保证内部字节恒为合法 UTF-8（所有写入与截断路径均维护 UTF-8 边界）。
                unsafe { ::core::str::from_utf8_unchecked(self.0.as_slice()) }
            }

            /// 以只读字节切片 `&[u8]` 查看内容。
            #[inline]
            pub fn as_bytes(&self) -> &[u8] {
                self.0.as_slice()
            }

            /// 以常量时间（Constant-Time）比较当前字符串与 `other`（如 `&str` 或 `&[u8]`）是否相等。
            #[inline]
            pub fn ct_eq(&self, other: impl ::core::convert::AsRef<[u8]>) -> bool {
                self.0.ct_eq(other.as_ref())
            }

            /// 清零全部底层存储并将长度置 0。
            #[inline]
            pub fn wipe_data(&mut self) {
                self.0.wipe_data();
            }

            /// 清零全部底层存储并将长度置 0（[`wipe_data`](Self::wipe_data) 的标准库风格别名）。
            #[inline]
            pub fn clear(&mut self) {
                self.0.wipe_data();
            }

            /// 从原始可变指针拷贝并擦除源缓冲构造安全字符串。
            ///
            /// 成功时将源 `len` 字节按 UTF-8 解析后拷贝并清零源内存。
            ///
            /// # Safety
            ///
            /// 调用此函数必须保证：
            /// 1. `ptr` 非空且在 `len` 范围内可读写、正确对齐；
            /// 2. 调用期间该内存不被其他线程并发访问。
            ///
            /// # Errors
            ///
            /// - `ptr` 为空时返回 [`ErrType::NullPointer`]。
            /// - 源内容非合法 UTF-8 时返回 [`ErrType::InvalidUtf8`]（源仍会被清零）。
            /// - 长度超出 `N` 时返回 [`ErrType::CapacityExceeded`]（源仍会被清零）。
            #[cfg_attr(coin100, cold)]
            #[cfg_attr(coin42, inline(never))]
            #[cfg_attr(not(coin42), inline(always))]
            pub unsafe fn from_raw_parts_mut(ptr: *mut u8, len: usize) -> Result<Self> {
                if ptr.is_null() {
                    bail!(ErrType::NullPointer);
                }
                if len == 0 {
                    return Ok(Self::new());
                }

                // SAFETY: `ptr` 非空已检查；有效性、对齐与独占性由本函数的 `# Safety` 约定保证。
                let slice = unsafe { ::core::slice::from_raw_parts_mut(ptr, len) };

                let mut out = Self::new();
                safe_parse_and_wipe(slice, |s| out.try_push_str(s))?;
                Ok(out)
            }

            /// 在末尾追加字符串切片 `s`。
            ///
            /// # Errors
            ///
            /// - 当追加后总字节长度超出容量 `N` 时返回 [`ErrType::CapacityExceeded`]，原内容保持不变。
            #[cfg_attr(coin62, cold)]
            #[cfg_attr(all(coin100, coin17), inline(never))]
            #[cfg_attr(not(all(coin100, coin17)), inline(always))]
            pub fn try_push_str(&mut self, s: &str) -> Result<()> {
                self.0.extend_from_slice(s.as_bytes())
            }

            /// 在末尾追加单个字符 `ch`，并在追加后擦除栈上临时编码缓冲。
            ///
            /// # Errors
            ///
            /// - 当追加后总字节长度超出容量 `N` 时返回 [`ErrType::CapacityExceeded`]，原内容保持不变。
            #[inline]
            pub fn try_push(&mut self, ch: char) -> Result<()> {
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                let res = self.0.extend_from_slice(encoded.as_bytes());
                volatile_zero(&mut buf);
                res
            }

            /// 解析可变字节切片 `buffer` 为 UTF-8 并追加到末尾，无论成功或失败均清零 `buffer`。
            ///
            /// # Errors
            ///
            /// - 当 `buffer` 非合法 UTF-8 时返回 [`ErrType::InvalidUtf8`]。
            /// - 当追加后超出容量 `N` 时返回 [`ErrType::CapacityExceeded`]。
            #[inline]
            pub fn try_push_and_wipe(&mut self, buffer: &mut [u8]) -> Result<()> {
                safe_parse_and_wipe(buffer, |s| self.try_push_str(s))
            }

            /// 移除并返回字符串末尾的最后一个字符，同时**立即清零**该字符在底层容器中的字节槽位。
            ///
            /// 若字符串为空则返回 `None`。
            #[inline]
            pub fn pop(&mut self) -> Option<char> {
                let ch = self.as_str().chars().next_back()?;
                let new_len = self.len() - ch.len_utf8();
                self.0.truncate(new_len);
                Some(ch)
            }

            /// 将字符串截断至不超过 `max_len` 的最大合法 UTF-8 字符边界，并**立即清零**被丢弃的尾部字节。
            ///
            /// 若 `max_len >= self.len()`，则不执行任何操作。
            /// 该方法永不 panic，即使 `max_len` 落在多字节 UTF-8 字符内部，也会安全地向下回退至该字符起始处。
            #[inline]
            pub fn truncate(&mut self, max_len: usize) {
                if max_len < self.len() {
                    let mut boundary = max_len;
                    while boundary > 0 && !self.as_str().is_char_boundary(boundary) {
                        boundary -= 1;
                    }
                    self.0.truncate(boundary);
                }
            }

            /// 将内容搬运到容量为 `M` 的同类字符串中，原容器随作用域结束自动 `Drop` 清零。
            ///
            /// # Feature Requirement
            ///
            /// 需要启用 `"types-str"` 特性（`HeapStr` 相关还需 `"alloc"` 特性）。
            ///
            /// # Examples
            ///
            /// ```rust
            /// use core::convert::TryInto;
            /// use lib_unknown::types::str::StackStr;
            ///
            /// let s: StackStr<4> = "hi".try_into().unwrap();
            /// let c = s.try_grow::<8>().unwrap();
            /// assert_eq!(&*c, "hi");
            /// ```
            ///
            /// # Errors
            ///
            /// - 当已有长度超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(coin36, cold)]
            #[cfg_attr(coin4, inline(never))]
            #[cfg_attr(not(coin4), inline(always))]
            pub fn try_grow<const M: usize>(self) -> Result<$name<M>> {
                self.0.try_grow::<M>().map($name)
            }

            /// 将自身与 `suffix` 拼接为容量 `M` 的新字符串。
            ///
            /// # Feature Requirement
            ///
            /// 需要启用 `"types-str"` 特性（`HeapStr` 相关还需 `"alloc"` 特性）。
            ///
            /// # Examples
            ///
            /// ```rust
            /// use core::convert::TryInto;
            /// use lib_unknown::types::str::StackStr;
            ///
            /// let s: StackStr<4> = "hi".try_into().unwrap();
            /// let c = s.push_str_into::<8>("!").unwrap();
            /// assert_eq!(&*c, "hi!");
            /// ```
            ///
            /// # Errors
            ///
            /// - 当拼接后长度超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(any(coin11, coin35), cold)]
            #[cfg_attr(coin113, inline(never))]
            #[cfg_attr(not(coin113), inline(always))]
            pub fn push_str_into<const M: usize>(self, suffix: &str) -> Result<$name<M>> {
                let required_len = self.len().checked_add(suffix.len()).ok_or_else(|| {
                    err!(ErrType::CapacityExceeded {
                        requested: usize::MAX,
                        max: M,
                    })
                })?;
                if required_len > M {
                    bail!(ErrType::CapacityExceeded {
                        requested: required_len,
                        max: M,
                    });
                }
                self.0.extend_into(suffix.as_bytes()).map($name)
            }

            /// 将自身与 `rhs` 拼接为容量 `M` 的新字符串（`rhs` 为任意可借用为 `str` 的类型）。
            ///
            /// # Feature Requirement
            ///
            /// 需要启用 `"types-str"` 特性（`HeapStr` 相关还需 `"alloc"` 特性）。
            ///
            /// # Examples
            ///
            /// ```rust
            /// use core::convert::TryInto;
            /// use lib_unknown::types::str::StackStr;
            ///
            /// let s: StackStr<4> = "hi".try_into().unwrap();
            /// let c = s.concat_into::<8>("!").unwrap();
            /// assert_eq!(&*c, "hi!");
            /// ```
            ///
            /// # Errors
            ///
            /// - 当拼接后长度超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(any(coin120, coin72), cold)]
            #[cfg_attr(any(coin89, coin47), inline(never))]
            #[cfg_attr(not(any(coin89, coin47)), inline(always))]
            pub fn concat_into<const M: usize>(
                self,
                rhs: impl ::core::convert::AsRef<str>,
            ) -> Result<$name<M>> {
                self.push_str_into::<M>(rhs.as_ref())
            }
        }

        impl<const N: usize> ::core::default::Default for $name<N> {
            #[cfg_attr(coin59, cold)]
            #[cfg_attr(coin88, inline(never))]
            #[cfg_attr(all(not(coin88), coin118), inline(always))]
            #[cfg_attr(all(not(coin88), not(coin118)), inline)]
            fn default() -> Self {
                Self::new()
            }
        }

        impl<const N: usize> Str for $name<N> {
            #[inline]
            fn capacity(&self) -> usize {
                N
            }

            #[inline]
            fn remaining_capacity(&self) -> usize {
                self.remaining_capacity()
            }

            #[cfg_attr(any(coin65, coin114), cold)]
            #[cfg_attr(coin111, inline(never))]
            #[cfg_attr(all(not(coin111), coin92), inline(always))]
            #[cfg_attr(all(not(coin111), not(coin92)), inline)]
            fn as_str(&self) -> &str {
                self.as_str()
            }

            #[inline]
            fn ct_eq(&self, other: &[u8]) -> bool {
                self.0.ct_eq(other)
            }

            #[cfg(feature = "alloc")]
            #[cfg_attr(coin85, cold)]
            #[cfg_attr(coin9, inline(never))]
            #[cfg_attr(not(coin9), inline(always))]
            fn dyn_clone(&self) -> alloc::boxed::Box<dyn Str> {
                alloc::boxed::Box::new(self.clone())
            }

            #[inline]
            fn wipe_data(&mut self) {
                self.wipe_data();
            }
        }

        impl<const N: usize> ::core::clone::Clone for $name<N> {
            #[cfg_attr(coin79, cold)]
            #[cfg_attr(coin86, inline(never))]
            #[cfg_attr(all(not(coin86), coin10), inline(always))]
            #[cfg_attr(all(not(coin86), not(coin10)), inline)]
            fn clone(&self) -> Self {
                let mut new_str = Self::new();
                let _ = new_str.0.extend_from_slice(self.as_bytes());
                new_str
            }
        }

        /// 支持通过 `write!(&mut s, ...)` 直接向安全字符串容器格式化写入，避免通过 `alloc::format!` 在堆上残留明文。
        impl<const N: usize> ::core::fmt::Write for $name<N> {
            #[inline]
            fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
                self.try_push_str(s).map_err(|_| ::core::fmt::Error)
            }

            #[inline]
            fn write_char(&mut self, c: char) -> ::core::fmt::Result {
                self.try_push(c).map_err(|_| ::core::fmt::Error)
            }
        }
    };
}

macro_rules! impl_try_from_traits {
    ($name:ident) => {
        impl<const N: usize> TryFrom<&str> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(all(coin69, coin92), cold)]
            #[cfg_attr(coin87, inline(never))]
            #[cfg_attr(all(not(coin87), coin3), inline(always))]
            #[cfg_attr(all(not(coin87), not(coin3)), inline)]
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                out.try_push_str(s)?;
                Ok(out)
            }
        }

        impl<const N: usize> ::core::str::FromStr for $name<N> {
            type Err = crate::errors::Error;
            #[cfg_attr(coin15, cold)]
            #[cfg_attr(all(coin43, coin25), inline(never))]
            #[cfg_attr(not(all(coin43, coin25)), inline(always))]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s)
            }
        }

        impl<const N: usize> TryFrom<&mut [u8; N]> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(any(coin93, coin75), cold)]
            #[cfg_attr(coin29, inline(never))]
            #[cfg_attr(all(not(coin29), coin87), inline(always))]
            #[cfg_attr(all(not(coin29), not(coin87)), inline)]
            fn try_from(arr: &mut [u8; N]) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_and_wipe(arr, |s| out.try_push_str(s))?;
                Ok(out)
            }
        }

        impl<const N: usize> TryFrom<&mut [u8]> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(all(coin82, coin78), cold)]
            #[cfg_attr(coin57, inline(never))]
            #[cfg_attr(all(not(coin57), coin98), inline(always))]
            #[cfg_attr(all(not(coin57), not(coin98)), inline)]
            fn try_from(arr: &mut [u8]) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_and_wipe(arr, |s| out.try_push_str(s))?;
                Ok(out)
            }
        }

        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::boxed::Box<[u8; N]>> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(any(coin108, coin43), cold)]
            #[cfg_attr(any(coin56, coin112), inline(never))]
            #[cfg_attr(not(any(coin56, coin112)), inline(always))]
            fn try_from(mut b: alloc::boxed::Box<[u8; N]>) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_and_wipe(&mut *b, |s| out.try_push_str(s))?;
                Ok(out)
            }
        }

        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::vec::Vec<u8>> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(coin9, cold)]
            #[cfg_attr(coin58, inline(never))]
            #[cfg_attr(not(coin58), inline(always))]
            fn try_from(mut v: alloc::vec::Vec<u8>) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_and_wipe(v.as_mut_slice(), |s| out.try_push_str(s))?;
                Ok(out)
            }
        }

        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::string::String> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(all(coin60, coin115), cold)]
            #[cfg_attr(any(coin85, coin72), inline(never))]
            #[cfg_attr(not(any(coin85, coin72)), inline(always))]
            fn try_from(s: alloc::string::String) -> Result<Self, Self::Error> {
                let mut bytes = s.into_bytes();
                let mut out = Self::new();
                // `String` 自身已保证为合法 UTF-8，直接拷贝字节并擦除原堆内存，免去二次 UTF-8 校验。
                let res = out.0.extend_from_slice(bytes.as_slice());
                volatile_zero(bytes.as_mut_slice());
                res?;
                Ok(out)
            }
        }
    };
}

macro_rules! __impl_common_str_traits {
    ($name:ident) => {
        impl<const N: usize> ::core::ops::Deref for $name<N> {
            type Target = str;
            #[cfg_attr(coin14, cold)]
            #[cfg_attr(any(coin67, coin111), inline(never))]
            #[cfg_attr(not(any(coin67, coin111)), inline(always))]
            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }
        impl<const N: usize> ::core::fmt::Display for $name<N> {
            #[cfg_attr(coin115, cold)]
            #[cfg_attr(coin33, inline(never))]
            #[cfg_attr(not(coin33), inline(always))]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl<const N: usize> ::core::fmt::Debug for $name<N> {
            #[cfg_attr(coin70, cold)]
            #[cfg_attr(coin19, inline(never))]
            #[cfg_attr(all(not(coin19), coin10), inline(always))]
            #[cfg_attr(all(not(coin19), not(coin10)), inline)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Debug::fmt(self.as_str(), f)
            }
        }
        impl<const N: usize> ::core::convert::AsRef<str> for $name<N> {
            #[cfg_attr(all(coin55, coin13), cold)]
            #[cfg_attr(any(coin125, coin112), inline(never))]
            #[cfg_attr(not(any(coin125, coin112)), inline(always))]
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
        impl<const N: usize> ::core::convert::AsRef<[u8]> for $name<N> {
            #[cfg_attr(coin7, cold)]
            #[cfg_attr(coin85, inline(never))]
            #[cfg_attr(all(not(coin85), coin48), inline(always))]
            #[cfg_attr(all(not(coin85), not(coin48)), inline)]
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }
        impl<const N: usize> ::core::cmp::PartialEq<str> for $name<N> {
            #[cfg_attr(all(coin30, coin42), cold)]
            #[cfg_attr(coin112, inline(never))]
            #[cfg_attr(all(not(coin112), coin5), inline(always))]
            #[cfg_attr(all(not(coin112), not(coin5)), inline)]
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }
        impl<'a, const N: usize> ::core::cmp::PartialEq<&'a str> for $name<N> {
            #[cfg_attr(any(coin84, coin97), cold)]
            #[cfg_attr(any(coin92, coin30), inline(never))]
            #[cfg_attr(not(any(coin92, coin30)), inline(always))]
            fn eq(&self, other: &&'a str) -> bool {
                self.as_str() == *other
            }
        }
        impl<const N: usize, const M: usize> ::core::cmp::PartialEq<$name<M>> for $name<N> {
            #[cfg_attr(any(coin86, coin98), cold)]
            #[cfg_attr(all(coin81, coin58), inline(never))]
            #[cfg_attr(not(all(coin81, coin58)), inline(always))]
            fn eq(&self, other: &$name<M>) -> bool {
                self.as_str() == other.as_str()
            }
        }
        #[cfg(feature = "alloc")]
        impl<const N: usize> ::core::cmp::PartialEq<alloc::string::String> for $name<N> {
            #[cfg_attr(any(coin53, coin64), cold)]
            #[cfg_attr(all(coin86, coin29), inline(never))]
            #[cfg_attr(not(all(coin86, coin29)), inline(always))]
            fn eq(&self, other: &alloc::string::String) -> bool {
                self.as_str() == other.as_str()
            }
        }
        #[cfg(feature = "alloc")]
        impl<const N: usize> ::core::cmp::PartialEq<$name<N>> for alloc::string::String {
            #[cfg_attr(coin117, cold)]
            #[cfg_attr(coin6, inline(never))]
            #[cfg_attr(not(coin6), inline(always))]
            fn eq(&self, other: &$name<N>) -> bool {
                self.as_str() == other.as_str()
            }
        }
        impl<const N: usize> ::core::cmp::Eq for $name<N> {}
        impl<const N: usize> ::core::cmp::PartialOrd for $name<N> {
            #[cfg_attr(any(coin86, coin8), cold)]
            #[cfg_attr(coin67, inline(never))]
            #[cfg_attr(not(coin67), inline(always))]
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<const N: usize> ::core::cmp::Ord for $name<N> {
            #[cfg_attr(any(coin8, coin98), cold)]
            #[cfg_attr(coin70, inline(never))]
            #[cfg_attr(not(coin70), inline(always))]
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.as_str().cmp(other.as_str())
            }
        }
        impl<const N: usize> ::core::hash::Hash for $name<N> {
            #[cfg_attr(coin8, cold)]
            #[cfg_attr(coin90, inline(never))]
            #[cfg_attr(all(not(coin90), coin9), inline(always))]
            #[cfg_attr(all(not(coin90), not(coin9)), inline)]
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.as_str().hash(state)
            }
        }
        impl<const N: usize> ::core::borrow::Borrow<str> for $name<N> {
            #[cfg_attr(any(coin58, coin108), cold)]
            #[cfg_attr(coin65, inline(never))]
            #[cfg_attr(not(coin65), inline(always))]
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }
    };
}

impl_secure_str_base!(StackStr);
impl_try_from_traits!(StackStr);
__impl_common_str_traits!(StackStr);

#[cfg(feature = "alloc")]
impl_secure_str_base!(HeapStr);
#[cfg(feature = "alloc")]
impl_try_from_traits!(HeapStr);
#[cfg(feature = "alloc")]
__impl_common_str_traits!(HeapStr);

#[cfg(feature = "alloc")]
impl<const N: usize> HeapStr<N> {
    /// 将内容复制为普通 `String`（敏感内存外泄，谨慎使用，已废弃）。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 与 `"alloc"` 特性。
    #[deprecated(note = "This leaks secure memory into alloc::string::String. Use with caution.")]
    #[cfg_attr(any(coin122, coin1), cold)]
    #[cfg_attr(coin6, inline(never))]
    #[cfg_attr(not(coin6), inline(always))]
    pub fn leak_into_string(self) -> alloc::string::String {
        use alloc::borrow::ToOwned;
        self.as_str().to_owned()
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize> StackStr<N> {
    /// 将栈字符串搬运为容量 `M` 的堆字符串。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-str"` 与 `"alloc"` 特性。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use core::convert::TryInto;
    /// use lib_unknown::types::str::StackStr;
    ///
    /// let s: StackStr<4> = "hi".try_into().unwrap();
    /// let h = s.into_heap::<8>().unwrap();
    /// assert_eq!(&*h, "hi");
    /// ```
    ///
    /// # Errors
    ///
    /// - 当已有长度超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
    #[cfg_attr(any(coin59, coin18), cold)]
    #[cfg_attr(coin35, inline(never))]
    #[cfg_attr(all(not(coin35), coin93), inline(always))]
    #[cfg_attr(all(not(coin35), not(coin93)), inline)]
    pub fn into_heap<const M: usize>(self) -> Result<HeapStr<M>> {
        self.0.extend_into::<HeapBytes<M>>(&[]).map(HeapStr)
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize, const M: usize> ::core::cmp::PartialEq<StackStr<M>> for HeapStr<N> {
    #[cfg_attr(all(coin104, coin99), cold)]
    #[cfg_attr(coin103, inline(never))]
    #[cfg_attr(not(coin103), inline(always))]
    fn eq(&self, other: &StackStr<M>) -> bool {
        self.as_str() == other.as_str()
    }
}
#[cfg(feature = "alloc")]
impl<const N: usize, const M: usize> ::core::cmp::PartialEq<HeapStr<N>> for StackStr<M> {
    #[cfg_attr(any(coin55, coin47), cold)]
    #[cfg_attr(coin96, inline(never))]
    #[cfg_attr(not(coin96), inline(always))]
    fn eq(&self, other: &HeapStr<N>) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(feature = "alloc")]
impl ::core::clone::Clone for alloc::boxed::Box<dyn Str> {
    #[cfg_attr(any(coin0, coin69), cold)]
    #[cfg_attr(coin111, inline(never))]
    #[cfg_attr(not(coin111), inline(always))]
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::PartialEq for alloc::boxed::Box<dyn Str> {
    #[cfg_attr(any(coin122, coin97), cold)]
    #[cfg_attr(any(coin30, coin25), inline(never))]
    #[cfg_attr(not(any(coin30, coin25)), inline(always))]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::Eq for alloc::boxed::Box<dyn Str> {}
#[cfg(feature = "alloc")]
impl ::core::cmp::PartialEq<str> for alloc::boxed::Box<dyn Str> {
    #[cfg_attr(all(coin53, coin82), cold)]
    #[cfg_attr(coin86, inline(never))]
    #[cfg_attr(not(coin86), inline(always))]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::PartialEq<&str> for alloc::boxed::Box<dyn Str> {
    #[cfg_attr(any(coin52, coin85), cold)]
    #[cfg_attr(all(coin11, coin50), inline(never))]
    #[cfg_attr(not(all(coin11, coin50)), inline(always))]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::PartialEq<alloc::string::String> for alloc::boxed::Box<dyn Str> {
    #[cfg_attr(all(coin31, coin93), cold)]
    #[cfg_attr(all(coin85, coin88), inline(never))]
    #[cfg_attr(not(all(coin85, coin88)), inline(always))]
    fn eq(&self, other: &alloc::string::String) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    #[cfg(feature = "alloc")]
    use alloc::{boxed::Box, string::String, vec, vec::Vec};
    use core::convert::TryInto;
    use core::fmt::Write;

    #[test]
    fn test_stack_from_str_literal() {
        let s: StackStr<8> = "hello".try_into().unwrap();
        assert_eq!(s, "hello");
        assert!(s.ct_eq("hello"));
        assert!(!s.ct_eq("world"));
        assert_eq!(s.capacity(), 8);
        assert_eq!(s.remaining_capacity(), 3);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_heap_from_str_literal() {
        let h: HeapStr<16> = "hello".try_into().unwrap();
        assert_eq!(h, "hello");
        assert!(h.ct_eq("hello"));

        let from_string = HeapStr::<16>::try_from(String::from("from_string")).unwrap();
        assert_eq!(from_string, "from_string");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_dyn_str() {
        let mut secret_list: Vec<Box<dyn Str>> = Vec::new();

        let h: HeapStr<256> = "SuperSecret_Heap".try_into().unwrap();
        secret_list.push(Box::new(h));

        let s: StackStr<16> = "Short_Stack".try_into().unwrap();
        secret_list.push(Box::new(s));

        let s2: StackStr<32> = "Another_Stack_32".try_into().unwrap();
        secret_list.push(Box::new(s2));

        assert_eq!(secret_list.len(), 3);
        assert!(secret_list[0].starts_with("Super"));
        assert!(secret_list[1].ends_with("Stack"));
        assert!(secret_list[0].ct_eq(b"SuperSecret_Heap"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn from_raw_ptr_with_wiping() {
        let mut source_data = b"secret_data".to_vec();
        let ptr = source_data.as_mut_ptr();
        let len = source_data.len();

        // SAFETY: `ptr` 与 `len` 来自刚分配的合法 Vec 底层存储，所有权随 from_raw_parts_mut 转移。
        let h = unsafe { HeapStr::<32>::from_raw_parts_mut(ptr, len) }.unwrap();
        assert_eq!(h.as_str(), "secret_data");

        assert_eq!(source_data, vec![0u8; 11]);
    }

    #[test]
    fn test_capacity_exceeded_fail_fast() {
        let res: Result<StackStr<4>, _> = "hello".try_into();
        assert!(res.is_err());
    }

    #[test]
    fn test_invalid_utf8_wiping() {
        let mut bad_utf8 = [0, 159, 146, 150];
        let res = StackStr::<16>::try_from(bad_utf8.as_mut_slice());
        assert!(res.is_err());
        assert_eq!(bad_utf8, [0u8; 4]);
    }

    #[test]
    fn test_in_place_mutation_and_utf8_boundary_truncate() {
        let mut s = StackStr::<16>::new();
        s.try_push_str("A").unwrap();
        s.try_push('中').unwrap(); // 3 字节 UTF-8
        s.try_push('🦀').unwrap(); // 4 字节 UTF-8
        assert_eq!(s.as_str(), "A中🦀");
        assert_eq!(s.len(), 8);

        // 测试 pop 弹出多字节字符并立即清零底层字节
        assert_eq!(s.pop(), Some('🦀'));
        assert_eq!(s.as_str(), "A中");
        assert_eq!(s.len(), 4);
        assert_eq!(s.0.as_slice(), "A中".as_bytes());

        // 测试 truncate 在落在多字节字符中间（索引 2 位于 '中' 内部）时自动回退至合法字符边界（索引 1）
        s.try_push('🦀').unwrap(); // "A中🦀" (len = 8)
        s.truncate(2);
        assert_eq!(s.as_str(), "A");
        assert_eq!(s.len(), 1);

        // 测试 fmt::Write 原地格式化写入
        s.clear();
        assert!(s.is_empty());
        write!(&mut s, "pin:{}", 1234).unwrap();
        assert_eq!(s.as_str(), "pin:1234");
    }
}
