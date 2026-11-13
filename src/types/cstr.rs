// SPDX-License-Identifier: MPL-2.0

//! 定容 C 字符串容器：基于 [`StackBytes`](crate::types::buffer::StackBytes) 与
//! [`HeapBytes`](crate::types::buffer::HeapBytes) 封装的栈上（[`StackCStr`](crate::types::cstr::StackCStr)）
//! 与堆上（[`HeapCStr`](crate::types::cstr::HeapCStr)，需 `alloc`）实现，内部恒以 `\0` 结尾，搬运/销毁时自动经底层安全容器清零。
//!
//! 与 [`crate::types::str`] 不同，本模块**允许非 UTF-8 字节**，仅校验 NUL 语义，
//! 可直接投喂 `sys_open` / `sys_execve` 等取 `core::ffi::CStr` 的调用。
//!
//! # 容量语义
//!
//! `N` **包含结尾 `\0`**（`N >= 1`）：有效载荷至多 `N - 1` 字节。
//! `len()` 返回不含 `\0` 的有效载荷长度，而底层 `Buffer` 的长度恒为 `len() + 1`。
//!
//! 需要启用 `"types-cstr"` 特性。

#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
// 显式目标保证 stable rustdoc 也能解析；nightly 的 redundant_explicit_links 会将其视为冗余。
#![allow(rustdoc::redundant_explicit_links)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use super::buffer::HeapBytes;
use super::buffer::{Buffer, StackBytes, volatile_zero};
use crate::errors::{ErrType, Result, bail, err};
use core::convert::TryFrom;
use core::ffi::CStr as StdCStr;
use core::hint::black_box;

#[cfg_attr(coin123, cold)]
#[cfg_attr(coin14, inline(never))]
#[cfg_attr(all(not(coin14), coin56), inline(always))]
#[cfg_attr(all(not(coin14), not(coin56)), inline)]
fn safe_parse_cstr_and_wipe<F>(buffer: &mut [u8], mut push_fn: F) -> Result<()>
where
    F: FnMut(&[u8]) -> Result<()>,
{
    let res = match buffer.iter().position(|&b| b == 0) {
        None => Err(err!(ErrType::MissingNul)),
        Some(pos) => push_fn(&buffer[..pos]),
    };
    volatile_zero(buffer);
    // SAFETY: 交给 push_fn 的借用在 wipe 前结束；此处仅对源做易失清零。
    res
}

#[inline]
fn ct_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= black_box(x ^ y);
    }
    black_box(diff) == 0
}

/// 安全 C 字符串的统一接口：只读视图，内部恒为合法 `CStr`（以 `\0` 结尾、无内部 NUL）。
///
/// 注意：本 trait 名为 `CStr`，与 `core::ffi::CStr` 同名。
/// [`StackCStr`] 与 [`HeapCStr`] 已提供全套同名固有方法，**未导入本 Trait 时所有方法仍可直接调用**。
///
/// # Feature Requirement
///
/// 需要启用 `"types-cstr"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::cstr::{CStr, StackCStr};
///
/// let s: StackCStr<8> = "hi".try_into().unwrap();
/// assert_eq!(s.as_cstr().to_bytes(), b"hi");
/// assert_eq!(s.len(), 2);
/// ```
pub trait CStr:
    ::core::ops::Deref<Target = StdCStr> + ::core::fmt::Display + ::core::fmt::Debug + Send + Sync
{
    /// 以 `&core::ffi::CStr` 形式查看内容，可直接传给 `sys_open` 等。
    fn as_cstr(&self) -> &StdCStr;

    /// 以字节切片查看载荷（不含结尾 `\0`），允许非 UTF-8。
    #[cfg_attr(coin30, cold)]
    #[cfg_attr(any(coin85, coin75), inline(never))]
    #[cfg_attr(not(any(coin85, coin75)), inline(always))]
    fn as_bytes(&self) -> &[u8] {
        self.as_cstr().to_bytes()
    }

    /// 以字节切片查看含结尾 `\0` 的完整存储。
    #[cfg_attr(coin46, cold)]
    #[cfg_attr(coin74, inline(never))]
    #[cfg_attr(not(coin74), inline(always))]
    fn as_bytes_with_nul(&self) -> &[u8] {
        self.as_cstr().to_bytes_with_nul()
    }

    /// 返回载荷的原始指针，指向容器内部存储（等价于 `as_cstr().as_ptr()`）。
    ///
    /// 返回的指针仅在 `self` 存活且未被搬运/修改期间有效；`self` 移动、`Drop`
    /// 或任何 `&mut self` 操作后不得再使用旧指针。
    #[cfg_attr(any(coin114, coin86), cold)]
    #[cfg_attr(any(coin0, coin111), inline(never))]
    #[cfg_attr(not(any(coin0, coin111)), inline(always))]
    fn as_ptr(&self) -> *const core::ffi::c_char {
        self.as_cstr().as_ptr()
    }

    /// 类型擦除的堆克隆，用于 `Box<dyn CStr>` 容器。
    #[cfg(feature = "alloc")]
    fn dyn_clone(&self) -> alloc::boxed::Box<dyn CStr>;

    /// 返回载荷的字节长度（不含结尾 `\0`）。
    #[cfg_attr(all(coin68, coin20), cold)]
    #[cfg_attr(coin38, inline(never))]
    #[cfg_attr(not(coin38), inline(always))]
    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// 载荷是否为空。
    #[cfg_attr(all(coin88, coin10), cold)]
    #[cfg_attr(any(coin64, coin46), inline(never))]
    #[cfg_attr(not(any(coin64, coin46)), inline(always))]
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// 以常量时间（Constant-Time）比较有效载荷与字节切片 `other` 是否相等，防止时序攻击。
    #[inline]
    fn ct_eq(&self, other: &[u8]) -> bool {
        ct_eq_bytes(self.as_bytes(), other)
    }

    /// 以常量时间（Constant-Time）比较当前对象与另一 `dyn CStr` 对象的载荷是否相等。
    #[inline]
    fn ct_eq_dyn(&self, other: &dyn CStr) -> bool {
        ct_eq_bytes(self.as_bytes(), other.as_bytes())
    }
}

/// 栈上定容安全 C 字符串（内部封装 [`StackBytes<N>`]），`Drop` 时自动清零，适用于短路径、argv 等小敏感文本。
///
/// `N` 包含结尾 `\0`，`N >= 1`。
///
/// # Feature Requirement
///
/// 需要启用 `"types-cstr"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::cstr::StackCStr;
///
/// let s: StackCStr<8> = "hi".try_into().unwrap();
/// assert_eq!(&*s, c"hi");
/// assert_eq!(s.len(), 2);
/// ```
pub struct StackCStr<const N: usize>(StackBytes<N>);

/// 堆上定容安全 C 字符串（内部封装 [`HeapBytes<N>`]），语义与 [`StackCStr`] 一致，适用于较长的敏感文本。
///
/// `N` 包含结尾 `\0`，`N >= 1`。
///
/// # Feature Requirement
///
/// 需要启用 `"types-cstr"` 与 `"alloc"` 特性。
///
/// # Examples
///
/// ```rust
/// use core::convert::TryInto;
/// use lib_unknown::types::cstr::HeapCStr;
///
/// let s: HeapCStr<16> = "hi".try_into().unwrap();
/// assert_eq!(&*s, c"hi");
/// ```
#[cfg(feature = "alloc")]
pub struct HeapCStr<const N: usize>(HeapBytes<N>);

