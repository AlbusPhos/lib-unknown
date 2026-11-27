// SPDX-License-Identifier: MPL-2.0
#![no_std]
//! 统一错误定义与处理。
//!
//! 本模块提供全库共用的 [`Error`](crate::Error) / [`ErrType`](crate::ErrType) /
//! [`Result`](crate::Result) / [`Context`](crate::Context) 以及
//! `err!`、`bail!`、`ensure!` 宏。各子模块不再各自定义错误枚举，
//! 而是复用这里的错误类型，以保证错误语义、`Display` 输出与模块溯源一致。
//!
//! # Examples
//!
//! ```rust
//! use lib_unknown_errors::{Error, ErrType, Result, Context};
//!
//! fn check_len(len: usize, max: usize) -> Result<usize> {
//!     if len > max {
//!         return Err(Error::new(
//!             module_path!(),
//!             ErrType::CapacityExceeded { requested: len, max },
//!         ));
//!     }
//!     Ok(len)
//! }
//!
//! let err = check_len(8, 4).context("failed to validate packet").unwrap_err();
//! assert_eq!(err.err_type(), ErrType::CapacityExceeded { requested: 8, max: 4 });
//! assert_eq!(err.context(), Some("failed to validate packet"));
//! ```

// 显式目标保证 stable rustdoc 也能解析；nightly 的 redundant_explicit_links 会将其视为冗余。
#![allow(rustdoc::redundant_explicit_links)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;
use core::{
    fmt,
    hash::{Hash, Hasher},
    panic::Location,
    str,
    time::Duration,
};

#[cfg(not(feature = "alloc"))]
const MAX_MSG_LEN: usize = 62;

/// 向下寻找小于等于 `index` 的最近 UTF-8 字符边界。
#[cfg(not(feature = "alloc"))]
#[inline]
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut len = index.min(s.len());
    while len > 0 && !s.is_char_boundary(len) {
        len -= 1;
    }
    len
}

/// 错误上下文文本（内部枚举隐藏，保证 Cargo feature 严格可加性）。
///
/// - 静态字符串直接借用（零分配）。
/// - 开启 `alloc` 特性时，动态文本使用 `String` 在堆上完整保存。
/// - 未开启 `alloc` 特性时，动态文本在固定的 62 字节内联缓冲中截断保存，
///   截断时保证落在 UTF-8 字符边界上。
#[derive(Clone)]
pub struct ErrorMessage(ErrorMessageInner);

#[derive(Clone)]
enum ErrorMessageInner {
    /// 直接借用的静态字符串（零分配）。
    Static(&'static str),
    /// 堆上保存的动态文本（启用 `alloc` 时）。
    #[cfg(feature = "alloc")]
    Inline(String),
    /// 定长内联缓冲与已写入字节数（未启用 `alloc` 时，最多 62 字节）。
    #[cfg(not(feature = "alloc"))]
    Inline([u8; MAX_MSG_LEN], u8),
}

// 静态断言：确保在 64 位无 alloc 环境下，Option<ErrorMessage> 利用 Niche 优化严格保持 64 字节（1 个缓存行）。
#[cfg(all(target_pointer_width = "64", not(feature = "alloc")))]
const _: () = assert!(core::mem::size_of::<Option<ErrorMessage>>() == 64);

impl ErrorMessage {
    /// 由静态字符串构造错误文本（零分配）。
    #[inline]
    pub const fn from_static(s: &'static str) -> Self {
        Self(ErrorMessageInner::Static(s))
    }

    /// 由运行时字符串构造错误文本：`alloc` 开启时转 `String`，否则截断至 62 字节内联缓冲（保证 UTF-8 边界）。
    #[cold]
    pub fn from_str_slice(value: &str) -> Self {
        #[cfg(feature = "alloc")]
        {
            Self(ErrorMessageInner::Inline(String::from(value)))
        }
        #[cfg(not(feature = "alloc"))]
        {
            let mut buf = [0u8; MAX_MSG_LEN];
            let len = floor_char_boundary(value, MAX_MSG_LEN);
            buf[..len].copy_from_slice(&value.as_bytes()[..len]);
            Self(ErrorMessageInner::Inline(buf, len as u8))
        }
    }

