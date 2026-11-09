// SPDX-License-Identifier: MPL-2.0

//! 系统接口：裸系统调用（`syscall`）、Unix 动态符号解析（`unix`，类 Unix）、Windows DLL 解析（`win`，Windows）。
//!
//! 需要启用 `"sys"`（或其子特性 `sys-syscall` / `sys-unix` / `sys-win`）特性。
/// 各平台裸系统调用 `syscall0`–`syscall6` 封装。
#[cfg(feature = "sys-syscall")]
pub mod syscall;
/// 类 Unix 平台：系统调用号常量与 `dlopen`/`dlsym` 动态符号解析。
#[cfg(all(unix, feature = "sys-unix"))]
pub mod unix;
/// Windows：`GetModuleHandleA`/`LoadLibraryA`/`GetProcAddress` 动态符号解析。
#[cfg(all(windows, feature = "sys-win"))]
pub mod win;