impl<const N: usize> StackCStr<N> {
    /// 容器总容量（含结尾 `\0`）。
    pub const CAPACITY: usize = N;

    /// 容器最大有效载荷字节数（不含结尾 `\0`）。
    pub const MAX_PAYLOAD_LEN: usize = N.saturating_sub(1);

    /// 创建空 C 字符串（仅含结尾 `\0`）。
    ///
    /// # Panics
    ///
    /// - 当 `N == 0` 时 panic：连结尾 `\0` 都放不下。
    #[cfg_attr(any(coin51, coin115), cold)]
    #[cfg_attr(coin86, inline(never))]
    #[cfg_attr(all(not(coin86), coin113), inline(always))]
    #[cfg_attr(all(not(coin86), not(coin113)), inline)]
    pub const fn new() -> Self {
        assert!(N > 0, "StackCStr capacity N must include the NUL (>= 1)");
        Self(StackBytes {
            len: 1,
            data: [0u8; N],
        })
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize> HeapCStr<N> {
    /// 容器总容量（含结尾 `\0`）。
    pub const CAPACITY: usize = N;

    /// 容器最大有效载荷字节数（不含结尾 `\0`）。
    pub const MAX_PAYLOAD_LEN: usize = N.saturating_sub(1);

    /// 创建空 C 字符串（仅含结尾 `\0`）。
    ///
    /// # Panics
    ///
    /// - 当 `N == 0` 时 panic：连结尾 `\0` 都放不下。
    #[cfg_attr(all(coin28, coin86), cold)]
    #[cfg_attr(all(coin62, coin0), inline(never))]
    #[cfg_attr(not(all(coin62, coin0)), inline(always))]
    pub fn new() -> Self {
        assert!(N > 0, "HeapCStr capacity N must include the NUL (>= 1)");
        let mut buf = HeapBytes::<N>::new();
        // `HeapBytes::new()` 底层已通过 `zeroed_box` 全零初始化，置 `len = 1` 即代表含首字节 `\0`
        buf.len = 1;
        Self(buf)
    }
}

macro_rules! impl_secure_cstr_base {
    ($name:ident, $inner:ident) => {
        impl<const N: usize> $name<N> {
            /// 返回容器总容量（含结尾 `\0`），恒为 `N`。
            #[inline]
            pub const fn capacity(&self) -> usize {
                N
            }

            /// 返回最大可写入的有效载荷长度（不含结尾 `\0`），恒为 `N - 1`。
            #[inline]
            pub const fn max_payload_len(&self) -> usize {
                N.saturating_sub(1)
            }

            /// 返回当前有效载荷的字节长度（不含结尾 `\0`）。
            #[inline]
            pub const fn len(&self) -> usize {
                // 不变量保证 `self.0.len >= 1`
                self.0.len - 1
            }

            /// 有效载荷是否为空。
            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.0.len <= 1
            }

            /// 返回剩余可追加的有效载荷字节数，等于 `(N - 1) - len()`。
            #[inline]
            pub const fn remaining_capacity(&self) -> usize {
                N.saturating_sub(self.0.len)
            }

            /// 获取内部底层安全字节容器（含结尾 `\0`，长度 `len() + 1`）的只读引用。
            #[inline]
            pub const fn as_buffer(&self) -> &$inner<N> {
                &self.0
            }

            /// 以 `&core::ffi::CStr` 形式查看内容，可直接传给 `sys_open` 等系统调用。
            #[inline]
            pub fn as_cstr(&self) -> &StdCStr {
                // SAFETY: 类型不变量保证 `self.0` 的 `data[..len - 1]` 无内部 NUL 且 `data[len - 1] == 0`。
                unsafe { StdCStr::from_bytes_with_nul_unchecked(self.0.as_slice()) }
            }

            /// 以字节切片查看有效载荷（不含结尾 `\0`），允许非 UTF-8。
            #[inline]
            pub fn as_bytes(&self) -> &[u8] {
                &self.0.data[..self.len()]
            }

            /// 以字节切片查看含结尾 `\0` 的完整有效存储。
            #[inline]
            pub fn as_bytes_with_nul(&self) -> &[u8] {
                self.0.as_slice()
            }

            /// 返回指向内部 C 字符串首字节的只读指针（等价于 `as_cstr().as_ptr()`）。
            #[inline]
            pub fn as_ptr(&self) -> *const core::ffi::c_char {
                self.0.as_ptr().cast()
            }

            /// 以常量时间（Constant-Time）比较有效载荷与字节切片 `other` 是否相等。
            #[inline]
            pub fn ct_eq(&self, other: &[u8]) -> bool {
                ct_eq_bytes(self.as_bytes(), other)
            }

            /// 以常量时间（Constant-Time）比较当前容器与另一 C 字符串（复用底层 `Buffer::ct_eq`）。
            #[inline]
            pub fn ct_eq_cstr(&self, other: &(impl AsRef<StdCStr> + ?Sized)) -> bool {
                self.0.ct_eq(other.as_ref().to_bytes_with_nul())
            }

            /// 清零全部底层存储并将有效载荷重置为空（保留首字节 `\0`）。
            #[inline]
            pub fn wipe_data(&mut self) {
                self.0.wipe_data();
                self.0.len = 1;
            }

            /// 清空内容并擦除底层数据（等价于 [`Self::wipe_data`]）。
            #[inline]
            pub fn clear(&mut self) {
                self.wipe_data();
            }

            /// 将有效载荷截断至 `new_len` 字节，复用底层 [`Buffer::truncate`] **立即清零**被丢弃尾部并置 `\0`。
            ///
            /// 若 `new_len >= self.len()`，则不执行任何操作。
            #[inline]
            pub fn truncate(&mut self, new_len: usize) {
                if new_len < self.len() {
                    // `self.0.truncate(new_len)` 会将 `data[new_len..self.0.len]` 全部易失清零，
                    // 从而使 `data[new_len]` 变为 `0`（即新的结尾 `\0`），随后将底层长度设为 `new_len + 1`。
                    self.0.truncate(new_len);
                    self.0.len = new_len + 1;
                }
            }

            /// 零拷贝消费自身，返回包含结尾 `\0`（长度为 `len() + 1`）的底层安全容器。
            #[inline]
            pub fn into_bytes_with_nul(self) -> $inner<N> {
                self.0
            }

            /// 零拷贝消费自身，弹出结尾 `\0` 后返回仅含有效载荷（长度为 `len()`）的底层安全容器。
            #[inline]
            pub fn into_bytes(mut self) -> $inner<N> {
                let _ = self.0.pop();
                self.0
            }

            /// 原地零拷贝：将同容量、含 `\0` 的底层容器转换为 C 字符串（首个 `\0` 后的多余字节会被立即清零）。
            ///
            /// # Errors
            ///
            /// - 当 `N == 0` 时返回 [`ErrType::CapacityExceeded`]。
            /// - 当 `buf` 已写入区间内不含 `\0` 时返回 [`ErrType::MissingNul`]（`buf` 随作用域结束自动 `Drop` 清零）。
            #[inline]
            pub fn from_inner_with_nul(mut buf: $inner<N>) -> Result<Self> {
                if N == 0 {
                    bail!(ErrType::CapacityExceeded {
                        requested: 1,
                        max: 0,
                    });
                }
                match buf.as_slice().iter().position(|&b| b == 0) {
                    None => bail!(ErrType::MissingNul),
                    Some(pos) => {
                        // 截断并立即清零首个 `\0` 之后的全部残留字节
                        buf.truncate(pos + 1);
                        Ok(Self(buf))
                    }
                }
            }

            /// 原地零拷贝：将同容量、不含 `\0` 的纯载荷底层容器转换为 C 字符串（自动在末尾追加 `\0`）。
            ///
            /// # Errors
            ///
            /// - 当 `buf` 含有内部 `\0` 时返回 [`ErrType::InteriorNul`]（`buf` 自动 `Drop` 清零）。
            /// - 当 `buf.len() + 1 > N`（无空間追加 `\0`）时返回 [`ErrType::CapacityExceeded`]（`buf` 自动 `Drop` 清零）。
            #[inline]
            pub fn from_inner_payload(mut buf: $inner<N>) -> Result<Self> {
                if let Some(pos) = buf.as_slice().iter().position(|&b| b == 0) {
                    bail!(ErrType::InteriorNul(pos));
                }
                buf.push(0)?;
                Ok(Self(buf))
            }

            /// 从任意 [`Buffer<u8>`] 的纯载荷（不含结尾 `\0`）拷贝构造 C 字符串。
            ///
            /// # Errors
            ///
            /// - 当载荷含内部 `\0` 时返回 [`ErrType::InteriorNul`]。
            /// - 当 `buf.len() + 1 > N` 时返回 [`ErrType::CapacityExceeded`]。
            #[inline]
            pub fn from_buffer_payload<B: Buffer<u8>>(buf: &B) -> Result<Self> {
                let mut out = Self::new();
                out.try_push_bytes(buf.as_slice())?;
                Ok(out)
            }

            /// 从任意 [`Buffer<u8>`] 的纯载荷构造 C 字符串，成功后调用 [`Buffer::wipe_data`] 擦除源容器。
            ///
            /// 若因容量不足或含内部 `\0` 失败，源容器 `buf` 保持不变。
            ///
            /// # Errors
            ///
            /// - 当载荷含内部 `\0` 时返回 [`ErrType::InteriorNul`]。
            /// - 当 `buf.len() + 1 > N` 时返回 [`ErrType::CapacityExceeded`]。
            #[inline]
            pub fn from_buffer_payload_and_wipe<B: Buffer<u8>>(buf: &mut B) -> Result<Self> {
                let mut out = Self::new();
                out.try_push_bytes(buf.as_slice())?;
                buf.wipe_data();
                Ok(out)
            }

            /// 拷贝有效载荷（不含结尾 `\0`）生成栈安全字节容器 [`StackBytes<N>`]。
            #[inline]
            pub fn to_stack_bytes(&self) -> StackBytes<N> {
                let mut buf = StackBytes::<N>::new();
                let _ = buf.extend_from_slice(self.as_bytes());
                buf
            }

            /// 拷贝含结尾 `\0` 的完整数据生成栈安全字节容器 [`StackBytes<N>`]。
            #[inline]
            pub fn to_stack_bytes_with_nul(&self) -> StackBytes<N> {
                let mut buf = StackBytes::<N>::new();
                let _ = buf.extend_from_slice(self.as_bytes_with_nul());
                buf
            }

            /// 拷贝有效载荷（不含结尾 `\0`）生成堆安全字节容器 [`HeapBytes<N>`]。
            #[cfg(feature = "alloc")]
            #[inline]
            pub fn to_heap_bytes(&self) -> HeapBytes<N> {
                let mut buf = HeapBytes::<N>::new();
                let _ = buf.extend_from_slice(self.as_bytes());
                buf
            }

            /// 拷贝含结尾 `\0` 的完整数据生成堆安全字节容器 [`HeapBytes<N>`]。
            #[cfg(feature = "alloc")]
            #[inline]
            pub fn to_heap_bytes_with_nul(&self) -> HeapBytes<N> {
                let mut buf = HeapBytes::<N>::new();
                let _ = buf.extend_from_slice(self.as_bytes_with_nul());
                buf
            }

            #[cfg_attr(coin24, cold)]
            #[cfg_attr(coin121, inline(never))]
            #[cfg_attr(all(not(coin121), coin110), inline(always))]
            #[cfg_attr(all(not(coin121), not(coin110)), inline)]
            fn try_push_bytes(&mut self, payload: &[u8]) -> Result<()> {
                let cur_len = self.len();
                if let Some(pos) = payload.iter().position(|&b| b == 0) {
                    bail!(ErrType::InteriorNul(cur_len + pos));
                }
                let required = cur_len
                    .checked_add(payload.len())
                    .and_then(|v| v.checked_add(1))
                    .ok_or_else(|| {
                        err!(ErrType::CapacityExceeded {
                            requested: usize::MAX,
                            max: N,
                        })
                    })?;
                if required > N {
                    bail!(ErrType::CapacityExceeded {
                        requested: required,
                        max: N,
                    });
                }
                let new_payload_len = cur_len + payload.len();
                self.0.data[cur_len..new_payload_len].copy_from_slice(payload);
                self.0.data[new_payload_len] = 0;
                self.0.len = required;
                Ok(())
            }

            /// 在末尾追加不含 `\0` 的载荷字节切片，并自动维护结尾 `\0`。
            ///
            /// # Errors
            ///
            /// - 当 `payload` 含有内部 `\0` 时返回 [`ErrType::InteriorNul`]，容器内容不变。
            /// - 当追加后总长（含 `\0`）超出 `N` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn push_bytes(&mut self, payload: &[u8]) -> Result<()> {
                self.try_push_bytes(payload)
            }

            /// 在末尾追加字符串切片 `&str`，并自动维护结尾 `\0`。
            ///
            /// # Errors
            ///
            /// - 当 `s` 含内部 `\0` 时返回 [`ErrType::InteriorNul`]，容器内容不变。
            /// - 当追加后总长（含 `\0`）超出 `N` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn push_str(&mut self, s: &str) -> Result<()> {
                self.try_push_bytes(s.as_bytes())
            }

            /// 在末尾追加标准库 `&core::ffi::CStr` 的有效载荷。
            ///
            /// # Errors
            ///
            /// - 当追加后总长（含 `\0`）超出 `N` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn push_cstr(&mut self, s: &StdCStr) -> Result<()> {
                self.try_push_bytes(s.to_bytes())
            }

