// SPDX-License-Identifier: MPL-2.0

//! 定容安全数据容器：支持多种基础位宽（`u8`/`u16`/`u32`/`u64` 等），提供栈上
//! （[`StackBuffer`](crate::types::buffer::StackBuffer) /
//! [`StackBytes`](crate::types::buffer::StackBytes)）与堆上
//! （[`HeapBuffer`](crate::types::buffer::HeapBuffer) /
//! [`HeapBytes`](crate::types::buffer::HeapBytes)，需 `alloc`）实现，`Drop` 与显式擦除均经清零。
//!
//! 需要启用 `"types-buffer"` 特性。

#![allow(rustdoc::redundant_explicit_links)]
#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]

#[cfg(feature = "alloc")]
extern crate alloc;

use crate::errors::{ErrType, bail, err};
use core::fmt;
use core::hint::black_box;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};
#[allow(unused_imports)]
use core::sync::atomic::{Ordering, compiler_fence, fence};

mod _private {
    /// sealed 占位：封闭 [`ZeroablePrimitive`]（crate 外不可实现）。
    pub trait Sealed {}
}

/// 标记可安全全零初始化、可按位复制且无内存填充（No Padding）的基础敏感数据类型。
///
/// 该 Trait 为封闭（Sealed）特性，仅在库内部为满足内存安全不变量的基础整数类型实现。
///
/// # Safety
///
/// 实现此 Trait 的类型必须满足：
/// 1. 全零位模式（All-bits-zero）是该类型的合法值（允许 `Box::new_zeroed().assume_init()`）。
/// 2. 内存布局紧凑无填充字节（允许安全地重解释为 `&[u8]` 字节切片视图）。
/// 3. 任意位模式均为合法值（允许通过 `&mut [u8]` 视图写入任意字节）。
/// 4. 实现 `Copy`（无自定义 `Drop` 副作用）。
pub unsafe trait ZeroablePrimitive:
    Copy
    + Default
    + PartialEq
    + Eq
    + BitXor<Output = Self>
    + BitXorAssign
    + BitOr<Output = Self>
    + BitOrAssign
    + BitAnd<Output = Self>
    + BitAndAssign
    + Not<Output = Self>
    + _private::Sealed
{
    /// 类型的零值常量（用于支持编译期 `const fn new()` 初始化）。
    const ZERO: Self;
}

macro_rules! impl_zeroable_primitive {
    ($($t:ty),* $(,)?) => {
        $(
            impl _private::Sealed for $t {}
            // SAFETY: 基础无符号与有符号整数类型满足全零合法、任意位模式合法、无填充字节且实现 `Copy`。
            unsafe impl ZeroablePrimitive for $t {
                const ZERO: Self = 0;
            }
        )*
    };
}

impl_zeroable_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

/// 安全容器操作的统一返回类型。
///
/// # Feature Requirement
///
/// 需要启用 `"types-buffer"` 特性。
///
/// # Errors
///
/// - 当请求长度超出容器容量时返回 [`ErrType::CapacityExceeded`]。
/// - 当源字节长度未对齐时返回 [`ErrType::UnalignedByteLength`]。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::errors::{ErrType, Result};
/// use lib_unknown::types::buffer::StackBytes;
///
/// let mut b = StackBytes::<4>::new();
/// let err: Result<()> = b.extend_from_slice(b"toolong");
/// assert_eq!(
///     err.unwrap_err().err_type(),
///     ErrType::CapacityExceeded { requested: 7, max: 4 }
/// );
/// ```
pub type BufferResult<T = ()> = crate::errors::Result<T>;

/// 将 `bytes` 按 `chunk_size` 字节分组并就地反转（仅在目标端序与主机端序不同时使用）。
///
/// `chunk_size` 由 `_le` / `_be` 路径取 `max(size_of::<M>(), size_of::<T>())`；
/// 基础整数宽度均为 2 的幂，且调用前已验证字节总长为 `size_of::<T>()` 的整数倍，
/// 因此 `bytes.len()` 必为 `chunk_size` 的整数倍。
#[inline]
fn reverse_endian_chunks(bytes: &mut [u8], chunk_size: usize) {
    if chunk_size <= 1 {
        return;
    }
    debug_assert!(chunk_size.is_power_of_two());
    debug_assert!(bytes.len().is_multiple_of(chunk_size));
    for chunk in bytes.chunks_exact_mut(chunk_size) {
        chunk.reverse();
    }
}

/// 对切片执行防编译器优化（DSE）的多态清零。
#[cfg_attr(any(dice2 = "1", dice2 = "2"), inline(always))]
#[cfg_attr(any(dice2 = "3", dice2 = "4"), inline(never))]
#[cfg_attr(dice2 = "5", inline)]
#[cfg_attr(dice2 = "6", cold)]
#[cfg_attr(dice2 = "6", inline(never))]
pub fn volatile_zero<T: ZeroablePrimitive>(buf: &mut [T]) -> T {
    let buf = black_box(buf);
    let ptr = buf.as_mut_ptr();
    let len = buf.len();

    #[cfg(any(
        dice8 = "1",
        not(any(
            dice8 = "1",
            dice8 = "2",
            dice8 = "3",
            dice8 = "4",
            dice8 = "5",
            dice8 = "6"
        ))
    ))]
    unsafe {
        // SAFETY: `ptr` 与 `len` 来自合法切片，`T` 全零位模式合法。
        core::ptr::write_bytes(black_box(ptr), 0, black_box(len));
    }

    #[cfg(dice8 = "2")]
    {
        for i in 0..black_box(len) {
            // SAFETY: `i < len`，指针在切片合法范围内且对齐。
            unsafe {
                core::ptr::write_volatile(ptr.add(i), black_box(T::ZERO));
            }
        }
    }

    #[cfg(dice8 = "3")]
    {
        let mut i = black_box(len);
        while i > 0 {
            i -= 1;
            // SAFETY: `i < len`，指针在切片合法范围内且对齐。
            unsafe {
                core::ptr::write_volatile(ptr.add(i), black_box(T::ZERO));
            }
        }
    }

    #[cfg(any(dice8 = "5", dice8 = "4"))]
    {
        // SAFETY: `T` 与 `usize` 均满足无填充、全零合法、任意位模式合法。
        let (prefix, middle, suffix) = unsafe { buf.align_to_mut::<usize>() };
        for x in prefix.iter_mut() {
            // SAFETY: `align_to_mut` 返回的前缀元素有效性由其自身保证，全零为 `T` 的合法位模式。
            unsafe { core::ptr::write_volatile(x, T::ZERO) };
        }
        for x in middle.iter_mut() {
            // SAFETY: 中间部分为对齐后的原生 `usize` 存储，写零合法。
            unsafe { core::ptr::write_volatile(x, 0usize) };
        }
        for x in suffix.iter_mut() {
            // SAFETY: 同前缀——`align_to_mut` 保证元素有效，全零为 `T` 合法位模式。
            unsafe { core::ptr::write_volatile(x, T::ZERO) };
        }
    }

    #[cfg(dice8 = "6")]
    unsafe {
        // SAFETY: 契约第 3 条保证任意位模式（如 0xAA / 0x55）对 `T` 均为合法值。
        const MASK: u8 = if cfg!(coin127) { 0xAA } else { 0x55 };
        core::ptr::write_bytes(black_box(ptr), MASK, black_box(len));
        compiler_fence(Ordering::SeqCst);
        core::ptr::write_bytes(black_box(ptr), 0, black_box(len));
    }

    black_box(buf);

    #[cfg(coin83)]
    compiler_fence(Ordering::SeqCst);
    #[cfg(not(coin83))]
    fence(Ordering::SeqCst);

    let mut ret: T = T::ZERO;

    #[cfg(any(
        dice6 = "1",
        dice6 = "2",
        not(any(
            dice6 = "1",
            dice6 = "2",
            dice6 = "3",
            dice6 = "4",
            dice6 = "5",
            dice6 = "6"
        ))
    ))]
    {
        if black_box(len) > 0 {
            // SAFETY: `len > 0` 保证首元素指针有效。
            let head = unsafe { core::ptr::read_volatile(ptr) };
            ret ^= black_box(head);
        }
    }

    #[cfg(any(dice6 = "3", dice6 = "4"))]
    {
        if black_box(len) > 0 {
            // SAFETY: `len > 0` 保证 `len - 1` 偏移有效。
            let tail = unsafe { core::ptr::read_volatile(ptr.add(len - 1)) };
            ret ^= black_box(tail);
        }
    }

    #[cfg(any(dice6 = "5", dice6 = "6"))]
    {
        if black_box(len) > 0 {
            // SAFETY: `len > 0` 保证 `len / 2` 与 `0` 偏移均在 `[0, len)` 内。
            let mid = unsafe { core::ptr::read_volatile(ptr.add(len >> 1)) };
            // SAFETY: `len > 0` 保证 `ptr` 指向的首元素有效。
            let head = unsafe { core::ptr::read_volatile(ptr) };
            ret ^= black_box(mid ^ head);
        }
    }

    ret
}