    /// 由 `format_args!` 构造错误文本；纯静态字面量在编译期内联折叠为 `Static`，不触发分配与慢路径调用。
    #[inline]
    pub fn from_fmt(args: fmt::Arguments<'_>) -> Self {
        if let Some(s) = args.as_str() {
            return Self(ErrorMessageInner::Static(s));
        }
        Self::from_fmt_slow(args)
    }

    #[cold]
    #[inline(never)]
    fn from_fmt_slow(args: fmt::Arguments<'_>) -> Self {
        #[cfg(feature = "alloc")]
        {
            Self(ErrorMessageInner::Inline(alloc::fmt::format(args)))
        }
        #[cfg(not(feature = "alloc"))]
        {
            let mut writer = InlineWriter {
                buf: [0u8; MAX_MSG_LEN],
                len: 0,
            };
            let _ = fmt::write(&mut writer, args);
            Self(ErrorMessageInner::Inline(writer.buf, writer.len as u8))
        }
    }

    /// 取得错误文本（`Static` 直接返回字面量；`Inline` 按构造时记录的长度切出）。
    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            ErrorMessageInner::Static(s) => s,
            #[cfg(feature = "alloc")]
            ErrorMessageInner::Inline(s) => s.as_str(),
            #[cfg(not(feature = "alloc"))]
            ErrorMessageInner::Inline(buf, len) => {
                // SAFETY: 构造时只写入完整 UTF-8 字符串（或截断到字符边界），长度 <= MAX_MSG_LEN。
                unsafe { str::from_utf8_unchecked(&buf[..(*len as usize)]) }
            }
        }
    }
}

impl PartialEq for ErrorMessage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ErrorMessage {}

impl Hash for ErrorMessage {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl From<&str> for ErrorMessage {
    #[inline]
    fn from(value: &str) -> Self {
        Self::from_str_slice(value)
    }
}

#[cfg(feature = "alloc")]
impl From<String> for ErrorMessage {
    #[inline]
    fn from(value: String) -> Self {
        Self(ErrorMessageInner::Inline(value))
    }
}

#[cfg(not(feature = "alloc"))]
struct InlineWriter {
    buf: [u8; MAX_MSG_LEN],
    len: usize,
}

#[cfg(not(feature = "alloc"))]
impl fmt::Write for InlineWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let remain = MAX_MSG_LEN - self.len;
        if remain < s.len() {
            let copy_len = floor_char_boundary(s, remain);
            self.buf[self.len..self.len + copy_len].copy_from_slice(&s.as_bytes()[..copy_len]);
            self.len += copy_len;
            return Err(fmt::Error);
        }
        self.buf[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}

impl fmt::Debug for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

/// 全库共用的错误种类。
///
/// 未来可能新增变体，请勿依赖穷尽匹配（匹配时请保留通配分支）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrType {
    /// 请求长度超出容器最大容量。
    CapacityExceeded {
        /// 请求的元素/字节数。
        requested: usize,
        /// 容器允许的最大容量。
        max: usize,
    },
    /// 目标缓冲区长度不足。
    BufferTooSmall {
        /// 实际需要的空间。
        required: usize,
        /// 可用的空间。
        available: usize,
    },
    /// 传入的长度不匹配。
    InvalidLength {
        /// 期望的长度。
        expected: usize,
        /// 实际长度。
        actual: usize,
    },
    /// 索引或切片范围越界。
    OutOfBounds {
        /// 访问的索引。
        index: usize,
        /// 容器或切片的长度。
        len: usize,
    },
    /// 源切片的总字节数不是目标容器单个元素字节大小的整数倍（末尾字节未对齐）。
    UnalignedByteLength {
        /// 源字节总数。
        byte_len: usize,
        /// 目标元素字节大小。
        elem_size: usize,
    },
    /// 收到空指针。
    NullPointer,
    /// 源内容不是合法 UTF-8。
    InvalidUtf8(str::Utf8Error),
    /// 载荷内部含 NUL，载荷为 NUL 在载荷内的字节下标。
    InteriorNul(usize),
    /// 输入中找不到结尾 NUL。
    MissingNul,
    /// 密钥长度错误。
    InvalidKeyLength {
        /// 期望的密钥字节数。
        expected: usize,
        /// 实际密钥字节数。
        actual: usize,
    },
    /// Nonce（或 IV）长度错误。
    InvalidNonceLength {
        /// 期望的 nonce 字节数。
        expected: usize,
        /// 实际 nonce 字节数。
        actual: usize,
    },
    /// 数据校验失败，数据可能被篡改或已损坏。
    DataTampered,
    /// 编码数据格式非法（如非法的 Hex / Base64 字符或填充）。
    InvalidEncoding,
    /// 系统调用返回错误码 (errno)。
    SysCallRet(isize),
    /// 传入的参数非法。
    InvalidArg,
    /// 其他未分类错误（详细原因见 [`Error::context`]）。
    Other,
    /// 操作超时，载荷为超时阈值。
    Timeout(Duration),
    /// 编译失败。
    CompileFailed,
    /// `cargo` 命令返回非零退出码。
    CargoFailed,
    /// 子进程无法启动（详细原因见 [`Error::context`]）。
    ProcessSpawnFailed,
}