            /// 将 `source` 载荷追加到自身，成功后立即清零 `source`；若失败则 `source` 保持不变。
            ///
            /// # Errors
            ///
            /// - 当 `source` 含内部 `\0` 时返回 [`ErrType::InteriorNul`]，`source` 保持不变。
            /// - 当追加后总长（含 `\0`）超出 `N` 时返回 [`ErrType::CapacityExceeded`]，`source` 保持不变。
            #[inline]
            pub fn extend_and_wipe<S>(&mut self, mut source: S) -> Result<()>
            where
                S: AsRef<[u8]> + AsMut<[u8]>,
            {
                let res = self.try_push_bytes(source.as_ref());
                if res.is_ok() {
                    volatile_zero(source.as_mut());
                }
                res
            }

            /// 从原始可变指针拷贝并擦除源缓冲构造安全 C 字符串。
            ///
            /// 在 `len` 范围内找首个 `\0` 截断为载荷；找不到则报
            /// [`ErrType::MissingNul`]。无论成败都清零源 `len` 字节。
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
            /// - `len` 内无 `\0` 时返回 [`ErrType::MissingNul`]。
            /// - 载荷（含结尾 `\0`）超出 `N` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(any(coin44, coin39), cold)]
            #[cfg_attr(coin66, inline(never))]
            #[cfg_attr(not(coin66), inline(always))]
            pub unsafe fn from_raw_parts_mut(ptr: *mut u8, len: usize) -> Result<Self> {
                if ptr.is_null() {
                    bail!(ErrType::NullPointer);
                }
                if len == 0 {
                    bail!(ErrType::MissingNul);
                }

                // SAFETY: `ptr` 非空已检查；有效性、对齐与独占性由本函数的 `# Safety` 约定保证。
                let slice = unsafe { ::core::slice::from_raw_parts_mut(ptr, len) };

                let mut out = Self::new();
                safe_parse_cstr_and_wipe(slice, |p| out.try_push_bytes(p))?;
                Ok(out)
            }

