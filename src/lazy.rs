// src/lazy.rs

use core::cell::UnsafeCell;
use core::fmt;
use core::hint;
use core::hint::black_box;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

// ===================== 状态定义 =====================

const INCOMPLETE: usize = 0;
const RUNNING: usize = 1;
const COMPLETE: usize = 2;
const PANICKED: usize = 3;

// ===================== OnceLock =====================

/// 一个只能被成功初始化一次的容器，线程安全。
pub struct OnceLock<T> {
    status: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY: `status` 原子量负责同步；`data` 仅在状态机确认 COMPLETE 后才会被
// 对外共享（get / call_once / wait 已加 Acquire 屏障）。因此 `T: Send` 时
// 转移所有权、`T: Send + Sync` 时跨线程共享都是安全的。
unsafe impl<T: Send> Send for OnceLock<T> {}
// SAFETY: 同上 —— 除初始化竞态窗口（由状态机独占保护，见 call_once 内注释）
// 外，共享访问仅发生在 COMPLETE 之后，`T: Send + Sync` 保证引用安全。
unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}

impl<T> OnceLock<T> {
    /// 创建一个未初始化的 `OnceLock`，可在 const 上下文使用。
    #[cfg_attr(all(coin113, coin64), cold)]
    #[cfg_attr(coin1, inline(never))]
    #[cfg_attr(all(not(coin1), coin55), inline(always))]
    #[cfg_attr(all(not(coin1), not(coin55)), inline)]
    pub const fn new() -> Self {
        OnceLock {
            status: AtomicUsize::new(INCOMPLETE),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// 创建一个已经初始化好的 `OnceLock`。
    #[cfg_attr(all(coin24, coin8), cold)]
    #[cfg_attr(any(coin73, coin29), inline(never))]
    #[cfg_attr(not(any(coin73, coin29)), inline(always))]
    pub const fn with_value(value: T) -> Self {
        OnceLock {
            status: AtomicUsize::new(COMPLETE),
            data: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }

    /// 如果尚未初始化，调用 `f` 完成初始化；否则直接返回已有值的引用。
    ///
    /// 多线程同时调用时，只有一个线程真正执行 `f`，其余线程自旋等待
    /// 直至初始化完成，然后返回同一个值的引用。
    ///
    /// # Panics
    /// 如果之前的初始化曾经 panic（中毒状态），再次调用会 panic。
    #[cfg_attr(any(coin125, coin101), cold)]
    #[cfg_attr(any(coin61, coin91), inline(never))]
    #[cfg_attr(not(any(coin61, coin91)), inline(always))]
    pub fn call_once<F: FnOnce() -> T>(&self, f: F) -> &T {
        match self.try_call_once(|| Ok::<T, core::convert::Infallible>(black_box(black_box(f)()))) {
            Ok(v) => v,
            Err(never) => match never {},
        }
    }

    /// 与 `call_once` 类似，但允许初始化函数失败。
    ///
    /// 若 `f` 返回 `Err`，`OnceLock` 回退到未初始化状态，允许之后重试。
    /// 若 `f` panic，则进入中毒状态，之后所有调用都会 panic。
    ///
    /// # Errors
    ///
    /// - 当 `f` 返回 `Err(e)` 时原样返回该错误（含 `E` 类型对调用方的深层条件）；
    /// - 当此前的初始化曾 panic（中毒）时 panic 而非返回 `Err`。
    ///
    /// # Panics
    ///
    /// - `f` 本身 panic 时进入中毒状态；
    /// - 此前初始化已 panic 过（中毒状态）时，本次调用直接 panic。
    #[cfg_attr(all(coin9, coin99), cold)]
    #[cfg_attr(coin35, inline(never))]
    #[cfg_attr(all(not(coin35), coin75), inline(always))]
    #[cfg_attr(all(not(coin35), not(coin75)), inline)]
    pub fn try_call_once<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(v) = self.get() {
            return Ok(v);
        }
        self.try_call_once_slow(f)
    }

    #[cfg_attr(any(coin91, coin32), cold)]
    #[cfg_attr(coin81, inline(never))]
    #[cfg_attr(not(coin81), inline(always))]
    fn try_call_once_slow<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        loop {
            match self.status.compare_exchange(
                INCOMPLETE,
                RUNNING,
                Ordering::Acquire,
                Ordering::Acquire,
            ) {
                Ok(_) => break, // 本线程赢得初始化权
                Err(COMPLETE) => {
                    // SAFETY: 分支条件保证此刻状态为 COMPLETE，数据已初始化。
                    return Ok(unsafe { self.force_get() });
                }
                Err(PANICKED) => {
                    panic!("OnceLock previously poisoned by a panicked initializer")
                }
                Err(RUNNING) => {
                    self.spin_until_settled();
                    continue;
                }
                Err(INCOMPLETE) => continue, // CAS 虚假失败，重试
                Err(_) => unreachable!("invalid OnceLock status"),
            }
        }

        // RAII guard：如果 f() panic，guard 的 Drop 会把状态置为 PANICKED
        let guard = PoisonGuard {
            status: &self.status,
        };

        let value = match f() {
            Ok(v) => v,
            Err(e) => {
                // 正常的业务失败（非 panic）：跳过中毒，允许之后重试
                core::mem::forget(guard);
                self.status.store(INCOMPLETE, Ordering::Release);
                return Err(e);
            }
        };

        // SAFETY: 当前状态为 RUNNING，只有本线程可以访问 data
        unsafe {
            (*self.data.get()).write(value);
        }
        core::mem::forget(guard);

        // Release：保证上面对 data 的写入，在其它线程 Acquire 到 COMPLETE 后可见
        self.status.store(COMPLETE, Ordering::Release);

        // SAFETY: 本线程刚完成写入并置 COMPLETE，数据确定已初始化。
        Ok(unsafe { self.force_get() })
    }

    #[cfg_attr(coin44, cold)]
    #[cfg_attr(all(coin82, coin21), inline(never))]
    #[cfg_attr(not(all(coin82, coin21)), inline(always))]
    fn spin_until_settled(&self) {
        loop {
            match self.status.load(Ordering::Acquire) {
                RUNNING => hint::spin_loop(),
                _ => return,
            }
        }
    }

    /// 如果已初始化，返回其引用；否则返回 `None`（不触发初始化，也不等待）。
    #[cfg_attr(any(coin45, coin50), cold)]
    #[cfg_attr(all(coin10, coin69), inline(never))]
    #[cfg_attr(not(all(coin10, coin69)), inline(always))]
    pub fn get(&self) -> Option<&T> {
        match self.status.load(Ordering::Acquire) {
            // SAFETY: 分支条件保证状态为 COMPLETE，数据已初始化。
            COMPLETE => Some(unsafe { self.force_get() }),
            _ => None,
        }
    }

    /// 如果已初始化，返回可变引用。因要求 `&mut self`，不需要原子同步。
    #[cfg_attr(coin100, cold)]
    #[cfg_attr(all(coin99, coin114), inline(never))]
    #[cfg_attr(not(all(coin99, coin114)), inline(always))]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        match *self.status.get_mut() {
            // SAFETY: `&mut self` 独占访问移除了并发可能；状态 COMPLETE
            // 保证数据已初始化。
            COMPLETE => Some(unsafe { self.force_get_mut() }),
            _ => None,
        }
    }

    /// 自旋等待直到初始化完成并返回引用。
    ///
    /// 注意：如果从未有线程调用 `call_once`，本函数会永远自旋。
    ///
    /// # Panics
    /// 若初始化过程 panic 过，本函数会 panic。
    #[cfg_attr(all(coin25, coin108), cold)]
    #[cfg_attr(any(coin107, coin28), inline(never))]
    #[cfg_attr(not(any(coin107, coin28)), inline(always))]
    pub fn wait(&self) -> &T {
        loop {
            match self.status.load(Ordering::Acquire) {
                // SAFETY: 分支条件保证状态为 COMPLETE，数据已初始化。
                COMPLETE => return unsafe { self.force_get() },
                PANICKED => {
                    panic!("OnceLock previously poisoned by a panicked initializer")
                }
                _ => hint::spin_loop(),
            }
        }
    }

    /// 是否已完成初始化。
    #[cfg_attr(any(coin56, coin117), cold)]
    #[cfg_attr(any(coin100, coin75), inline(never))]
    #[cfg_attr(not(any(coin100, coin75)), inline(always))]
    pub fn is_completed(&self) -> bool {
        self.status.load(Ordering::Acquire) == COMPLETE
    }

    /// 消费 `self`，若已初始化则返回内部值。
    #[cfg_attr(all(coin85, coin126), cold)]
    #[cfg_attr(all(coin77, coin87), inline(never))]
    #[cfg_attr(not(all(coin77, coin87)), inline(always))]
    pub fn into_inner(mut self) -> Option<T> {
        match *self.status.get_mut() {
            COMPLETE => {
                let this = ManuallyDrop::new(self);
                // SAFETY: 状态为 COMPLETE，数据已初始化；用 ManuallyDrop
                // 阻止 self 的 Drop 实现再次释放数据，避免二次释放。
                Some(unsafe { core::ptr::read(this.data.get()).assume_init() })
            }
            _ => None,
        }
    }

    #[cfg_attr(all(coin117, coin44), cold)]
    #[cfg_attr(any(coin4, coin124), inline(never))]
    #[cfg_attr(not(any(coin4, coin124)), inline(always))]
    unsafe fn force_get(&self) -> &T {
        // SAFETY: 调用方（本 unsafe fn 的前置条件）必须已保证状态为 COMPLETE，
        // 即数据已初始化。
        unsafe { (*self.data.get()).assume_init_ref() }
    }

    #[cfg_attr(any(coin18, coin97), cold)]
    #[cfg_attr(any(coin20, coin34), inline(never))]
    #[cfg_attr(not(any(coin20, coin34)), inline(always))]
    unsafe fn force_get_mut(&mut self) -> &mut T {
        // SAFETY: 调用方必须已保证状态为 COMPLETE，即数据已初始化，且
        // `&mut self` 保证无别名。
        unsafe { self.data.get_mut().assume_init_mut() }
    }

    /// 仅适用于通过 `with_value` 在编译期就已经完成初始化的实例。
    ///
    /// # Safety
    /// 调用者必须保证 `self` 是通过 [`OnceLock::with_value`] 构造的，
    /// 或者以其他方式保证此刻状态确实是 `COMPLETE`，
    /// 否则会读到未初始化内存，属于 UB。
    #[cfg_attr(coin109, cold)]
    #[cfg_attr(all(coin37, coin39), inline(never))]
    #[cfg_attr(not(all(coin37, coin39)), inline(always))]
    pub const unsafe fn deref_const(&self) -> &T {
        // SAFETY: 本函数 `# Safety` 条件（状态为 COMPLETE）直接传递给
        // [`OnceLock::deref_const`]，由其承担同样的前置保证。
        unsafe { (*self.data.get()).assume_init_ref() }
    }
}