impl fmt::Display for ErrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrType::CapacityExceeded { requested, max } => {
                write!(f, "capacity exceeded (requested: {requested}, max: {max})")
            }
            ErrType::BufferTooSmall {
                required,
                available,
            } => write!(
                f,
                "destination buffer too small (required: {required}, available: {available})"
            ),
            ErrType::InvalidLength { expected, actual } => {
                write!(f, "invalid length (expected: {expected}, actual: {actual})")
            }
            ErrType::OutOfBounds { index, len } => {
                write!(f, "index out of bounds (index: {index}, len: {len})")
            }
            ErrType::UnalignedByteLength {
                byte_len,
                elem_size,
            } => write!(
                f,
                "unaligned byte length: source has {byte_len} bytes, not a multiple of element size ({elem_size} bytes)"
            ),
            ErrType::NullPointer => f.write_str("received null pointer"),
            ErrType::InvalidUtf8(e) => write!(f, "invalid UTF-8 sequence: {e}"),
            ErrType::InteriorNul(pos) => write!(f, "interior NUL at payload index {pos}"),
            ErrType::MissingNul => f.write_str("missing terminating NUL"),
            ErrType::InvalidKeyLength { expected, actual } => {
                write!(
                    f,
                    "invalid key length (expected: {expected}, actual: {actual})"
                )
            }
            ErrType::InvalidNonceLength { expected, actual } => {
                write!(
                    f,
                    "invalid nonce length (expected: {expected}, actual: {actual})"
                )
            }
            ErrType::DataTampered => {
                f.write_str("integrity check failed: data may be tampered with")
            }
            ErrType::InvalidEncoding => f.write_str("invalid data encoding"),
            ErrType::SysCallRet(code) => write!(f, "syscall failed with errno: {code}"),
            ErrType::InvalidArg => f.write_str("invalid argument"),
            ErrType::Other => f.write_str("other error"),
            ErrType::Timeout(limit) => write!(f, "operation timed out after {limit:?}"),
            ErrType::CompileFailed => f.write_str("compilation failed"),
            ErrType::CargoFailed => f.write_str("cargo command failed"),
            ErrType::ProcessSpawnFailed => f.write_str("failed to spawn process"),
        }
    }
}

/// 全库共用的错误类型，携带错误种类、来源模块、调用位置与可选上下文。
#[derive(Clone)]
pub struct Error {
    module: &'static str,
    location: &'static Location<'static>,
    err_type: ErrType,
    context: Option<ErrorMessage>,
}

impl Error {
    /// 以来源模块名与错误种类构造错误（无附加上下文），自动记录调用处源码位置。
    #[inline]
    #[track_caller]
    pub const fn new(module: &'static str, err_type: ErrType) -> Self {
        Self {
            module,
            location: Location::caller(),
            err_type,
            context: None,
        }
    }

    /// 编译期以来源模块名、错误种类和静态上下文构造错误。
    #[inline]
    #[track_caller]
    pub const fn new_with_static_context(
        module: &'static str,
        err_type: ErrType,
        msg: &'static str,
    ) -> Self {
        Self {
            module,
            location: Location::caller(),
            err_type,
            context: Some(ErrorMessage::from_static(msg)),
        }
    }

    /// 附加静态字符串上下文并返回自身。
    ///
    /// 若此前已有上下文，会以 `"{new}: {old}"` 的形式串联保留，不会丢失底层原因。
    #[inline]
    pub fn with_static_context(mut self, msg: &'static str) -> Self {
        if self.context.is_none() {
            self.context = Some(ErrorMessage::from_static(msg));
            self
        } else {
            self.chain_fmt_context(core::format_args!("{msg}"))
        }
    }