            /// 将内容搬运到容量为 `M` 的同类 C 字符串中（直接委托底层 `Buffer::try_grow`）。
            ///
            /// 本方法按值取走 `self`，原容器在返回前（无论成败）自动被清零。
            ///
            /// # Errors
            ///
            /// - 当已有长度（含 `\0`）超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(all(coin122, coin116), cold)]
            #[cfg_attr(coin26, inline(never))]
            #[cfg_attr(not(coin26), inline(always))]
            pub fn try_grow<const M: usize>(self) -> Result<$name<M>> {
                self.0.try_grow::<M>().map($name)
            }

            /// 将自身与 `suffix` 拼接为容量 `M` 的新 C 字符串。
            ///
            /// `suffix` 为不含 `\0` 的载荷字节（允许非 UTF-8）；含 `\0` 时报
            /// [`ErrType::InteriorNul`]，其下标为拼接后载荷中的绝对下标。
            /// 本方法按值取走 `self`，原容器在返回前（无论成败）被清零。
            ///
            /// # Errors
            ///
            /// - 当拼接后长度（含 `\0`）超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            /// - 当 `suffix` 含 `\0` 时返回 [`ErrType::InteriorNul`]。
            #[cfg_attr(all(coin127, coin122), cold)]
            #[cfg_attr(all(coin95, coin75), inline(never))]
            #[cfg_attr(not(all(coin95, coin75)), inline(always))]
            pub fn push_bytes_into<const M: usize>(self, suffix: &[u8]) -> Result<$name<M>> {
                let cur_len = self.len();
                if let Some(rel) = suffix.iter().position(|&b| b == 0) {
                    bail!(ErrType::InteriorNul(cur_len + rel));
                }
                let required = cur_len
                    .checked_add(suffix.len())
                    .and_then(|v| v.checked_add(1))
                    .ok_or_else(|| {
                        err!(ErrType::CapacityExceeded {
                            requested: usize::MAX,
                            max: M,
                        })
                    })?;
                if required > M {
                    bail!(ErrType::CapacityExceeded {
                        requested: required,
                        max: M,
                    });
                }
                let mut larger = $name::<M>::new();
                let new_payload_len = cur_len + suffix.len();
                larger.0.data[..cur_len].copy_from_slice(self.as_bytes());
                larger.0.data[cur_len..new_payload_len].copy_from_slice(suffix);
                larger.0.data[new_payload_len] = 0;
                larger.0.len = required;
                Ok(larger)
            }

            /// 将自身与 `rhs` 拼接为容量 `M` 的新 C 字符串（`rhs` 为任意可借用为字节切片的类型）。
            ///
            /// 本方法按值取走 `self`，原容器在返回前（无论成败）被清零。
            ///
            /// # Errors
            ///
            /// - 当拼接后长度（含 `\0`）超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            /// - 当 `rhs` 含 `\0` 时返回 [`ErrType::InteriorNul`]，其下标为拼接后载荷中的绝对下标。
            #[cfg_attr(all(coin120, coin32), cold)]
            #[cfg_attr(any(coin69, coin73), inline(never))]
            #[cfg_attr(not(any(coin69, coin73)), inline(always))]
            pub fn concat_into<const M: usize>(
                self,
                rhs: impl ::core::convert::AsRef<[u8]>,
            ) -> Result<$name<M>> {
                self.push_bytes_into(rhs.as_ref())
            }
        }

        impl<const N: usize> ::core::default::Default for $name<N> {
            #[cfg_attr(coin27, cold)]
            #[cfg_attr(coin102, inline(never))]
            #[cfg_attr(all(not(coin102), coin31), inline(always))]
            #[cfg_attr(all(not(coin102), not(coin31)), inline)]
            fn default() -> Self {
                Self::new()
            }
        }

        impl<const N: usize> CStr for $name<N> {
            #[cfg_attr(coin108, cold)]
            #[cfg_attr(all(coin121, coin17), inline(never))]
            #[cfg_attr(not(all(coin121, coin17)), inline(always))]
            fn as_cstr(&self) -> &StdCStr {
                self.as_cstr()
            }

            #[inline]
            fn as_bytes(&self) -> &[u8] {
                self.as_bytes()
            }

            #[inline]
            fn as_bytes_with_nul(&self) -> &[u8] {
                self.as_bytes_with_nul()
            }

            #[inline]
            fn as_ptr(&self) -> *const core::ffi::c_char {
                self.as_ptr()
            }

            #[inline]
            fn len(&self) -> usize {
                self.len()
            }

            #[inline]
            fn is_empty(&self) -> bool {
                self.is_empty()
            }

            #[inline]
            fn ct_eq(&self, other: &[u8]) -> bool {
                self.ct_eq(other)
            }

            #[cfg(feature = "alloc")]
            #[cfg_attr(any(coin29, coin2), cold)]
            #[cfg_attr(coin22, inline(never))]
            #[cfg_attr(all(not(coin22), coin96), inline(always))]
            #[cfg_attr(all(not(coin22), not(coin96)), inline)]
            fn dyn_clone(&self) -> alloc::boxed::Box<dyn CStr> {
                alloc::boxed::Box::new(self.clone())
            }
        }

        impl<const N: usize> ::core::clone::Clone for $name<N> {
            #[cfg_attr(any(coin67, coin43), cold)]
            #[cfg_attr(coin19, inline(never))]
            #[cfg_attr(all(not(coin19), coin6), inline(always))]
            #[cfg_attr(all(not(coin19), not(coin6)), inline)]
            fn clone(&self) -> Self {
                let mut out = Self::new();
                out.0.data[..self.0.len].copy_from_slice(self.0.as_slice());
                out.0.len = self.0.len;
                out
            }
        }
    };
}

