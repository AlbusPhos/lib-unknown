// SPDX-License-Identifier: MPL-2.0

//! Windows 系统封装：经 `GetModuleHandleA` / `LoadLibraryA` / `GetProcAddress` 动态解析 DLL 导出（`resolve`）。
//!
//! 需要启用 `"sys-win"` 特性，仅 Windows 目标有效。
#[cfg(all(windows, target_arch = "x86"))]
macro_rules! __winlink {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim", import_name_type = "undecorated")]
        unsafe extern $abi { $(#[link_name=$link_name])? pub fn $($function)*; }
    )
}

#[cfg(all(windows, not(target_arch = "x86")))]
macro_rules! __winlink {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim")]
        unsafe extern $abi { $(#[link_name=$link_name])? pub fn $($function)*; }
    )
}

/// 通过 `GetModuleHandleA/LoadLibraryA/GetProcAddress` 动态解析 DLL 导出。
///
/// 先尝试已加载模块，失败则 `LoadLibraryA`；名称超长（≥260）、含 `\0` 或
/// `T` 的大小与指针不一致时返回 `None`。
///
/// # Safety
///
/// `T` 必须与该导出符号的真实 ABI/签名一致。若签名不匹配，调用返回的函数
/// 指针会导致未定义行为。
///
/// # Examples
///
/// ```rust,no_run
/// #[cfg(windows)]
/// {
///     use lib_unknown::sys::win::resolve;
///     let f: Option<unsafe extern "system" fn() -> u32> =
///         unsafe { resolve("kernel32.dll", "GetTickCount") };
///     let _ = f;
/// }
/// ```
#[cfg_attr(coin23, cold)]
#[cfg_attr(all(coin105, coin89), inline(never))]
#[cfg_attr(not(all(coin105, coin89)), inline(always))]
pub unsafe fn resolve<T: Sized + Copy>(dll: &str, name: &str) -> Option<T> {
    __winlink!("kernel32.dll" "system" fn GetModuleHandleA(lpmodulename: *const u8) -> *mut ::core::ffi::c_void);
    __winlink!("kernel32.dll" "system" fn GetProcAddress(hmodule: *mut ::core::ffi::c_void, lpprocname: *const u8) -> *const u8);
    __winlink!("kernel32.dll" "system" fn LoadLibraryA(lplibfilename: *const u8) -> *mut ::core::ffi::c_void);

    const MAX_LEN: usize = 260;

    if ::core::mem::size_of::<T>() != ::core::mem::size_of::<*mut ::core::ffi::c_void>() {
        return None;
    }

    if dll.len() >= MAX_LEN || name.len() >= MAX_LEN {
        return None;
    }

    let dll_bytes = dll.as_bytes();
    let name_bytes = name.as_bytes();
    if dll_bytes.contains(&0) || name_bytes.contains(&0) {
        return None;
    }

    let mut dll_buf = [0u8; MAX_LEN];
    let mut name_buf = [0u8; MAX_LEN];

    dll_buf[..dll.len()].copy_from_slice(dll_bytes);
    name_buf[..name.len()].copy_from_slice(name_bytes);

    let module = unsafe {
        let mut h = GetModuleHandleA(dll_buf.as_ptr());
        if h.is_null() {
            h = LoadLibraryA(dll_buf.as_ptr());
        }
        h
    };

    if module.is_null() {
        return None;
    }

    let proc = unsafe { GetProcAddress(module, name_buf.as_ptr()) };

    // SAFETY: `proc` 非空，且大小已与指针校验一致；签名正确性由调用方的
    // `# Safety` 约定保证。
    (!proc.is_null()).then(|| unsafe { ::core::mem::transmute_copy(&proc) })
}