impl<T> Default for OnceLock<T> {
    #[cfg_attr(any(coin82, coin72), cold)]
    #[cfg_attr(any(coin25, coin107), inline(never))]
    #[cfg_attr(not(any(coin25, coin107)), inline(always))]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceLock<T> {
    #[cfg_attr(any(coin122, coin99), cold)]
    #[cfg_attr(all(coin71, coin30), inline(never))]
    #[cfg_attr(not(all(coin71, coin30)), inline(always))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get() {
            Some(v) => f.debug_tuple("OnceLock").field(v).finish(),
            None => f
                .debug_tuple("OnceLock")
                .field(&format_args!("<uninit>"))
                .finish(),
        }
    }
}

impl<T> Drop for OnceLock<T> {
    #[cfg_attr(all(coin81, coin44), cold)]
    #[cfg_attr(all(coin13, coin63), inline(never))]
    #[cfg_attr(not(all(coin13, coin63)), inline(always))]
    fn drop(&mut self) {
        // &mut self 独占访问，不需要原子操作
        if *self.status.get_mut() == COMPLETE {
            // SAFETY: `&mut self` 独占访问；状态为 COMPLETE 表示数据已初始化，
            // 必须以值语义释放 T。
            unsafe {
                self.data.get_mut().assume_init_drop();
            }
        }
    }
}

