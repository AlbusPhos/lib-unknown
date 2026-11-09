// SPDX-License-Identifier: MPL-2.0

//! 统一错误定义与处理。
//!
//! 本模块提供全库共用的 [`Error`](crate::errors::Error) / [`ErrType`](crate::errors::ErrType) / [`Result`](crate::errors::Result) 以及
//! `err!`、`bail!`、`ensure!` 宏。各子模块不再各自定义错误枚举，
//! 而是复用这里的错误类型，以保证错误语义、`Display` 输出与模块溯源一致。
//!
//! # Examples
//!
//! ```rust
//! use lib_unknown::errors::{Error, ErrType, Result};
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
//! let err = check_len(8, 4).unwrap_err();
//! assert_eq!(err.err_type(), ErrType::CapacityExceeded { requested: 8, max: 4 });
//! ```

// 显式目标保证 stable rustdoc 也能解析；nightly 的 redundant_explicit_links 会将其视为冗余。
#![allow(rustdoc::redundant_explicit_links)]

#[cfg(feature = "alloc")]
use alloc::string::String;
use core::{fmt, str, time::Duration};

#[cfg(not(feature = "alloc"))]
const MAX_MSG_LEN: usize = 62;

/// 错误上下文文本。
///
/// - 静态字符串直接借用（零分配）。
/// - 开启 `alloc` 特性时，动态文本使用 `String` 在堆上完整保存。
/// - 未开启 `alloc` 特性时，动态文本在固定的 62 字节内联缓冲中截断保存，
///   截断时保证落在 UTF-8 字符边界上。
#[derive(Clone)]
pub enum ErrorMessage {
    /// 直接借用的静态字符串（零分配）。
    Static(&'static str),
    /// 堆上保存的动态文本（启用 `alloc` 时）。
    #[cfg(feature = "alloc")]
    Inline(String),
    /// 定长内联缓冲与已写入字节数（未启用 `alloc` 时，最多 62 字节）。
    #[cfg(not(feature = "alloc"))]
    Inline([u8; MAX_MSG_LEN], u8),
}

impl ErrorMessage {
    /// 由静态字符串构造错误文本（零分配）。
    #[inline]
    pub const fn from_static(s: &'static str) -> Self {
        Self::Static(s)
    }

    /// 由运行时字符串构造错误文本：`alloc` 开启时转 `String`，否则截断至 62 字节内联缓冲（保证 UTF-8 边界）。
    pub fn from_str_slice(value: &str) -> Self {
        #[cfg(feature = "alloc")]
        {
            Self::Inline(String::from(value))
        }
        #[cfg(not(feature = "alloc"))]
        {
            let mut buf = [0u8; MAX_MSG_LEN];
            let max_len = MAX_MSG_LEN.min(value.len());

            let mut len = max_len;
            while len > 0 && !value.is_char_boundary(len) {
                len -= 1;
            }

            buf[..len].copy_from_slice(&value.as_bytes()[..len]);
            Self::Inline(buf, len as u8)
        }
    }

    /// 由 `format_args!` 构造错误文本；纯静态字面量直接走 `Static`，不触发分配。
    pub fn from_fmt(args: fmt::Arguments<'_>) -> Self {
        // 即使是 alloc 模式，纯静态字面量也直接走 Static 避免堆分配
        if let Some(s) = args.as_str() {
            return Self::Static(s);
        }

        #[cfg(feature = "alloc")]
        {
            Self::Inline(alloc::fmt::format(args))
        }
        #[cfg(not(feature = "alloc"))]
        {
            let mut writer = InlineWriter {
                buf: [0u8; MAX_MSG_LEN],
                len: 0,
            };
            let _ = fmt::write(&mut writer, args);
            Self::Inline(writer.buf, writer.len as u8)
        }
    }

    /// 取得错误文本（`Static` 直接返回字面量；`Inline` 按构造时记录的长度切出）。
    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            ErrorMessage::Static(s) => s,
            #[cfg(feature = "alloc")]
            ErrorMessage::Inline(s) => s.as_str(),
            #[cfg(not(feature = "alloc"))]
            ErrorMessage::Inline(buf, len) => {
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
        Self::Inline(value)
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
            let mut copy_len = remain;
            while copy_len > 0 && !s.is_char_boundary(copy_len) {
                copy_len -= 1;
            }
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

/// 全库共用的错误类型，携带错误种类、来源模块与可选上下文。
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    module: &'static str,
    err_type: ErrType,
    context: Option<ErrorMessage>,
}

impl Error {
    /// 以来源模块名与错误种类构造错误（无附加上下文）。
    #[inline]
    pub const fn new(module: &'static str, err_type: ErrType) -> Self {
        Self {
            module,
            err_type,
            context: None,
        }
    }