macro_rules! impl_try_from_traits {
    ($name:ident) => {
        impl<const N: usize> TryFrom<&StdCStr> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(coin61, cold)]
            #[cfg_attr(coin108, inline(never))]
            #[cfg_attr(not(coin108), inline(always))]
            fn try_from(s: &StdCStr) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                out.try_push_bytes(s.to_bytes())?;
                Ok(out)
            }
        }

        impl<const N: usize> TryFrom<&str> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(coin8, cold)]
            #[cfg_attr(any(coin37, coin52), inline(never))]
            #[cfg_attr(not(any(coin37, coin52)), inline(always))]
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                out.try_push_bytes(s.as_bytes())?;
                Ok(out)
            }
        }

        impl<const N: usize> TryFrom<&[u8]> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(coin53, cold)]
            #[cfg_attr(coin75, inline(never))]
            #[cfg_attr(all(not(coin75), coin76), inline(always))]
            #[cfg_attr(all(not(coin75), not(coin76)), inline)]
            fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
                match s.iter().position(|&b| b == 0) {
                    None => Err(err!(ErrType::MissingNul)),
                    Some(pos) => {
                        let mut out = Self::new();
                        out.try_push_bytes(&s[..pos])?;
                        Ok(out)
                    }
                }
            }
        }

        impl<const N: usize> ::core::str::FromStr for $name<N> {
            type Err = crate::errors::Error;
            #[cfg_attr(coin110, cold)]
            #[cfg_attr(any(coin96, coin100), inline(never))]
            #[cfg_attr(not(any(coin96, coin100)), inline(always))]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s)
            }
        }

        impl<const N: usize> TryFrom<&mut [u8; N]> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(any(coin4, coin87), cold)]
            #[cfg_attr(all(coin0, coin23), inline(never))]
            #[cfg_attr(not(all(coin0, coin23)), inline(always))]
            fn try_from(arr: &mut [u8; N]) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_cstr_and_wipe(&mut arr[..], |p| out.try_push_bytes(p))?;
                Ok(out)
            }
        }

        impl<const N: usize> TryFrom<&mut [u8]> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(coin78, cold)]
            #[cfg_attr(all(coin103, coin28), inline(never))]
            #[cfg_attr(not(all(coin103, coin28)), inline(always))]
            fn try_from(arr: &mut [u8]) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_cstr_and_wipe(arr, |p| out.try_push_bytes(p))?;
                Ok(out)
            }
        }

        // ==================== 与 StackBytes<M> 联动转换 ====================

        /// 从含 `\0` 的 [`StackBytes<M>`] 只读引用解析构造 C 字符串。
        impl<const N: usize, const M: usize> TryFrom<&StackBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: &StackBytes<M>) -> Result<Self, Self::Error> {
                Self::try_from(buf.as_slice())
            }
        }

        /// 从含 `\0` 的 [`StackBytes<M>`] 可变引用解析构造 C 字符串，无论成败均调用 [`StackBytes::wipe_data`] 擦除源容器。
        impl<const N: usize, const M: usize> TryFrom<&mut StackBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: &mut StackBytes<M>) -> Result<Self, Self::Error> {
                let res = Self::try_from(buf.as_slice());
                buf.wipe_data();
                res
            }
        }

        /// 按值消费含 `\0` 的 [`StackBytes<M>`] 构造 C 字符串（源容器随作用域结束自动 `Drop` 清零）。
        impl<const N: usize, const M: usize> TryFrom<StackBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: StackBytes<M>) -> Result<Self, Self::Error> {
                Self::try_from(buf.as_slice())
            }
        }

        // ==================== Alloc 特性相关转换 ====================

        /// 从含 `\0` 的 [`HeapBytes<M>`] 只读引用解析构造 C 字符串。
        #[cfg(feature = "alloc")]
        impl<const N: usize, const M: usize> TryFrom<&HeapBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: &HeapBytes<M>) -> Result<Self, Self::Error> {
                Self::try_from(buf.as_slice())
            }
        }

        /// 从含 `\0` 的 [`HeapBytes<M>`] 可变引用解析构造 C 字符串，无论成败均调用 [`HeapBytes::wipe_data`] 擦除源容器。
        #[cfg(feature = "alloc")]
        impl<const N: usize, const M: usize> TryFrom<&mut HeapBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: &mut HeapBytes<M>) -> Result<Self, Self::Error> {
                let res = Self::try_from(buf.as_slice());
                buf.wipe_data();
                res
            }
        }

        /// 按值消费含 `\0` 的 [`HeapBytes<M>`] 构造 C 字符串（源容器随作用域结束自动 `Drop` 清零）。
        #[cfg(feature = "alloc")]
        impl<const N: usize, const M: usize> TryFrom<HeapBytes<M>> for $name<N> {
            type Error = crate::errors::Error;
            #[inline]
            fn try_from(buf: HeapBytes<M>) -> Result<Self, Self::Error> {
                Self::try_from(buf.as_slice())
            }
        }

        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::boxed::Box<[u8; N]>> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(any(coin42, coin22), cold)]
            #[cfg_attr(all(coin26, coin44), inline(never))]
            #[cfg_attr(not(all(coin26, coin44)), inline(always))]
            fn try_from(mut b: alloc::boxed::Box<[u8; N]>) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_cstr_and_wipe(&mut *b, |p| out.try_push_bytes(p))?;
                Ok(out)
            }
        }

        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::vec::Vec<u8>> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(all(coin18, coin8), cold)]
            #[cfg_attr(any(coin68, coin70), inline(never))]
            #[cfg_attr(not(any(coin68, coin70)), inline(always))]
            fn try_from(mut v: alloc::vec::Vec<u8>) -> Result<Self, Self::Error> {
                let mut out = Self::new();
                safe_parse_cstr_and_wipe(v.as_mut_slice(), |p| out.try_push_bytes(p))?;
                Ok(out)
            }
        }

        /// 从 [`alloc::string::String`] 构造 C 字符串（与 `TryFrom<&str>` 语义一致，自动补结尾 `\0` 并清零原 `String` 堆缓冲）。
        #[cfg(feature = "alloc")]
        impl<const N: usize> TryFrom<alloc::string::String> for $name<N> {
            type Error = crate::errors::Error;
            #[cfg_attr(any(coin18, coin126), cold)]
            #[cfg_attr(coin100, inline(never))]
            #[cfg_attr(all(not(coin100), coin121), inline(always))]
            #[cfg_attr(all(not(coin100), not(coin121)), inline)]
            fn try_from(s: alloc::string::String) -> Result<Self, Self::Error> {
                let mut bytes = s.into_bytes();
                let mut out = Self::new();
                let res = out.try_push_bytes(bytes.as_slice());
                volatile_zero(bytes.as_mut_slice());
                res?;
                Ok(out)
            }
        }
    };
}

#[cfg_attr(all(coin67, coin100), cold)]
#[cfg_attr(coin82, inline(never))]
#[cfg_attr(all(not(coin82), coin93), inline(always))]
#[cfg_attr(all(not(coin82), not(coin93)), inline)]
fn fmt_cstr_lossy(f: &mut core::fmt::Formatter<'_>, mut rest: &[u8]) -> core::fmt::Result {
    loop {
        match core::str::from_utf8(rest) {
            Ok(valid) => return f.write_str(valid),
            Err(e) => {
                let valid = e.valid_up_to();
                if valid > 0 {
                    // SAFETY: `valid_up_to` 前缀经校验为合法 UTF-8。
                    f.write_str(unsafe { core::str::from_utf8_unchecked(&rest[..valid]) })?;
                }
                f.write_str("\u{FFFD}")?;
                rest = &rest[valid + e.error_len().unwrap_or(1)..];
                if rest.is_empty() {
                    return Ok(());
                }
            }
        }
    }
}