    /// 附加运行时字符串上下文并返回自身（已有上下文时自动串联）。
    #[inline]
    pub fn with_str_context(mut self, msg: &str) -> Self {
        if self.context.is_none() {
            self.context = Some(ErrorMessage::from_str_slice(msg));
            self
        } else {
            self.chain_fmt_context(core::format_args!("{msg}"))
        }
    }

    /// 附加 `format_args!` 上下文并返回自身（已有上下文时自动串联）。
    #[inline]
    pub fn with_fmt_context(mut self, args: fmt::Arguments<'_>) -> Self {
        if self.context.is_none() {
            self.context = Some(ErrorMessage::from_fmt(args));
            self
        } else {
            self.chain_fmt_context(args)
        }
    }

    #[cold]
    #[inline(never)]
    fn chain_fmt_context(mut self, args: fmt::Arguments<'_>) -> Self {
        let chained = match &self.context {
            Some(prev) => {
                let prev_str = prev.as_str();
                ErrorMessage::from_fmt_slow(core::format_args!("{args}: {prev_str}"))
            }
            None => ErrorMessage::from_fmt(args),
        };
        self.context = Some(chained);
        self
    }

    /// 出错的模块名（编译期静态字符串）。
    #[inline]
    pub const fn module(&self) -> &'static str {
        self.module
    }

    /// 触发错误的源码位置（文件、行号、列号）。
    #[inline]
    pub const fn location(&self) -> &'static Location<'static> {
        self.location
    }

    /// 错误类别（可 `match` 程序化处理；`Eq` 判等可直接用于测试断言）。
    #[inline]
    pub const fn err_type(&self) -> ErrType {
        self.err_type
    }

    /// 附加上下文文本（经 `with_*_context` 或 [`Context`] 设置时可取回）。
    #[inline]
    pub fn context(&self) -> Option<&str> {
        self.context.as_ref().map(ErrorMessage::as_str)
    }
}

// 判等与哈希仅比较语义字段（module、err_type、context），忽略触发行号 location，便于单元测试 assert_eq!。
impl PartialEq for Error {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module
            && self.err_type == other.err_type
            && self.context == other.context
    }
}

impl Eq for Error {}

impl Hash for ErrType {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);

        match self {
            ErrType::CapacityExceeded { requested, max } => {
                requested.hash(state);
                max.hash(state);
            }
            ErrType::BufferTooSmall {
                required,
                available,
            } => {
                required.hash(state);
                available.hash(state);
            }
            ErrType::InvalidLength { expected, actual }
            | ErrType::InvalidKeyLength { expected, actual }
            | ErrType::InvalidNonceLength { expected, actual } => {
                expected.hash(state);
                actual.hash(state);
            }
            ErrType::OutOfBounds { index, len } => {
                index.hash(state);
                len.hash(state);
            }
            ErrType::UnalignedByteLength {
                byte_len,
                elem_size,
            } => {
                byte_len.hash(state);
                elem_size.hash(state);
            }
            ErrType::InvalidUtf8(e) => {
                e.valid_up_to().hash(state);
                e.error_len().hash(state);
            }
            ErrType::InteriorNul(pos) => {
                pos.hash(state);
            }
            ErrType::SysCallRet(code) => {
                code.hash(state);
            }
            ErrType::Timeout(limit) => {
                limit.hash(state);
            }
            ErrType::NullPointer
            | ErrType::MissingNul
            | ErrType::DataTampered
            | ErrType::InvalidEncoding
            | ErrType::InvalidArg
            | ErrType::Other
            | ErrType::CompileFailed
            | ErrType::CargoFailed
            | ErrType::ProcessSpawnFailed => {}
        }
    }
}
impl Hash for Error {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.module.hash(state);
        self.err_type.hash(state);
        self.context.hash(state);
    }
}

