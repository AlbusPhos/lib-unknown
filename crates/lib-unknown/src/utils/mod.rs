// SPDX-License-Identifier: MPL-2.0

//! 门面工具模块：保留历史 `utils` 路径，当前承载 `cfg_if!` / `cfg_match!` 的实现源码。
//!
//! 两个宏同时通过 `#[macro_export]` 暴露在 crate 根路径，可直接使用
//! `lib_unknown::cfg_if!` / `lib_unknown::cfg_match!`。

pub mod cfg;