macro_rules! __impl_common_cstr_traits {
    ($name:ident) => {
        impl<const N: usize> ::core::ops::Deref for $name<N> {
            type Target = StdCStr;
            #[cfg_attr(all(coin123, coin105), cold)]
            #[cfg_attr(any(coin32, coin84), inline(never))]
            #[cfg_attr(not(any(coin32, coin84)), inline(always))]
            fn deref(&self) -> &Self::Target {
                self.as_cstr()
            }
        }
        impl<const N: usize> ::core::fmt::Display for $name<N> {
            #[cfg_attr(coin15, cold)]
            #[cfg_attr(all(coin90, coin64), inline(never))]
            #[cfg_attr(not(all(coin90, coin64)), inline(always))]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                fmt_cstr_lossy(f, self.as_bytes())
            }
        }
        impl<const N: usize> ::core::fmt::Debug for $name<N> {
            #[cfg_attr(coin100, cold)]
            #[cfg_attr(coin77, inline(never))]
            #[cfg_attr(all(not(coin77), coin117), inline(always))]
            #[cfg_attr(all(not(coin77), not(coin117)), inline)]
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Debug::fmt(self.as_cstr(), f)
            }
        }
        impl<const N: usize> ::core::convert::AsRef<StdCStr> for $name<N> {
            #[cfg_attr(all(coin90, coin61), cold)]
            #[cfg_attr(coin66, inline(never))]
            #[cfg_attr(all(not(coin66), coin124), inline(always))]
            #[cfg_attr(all(not(coin66), not(coin124)), inline)]
            fn as_ref(&self) -> &StdCStr {
                self.as_cstr()
            }
        }
        impl<const N: usize> ::core::convert::AsRef<[u8]> for $name<N> {
            #[cfg_attr(any(coin105, coin55), cold)]
            #[cfg_attr(coin123, inline(never))]
            #[cfg_attr(all(not(coin123), coin83), inline(always))]
            #[cfg_attr(all(not(coin123), not(coin83)), inline)]
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }
        impl<const N: usize> ::core::cmp::PartialEq<StdCStr> for $name<N> {
            #[cfg_attr(coin77, cold)]
            #[cfg_attr(all(coin49, coin39), inline(never))]
            #[cfg_attr(not(all(coin49, coin39)), inline(always))]
            fn eq(&self, other: &StdCStr) -> bool {
                self.as_cstr() == other
            }
        }
        impl<'a, const N: usize> ::core::cmp::PartialEq<&'a StdCStr> for $name<N> {
            #[cfg_attr(any(coin115, coin75), cold)]
            #[cfg_attr(any(coin127, coin41), inline(never))]
            #[cfg_attr(not(any(coin127, coin41)), inline(always))]
            fn eq(&self, other: &&'a StdCStr) -> bool {
                self.as_cstr() == *other
            }
        }
        impl<const N: usize> ::core::cmp::PartialEq<str> for $name<N> {
            #[cfg_attr(coin82, cold)]
            #[cfg_attr(all(coin78, coin98), inline(never))]
            #[cfg_attr(not(all(coin78, coin98)), inline(always))]
            fn eq(&self, other: &str) -> bool {
                self.as_bytes() == other.as_bytes()
            }
        }
        impl<'a, const N: usize> ::core::cmp::PartialEq<&'a str> for $name<N> {
            #[cfg_attr(coin72, cold)]
            #[cfg_attr(all(coin69, coin124), inline(never))]
            #[cfg_attr(not(all(coin69, coin124)), inline(always))]
            fn eq(&self, other: &&'a str) -> bool {
                self.as_bytes() == other.as_bytes()
            }
        }
        impl<const N: usize> ::core::cmp::PartialEq<[u8]> for $name<N> {
            #[inline]
            fn eq(&self, other: &[u8]) -> bool {
                self.as_bytes() == other
            }
        }
        impl<'a, const N: usize> ::core::cmp::PartialEq<&'a [u8]> for $name<N> {
            #[inline]
            fn eq(&self, other: &&'a [u8]) -> bool {
                self.as_bytes() == *other
            }
        }
        impl<const N: usize, const M: usize> ::core::cmp::PartialEq<$name<M>> for $name<N> {
            #[cfg_attr(all(coin59, coin46), cold)]
            #[cfg_attr(any(coin124, coin11), inline(never))]
            #[cfg_attr(not(any(coin124, coin11)), inline(always))]
            fn eq(&self, other: &$name<M>) -> bool {
                self.as_cstr() == other.as_cstr()
            }
        }
        #[cfg(feature = "alloc")]
        impl<const N: usize> ::core::cmp::PartialEq<alloc::string::String> for $name<N> {
            #[cfg_attr(coin39, cold)]
            #[cfg_attr(coin76, inline(never))]
            #[cfg_attr(all(not(coin76), coin51), inline(always))]
            #[cfg_attr(all(not(coin76), not(coin51)), inline)]
            fn eq(&self, other: &alloc::string::String) -> bool {
                self.as_bytes() == other.as_bytes()
            }
        }
        impl<const N: usize> ::core::cmp::Eq for $name<N> {}
        impl<const N: usize> ::core::cmp::PartialOrd for $name<N> {
            #[cfg_attr(any(coin14, coin65), cold)]
            #[cfg_attr(all(coin104, coin121), inline(never))]
            #[cfg_attr(not(all(coin104, coin121)), inline(always))]
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl<const N: usize> ::core::cmp::Ord for $name<N> {
            #[cfg_attr(coin99, cold)]
            #[cfg_attr(coin81, inline(never))]
            #[cfg_attr(not(coin81), inline(always))]
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.as_cstr().cmp(other.as_cstr())
            }
        }
        impl<const N: usize> ::core::hash::Hash for $name<N> {
            #[cfg_attr(all(coin93, coin37), cold)]
            #[cfg_attr(all(coin29, coin96), inline(never))]
            #[cfg_attr(not(all(coin29, coin96)), inline(always))]
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                // 必须与 `core::ffi::CStr` 自身的 `Hash`（派生自含结尾 `\0` 的内部切片）一致，
                // 否则违反 `Borrow<StdCStr>` 的哈希契约（以 `&CStr` 查 `HashMap` 会失效）。
                self.as_cstr().hash(state)
            }
        }
        impl<const N: usize> ::core::borrow::Borrow<StdCStr> for $name<N> {
            #[cfg_attr(all(coin42, coin40), cold)]
            #[cfg_attr(coin94, inline(never))]
            #[cfg_attr(all(not(coin94), coin107), inline(always))]
            #[cfg_attr(all(not(coin94), not(coin107)), inline)]
            fn borrow(&self) -> &StdCStr {
                self.as_cstr()
            }
        }
    };
}

impl_secure_cstr_base!(StackCStr, StackBytes);
impl_try_from_traits!(StackCStr);
__impl_common_cstr_traits!(StackCStr);

#[cfg(feature = "alloc")]
impl_secure_cstr_base!(HeapCStr, HeapBytes);
#[cfg(feature = "alloc")]
impl_try_from_traits!(HeapCStr);
#[cfg(feature = "alloc")]
__impl_common_cstr_traits!(HeapCStr);