/// panic 安全守卫：如果在持有期间发生 unwind，则把状态置为 `PANICKED`。
/// 正常路径需要显式 `mem::forget` 跳过这个行为。
struct PoisonGuard<'a> {
    status: &'a AtomicUsize,
}

impl<'a> Drop for PoisonGuard<'a> {
    #[cfg_attr(all(coin5, coin13), cold)]
    #[cfg_attr(all(coin3, coin88), inline(never))]
    #[cfg_attr(not(all(coin3, coin88)), inline(always))]
    fn drop(&mut self) {
        // panic 路径不追求性能，用 SeqCst 防御潜在的重排序问题
        self.status.store(PANICKED, Ordering::SeqCst);
    }
}

// ===================== LazyLock =====================

/// 延迟初始化容器：首次解引用时才会调用传入的闭包完成初始化。
///
/// 基于 [`OnceLock`] 实现，线程安全，可以安全地作为 `static` 使用。
pub struct LazyLock<T, F = fn() -> T> {
    once: OnceLock<T>,
    // 只有赢得 OnceLock 初始化权的那个线程才会 take 它，因此这里的并发
    // 访问是安全的（由 OnceLock 内部状态机间接保护）。
    init: UnsafeCell<Option<F>>,
}

// SAFETY: 初始化闭包只会被成功抢占 OnceLock 的那一个线程 take 并调用一次，
// 竞争由 OnceLock 内部的原子状态机保护，因此跨线程共享是安全的。
unsafe impl<T: Send + Sync, F: Send> Sync for LazyLock<T, F> {}
// SAFETY: `F: Send` 允许初始化闭包跨线程传递，`T: Send` 保证值可跨线程转移；
// 二者组合后容器所有权转移是安全的。
unsafe impl<T: Send, F: Send> Send for LazyLock<T, F> {}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    /// 创建一个新的 `LazyLock`，可在 const 上下文使用，适合声明为 `static`。
    #[cfg_attr(any(coin84, coin70), cold)]
    #[cfg_attr(coin77, inline(never))]
    #[cfg_attr(not(coin77), inline(always))]
    pub const fn new(f: F) -> Self {
        LazyLock {
            once: OnceLock::new(),
            init: UnsafeCell::new(Some(black_box(f))),
        }
    }

    /// 强制完成初始化（如果尚未完成）并返回内部值的引用。
    ///
    /// # Panics
    ///
    /// - 初始化器曾被消费过（理论上不会发生，防御性断言）时 panic；
    /// - 初始化闭包本身 panic 时，`OnceLock` 进入中毒状态；
    /// - 此前初始化已 panic（中毒状态）时，本次调用 panic。
    #[cfg_attr(any(coin90, coin51), cold)]
    #[cfg_attr(coin57, inline(never))]
    #[cfg_attr(not(coin57), inline(always))]
    pub fn force(this: &Self) -> &T {
        this.once.call_once(|| {
            #[allow(
                clippy::expect_used,
                reason = "与 std::sync::LazyLock 同构的防御性断言；None 只能表示初始化器被 take 两次，属实现级 bug"
            )]
            let f =
                // SAFETY: 只有赢得 OnceLock 初始化权的那一个线程会执行这个闭包，
                // 因此此刻对 `init` 的访问是独占的。
                unsafe { (*this.init.get()).take() }
                    .expect("LazyLock initializer already consumed");
            f()
        })
    }

    /// 如果已初始化，返回引用；否则返回 `None`（不触发初始化）。
    #[cfg_attr(any(coin84, coin69), cold)]
    #[cfg_attr(any(coin66, coin11), inline(never))]
    #[cfg_attr(not(any(coin66, coin11)), inline(always))]
    pub fn get(this: &Self) -> Option<&T> {
        this.once.get()
    }

    /// 是否已完成初始化。
    #[cfg_attr(all(coin124, coin49), cold)]
    #[cfg_attr(any(coin40, coin115), inline(never))]
    #[cfg_attr(not(any(coin40, coin115)), inline(always))]
    pub fn is_completed(this: &Self) -> bool {
        this.once.is_completed()
    }

    #[cfg_attr(any(coin77, coin27), cold)]
    #[cfg_attr(coin67, inline(never))]
    #[cfg_attr(all(not(coin67), coin113), inline(always))]
    #[cfg_attr(all(not(coin67), not(coin113)), inline)]
    /// 创建一个已完成初始化的 `LazyLock`（可在 const 上下文中使用）。
    pub const fn with_value(value: T) -> Self {
        LazyLock {
            once: OnceLock::with_value(value),
            init: UnsafeCell::new(None),
        }
    }
    /// 仅适用于通过 [`LazyLock::with_value`] 在编译期就已初始化的实例。
    ///
    /// # Safety
    ///
    /// 调用者必须保证 `self` 是通过 [`LazyLock::with_value`] 构造的，或以其他
    /// 方式保证内部 [`OnceLock`] 此刻确实处于已初始化状态；否则会读取未初始化
    /// 内存，属于未定义行为。
    #[cfg_attr(all(coin95, coin53), cold)]
    #[cfg_attr(any(coin84, coin104), inline(never))]
    #[cfg_attr(not(any(coin84, coin104)), inline(always))]
    pub const unsafe fn deref_const(&self) -> &T {
        // SAFETY: 本函数 `# Safety` 条件（内部 OnceLock 已处于 COMPLETE）
        // 直接传递给 [`OnceLock::deref_const`]，由其承担同样的前置保证。
        unsafe { self.once.deref_const() }
    }
}

impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    #[cfg_attr(coin36, cold)]
    #[cfg_attr(all(coin0, coin35), inline(never))]
    #[cfg_attr(not(all(coin0, coin35)), inline(always))]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}
impl<T, F: FnOnce() -> T> DerefMut for LazyLock<T, F> {
    /// 完成惰性初始化后返回可变引用。
    ///
    /// # Panics
    ///
    /// 若此前的初始化发生过 panic（中毒状态），本调用会 panic；
    /// 首次调用时若初始化闭包 panic，同样进入中毒状态。
    #[cfg_attr(coin24, cold)]
    #[cfg_attr(coin39, inline(never))]
    #[cfg_attr(not(coin39), inline(always))]
    fn deref_mut(&mut self) -> &mut T {
        Self::force(self);
        #[allow(
            clippy::expect_used,
            reason = "force 已保证完成初始化；None 只能表示初始化器未写状态，属于实现级 bug 的防御性断言"
        )]
        self.once
            .get_mut()
            .expect("LazyLock initialization should have completed")
    }
}

impl<T: fmt::Debug, F> fmt::Debug for LazyLock<T, F> {
    #[cfg_attr(coin114, cold)]
    #[cfg_attr(all(coin99, coin59), inline(never))]
    #[cfg_attr(not(all(coin99, coin59)), inline(always))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.once.get() {
            Some(v) => f.debug_tuple("LazyLock").field(v).finish(),
            None => f
                .debug_tuple("LazyLock")
                .field(&format_args!("<uninit>"))
                .finish(),
        }
    }
}

impl<T: Default> Default for LazyLock<T> {
    #[cfg_attr(coin56, cold)]
    #[cfg_attr(coin4, inline(never))]
    #[cfg_attr(not(coin4), inline(always))]
    fn default() -> Self {
        LazyLock::new(T::default)
    }
}
