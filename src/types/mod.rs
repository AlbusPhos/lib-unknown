// SPDX-License-Identifier: MPL-2.0

//! 定容安全内存类型：字节容器（`bytes`）、UTF-8 字符串容器（`str`）与 C 字符串容器（`cstr`），搬运/销毁时清零残留。
//!
//! 需要启用 `"types"`（或其子特性 `types-buffer` / `types-str` / `types-cstr`）特性。
/// 定容擦除式字节容器（`StackBytes`/`HeapBytes`）、`ct_eq` 常数时间比较与字节序视图。
#[cfg(feature = "types-buffer")]
pub mod buffer;
/// NUL 终止 C 字符串容器（`StackCStr`/`HeapCStr`），写入校验、清除换容器清零。
#[cfg(feature = "types-cstr")]
pub mod cstr;
/// UTF-8 字符串容器（`StackStr`/`HeapStr`），搬运与销毁时 volatile 清零。
#[cfg(feature = "types-str")]
pub mod str;