#[cfg(feature = "alloc")]
impl<const N: usize> StackCStr<N> {
    /// 将栈 C 字符串搬运为容量 `M` 的堆 C 字符串。
    ///
    /// 本方法按值取走 `self`，原栈容器在返回前（无论成败）自动经 `Drop` 清零。
    ///
    /// # Errors
    ///
    /// - 当已有长度（含 `\0`）超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
    #[cfg_attr(any(coin36, coin63), cold)]
    #[cfg_attr(coin14, inline(never))]
    #[cfg_attr(not(coin14), inline(always))]
    pub fn into_heap<const M: usize>(self) -> Result<HeapCStr<M>> {
        if self.0.len > M {
            bail!(ErrType::CapacityExceeded {
                requested: self.0.len,
                max: M,
            });
        }
        let mut heap = HeapCStr::<M>::new();
        heap.0.data[..self.0.len].copy_from_slice(self.0.as_slice());
        heap.0.len = self.0.len;
        Ok(heap)
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize> HeapCStr<N> {
    /// 将堆 C 字符串搬运为容量 `M` 的栈 C 字符串。
    ///
    /// 本方法按值取走 `self`，原堆容器在返回前（无论成败）自动经 `Drop` 清零并释放。
    ///
    /// # Errors
    ///
    /// - 当已有长度（含 `\0`）超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
    #[inline]
    pub fn into_stack<const M: usize>(self) -> Result<StackCStr<M>> {
        if self.0.len > M {
            bail!(ErrType::CapacityExceeded {
                requested: self.0.len,
                max: M,
            });
        }
        let mut stack = StackCStr::<M>::new();
        stack.0.data[..self.0.len].copy_from_slice(self.0.as_slice());
        stack.0.len = self.0.len;
        Ok(stack)
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize, const M: usize> ::core::cmp::PartialEq<StackCStr<M>> for HeapCStr<N> {
    #[cfg_attr(any(coin25, coin41), cold)]
    #[cfg_attr(coin2, inline(never))]
    #[cfg_attr(all(not(coin2), coin114), inline(always))]
    #[cfg_attr(all(not(coin2), not(coin114)), inline)]
    fn eq(&self, other: &StackCStr<M>) -> bool {
        self.as_cstr() == other.as_cstr()
    }
}
#[cfg(feature = "alloc")]
impl<const N: usize, const M: usize> ::core::cmp::PartialEq<HeapCStr<N>> for StackCStr<M> {
    #[cfg_attr(any(coin51, coin119), cold)]
    #[cfg_attr(coin71, inline(never))]
    #[cfg_attr(not(coin71), inline(always))]
    fn eq(&self, other: &HeapCStr<N>) -> bool {
        self.as_cstr() == other.as_cstr()
    }
}

#[cfg(feature = "alloc")]
impl ::core::clone::Clone for alloc::boxed::Box<dyn CStr> {
    #[cfg_attr(coin69, cold)]
    #[cfg_attr(coin60, inline(never))]
    #[cfg_attr(not(coin60), inline(always))]
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::PartialEq for alloc::boxed::Box<dyn CStr> {
    #[cfg_attr(coin118, cold)]
    #[cfg_attr(coin61, inline(never))]
    #[cfg_attr(not(coin61), inline(always))]
    fn eq(&self, other: &Self) -> bool {
        self.as_cstr() == other.as_cstr()
    }
}
#[cfg(feature = "alloc")]
impl ::core::cmp::Eq for alloc::boxed::Box<dyn CStr> {}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{ErrType, Result, StackBytes, StackCStr};
    #[cfg(feature = "alloc")]
    use super::{HeapBytes, HeapCStr};
    #[cfg(feature = "alloc")]
    use alloc::{boxed::Box, string::String, vec, vec::Vec};
    use core::convert::{TryFrom, TryInto};

    #[test]
    fn test_inherent_methods_without_cstr_trait_imported() {
        // 本测试模块未在顶部导入 `super::CStr`，验证所有固有方法在未引入 Trait 时均可直接调用
        let mut s: StackCStr<8> = "hi".try_into().unwrap();
        assert_eq!(s.capacity(), 8);
        assert_eq!(s.max_payload_len(), 7);
        assert_eq!(s.len(), 2);
        assert_eq!(s.remaining_capacity(), 5);
        assert!(!s.is_empty());
        assert_eq!(s.as_cstr(), c"hi");
        assert_eq!(s.as_bytes(), b"hi");
        assert_eq!(s.as_bytes_with_nul(), b"hi\0");
        assert_eq!(s.as_buffer().len(), 3); // 底层 StackBytes 长度为 len() + 1

        // 追加与常量时间比较
        s.push_str("!").unwrap();
        assert_eq!(s.as_bytes(), b"hi!");
        assert!(s.ct_eq(b"hi!"));
        assert!(!s.ct_eq(b"hi?"));
        assert!(s.ct_eq_cstr(c"hi!"));

        // 截断并验证底层被截断字节已通过 Buffer::truncate 易失清零
        s.truncate(2);
        assert_eq!(s.len(), 2);
        assert_eq!(s.as_bytes_with_nul(), b"hi\0");
        assert_eq!(s.as_buffer().as_slice(), b"hi\0");

        // 清空并验证恢复为空 C 字符串
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.as_bytes_with_nul(), b"\0");
    }

    #[test]
    fn test_zero_copy_in_place_buffer_conversions() {
        // 1. 原地零拷贝：StackBytes<16>（纯载荷） -> StackCStr<16> -> StackBytes<16>
        let mut raw = StackBytes::<16>::new();
        raw.extend_from_slice(b"zero_copy").unwrap();

        let cstr = StackCStr::<16>::from_inner_payload(raw).unwrap();
        assert_eq!(cstr.as_bytes_with_nul(), b"zero_copy\0");

        let back_no_nul = cstr.clone().into_bytes();
        assert_eq!(back_no_nul.as_slice(), b"zero_copy");

        let back_with_nul = cstr.into_bytes_with_nul();
        assert_eq!(back_with_nul.as_slice(), b"zero_copy\0");

        // 2. 原地零拷贝：含 `\0` 及尾部垃圾数据的 StackBytes<16> -> StackCStr<16>
        let mut dirty = StackBytes::<16>::new();
        dirty.extend_from_slice(b"clean\0garbage!!").unwrap();
        let cstr2 = StackCStr::<16>::from_inner_with_nul(dirty).unwrap();
        assert_eq!(cstr2.as_bytes(), b"clean");
        assert_eq!(cstr2.as_bytes_with_nul(), b"clean\0");
        // 验证原 `\0` 之后的 garbage 已被 truncate 清零
        assert_eq!(&cstr2.as_buffer().data[6..], &[0u8; 10]);
    }

    #[test]
    fn test_cross_capacity_buffer_interop() {
        // 不同容量间转换并自动 wipe 源容器
        let mut b = StackBytes::<32>::new();
        b.extend_from_slice(b"buffer_pass\0tail").unwrap();

        let s = StackCStr::<16>::try_from(&mut b).unwrap();
        assert_eq!(s.as_bytes(), b"buffer_pass");
        assert!(b.is_empty());

        let mut payload_buf = StackBytes::<32>::new();
        payload_buf.extend_from_slice(b"raw_secret").unwrap();
        let s2 = StackCStr::<16>::from_buffer_payload_and_wipe(&mut payload_buf).unwrap();
        assert_eq!(s2.as_bytes_with_nul(), b"raw_secret\0");
        assert!(payload_buf.is_empty());
    }

    #[test]
    fn test_stack_from_str_literal() {
        let s: StackCStr<8> = "hello".try_into().unwrap();
        assert_eq!(&*s, c"hello");
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_bytes_with_nul(), b"hello\0");
    }

    #[test]
    fn test_non_utf8_payload() {
        let s: StackCStr<8> = StackCStr::try_from([0xFFu8, 0xFE, 0].as_slice()).unwrap();
        assert_eq!(s.as_bytes(), &[0xFF, 0xFE]);
        let _ = alloc_display(&s);
    }

    #[cfg(feature = "alloc")]
    fn alloc_display(s: &StackCStr<8>) -> alloc::string::String {
        use alloc::string::ToString;
        s.to_string()
    }

    #[cfg(not(feature = "alloc"))]
    fn alloc_display(s: &StackCStr<8>) -> usize {
        use core::fmt::Write;
        struct Counter(usize);
        impl Write for Counter {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                self.0 += s.len();
                Ok(())
            }
        }
        let mut c = Counter(0);
        let _ = write!(c, "{s}");
        c.0
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_heap_zero_copy_and_string() {
        let h: HeapCStr<16> = "hello".try_into().unwrap();
        assert_eq!(&*h, c"hello");

        // 验证 HeapCStr <-> HeapBytes 原地零堆分配转移（对比底层堆地址不变）
        let ptr_before = h.as_ptr() as usize;
        let hb: HeapBytes<16> = h.into_bytes();
        assert_eq!(hb.as_ptr() as usize, ptr_before);
        assert_eq!(hb.as_slice(), b"hello");

        let h_restored = HeapCStr::<16>::from_inner_payload(hb).unwrap();
        assert_eq!(h_restored.as_ptr() as usize, ptr_before);
        assert_eq!(&*h_restored, c"hello");

        // 验证 TryFrom<String>
        let owned = String::from("owned_secret");
        let h2 = HeapCStr::<16>::try_from(owned).unwrap();
        assert_eq!(h2.as_bytes(), b"owned_secret");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_vec_dyn_cstr() {
        use super::CStr;
        let mut list: Vec<Box<dyn CStr>> = Vec::new();
        let h: HeapCStr<256> = "/tmp/secret".try_into().unwrap();
        list.push(Box::new(h));
        let s: StackCStr<16> = "/bin/sh".try_into().unwrap();
        list.push(Box::new(s));
        assert_eq!(list.len(), 2);
        assert!(list[1].as_bytes().starts_with(b"/bin"));
        assert!(list[1].ct_eq(b"/bin/sh"));

        let cloned = list.clone();
        assert_eq!(cloned, list);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn from_raw_ptr_with_wiping() {
        let mut source_data = b"/tmp/x\0".to_vec();
        let ptr = source_data.as_mut_ptr();
        let len = source_data.len();

        // SAFETY: `ptr` 指向 `source_data` 的独占可写借用派生的有效内存，长度 `len` 精确覆盖向量本体，测试内无并发访问。
        let h = unsafe { HeapCStr::<32>::from_raw_parts_mut(ptr, len) }.unwrap();
        assert_eq!(h.as_bytes(), b"/tmp/x");
        assert_eq!(source_data, vec![0u8; 7]);
    }

    #[test]
    fn test_capacity_exceeded_fail_fast() {
        let res: Result<StackCStr<4>> = "hello".try_into();
        assert!(res.is_err());
        let ok: Result<StackCStr<4>> = "hey".try_into();
        assert!(ok.is_ok());
    }

    #[test]
    fn test_interior_nul_rejected() {
        let res = StackCStr::<16>::try_from("a\0b");
        assert_eq!(res.unwrap_err().err_type(), ErrType::InteriorNul(1));

        let mut s: StackCStr<16> = "hi".try_into().unwrap();
        let err = s.push_bytes(b"a\0b").unwrap_err();
        assert_eq!(err.err_type(), ErrType::InteriorNul(3));
    }

    #[test]
    fn test_missing_nul_wiping() {
        let mut no_nul = *b"abc";
        let res = StackCStr::<16>::try_from(no_nul.as_mut_slice());
        assert_eq!(res.unwrap_err().err_type(), ErrType::MissingNul);
        assert_eq!(no_nul, [0u8; 3]);
    }

    #[test]
    fn test_truncates_at_first_nul_and_wipes() {
        let mut buf = *b"hi\0garbage";
        let s = StackCStr::<16>::try_from(buf.as_mut_slice()).unwrap();
        assert_eq!(s.as_bytes(), b"hi");
        assert_eq!(buf, [0u8; 10]);
    }

    #[test]
    fn test_capacity_one_holds_only_empty() {
        let e: StackCStr<1> = "".try_into().unwrap();
        assert!(e.is_empty());
        assert_eq!(e.as_bytes_with_nul(), b"\0");
        let res: Result<StackCStr<1>> = "a".try_into();
        assert!(matches!(
            res.unwrap_err().err_type(),
            ErrType::CapacityExceeded { .. }
        ));
    }

    #[test]
    fn test_try_grow_and_shrink_fails() {
        let s: StackCStr<4> = "hi".try_into().unwrap();
        let big = s.try_grow::<8>().unwrap();
        assert_eq!(&*big, c"hi");
        let res = big.try_grow::<2>();
        assert!(matches!(
            res.unwrap_err().err_type(),
            ErrType::CapacityExceeded { .. }
        ));
    }

    #[test]
    fn test_concat_into_overflow() {
        let s: StackCStr<4> = "hi".try_into().unwrap();
        let res = s.concat_into::<4>(b"!!");
        assert!(matches!(
            res.unwrap_err().err_type(),
            ErrType::CapacityExceeded { .. }
        ));
    }

    #[test]
    fn test_display_lossy_replaces_invalid() {
        use core::fmt::Write;
        let s: StackCStr<8> = StackCStr::try_from([0xFFu8, 0xFE, 0].as_slice()).unwrap();
        struct Buf<'a>(&'a mut [u8; 16], usize);
        impl Write for Buf<'_> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let b = s.as_bytes();
                self.0[self.1..self.1 + b.len()].copy_from_slice(b);
                self.1 += b.len();
                Ok(())
            }
        }
        let mut storage = [0u8; 16];
        let written = {
            let mut buf = Buf(&mut storage, 0);
            write!(buf, "{s}").unwrap();
            buf.1
        };
        assert_eq!(&storage[..written], "\u{FFFD}\u{FFFD}".as_bytes());
    }

    #[test]
    fn test_as_ptr_and_borrow_hash_consistent() {
        use core::borrow::Borrow;
        use core::ffi::CStr as StdCStr;
        use core::hash::{Hash, Hasher};
        struct Fnv(u64);
        impl Hasher for Fnv {
            fn write(&mut self, b: &[u8]) {
                for &x in b {
                    self.0 = self.0.wrapping_mul(0x100000001b3).wrapping_add(x as u64);
                }
            }
            fn finish(&self) -> u64 {
                self.0
            }
        }
        let s: StackCStr<8> = "hi".try_into().unwrap();
        assert!(!s.as_ptr().is_null());
        // SAFETY: `as_ptr` 指向 `s` 内部以 `\0` 结尾的有效存储，断言求值期间 `s` 存活且无修改。
        assert_eq!(unsafe { StdCStr::from_ptr(s.as_ptr()) }, c"hi");
        let borrowed: &StdCStr = s.borrow();
        assert_eq!(borrowed, c"hi");
        let mut h1 = Fnv(0xcbf29ce484222325);
        s.hash(&mut h1);
        let mut h2 = Fnv(0xcbf29ce484222325);
        borrowed.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_from_str_parse() {
        let s: StackCStr<8> = "hi".parse().unwrap();
        assert_eq!(&*s, c"hi");
        let res = "a\0b".parse::<StackCStr<8>>();
        assert_eq!(res.unwrap_err().err_type(), ErrType::InteriorNul(1));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_into_heap_and_into_stack_roundtrip() {
        let s: StackCStr<4> = "hi".try_into().unwrap();
        let h = s.into_heap::<8>().unwrap();
        assert_eq!(&*h, c"hi");
        assert_eq!(h, HeapCStr::<8>::try_from("hi").unwrap());

        let s2 = h.into_stack::<4>().unwrap();
        assert_eq!(&*s2, c"hi");
    }
}