/// 定容安全容器的统一接口：写入、擦除、跨位宽字节视图与向更大容器搬运。
///
/// 泛型参数 `T` 默认为 `u8`，因此 `impl Buffer` 等价于 `impl Buffer<u8>`。
/// 所有实现（[`StackBuffer`] / [`HeapBuffer`]）在 `Drop` 与显式擦除时均经清零。
///
/// # Feature Requirement
///
/// 需要启用 `"types-buffer"` 特性。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::types::buffer::{Buffer, StackBytes};
///
/// let mut b = StackBytes::<8>::new();
/// b.extend_from_slice(b"hi").unwrap();
/// assert_eq!(b.as_slice(), b"hi");
/// b.wipe_data();
/// assert!(b.is_empty());
/// ```
pub trait Buffer<T: ZeroablePrimitive = u8>:
    core::ops::Deref<Target = [T]>
    + core::ops::DerefMut
    + AsRef<[T]>
    + AsMut<[T]>
    + Default
    + Sized
    + for<'a> TryFrom<&'a [T], Error = crate::errors::Error>
    + for<'a> TryFrom<&'a mut [T], Error = crate::errors::Error>
{
    /// 返回容器容量（元素个数），与已写入长度无关。
    fn capacity(&self) -> usize;

    /// 返回已写入数据的长度（元素个数）。
    fn len(&self) -> usize;

    /// 返回容器剩余可写入容量（元素个数），等于 `capacity() - len()`。
    #[inline]
    fn remaining_capacity(&self) -> usize {
        self.capacity().saturating_sub(self.len())
    }

    /// 已写入数据是否为空。
    #[cfg_attr(all(coin60, coin62), cold)]
    #[cfg_attr(coin101, inline(never))]
    #[cfg_attr(not(coin101), inline(always))]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 将容器长度截断至 `new_len`，并**立即清零**被丢弃的尾部区间。
    ///
    /// 若 `new_len >= self.len()`，则不执行任何操作。
    fn truncate(&mut self, new_len: usize);

    /// 移除并返回容器末尾元素，同时**立即清零**该元素在容器中的原内存槽位。
    ///
    /// 若容器为空则返回 `None`。
    fn pop(&mut self) -> Option<T>;

    /// 在容器末尾追加单个元素，并在追加后清零入参副本 `value`。
    ///
    /// # Errors
    ///
    /// - 当容器已满时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[inline]
    fn push(&mut self, mut value: T) -> BufferResult {
        let res = self.extend_from_slice(core::slice::from_ref(&value));
        volatile_zero(core::slice::from_mut(&mut value));
        res
    }

    /// 将容器长度调整为 `new_len`。
    ///
    /// - 若 `new_len > self.len()`，则以 `value` 填充新增区间（随后清零入参副本 `value`）；
    /// - 若 `new_len <= self.len()`，则截断并立即清零被丢弃的尾部区间。
    ///
    /// # Errors
    ///
    /// - 当 `new_len > self.capacity()` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    fn resize(&mut self, new_len: usize, value: T) -> BufferResult;

    /// 将容器长度调整为 `new_len`，新增元素以全零（`T::ZERO`）初始化；若缩短则清零被截断尾部。
    ///
    /// # Errors
    ///
    /// - 当 `new_len > self.capacity()` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[inline]
    fn resize_zeroed(&mut self, new_len: usize) -> BufferResult {
        self.resize(new_len, T::ZERO)
    }

    /// 在容器末尾追加 `added_len` 个全零元素（`T::ZERO`）。
    ///
    /// # Errors
    ///
    /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[inline]
    fn extend_zeroed(&mut self, added_len: usize) -> BufferResult {
        let max = self.capacity();
        let new_len = self.len().checked_add(added_len).ok_or_else(|| {
            err!(ErrType::CapacityExceeded {
                requested: usize::MAX,
                max,
            })
        })?;
        self.resize_zeroed(new_len)
    }

    /// 追加 `other` 的全部元素。
    ///
    /// # Errors
    ///
    /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    fn extend_from_slice(&mut self, other: &[T]) -> BufferResult;

    /// 按本机字节序（Native Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
    ///
    /// # Errors
    ///
    /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
    /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[cfg_attr(any(coin55, coin42), cold)]
    #[cfg_attr(any(coin56, coin4), inline(never))]
    #[cfg_attr(not(any(coin56, coin4)), inline(always))]
    fn extend_from_zeroable_primitive_slice<M: ZeroablePrimitive>(
        &mut self,
        other: &[M],
    ) -> BufferResult {
        let elem_size = core::mem::size_of::<T>();
        let byte_len = core::mem::size_of_val(other);

        if !byte_len.is_multiple_of(elem_size) {
            bail!(ErrType::UnalignedByteLength {
                byte_len,
                elem_size,
            });
        }

        let added_len = byte_len / elem_size;
        let max = self.capacity();

        let new_len = self.len().checked_add(added_len).ok_or_else(|| {
            err!(ErrType::CapacityExceeded {
                requested: usize::MAX,
                max,
            })
        })?;
        if new_len > max {
            bail!(ErrType::CapacityExceeded {
                requested: new_len,
                max,
            });
        }

        if added_len == 0 {
            return Ok(());
        }

        let src_ptr = other.as_ptr();
        if (src_ptr as usize).is_multiple_of(core::mem::align_of::<T>()) {
            // SAFETY:
            // 1. `M` 与 `T` 均实现 `ZeroablePrimitive`（无填充字节且任意位模式合法）。
            // 2. 已验证 `src_ptr` 满足 `T` 的对齐要求，且 `added_len * size_of::<T>() == byte_len`。
            let slice_t = unsafe { core::slice::from_raw_parts(src_ptr.cast::<T>(), added_len) };
            self.extend_from_slice(slice_t)
        } else {
            let src_bytes = src_ptr.cast::<u8>();
            for i in 0..added_len {
                // SAFETY:
                // 1. `i * elem_size + elem_size <= byte_len`，指针偏移在 `other` 有效范围内。
                // 2. `read_unaligned` 不要求源地址对齐，`T: ZeroablePrimitive` 保证任意位模式合法。
                let mut elem =
                    unsafe { core::ptr::read_unaligned(src_bytes.add(i * elem_size).cast::<T>()) };
                let res = self.extend_from_slice(core::slice::from_ref(&elem));
                volatile_zero(core::slice::from_mut(&mut elem));
                res?;
            }
            Ok(())
        }
    }

    /// 按小端字节序（Little Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
    ///
    /// 端序转换的分组宽度取 `max(size_of::<M>(), size_of::<T>())`：
    ///
    /// - `M` 与 `T` 同宽（或其中一侧为 1 字节）时，按较宽一侧逐元素转换；
    /// - `M > T`（源更宽）时，容器字节序列等于每个源值按小端编码后的拼接；
    /// - `M < T`（目标更宽）时，把源字节流按 `T` 大小分组，解释为目标宽度的
    ///   小端值（因此结果数值会随主机端序不同而变化）。
    ///
    /// # Errors
    ///
    /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
    /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[inline]
    fn extend_from_zeroable_primitive_slice_le<M: ZeroablePrimitive>(
        &mut self,
        other: &[M],
    ) -> BufferResult {
        let old_byte_len = self.len() * core::mem::size_of::<T>();
        self.extend_from_zeroable_primitive_slice(other)?;
        if cfg!(target_endian = "big") {
            let chunk_size = core::cmp::max(core::mem::size_of::<M>(), core::mem::size_of::<T>());
            reverse_endian_chunks(&mut self.as_mut_byte_slice()[old_byte_len..], chunk_size);
        }
        Ok(())
    }

    /// 按大端字节序（Big Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
    ///
    /// 端序转换的分组宽度取 `max(size_of::<M>(), size_of::<T>())`：
    ///
    /// - `M` 与 `T` 同宽（或其中一侧为 1 字节）时，按较宽一侧逐元素转换；
    /// - `M > T`（源更宽）时，容器字节序列等于每个源值按大端编码后的拼接；
    /// - `M < T`（目标更宽）时，把源字节流按 `T` 大小分组，解释为目标宽度的
    ///   大端值（因此结果数值会随主机端序不同而变化）。
    ///
    /// # Errors
    ///
    /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
    /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
    #[inline]
    fn extend_from_zeroable_primitive_slice_be<M: ZeroablePrimitive>(
        &mut self,
        other: &[M],
    ) -> BufferResult {
        let old_byte_len = self.len() * core::mem::size_of::<T>();
        self.extend_from_zeroable_primitive_slice(other)?;
        if cfg!(target_endian = "little") {
            let chunk_size = core::cmp::max(core::mem::size_of::<M>(), core::mem::size_of::<T>());
            reverse_endian_chunks(&mut self.as_mut_byte_slice()[old_byte_len..], chunk_size);
        }
        Ok(())
    }

    /// 清零全部底层存储并将长度置 0。
    fn wipe_data(&mut self);

    /// 以不可变切片查看已写入数据。
    #[cfg_attr(all(coin108, coin39), cold)]
    #[cfg_attr(coin30, inline(never))]
    #[cfg_attr(all(not(coin30), coin82), inline(always))]
    #[cfg_attr(all(not(coin30), not(coin82)), inline)]
    fn as_slice(&self) -> &[T] {
        self.as_ref()
    }

    /// 以可变切片访问已写入数据（不改变长度）。
    #[cfg_attr(all(coin97, coin102), cold)]
    #[cfg_attr(coin13, inline(never))]
    #[cfg_attr(all(not(coin13), coin89), inline(always))]
    #[cfg_attr(all(not(coin13), not(coin89)), inline)]
    fn as_mut_slice(&mut self) -> &mut [T] {
        self.as_mut()
    }

    /// 获取已写入数据的只读字节切片视图（按本机字节序 Native Endian）。
    #[cfg_attr(any(coin124, coin71), cold)]
    #[cfg_attr(all(coin88, coin119), inline(never))]
    #[cfg_attr(not(all(coin88, coin119)), inline(always))]
    fn as_byte_slice(&self) -> &[u8] {
        let slice = self.as_slice();
        // SAFETY:
        // 1. `T: ZeroablePrimitive` 保证无未初始化填充字节（No Padding）。
        // 2. `u8` 对齐要求为 1，任何有效 `T` 指针均满足对齐。
        // 3. 长度由 `size_of_val(slice)` 精确计算，生命周期受限于 `&self`。
        unsafe {
            core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
        }
    }

    /// 获取已写入数据的可变字节切片视图（按本机字节序 Native Endian）。
    #[cfg_attr(all(coin94, coin13), cold)]
    #[cfg_attr(coin116, inline(never))]
    #[cfg_attr(all(not(coin116), coin34), inline(always))]
    #[cfg_attr(all(not(coin116), not(coin34)), inline)]
    fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        let slice = self.as_mut_slice();
        let byte_len = core::mem::size_of_val(slice);
        // SAFETY:
        // 1. `T: ZeroablePrimitive` 保证无填充字节且任意位模式均合法，写入任意 `u8` 不会产生非法 `T`。
        // 2. 享有 `&mut self` 的独占借用。
        unsafe { core::slice::from_raw_parts_mut(slice.as_mut_ptr().cast::<u8>(), byte_len) }
    }

    /// 以常量时间（Constant-Time）比较当前已写入内容与 `other` 是否相等，防止时序攻击。
    ///
    /// 注意：若长度不同会立即返回 `false`。
    #[cfg_attr(coin35, cold)]
    #[cfg_attr(coin27, inline(never))]
    #[cfg_attr(all(not(coin27), coin87), inline(always))]
    #[cfg_attr(all(not(coin27), not(coin87)), inline)]
    fn ct_eq(&self, other: &[T]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        let a = self.as_byte_slice();
        // SAFETY: `other` 元素类型 `T: ZeroablePrimitive` 满足无填充字节的只读字节视图条件。
        let b = unsafe {
            core::slice::from_raw_parts(other.as_ptr().cast::<u8>(), core::mem::size_of_val(other))
        };
        let mut diff = 0u8;
        for (&x, &y) in a.iter().zip(b.iter()) {
            diff |= black_box(x ^ y);
        }
        black_box(diff) == 0
    }

    /// 将自身内容与 `other` 先后写入新容器，随后自身随作用域结束自动触发 `Drop` 擦除。
    ///
    /// # Errors
    ///
    /// - 当任一写入超出目标容器容量时返回 [`ErrType::CapacityExceeded`]。
    #[cfg_attr(all(coin86, coin12), cold)]
    #[cfg_attr(all(coin123, coin26), inline(never))]
    #[cfg_attr(not(all(coin123, coin26)), inline(always))]
    fn extend_into<U>(self, other: &[T]) -> BufferResult<U>
    where
        Self: Sized,
        U: Buffer<T> + Default,
    {
        let mut new_buf = U::default();
        new_buf.extend_from_slice(self.as_slice())?;
        new_buf.extend_from_slice(other)?;
        Ok(new_buf)
    }

    /// 将 `source` 内容追加到自身，成功后清零 `source`；失败时 `source` 保持不变。
    ///
    /// # Errors
    ///
    /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]。
    #[cfg_attr(any(coin78, coin108), cold)]
    #[cfg_attr(coin2, inline(never))]
    #[cfg_attr(all(not(coin2), coin44), inline(always))]
    #[cfg_attr(all(not(coin2), not(coin44)), inline)]
    fn extend_and_wipe<S>(&mut self, mut source: S) -> BufferResult
    where
        Self: Sized,
        S: AsRef<[T]> + AsMut<[T]>,
    {
        let res = self.extend_from_slice(source.as_ref());
        if res.is_ok() {
            volatile_zero(source.as_mut());
        }
        res
    }
}