    /// 附加静态字符串上下文并返回自身（可链式调用多次，保留最后一次/首条记录依实现而定）。
    #[inline]
    pub fn with_static_context(mut self, msg: &'static str) -> Self {
        self.context = Some(ErrorMessage::from_static(msg));
        self
    }

    /// 附加运行时字符串上下文并返回自身。
    #[inline]
    pub fn with_str_context(mut self, msg: &str) -> Self {
        self.context = Some(ErrorMessage::from_str_slice(msg));
        self
    }

    /// 附加 `format_args!` 上下文并返回自身。
    #[inline]
    pub fn with_fmt_context(mut self, args: fmt::Arguments<'_>) -> Self {
        self.context = Some(ErrorMessage::from_fmt(args));
        self
    }

    /// 出错的模块名（编译期静态字符串）。
    #[inline]
    pub const fn module(&self) -> &'static str {
        self.module
    }

    /// 错误类别（可 `match` 程序化处理；`Eq` 判等可直接用于测试断言）。
    #[inline]
    pub const fn err_type(&self) -> ErrType {
        self.err_type
    }

    /// 附加上下文文本（经 `with_*_context` 设置时可取回）。
    #[inline]
    pub fn context(&self) -> Option<&str> {
        self.context.as_ref().map(|c| c.as_str())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Error");
        s.field("module", &self.module)
            .field("err_type", &self.err_type);
        if let Some(ctx) = &self.context {
            s.field("context", &ctx.as_str());
        }
        s.finish()
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

impl core::error::Error for Error {}

/// 全库共用的返回类型。
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// 构造当前模块的 [`Error`]。
///
/// - `err!(kind)`：无上下文。
/// - `err!(kind, "fmt {}", arg)`：附带格式化上下文（开启 `alloc` 时为完整 `String`，否则为最多 62 字节内联缓冲）。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::{err, errors::ErrType};
///
/// let e = err!(ErrType::InvalidArg);
/// assert_eq!(e.err_type(), ErrType::InvalidArg);
/// ```
#[macro_export]
macro_rules! err {
    ($err_type:expr $(,)?) => {
        $crate::errors::Error::new(core::module_path!(), $err_type)
    };
    ($err_type:expr, $($arg:tt)+) => {
        $crate::errors::Error::new(core::module_path!(), $err_type)
            .with_fmt_context(core::format_args!($($arg)+))
    };
}

/// 返回 `Err(err!(...))`。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::{bail, errors::{ErrType, Result}};
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
        return Err($crate::err!($($tt)+))
    };
}

/// 条件不满足时返回错误。
///
/// 开启 `alloc` 特性时，会自动在错误上下文中附带失败的条件表达式（如 `condition failed: a == b`）。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::{ensure, errors::{ErrType, Result}};
///
/// fn check(n: usize) -> Result<()> {
///     ensure!(n > 0, ErrType::InvalidArg);
///     Ok(())
/// }
///
/// assert!(check(1).is_ok());
/// assert!(check(0).is_err());
/// ```
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err_type:expr $(,)?) => {
        if !($cond) {
            return Err(
                $crate::errors::Error::new(core::module_path!(), $err_type)
                    .with_static_context(core::concat!(
                        "condition failed: ",
                        core::stringify!($cond)
                    )),
            );
        }
    };
    ($cond:expr, $err_type:expr, $($arg:tt)+) => {
        if !($cond) {
            return Err(
                $crate::errors::Error::new(core::module_path!(), $err_type)
                    .with_fmt_context(core::format_args!(
                        "{} (condition failed: {})",
                        core::format_args!($($arg)+),
                        core::stringify!($cond)
                    )),
            );
        }
    };
}
/// 条件不满足时返回错误。
///
/// 开启 `alloc` 特性时，会自动在错误上下文中附带失败的条件表达式（如 `condition failed: a == b`）。
///
/// # Examples
///
/// ```rust
/// use lib_unknown::{ensure, errors::{ErrType, Result}};
///
/// fn check(n: usize) -> Result<()> {
///     ensure!(n > 0, ErrType::InvalidArg);
///     Ok(())
/// }
///
/// assert!(check(1).is_ok());
/// assert!(check(0).is_err());
/// ```
#[cfg(not(feature = "alloc"))]
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($tt:tt)+) => {
        if !($cond) {
            $crate::bail!($($tt)+);
        }
    };
}

#[allow(unused_imports)]
pub(crate) use bail;
#[allow(unused_imports)]
pub(crate) use ensure;
#[allow(unused_imports)]
pub(crate) use err;