impl From<str::Utf8Error> for Error {
    #[cold]
    #[track_caller]
    fn from(err: str::Utf8Error) -> Self {
        Self {
            module: "<utf8>",
            location: Location::caller(),
            err_type: ErrType::InvalidUtf8(err),
            context: None,
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            // {:#?} 输出结构化字段详情
            let mut s = f.debug_struct("Error");
            s.field("module", &self.module)
                .field("location", &self.location)
                .field("err_type", &self.err_type);
            if let Some(ctx) = &self.context {
                s.field("context", &ctx.as_str());
            }
            s.finish()
        } else {
            // {:?}（如 unwrap() 或 fn main() -> Result<()> 报错退出）输出高可读性信息与源码位置
            write!(
                f,
                "[{module} at {file}:{line}:{col}] {err_type}",
                module = self.module,
                file = self.location.file(),
                line = self.location.line(),
                col = self.location.column(),
                err_type = self.err_type,
            )?;
            if let Some(ctx) = &self.context {
                write!(f, " - context: {}", ctx.as_str())?;
            }
            Ok(())
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.module, self.err_type)?;
        if let Some(ctx) = &self.context {
            write!(f, " - context: {}", ctx.as_str())?;
        }
        Ok(())
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match &self.err_type {
            ErrType::InvalidUtf8(e) => Some(e),
            _ => None,
        }
    }
}

/// 全库共用的返回类型。
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// 为 [`Result`] 提供类似 `anyhow::Context` 的链式上下文附加能力。
pub trait Context<T> {
    /// 为错误附加静态字符串上下文（零分配）。
    fn context(self, msg: &'static str) -> Result<T>;

    /// 为错误惰性附加动态上下文（仅在 `Err` 分支执行闭包，`Ok` 路径零开销）。
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C;
}

impl<T, E> Context<T> for core::result::Result<T, E>
where
    E: Into<Error>,
{
    #[inline]
    #[track_caller]
    fn context(self, msg: &'static str) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(err.into().with_static_context(msg)),
        }
    }

    #[inline]
    #[track_caller]
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => {
                let ctx = f();
                Err(err.into().with_fmt_context(core::format_args!("{ctx}")))
            }
        }
    }
}

/// 构造当前模块的 [`Error`]。
///
/// - `err!(kind)`：无上下文。
/// - `err!(kind, "fmt {}", arg)`：附带格式化上下文（纯字面量零分配；动态文本在开启 `alloc` 时为 `String`，否则为最多 62 字节内联缓冲）。
///
/// # Examples
///
/// ```rust
/// use lib_unknown_errors::{err, ErrType};
///
/// let e = err!(ErrType::InvalidArg);
/// assert_eq!(e.err_type(), ErrType::InvalidArg);
/// ```
#[macro_export]
macro_rules! err {
    ($err_type:expr $(,)?) => {
        $crate::Error::new(::core::module_path!(), $err_type)
    };
    ($err_type:expr, $($arg:tt)+) => {
        $crate::Error::new(::core::module_path!(), $err_type)
            .with_fmt_context(::core::format_args!($($arg)+))
    };
}

/// 返回 `Err(err!(...))`。
///
/// # Examples
///
/// ```rust
/// use lib_unknown_errors::{bail, ErrType, Result};
///
/// fn check(n: usize) -> Result<()> {
///     if n == 0 {
///         bail!(ErrType::InvalidArg, "n must be non-zero");
///     }
///     Ok(())
/// }
///
/// assert!(check(1).is_ok());
/// assert!(check(0).is_err());
/// ```
#[macro_export]
macro_rules! bail {
    ($($tt:tt)+) => {
        return ::core::result::Result::Err($crate::err!($($tt)+))
    };
}

/// 条件不满足时返回错误，并在错误上下文中自动附带失败的条件表达式（如 `condition failed: n > 0`）。
///
/// # Examples
///
/// ```rust
/// use lib_unknown_errors::{ensure, ErrType, Result};
///
/// fn check(n: usize) -> Result<()> {
///     ensure!(n > 0, ErrType::InvalidArg);
///     Ok(())
/// }
///
/// assert!(check(1).is_ok());
/// let err = check(0).unwrap_err();
/// assert_eq!(err.context(), Some("condition failed: n > 0"));
/// ```
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err_type:expr $(,)?) => {
        if !($cond) {
            return ::core::result::Result::Err(
                $crate::Error::new_with_static_context(
                    ::core::module_path!(),
                    $err_type,
                    ::core::concat!("condition failed: ", ::core::stringify!($cond)),
                ),
            );
        }
    };
    ($cond:expr, $err_type:expr, $($arg:tt)+) => {
        if !($cond) {
            return ::core::result::Result::Err(
                $crate::Error::new(::core::module_path!(), $err_type)
                    .with_fmt_context(::core::format_args!(
                        "{} (condition failed: {})",
                        ::core::format_args!($($arg)+),
                        ::core::stringify!($cond)
                    )),
            );
        }
    };
}