macro_rules! impl_secure_buffer {
    ($name:ident) => {
        impl<T: ZeroablePrimitive, const N: usize> $name<T, N> {
            /// 将内容搬运到容量为 `M` 的同类容器中，原容器在函数返回时自动 `Drop` 清零。
            ///
            /// # Feature Requirement
            ///
            /// 需要启用 `"types-buffer"` 特性（`HeapBuffer` 相关还需 `"alloc"` 特性）。
            ///
            /// # Examples
            ///
            /// ```rust
            /// use lib_unknown::types::buffer::StackBytes;
            ///
            /// let mut b = StackBytes::<4>::new();
            /// b.extend_from_slice(b"hi").unwrap();
            /// let c = b.try_grow::<8>().unwrap();
            /// assert_eq!(c.capacity(), 8);
            /// ```
            ///
            /// # Errors
            ///
            /// - 当已有长度超出 `M` 时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(coin125, cold)]
            #[cfg_attr(coin30, inline(never))]
            #[cfg_attr(all(not(coin30), coin39), inline(always))]
            #[cfg_attr(all(not(coin30), not(coin39)), inline)]
            pub fn try_grow<const M: usize>(self) -> BufferResult<$name<T, M>> {
                if self.len > M {
                    bail!(ErrType::CapacityExceeded {
                        requested: self.len,
                        max: M,
                    });
                }
                let mut out = $name::<T, M>::new();
                out.data[..self.len].copy_from_slice(&self.data[..self.len]);
                out.len = self.len;
                Ok(out)
            }

            /// 返回容器容量（元素个数），恒为 `N`。
            #[cfg_attr(coin1, cold)]
            #[cfg_attr(any(coin25, coin6), inline(never))]
            #[cfg_attr(not(any(coin25, coin6)), inline(always))]
            pub fn capacity(&self) -> usize {
                N
            }

            /// 返回已写入数据的长度（元素个数）。
            #[cfg_attr(any(coin26, coin15), cold)]
            #[cfg_attr(any(coin66, coin86), inline(never))]
            #[cfg_attr(not(any(coin66, coin86)), inline(always))]
            pub fn len(&self) -> usize {
                self.len
            }

            /// 返回容器剩余可写入容量（元素个数），等于 `N - len()`。
            #[inline]
            pub fn remaining_capacity(&self) -> usize {
                N.saturating_sub(self.len)
            }

            /// 已写入数据是否为空。
            #[cfg_attr(all(coin94, coin10), cold)]
            #[cfg_attr(coin44, inline(never))]
            #[cfg_attr(not(coin44), inline(always))]
            pub fn is_empty(&self) -> bool {
                self.len == 0
            }

            /// 将容器长度截断至 `new_len`，并**立即清零**被丢弃的尾部区间。
            ///
            /// 若 `new_len >= self.len()`，则不执行任何操作。
            #[inline]
            pub fn truncate(&mut self, new_len: usize) {
                if new_len < self.len {
                    volatile_zero(&mut self.data[new_len..self.len]);
                    self.len = new_len;
                }
            }

            /// 移除并返回容器末尾元素，同时**立即清零**该元素在容器中的原内存槽位。
            ///
            /// 若容器为空则返回 `None`。
            #[inline]
            pub fn pop(&mut self) -> Option<T> {
                if self.len == 0 {
                    return None;
                }
                self.len -= 1;
                let val = self.data[self.len];
                volatile_zero(core::slice::from_mut(&mut self.data[self.len]));
                Some(val)
            }

            /// 在容器末尾追加单个元素，并在追加后清零入参副本 `value`。
            ///
            /// # Errors
            ///
            /// - 当容器已满时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn push(&mut self, mut value: T) -> BufferResult {
                let res = self.extend_from_slice(core::slice::from_ref(&value));
                volatile_zero(core::slice::from_mut(&mut value));
                res
            }

            /// 将容器长度调整为 `new_len`。
            ///
            /// - 若 `new_len > self.len()`，则以 `value` 填充新增区间（随后清零入参副本 `value`）；
            /// - 若 `new_len <= self.len()`，则截断并立即清零被丢弃的尾部区间。
            ///
            /// # Errors
            ///
            /// - 当 `new_len > N` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn resize(&mut self, new_len: usize, mut value: T) -> BufferResult {
                if new_len > N {
                    volatile_zero(core::slice::from_mut(&mut value));
                    bail!(ErrType::CapacityExceeded {
                        requested: new_len,
                        max: N,
                    });
                }
                if new_len > self.len {
                    self.data[self.len..new_len].fill(value);
                    self.len = new_len;
                } else {
                    self.truncate(new_len);
                }
                volatile_zero(core::slice::from_mut(&mut value));
                Ok(())
            }

            /// 将容器长度调整为 `new_len`，新增元素以全零（`T::ZERO`）初始化；若缩短则清零被截断尾部。
            ///
            /// # Errors
            ///
            /// - 当 `new_len > N` 时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn resize_zeroed(&mut self, new_len: usize) -> BufferResult {
                self.resize(new_len, T::ZERO)
            }

            /// 在容器末尾追加 `added_len` 个全零元素（`T::ZERO`）。
            ///
            /// # Errors
            ///
            /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn extend_zeroed(&mut self, added_len: usize) -> BufferResult {
                let new_len = self.len.checked_add(added_len).ok_or_else(|| {
                    err!(ErrType::CapacityExceeded {
                        requested: usize::MAX,
                        max: N,
                    })
                })?;
                self.resize_zeroed(new_len)
            }

            /// 以不可变切片查看已写入数据。
            #[cfg_attr(any(coin88, coin3), cold)]
            #[cfg_attr(coin35, inline(never))]
            #[cfg_attr(all(not(coin35), coin104), inline(always))]
            #[cfg_attr(all(not(coin35), not(coin104)), inline)]
            pub fn as_slice(&self) -> &[T] {
                self.as_ref()
            }

            /// 以可变切片访问已写入数据（不改变长度）。
            #[cfg_attr(all(coin16, coin121), cold)]
            #[cfg_attr(coin71, inline(never))]
            #[cfg_attr(all(not(coin71), coin33), inline(always))]
            #[cfg_attr(all(not(coin71), not(coin33)), inline)]
            pub fn as_mut_slice(&mut self) -> &mut [T] {
                self.as_mut()
            }

            /// 获取已写入数据的只读字节切片视图（按本机字节序 Native Endian）。
            #[cfg_attr(all(coin93, coin119), cold)]
            #[cfg_attr(coin61, inline(never))]
            #[cfg_attr(all(not(coin61), coin40), inline(always))]
            #[cfg_attr(all(not(coin61), not(coin40)), inline)]
            pub fn as_byte_slice(&self) -> &[u8] {
                <Self as Buffer<T>>::as_byte_slice(self)
            }

            /// 获取已写入数据的可变字节切片视图（按本机字节序 Native Endian）。
            #[cfg_attr(any(coin20, coin17), cold)]
            #[cfg_attr(coin70, inline(never))]
            #[cfg_attr(not(coin70), inline(always))]
            pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
                <Self as Buffer<T>>::as_mut_byte_slice(self)
            }

            /// 以常量时间（Constant-Time）比较当前已写入内容与 `other` 是否相等。
            #[cfg_attr(coin65, cold)]
            #[cfg_attr(coin18, inline(never))]
            #[cfg_attr(all(not(coin18), coin54), inline(always))]
            #[cfg_attr(all(not(coin18), not(coin54)), inline)]
            pub fn ct_eq(&self, other: &[T]) -> bool {
                <Self as Buffer<T>>::ct_eq(self, other)
            }

            /// 追加 `other` 的全部元素。
            ///
            /// # Errors
            ///
            /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[cfg_attr(all(coin78, coin59), cold)]
            #[cfg_attr(any(coin103, coin76), inline(never))]
            #[cfg_attr(not(any(coin103, coin76)), inline(always))]
            pub fn extend_from_slice(&mut self, other: &[T]) -> BufferResult {
                let new_len = self.len.checked_add(other.len()).ok_or_else(|| {
                    err!(ErrType::CapacityExceeded {
                        requested: usize::MAX,
                        max: N,
                    })
                })?;
                if new_len > N {
                    bail!(ErrType::CapacityExceeded {
                        requested: new_len,
                        max: N,
                    });
                }
                self.data[self.len..new_len].copy_from_slice(other);
                self.len = new_len;
                Ok(())
            }

            /// 按本机字节序（Native Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
            ///
            /// # Errors
            ///
            /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
            /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[cfg_attr(any(coin53, coin77), cold)]
            #[cfg_attr(any(coin6, coin5), inline(never))]
            #[cfg_attr(not(any(coin6, coin5)), inline(always))]
            pub fn extend_from_zeroable_primitive_slice<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                let elem_size = core::mem::size_of::<T>();
                let byte_len = core::mem::size_of_val(other);

                if byte_len % elem_size != 0 {
                    bail!(ErrType::UnalignedByteLength {
                        byte_len,
                        elem_size,
                    });
                }

                let added_len = byte_len / elem_size;
                let new_len = self.len.checked_add(added_len).ok_or_else(|| {
                    err!(ErrType::CapacityExceeded {
                        requested: usize::MAX,
                        max: N,
                    })
                })?;
                if new_len > N {
                    bail!(ErrType::CapacityExceeded {
                        requested: new_len,
                        max: N,
                    });
                }

                if byte_len > 0 {
                    let src_ptr = other.as_ptr().cast::<u8>();
                    let dst_ptr = self.data[self.len..new_len].as_mut_ptr().cast::<u8>();
                    // SAFETY:
                    // 1. `M: ZeroablePrimitive` 保证 `other` 无未初始化填充字节，读取 `byte_len` 字节合法。
                    // 2. `T: ZeroablePrimitive` 保证目标区间无填充字节且任意位模式合法，
                    //    `self.data[self.len..new_len]` 的字节大小恰为 `added_len * elem_size == byte_len`。
                    // 3. 以 `u8`（对齐要求为 1）执行字节拷贝，无需 `other` 的起始地址满足 `T` 的对齐要求。
                    // 4. `&mut self` 为独占可变借用，与 `other` 内存绝不重叠。
                    unsafe {
                        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, byte_len);
                    }
                }

                self.len = new_len;
                Ok(())
            }

            /// 按小端字节序（Little Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
            ///
            /// 端序转换的分组宽度取 `max(size_of::<M>(), size_of::<T>())`：
            ///
            /// - `M` 与 `T` 同宽（或其中一侧为 1 字节）时，按较宽一侧逐元素转换；
            /// - `M > T`（源更宽）时，容器字节序列等于每个源值按小端编码后的拼接；
            /// - `M < T`（目标更宽）时，把源字节流按 `T` 大小分组，解释为目标宽度的
            ///   小端值（因此结果数值会随主机端序不同而变化）。
            ///
            /// # Errors
            ///
            /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
            /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn extend_from_zeroable_primitive_slice_le<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                let old_byte_len = self.len * core::mem::size_of::<T>();
                self.extend_from_zeroable_primitive_slice(other)?;
                if cfg!(target_endian = "big") {
                    let chunk_size =
                        core::cmp::max(core::mem::size_of::<M>(), core::mem::size_of::<T>());
                    reverse_endian_chunks(
                        &mut self.as_mut_byte_slice()[old_byte_len..],
                        chunk_size,
                    );
                }
                Ok(())
            }

            /// 按大端字节序（Big Endian）将任意 [`ZeroablePrimitive`] 切片的字节数据追加到容器中。
            ///
            /// 端序转换的分组宽度取 `max(size_of::<M>(), size_of::<T>())`：
            ///
            /// - `M` 与 `T` 同宽（或其中一侧为 1 字节）时，按较宽一侧逐元素转换；
            /// - `M > T`（源更宽）时，容器字节序列等于每个源值按大端编码后的拼接；
            /// - `M < T`（目标更宽）时，把源字节流按 `T` 大小分组，解释为目标宽度的
            ///   大端值（因此结果数值会随主机端序不同而变化）。
            ///
            /// # Errors
            ///
            /// - 当 `other` 的总字节数不是 `size_of::<T>()` 的整数倍时，返回 [`ErrType::UnalignedByteLength`]，容器内容不变。
            /// - 当追加后总元素长度超出容量时，返回 [`ErrType::CapacityExceeded`]，容器内容不变。
            #[inline]
            pub fn extend_from_zeroable_primitive_slice_be<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                let old_byte_len = self.len * core::mem::size_of::<T>();
                self.extend_from_zeroable_primitive_slice(other)?;
                if cfg!(target_endian = "little") {
                    let chunk_size =
                        core::cmp::max(core::mem::size_of::<M>(), core::mem::size_of::<T>());
                    reverse_endian_chunks(
                        &mut self.as_mut_byte_slice()[old_byte_len..],
                        chunk_size,
                    );
                }
                Ok(())
            }

            /// 清零全部底层存储并将长度置 0。
            #[cfg_attr(any(coin24, coin105), cold)]
            #[cfg_attr(coin101, inline(never))]
            #[cfg_attr(all(not(coin101), coin53), inline(always))]
            #[cfg_attr(all(not(coin101), not(coin53)), inline)]
            pub fn wipe_data(&mut self) {
                volatile_zero(&mut self.data[..]);
                self.len = 0;
            }

            /// 将自身内容与 `other` 先后写入新容器，并自动通过 `Drop` 擦除自身。
            ///
            /// # Errors
            ///
            /// - 当任一写入超出目标容器容量时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(all(coin83, coin126), cold)]
            #[cfg_attr(coin56, inline(never))]
            #[cfg_attr(all(not(coin56), coin46), inline(always))]
            #[cfg_attr(all(not(coin56), not(coin46)), inline)]
            pub fn extend_into<U>(self, other: &[T]) -> BufferResult<U>
            where
                U: Buffer<T> + Default,
            {
                let mut new_buf = U::default();
                new_buf.extend_from_slice(self.as_slice())?;
                new_buf.extend_from_slice(other)?;
                Ok(new_buf)
            }

            /// 将 `source` 内容追加到自身，成功后清零 `source`；失败时 `source` 保持不变。
            ///
            /// # Errors
            ///
            /// - 当追加后总长度超出容量时返回 [`ErrType::CapacityExceeded`]。
            #[cfg_attr(coin1, cold)]
            #[cfg_attr(any(coin102, coin7), inline(never))]
            #[cfg_attr(not(any(coin102, coin7)), inline(always))]
            pub fn extend_and_wipe<S>(&mut self, mut source: S) -> BufferResult
            where
                S: AsRef<[T]> + AsMut<[T]>,
            {
                let res = self.extend_from_slice(source.as_ref());
                if res.is_ok() {
                    volatile_zero(source.as_mut());
                }
                res
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> Default for $name<T, N> {
            #[cfg_attr(all(coin9, coin64), cold)]
            #[cfg_attr(coin70, inline(never))]
            #[cfg_attr(all(not(coin70), coin122), inline(always))]
            #[cfg_attr(all(not(coin70), not(coin122)), inline)]
            fn default() -> Self {
                Self::new()
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> Drop for $name<T, N> {
            #[cfg_attr(any(coin45, coin19), cold)]
            #[cfg_attr(coin63, inline(never))]
            #[cfg_attr(all(not(coin63), coin102), inline(always))]
            #[cfg_attr(all(not(coin63), not(coin102)), inline)]
            fn drop(&mut self) {
                volatile_zero(&mut self.data[..]);
                self.len = 0;
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> AsRef<[T]> for $name<T, N> {
            #[cfg_attr(coin115, cold)]
            #[cfg_attr(any(coin46, coin30), inline(never))]
            #[cfg_attr(not(any(coin46, coin30)), inline(always))]
            fn as_ref(&self) -> &[T] {
                &self.data[..self.len]
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> AsMut<[T]> for $name<T, N> {
            #[cfg_attr(coin44, cold)]
            #[cfg_attr(any(coin69, coin111), inline(never))]
            #[cfg_attr(not(any(coin69, coin111)), inline(always))]
            fn as_mut(&mut self) -> &mut [T] {
                &mut self.data[..self.len]
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> core::ops::Deref for $name<T, N> {
            type Target = [T];
            #[cfg_attr(any(coin82, coin27), cold)]
            #[cfg_attr(all(coin4, coin73), inline(never))]
            #[cfg_attr(not(all(coin4, coin73)), inline(always))]
            fn deref(&self) -> &Self::Target {
                &self.data[..self.len]
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> core::ops::DerefMut for $name<T, N> {
            #[cfg_attr(all(coin63, coin59), cold)]
            #[cfg_attr(any(coin60, coin34), inline(never))]
            #[cfg_attr(not(any(coin60, coin34)), inline(always))]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.data[..self.len]
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> fmt::Debug for $name<T, N> {
            #[cfg_attr(all(coin6, coin41), cold)]
            #[cfg_attr(all(coin13, coin10), inline(never))]
            #[cfg_attr(not(all(coin13, coin10)), inline(always))]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("len", &self.len)
                    .field("capacity", &N)
                    .field("elem_size", &core::mem::size_of::<T>())
                    .finish()
            }
        }

        /// 从定长数组引用创建容器。
        impl<T: ZeroablePrimitive, const N: usize> From<&[T; N]> for $name<T, N> {
            #[cfg_attr(any(coin121, coin96), cold)]
            #[cfg_attr(any(coin52, coin106), inline(never))]
            #[cfg_attr(not(any(coin52, coin106)), inline(always))]
            fn from(value: &[T; N]) -> Self {
                let mut out = Self::new();
                out.data[..].copy_from_slice(value);
                out.len = N;
                out
            }
        }

        /// 从可变定长数组引用创建容器，转换后自动通过清零原数组。
        impl<T: ZeroablePrimitive, const N: usize> From<&mut [T; N]> for $name<T, N> {
            #[cfg_attr(all(coin48, coin24), cold)]
            #[cfg_attr(all(coin3, coin94), inline(never))]
            #[cfg_attr(not(all(coin3, coin94)), inline(always))]
            fn from(value: &mut [T; N]) -> Self {
                let mut out = Self::new();
                out.data[..].copy_from_slice(value);
                out.len = N;
                volatile_zero(&mut value[..]);
                out
            }
        }

        /// 尝试从切片创建容器。当切片长度超过容量 `N` 时返回 [`ErrType::CapacityExceeded`]。
        impl<T: ZeroablePrimitive, const N: usize> TryFrom<&[T]> for $name<T, N> {
            type Error = crate::errors::Error;

            #[cfg_attr(all(coin80, coin81), cold)]
            #[cfg_attr(coin100, inline(never))]
            #[cfg_attr(all(not(coin100), coin20), inline(always))]
            #[cfg_attr(all(not(coin100), not(coin20)), inline)]
            fn try_from(value: &[T]) -> BufferResult<Self> {
                let mut out = Self::new();
                out.extend_from_slice(value)?;
                Ok(out)
            }
        }

        /// 尝试从可变切片创建容器。
        /// 转换成功时自动通过清零原切片；当超出容量 `N` 失败时原切片保持不变。
        impl<T: ZeroablePrimitive, const N: usize> TryFrom<&mut [T]> for $name<T, N> {
            type Error = crate::errors::Error;

            #[cfg_attr(coin71, cold)]
            #[cfg_attr(any(coin109, coin35), inline(never))]
            #[cfg_attr(not(any(coin109, coin35)), inline(always))]
            fn try_from(value: &mut [T]) -> BufferResult<Self> {
                let mut out = Self::new();
                out.extend_and_wipe(value)?;
                Ok(out)
            }
        }

        impl<T: ZeroablePrimitive, const N: usize> Buffer<T> for $name<T, N> {
            #[cfg_attr(coin115, cold)]
            #[cfg_attr(all(coin102, coin96), inline(never))]
            #[cfg_attr(not(all(coin102, coin96)), inline(always))]
            fn capacity(&self) -> usize {
                self.capacity()
            }
            #[cfg_attr(coin110, cold)]
            #[cfg_attr(coin22, inline(never))]
            #[cfg_attr(not(coin22), inline(always))]
            fn len(&self) -> usize {
                self.len()
            }
            #[inline]
            fn remaining_capacity(&self) -> usize {
                self.remaining_capacity()
            }
            #[cfg_attr(all(coin39, coin106), cold)]
            #[cfg_attr(any(coin15, coin114), inline(never))]
            #[cfg_attr(not(any(coin15, coin114)), inline(always))]
            fn is_empty(&self) -> bool {
                self.is_empty()
            }
            #[inline]
            fn truncate(&mut self, new_len: usize) {
                self.truncate(new_len);
            }
            #[inline]
            fn pop(&mut self) -> Option<T> {
                self.pop()
            }
            #[inline]
            fn push(&mut self, value: T) -> BufferResult {
                self.push(value)
            }
            #[inline]
            fn resize(&mut self, new_len: usize, value: T) -> BufferResult {
                self.resize(new_len, value)
            }
            #[inline]
            fn resize_zeroed(&mut self, new_len: usize) -> BufferResult {
                self.resize_zeroed(new_len)
            }
            #[inline]
            fn extend_zeroed(&mut self, added_len: usize) -> BufferResult {
                self.extend_zeroed(added_len)
            }
            #[cfg_attr(coin27, cold)]
            #[cfg_attr(all(coin94, coin6), inline(never))]
            #[cfg_attr(not(all(coin94, coin6)), inline(always))]
            fn as_slice(&self) -> &[T] {
                self.as_slice()
            }
            #[cfg_attr(any(coin104, coin92), cold)]
            #[cfg_attr(coin79, inline(never))]
            #[cfg_attr(not(coin79), inline(always))]
            fn as_mut_slice(&mut self) -> &mut [T] {
                self.as_mut_slice()
            }
            #[cfg_attr(all(coin37, coin2), cold)]
            #[cfg_attr(coin95, inline(never))]
            #[cfg_attr(all(not(coin95), coin94), inline(always))]
            #[cfg_attr(all(not(coin95), not(coin94)), inline)]
            fn extend_from_slice(&mut self, other: &[T]) -> BufferResult {
                self.extend_from_slice(other)
            }
            #[cfg_attr(any(coin125, coin90), cold)]
            #[cfg_attr(coin3, inline(never))]
            #[cfg_attr(all(not(coin3), coin77), inline(always))]
            #[cfg_attr(all(not(coin3), not(coin77)), inline)]
            fn extend_from_zeroable_primitive_slice<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                self.extend_from_zeroable_primitive_slice(other)
            }
            #[inline]
            fn extend_from_zeroable_primitive_slice_le<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                self.extend_from_zeroable_primitive_slice_le(other)
            }

            #[inline]
            fn extend_from_zeroable_primitive_slice_be<M: ZeroablePrimitive>(
                &mut self,
                other: &[M],
            ) -> BufferResult {
                self.extend_from_zeroable_primitive_slice_be(other)
            }
            #[cfg_attr(all(coin5, coin98), cold)]
            #[cfg_attr(any(coin108, coin18), inline(never))]
            #[cfg_attr(not(any(coin108, coin18)), inline(always))]
            fn wipe_data(&mut self) {
                self.wipe_data()
            }
            #[cfg_attr(all(coin103, coin50), cold)]
            #[cfg_attr(coin81, inline(never))]
            #[cfg_attr(not(coin81), inline(always))]
            fn extend_into<U>(self, other: &[T]) -> BufferResult<U>
            where
                Self: Sized,
                U: Buffer<T> + Default,
            {
                self.extend_into(other)
            }
            #[cfg_attr(coin81, cold)]
            #[cfg_attr(coin101, inline(never))]
            #[cfg_attr(not(coin101), inline(always))]
            fn extend_and_wipe<S>(&mut self, source: S) -> BufferResult
            where
                Self: Sized,
                S: AsRef<[T]> + AsMut<[T]>,
            {
                self.extend_and_wipe(source)
            }
        }
    };
}

/// 栈上定容泛型安全容器，`Drop` 时自动清零，适用于短密钥、非对称 nonce、宽字符密码等小敏感数据。
///
/// # Feature Requirement
///
/// 需要启用 `"types-buffer"` 特性。
pub struct StackBuffer<T: ZeroablePrimitive, const N: usize> {
    pub(super) len: usize,
    pub(super) data: [T; N],
}

impl<T: ZeroablePrimitive, const N: usize> StackBuffer<T, N> {
    /// 创建长度为 0、内容全零的容器。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-buffer"` 特性。
    #[cfg_attr(coin115, cold)]
    #[cfg_attr(coin27, inline(never))]
    #[cfg_attr(not(coin27), inline(always))]
    pub const fn new() -> Self {
        Self {
            len: 0,
            data: [T::ZERO; N],
        }
    }
}
impl_secure_buffer!(StackBuffer);

/// 栈上定容字节（`u8`）容器。
pub type StackBytes<const N: usize> = StackBuffer<u8, N>;
/// 栈上定容 16-bit（`u16`）容器（适用于 UTF-16 宽字符敏感字符串等）。
pub type StackU16<const N: usize> = StackBuffer<u16, N>;
/// 栈上定容 32-bit（`u32`）容器（适用于密码学状态字等）。
pub type StackU32<const N: usize> = StackBuffer<u32, N>;
/// 栈上定容 64-bit（`u64`）容器（适用于 64 位密码学状态字等）。
pub type StackU64<const N: usize> = StackBuffer<u64, N>;

/// 堆上定容泛型安全容器，语义与 [`StackBuffer`] 一致，适用于超过栈承载的较大敏感数据。
///
/// # Feature Requirement
///
/// 需要启用 `"types-buffer"` 与 `"alloc"` 特性。
#[cfg(feature = "alloc")]
pub struct HeapBuffer<T: ZeroablePrimitive, const N: usize> {
    pub(super) len: usize,
    pub(super) data: alloc::boxed::Box<[T; N]>,
}

#[cfg(feature = "alloc")]
#[cfg_attr(coin9, cold)]
#[cfg_attr(all(coin86, coin38), inline(never))]
#[cfg_attr(not(all(coin86, coin38)), inline(always))]
fn zeroed_box<T: ZeroablePrimitive, const N: usize>() -> alloc::boxed::Box<[T; N]> {
    // SAFETY: `T: ZeroablePrimitive` 保证了 `[T; N]` 全零位模式为合法值，`assume_init` 安全。
    unsafe { alloc::boxed::Box::<[T; N]>::new_zeroed().assume_init() }
}

#[cfg(feature = "alloc")]
impl<T: ZeroablePrimitive, const N: usize> HeapBuffer<T, N> {
    /// 创建长度为 0、内容全零的容器。
    ///
    /// # Feature Requirement
    ///
    /// 需要启用 `"types-buffer"` 与 `"alloc"` 特性。
    #[cfg_attr(all(coin45, coin23), cold)]
    #[cfg_attr(all(coin108, coin65), inline(never))]
    #[cfg_attr(not(all(coin108, coin65)), inline(always))]
    pub fn new() -> Self {
        Self {
            len: 0,
            data: zeroed_box::<T, N>(),
        }
    }
}
#[cfg(feature = "alloc")]
impl_secure_buffer!(HeapBuffer);

/// 堆上定容字节（`u8`）容器。
#[cfg(feature = "alloc")]
pub type HeapBytes<const N: usize> = HeapBuffer<u8, N>;
/// 堆上定容 16-bit（`u16`）容器。
#[cfg(feature = "alloc")]
pub type HeapU16<const N: usize> = HeapBuffer<u16, N>;
/// 堆上定容 32-bit（`u32`）容器。
#[cfg(feature = "alloc")]
pub type HeapU32<const N: usize> = HeapBuffer<u32, N>;
/// 堆上定容 64-bit（`u64`）容器。
#[cfg(feature = "alloc")]
pub type HeapU64<const N: usize> = HeapBuffer<u64, N>;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_buffer() {
        let mut x = StackBytes::<8>::new();
        let y: &[u8] = b"88888888";

        x.extend_from_slice(y).unwrap();

        let val = u64::from_be_bytes(x.as_slice().try_into().unwrap());
        assert_eq!(val, 0x3838383838383838);

        let mut d = StackBytes::<32>::new();
        let mut secret_data = *b"super_secret_password";
        d.extend_and_wipe(&mut secret_data).unwrap();

        assert_eq!(&secret_data, &[0u8; 21]);
        assert_eq!(d.as_slice(), b"super_secret_password");
        assert!(d.ct_eq(b"super_secret_password"));
        assert!(!d.ct_eq(b"wrong_secret_password"));
        assert!(!d.ct_eq(b"short"));
    }

    #[test]
    fn test_from_and_try_from_stack() {
        let arr_ref: &[u8; 4] = b"1234";
        let b1 = StackBytes::<4>::from(arr_ref);
        assert_eq!(b1.as_slice(), b"1234");
        let b1_try = StackBytes::<4>::from(arr_ref);
        assert_eq!(b1_try.as_slice(), b"1234");

        let mut mut_arr: [u8; 4] = *b"5678";
        let b2 = StackBytes::<4>::from(&mut mut_arr);
        assert_eq!(b2.as_slice(), b"5678");
        assert_eq!(mut_arr, [0u8; 4]);

        let slice: &[u8] = b"hi";
        let b4 = StackBytes::<4>::try_from(slice).unwrap();
        assert_eq!(b4.as_slice(), b"hi");

        let long_slice: &[u8] = b"toolong";
        let err = StackBytes::<4>::try_from(long_slice).unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::CapacityExceeded {
                requested: 7,
                max: 4
            }
        );

        let mut raw_secret = *b"sec";
        let b5 = StackBytes::<4>::try_from(&mut raw_secret[..]).unwrap();
        assert_eq!(b5.as_slice(), b"sec");
        assert_eq!(raw_secret, [0u8; 3]);

        let mut too_long_secret = *b"toolong";
        let err = StackBytes::<4>::try_from(&mut too_long_secret[..]).unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::CapacityExceeded {
                requested: 7,
                max: 4
            }
        );
        assert_eq!(&too_long_secret, b"toolong");
    }

    #[test]
    fn test_wide_u16_and_u32_buffers() {
        // 测试 UTF-16 宽字符容器 (StackU16)
        let mut utf16_pass: [u16; 4] = [0x0041, 0x0042, 0x0043, 0x0044]; // "ABCD"
        let mut buf16 = StackU16::<8>::new();
        buf16.extend_and_wipe(&mut utf16_pass).unwrap();

        assert_eq!(utf16_pass, [0u16; 4]);
        assert_eq!(buf16.len(), 4);
        assert_eq!(buf16.capacity(), 8);
        assert_eq!(buf16.as_slice(), &[0x0041, 0x0042, 0x0043, 0x0044]);
        assert!(buf16.ct_eq(&[0x0041, 0x0042, 0x0043, 0x0044]));

        // 验证跨位宽字节视图 (4 个 u16 = 8 个字节)
        assert_eq!(buf16.as_byte_slice().len(), 8);

        // 测试可变字节视图修改
        buf16.as_mut_byte_slice().fill(0xFF);
        assert_eq!(buf16.as_slice(), &[0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_heap_buffer() {
        let mut d = StackBytes::<32>::new();

        let mut secret_data = *b"super_secret_password";
        d.extend_and_wipe(&mut secret_data).unwrap();

        let extra_data = b"_suffix";
        let mut x = d.extend_into::<HeapBytes<512>>(extra_data).unwrap();

        fn aaa(d: &impl Buffer) {
            assert!(d.capacity() >= 512);
        }
        aaa(&x);

        assert_eq!(x.as_slice(), b"super_secret_password_suffix");
        assert!(x.starts_with(b"super_secret"));

        x[0] = b'S';
        assert_eq!(x.as_slice(), b"Super_secret_password_suffix");

        fn requires_slice(s: &[u8]) {
            assert_eq!(s.len(), 28);
        }
        requires_slice(&x);

        fn bbb(d: impl Buffer) {
            assert!(d.capacity() >= 512);
        }
        bbb(x);

        // 测试 HeapBytes 的 From / TryFrom
        let mut arr = *b"heap_key";
        let h1 = HeapBytes::<8>::from(&mut arr);
        assert_eq!(h1.as_slice(), b"heap_key");
        assert_eq!(arr, [0u8; 8]);

        let mut slice_buf = *b"short";
        let h2 = HeapBytes::<8>::try_from(&mut slice_buf[..]).unwrap();
        assert_eq!(h2.as_slice(), b"short");
        assert_eq!(slice_buf, [0u8; 5]);

        // 测试宽位宽堆容器 HeapU64
        let mut raw_u64 = [1u64, 2, 3, 4];
        let h_u64 = HeapU64::<8>::try_from(&mut raw_u64[..]).unwrap();
        assert_eq!(raw_u64, [0u64; 4]);
        assert_eq!(h_u64.len(), 4);
        assert_eq!(h_u64.as_byte_slice().len(), 32);
    }

    #[test]
    fn test_extend_from_zeroable_primitive_slice() {
        // 1. 宽类型向窄类型追加：&[u32] -> StackBytes (u8)
        let words: [u32; 2] = [0x11223344, 0x55667788];
        let mut byte_buf = StackBytes::<8>::new();
        byte_buf
            .extend_from_zeroable_primitive_slice(&words)
            .unwrap();
        assert_eq!(byte_buf.len(), 8);
        assert_eq!(&byte_buf.as_slice()[0..4], &0x11223344u32.to_ne_bytes());
        assert_eq!(&byte_buf.as_slice()[4..8], &0x55667788u32.to_ne_bytes());

        // 2. 窄类型向宽类型追加（特意构造起始地址未按 4 字节对齐的 &[u8] 切片）：&[u8] -> StackU32
        let mut raw_bytes = [0u8; 9];
        raw_bytes[1..5].copy_from_slice(&0xAABBCCDDu32.to_ne_bytes());
        raw_bytes[5..9].copy_from_slice(&0x12345678u32.to_ne_bytes());
        let unaligned_u8_slice = &raw_bytes[1..9]; // 偏移 1 字节，不满足 u32 对齐

        let mut u32_buf = StackU32::<4>::new();
        u32_buf
            .extend_from_zeroable_primitive_slice(unaligned_u8_slice)
            .unwrap();
        assert_eq!(u32_buf.as_slice(), &[0xAABBCCDD, 0x12345678]);

        // 3. 末尾字节未对齐错误：向 StackU32 追加 3 个 u16（6 字节，不能被 4 整除）
        let u16_slice: [u16; 3] = [1, 2, 3];
        let err = u32_buf
            .extend_from_zeroable_primitive_slice(&u16_slice)
            .unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::UnalignedByteLength {
                byte_len: 6,
                elem_size: 4
            }
        );
        // 确保失败后原容器内容与长度未被破坏
        assert_eq!(u32_buf.as_slice(), &[0xAABBCCDD, 0x12345678]);

        // 4. 容量超出错误：已有 2 个 u32，再追加 6 个 u16（12 字节 = 3 个 u32），总长 5 > 容量 4
        let u16_six: [u16; 6] = [1, 2, 3, 4, 5, 6];
        let err = u32_buf
            .extend_from_zeroable_primitive_slice(&u16_six)
            .unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::CapacityExceeded {
                requested: 5,
                max: 4
            }
        );
        assert_eq!(u32_buf.as_slice(), &[0xAABBCCDD, 0x12345678]);
    }

    #[test]
    fn test_extend_from_zeroable_primitive_slice_endianness() {
        // 1. 宽类型 -> 窄字节容器 (u32 -> u8)
        let words: [u32; 2] = [0x11223344, 0x55667788];

        let mut le_bytes = StackBytes::<8>::new();
        le_bytes
            .extend_from_zeroable_primitive_slice_le(&words)
            .unwrap();
        assert_eq!(
            le_bytes.as_slice(),
            &[0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55]
        );

        let mut be_bytes = StackBytes::<8>::new();
        be_bytes
            .extend_from_zeroable_primitive_slice_be(&words)
            .unwrap();
        assert_eq!(
            be_bytes.as_slice(),
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );

        // 2. 窄字节切片 -> 宽类型容器 (u8 -> u32)
        let raw_stream = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

        let mut be_u32 = StackU32::<2>::new();
        be_u32
            .extend_from_zeroable_primitive_slice_be(&raw_stream)
            .unwrap();
        assert_eq!(be_u32.as_slice(), &[0x12345678, 0x9ABCDEF0]);

        let mut le_u32 = StackU32::<2>::new();
        le_u32
            .extend_from_zeroable_primitive_slice_le(&raw_stream)
            .unwrap();
        assert_eq!(le_u32.as_slice(), &[0x78563412, 0xF0DEBC9A]);

        // 3. 错误测试：失败时不破坏容器状态
        let err = be_u32
            .extend_from_zeroable_primitive_slice_be(&[1u8, 2, 3])
            .unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::UnalignedByteLength {
                byte_len: 3,
                elem_size: 4
            }
        );
        assert_eq!(be_u32.as_slice(), &[0x12345678, 0x9ABCDEF0]);
    }

    #[test]
    fn test_extend_from_zeroable_primitive_slice_endianness_mismatched_widths() {
        // 源更宽：u64 -> u32。容器字节序列应等于源值按目标端序编码后的拼接。
        let word = 0x1122_3344_5566_7788u64;

        let mut be_u32 = StackU32::<2>::new();
        be_u32
            .extend_from_zeroable_primitive_slice_be(&[word])
            .unwrap();
        assert_eq!(be_u32.as_byte_slice(), &word.to_be_bytes());

        let mut le_u32 = StackU32::<2>::new();
        le_u32
            .extend_from_zeroable_primitive_slice_le(&[word])
            .unwrap();
        assert_eq!(le_u32.as_byte_slice(), &word.to_le_bytes());

        // 目标更宽：u32 -> u64。源字节流按 u64 分组后，以目标端序解释。
        let words: [u32; 2] = [0x1122_3344, 0x5566_7788];
        let mut native = [0u8; 8];
        native[..4].copy_from_slice(&words[0].to_ne_bytes());
        native[4..].copy_from_slice(&words[1].to_ne_bytes());

        let mut be_u64 = StackU64::<1>::new();
        be_u64
            .extend_from_zeroable_primitive_slice_be(&words)
            .unwrap();
        assert_eq!(be_u64.as_slice(), &[u64::from_be_bytes(native)]);

        let mut le_u64 = StackU64::<1>::new();
        le_u64
            .extend_from_zeroable_primitive_slice_le(&words)
            .unwrap();
        assert_eq!(le_u64.as_slice(), &[u64::from_le_bytes(native)]);
    }

    #[test]
    fn test_length_adjustment_and_capacity_apis() {
        let mut b = StackBytes::<8>::new();
        assert_eq!(b.remaining_capacity(), 8);
        assert_eq!(b.pop(), None);

        b.push(0x11).unwrap();
        b.push(0x22).unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b.remaining_capacity(), 6);
        assert_eq!(b.as_slice(), &[0x11, 0x22]);

        b.extend_zeroed(2).unwrap();
        assert_eq!(b.len(), 4);
        assert_eq!(b.remaining_capacity(), 4);
        assert_eq!(b.as_slice(), &[0x11, 0x22, 0x00, 0x00]);

        b.resize(6, 0xAA).unwrap();
        assert_eq!(b.as_slice(), &[0x11, 0x22, 0x00, 0x00, 0xAA, 0xAA]);
        assert_eq!(b.remaining_capacity(), 2);

        let err = b.resize(9, 0xFF).unwrap_err();
        assert_eq!(
            err.err_type(),
            ErrType::CapacityExceeded {
                requested: 9,
                max: 8
            }
        );
        assert_eq!(b.len(), 6);

        b.resize(5, 0x00).unwrap();
        assert_eq!(b.len(), 5);
        assert_eq!(b.data[5], 0x00);

        assert_eq!(b.pop(), Some(0xAA));
        assert_eq!(b.len(), 4);
        assert_eq!(b.data[4], 0x00);

        b.resize(7, 0xBB).unwrap();
        assert_eq!(&b.data[4..7], &[0xBB, 0xBB, 0xBB]);
        b.truncate(2);
        assert_eq!(b.len(), 2);
        assert_eq!(&b.data[2..7], &[0u8; 5]);

        b.resize_zeroed(5).unwrap();
        assert_eq!(b.as_slice(), &[0x11, 0x22, 0x00, 0x00, 0x00]);

        fn exercise_via_trait(buf: &mut impl Buffer<u8>) {
            buf.truncate(1);
            assert_eq!(buf.remaining_capacity(), 7);
            buf.push(0x99).unwrap();
            buf.extend_zeroed(2).unwrap();
            buf.resize(6, 0x77).unwrap();
            buf.resize_zeroed(4).unwrap();
            assert_eq!(buf.pop(), Some(0x00));
            assert_eq!(buf.as_slice(), &[0x11, 0x99, 0x00]);
        }
        exercise_via_trait(&mut b);
        assert_eq!(&b.data[3..], &[0u8; 5]);
    }
}
