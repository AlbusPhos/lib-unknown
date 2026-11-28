// SPDX-License-Identifier: MPL-2.0

//! 运行时熵源：微架构探针与种子派生。

#![no_std]

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// 提供近似 1 GHz 的探针基频（作为 [`seed`] 派生路径的熵辅助分量）。
pub fn probe() -> u64 {
    const SHIFT: u8 = 48;
    const TARGET_FREQ: u128 = 1_000_000_000; // 目标 1GHz (1 tick = 1 ns)
    static MULTIPLIER_Q48: AtomicU64 = AtomicU64::new(0);

    fn raw_probe() -> u64 {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: `_rdtsc` 仅读取时间戳计数器，不访问内存，不影响内存安全。
        unsafe {
            core::arch::x86_64::_rdtsc()
        }

        #[cfg(target_arch = "x86")]
        // SAFETY: 同上。
        unsafe {
            core::arch::x86::_rdtsc()
        }

        #[cfg(target_arch = "aarch64")]
        {
            let tsc: u64;
            // SAFETY: 读取用户态虚拟计数器 cntvct_el0，无内存与栈副作用。
            unsafe {
                core::arch::asm!(
                "mrs {}, cntvct_el0",
                out(reg) tsc,
                options(nomem, nostack, preserves_flags)
                );
            }
            tsc
        }

        #[cfg(all(target_arch = "arm", target_feature = "v7"))]
        {
            let lo: u32;
            let hi: u32;
            // SAFETY: ARMv7+ 读取 CNTVCT 64位协处理器计数器。
            unsafe {
                core::arch::asm!(
                "mrrc p15, 1, {}, {}, c14",
                out(reg) lo,
                out(reg) hi,
                options(nomem, nostack, preserves_flags)
                );
            }
            ((hi as u64) << 32) | (lo as u64)
        }

        #[cfg(target_arch = "riscv64")]
        {
            let tsc: u64;
            // SAFETY: 读取 RISC-V time CSR。
            unsafe {
                core::arch::asm!(
                "rdtime {}",
                out(reg) tsc,
                options(nomem, nostack, preserves_flags)
                );
            }
            tsc
        }

        #[cfg(target_arch = "riscv32")]
        {
            let mut hi: u32;
            let mut lo: u32;
            let mut hi2: u32;
            // SAFETY: 32位下循环读取高32位与低32位，防止跨位进位撕裂。
            unsafe {
                core::arch::asm!(
                "2:",
                "rdtimeh {hi}",
                "rdtime {lo}",
                "rdtimeh {hi2}",
                "bne {hi}, {hi2}, 2b",
                hi = out(reg) hi,
                lo = out(reg) lo,
                hi2 = out(reg) hi2,
                options(nomem, nostack, preserves_flags)
                );
            }
            ((hi as u64) << 32) | (lo as u64)
        }

        #[cfg(target_arch = "loongarch64")]
        {
            let tsc: u64;
            // SAFETY: 龙芯64位读取稳定计数器。
            unsafe {
                core::arch::asm!(
                "rdtime.d {}, $r0",
                out(reg) tsc,
                options(nomem, nostack, preserves_flags)
                );
            }
            tsc
        }

        #[cfg(target_arch = "powerpc64")]
        {
            let tsc: u64;
            // SAFETY: 64位 PowerPC 读取完整的 64 位 Time Base 寄存器。
            unsafe {
                core::arch::asm!(
                "mftb {}",
                out(reg) tsc,
                options(nomem, nostack, preserves_flags)
                );
            }
            tsc
        }

        #[cfg(target_arch = "powerpc")]
        {
            let mut hi: u32;
            let mut lo: u32;
            let mut hi2: u32;
            // SAFETY: 32位 PowerPC 需分别读取 TBU 和 TBL 并校验进位。
            unsafe {
                core::arch::asm!(
                "2:",
                "mftbu {hi}",
                "mftb {lo}",
                "mftbu {hi2}",
                "cmpw {hi}, {hi2}",
                "bne 2b",
                hi = out(reg) hi,
                lo = out(reg) lo,
                hi2 = out(reg) hi2,
                options(nomem, nostack)
                );
            }
            ((hi as u64) << 32) | (lo as u64)
        }

        #[cfg(target_arch = "wasm32")]
        {
            #[cfg(target_os = "wasi")]
            {
                // WASI 0.2（preview2）单调时钟：`now()` 为安全函数，仅返回时间戳标量，
                // 不访问宿主内存；不可表示的时间值由宿主 trap，不影响 `raw_probe` 的熵混合流程。
                wasi::clocks::monotonic_clock::now()
            }
            #[cfg(not(target_os = "wasi"))]
            {
                #[wasm_bindgen::prelude::wasm_bindgen]
                unsafe extern "C" {
                    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = performance)]
                    fn now() -> f64;
                }
                // SAFETY: `performance.now()` 仅读取宿主单调时钟，不访问 Rust 内存；
                // `wasm_bindgen` 将该绑定生成为安全函数。
                (now() * 1_000_000.0) as u64
            }
        }

        // 其他未覆盖架构的软件降级探针（确保全平台可编译）
        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "x86",
            target_arch = "aarch64",
            all(target_arch = "arm", target_feature = "v7"),
            target_arch = "riscv64",
            target_arch = "riscv32",
            target_arch = "loongarch64",
            target_arch = "powerpc64",
            target_arch = "powerpc",
            target_arch = "wasm32"
        )))]
        {
            static FALLBACK_TICKS: AtomicU64 = AtomicU64::new(1);
            let tick = FALLBACK_TICKS.fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed);
            tick ^ (&tick as *const u64 as usize as u64)
        }
    }

    fn autodetect_hardware_freq() -> u64 {
        #[cfg(target_arch = "aarch64")]
        {
            let freq: u64;
            unsafe {
                core::arch::asm!(
                "mrs {}, cntfrq_el0",
                out(reg) freq,
                options(nomem, nostack, preserves_flags)
                );
            }
            freq
        }

        #[cfg(target_arch = "loongarch64")]
        {
            let freq: u32;
            unsafe {
                core::arch::asm!(
                "cpucfg {}, {}",
                out(reg) freq,
                in(reg) 4,
                options(nomem, nostack, preserves_flags)
                );
            }
            freq as u64
        }

        #[cfg(target_arch = "wasm32")]
        {
            1_000_000_000
        }
        #[cfg(all(target_arch = "arm", target_feature = "v7"))]
        {
            let freq: u32;
            // SAFETY: 读取 ARMv7 Generic Timer 频率寄存器 CNTFRQ。
            unsafe {
                core::arch::asm!(
                "mrc p15, 0, {}, c14, c0, 0",
                out(reg) freq,
                options(nomem, nostack, preserves_flags)
                );
            }
            freq as u64
        }

        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "loongarch64",
            target_arch = "wasm32",
            all(target_arch = "arm", target_feature = "v7")
        )))]
        {
            2_500_000_000
        }
    }

    let raw = raw_probe();
    let mut mult = MULTIPLIER_Q48.load(Ordering::Relaxed);

    if mult == 0 {
        let hz = autodetect_hardware_freq();
        let final_mult = if hz == 0 {
            1u64 << SHIFT
        } else {
            // 使用 clamp 防止低频时钟(<15.2MHz)溢出截断或超高频时钟归零
            (((TARGET_FREQ << SHIFT) / hz as u128).clamp(1, u64::MAX as u128)) as u64
        };
        MULTIPLIER_Q48.store(final_mult, Ordering::Relaxed);
        mult = final_mult;
    }

    ((raw as u128 * mult as u128) >> SHIFT) as u64
}

/// 采集当前上下文寄存器快照与 ASLR 布局特征（无 UB，无特权指令）。
fn reg_sig() -> u64 {
    let code: u64;
    let stack: u64;
    let frame: u64;
    let extra: u64;

    #[cfg(target_arch = "x86_64")]
    // SAFETY: 显式读取指令指针、栈指针、帧指针与标志寄存器，所有输出均被写入。
    unsafe {
        core::arch::asm!(
        "pushfq",
        "pop {extra}",
        "lea {code}, [rip]",
        "mov {stack}, rsp",
        "mov {frame}, rbp",
        code = out(reg) code,
        stack = out(reg) stack,
        frame = out(reg) frame,
        extra = out(reg) extra,
        options(preserves_flags)
        );
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: 读取 nzcv 状态、PC、SP 与帧指针 x29，均在 EL0 安全可用。
    unsafe {
        core::arch::asm!(
        "mrs {extra}, nzcv",
        "adr {code}, .",
        "mov {stack}, sp",
        "mov {frame}, x29",
        code = out(reg) code,
        stack = out(reg) stack,
        frame = out(reg) frame,
        extra = out(reg) extra,
        options(nomem, nostack, preserves_flags)
        );
    }

    #[cfg(target_arch = "riscv64")]
    // SAFETY: 使用 tp(线程指针) 与 s0(帧指针) 替代可能在 Linux 6.6+ 触发 SIGILL 的 rdinstret。
    unsafe {
        core::arch::asm!(
        "auipc {code}, 0",
        "mv {stack}, sp",
        "mv {frame}, s0",
        "mv {extra}, tp",
        code = out(reg) code,
        stack = out(reg) stack,
        frame = out(reg) frame,
        extra = out(reg) extra,
        options(nomem, nostack, preserves_flags)
        );
    }

    #[cfg(target_arch = "x86")]
    unsafe {
        let (c32, sp32, fp32, ex32): (u32, u32, u32, u32);
        core::arch::asm!(
        "pushfd",
        "pop {extra}",
        "call 2f",
        "2:",
        "pop {code}",
        "mov {stack}, esp",
        "mov {frame}, ebp",
        code = out(reg) c32,
        stack = out(reg) sp32,
        frame = out(reg) fp32,
        extra = out(reg) ex32,
        options(preserves_flags)
        );
        code = c32 as u64;
        stack = sp32 as u64;
        frame = fp32 as u64;
        extra = ex32 as u64;
    }

    #[cfg(target_arch = "arm")]
    unsafe {
        let (c32, sp32, fp32, ex32): (u32, u32, u32, u32);
        // SAFETY: 使用用户态安全的 apsr 替代 CPSR。
        core::arch::asm!(
        "mrs {extra}, apsr",
        "mov {code}, pc",
        "mov {stack}, sp",
        "mov {frame}, r11",
        code = out(reg) c32,
        stack = out(reg) sp32,
        frame = out(reg) fp32,
        extra = out(reg) ex32,
        options(nomem, nostack, preserves_flags)
        );
        code = c32 as u64;
        stack = sp32 as u64;
        frame = fp32 as u64;
        extra = ex32 as u64;
    }

    #[cfg(target_arch = "riscv32")]
    unsafe {
        let (c32, sp32, fp32, ex32): (u32, u32, u32, u32);
        core::arch::asm!(
        "auipc {code}, 0",
        "mv {stack}, sp",
        "mv {frame}, s0",
        "mv {extra}, tp",
        code = out(reg) c32,
        stack = out(reg) sp32,
        frame = out(reg) fp32,
        extra = out(reg) ex32,
        options(nomem, nostack, preserves_flags)
        );
        code = c32 as u64;
        stack = sp32 as u64;
        frame = fp32 as u64;
        extra = ex32 as u64;
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "arm",
        target_arch = "riscv32"
    )))]
    {
        let local_anchor = 0u8;
        code = reg_sig as *const () as usize as u64;
        stack = core::ptr::addr_of!(local_anchor) as usize as u64;
        frame = stack.rotate_left(13);
        extra = code.rotate_right(7);
    }

    let mut s = extra.rotate_left(7);
    s ^= code.wrapping_mul(0x9E3779B97F4A7C15);
    s ^= stack.rotate_right(17).wrapping_mul(0xBF58476D1CE4E5B9);
    s ^= frame.rotate_left(29).wrapping_mul(0x94D049BB133111EB);
    s ^ (s >> 31)
}

/// 尝试从硬件真随机数生成器（TRNG / 硬件熵源指令）读取 64 位熵值。
///
/// - 返回 `Some(u64)`：硬件熵源采样成功。
/// - 返回 `None`：当前架构/CPU 不支持用户态 TRNG 指令，或硬件熵池瞬时耗尽。
pub fn trng() -> Option<u64> {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        x86_trng()
    }

    #[cfg(target_arch = "aarch64")]
    {
        aarch64_trng()
    }

    #[cfg(all(
        any(target_arch = "riscv64", target_arch = "riscv32"),
        target_feature = "zkr"
    ))]
    {
        riscv_trng()
    }

    #[cfg(all(target_arch = "powerpc64", target_feature = "power9-vector"))]
    {
        ppc64_trng()
    }

    #[cfg(target_arch = "wasm32")]
    {
        wasm_trng()
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        all(
            any(target_arch = "riscv64", target_arch = "riscv32"),
            target_feature = "zkr"
        ),
        all(target_arch = "powerpc64", target_feature = "power9-vector"),
        target_arch = "wasm32"
    )))]
    {
        None
    }
}
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn x86_trng() -> Option<u64> {
    use core::sync::atomic::AtomicU8;
    // 0: 未检测, 1: 不支持, 2: 仅 RDRAND, 3: 支持 RDSEED + RDRAND
    static CAP: AtomicU8 = AtomicU8::new(0);

    #[cfg(target_arch = "x86")]
    use core::arch::x86::{__cpuid, __cpuid_count, _rdrand32_step, _rdseed32_step};
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::{__cpuid, __cpuid_count, _rdrand64_step, _rdseed64_step};

    let mut cap = CAP.load(Ordering::Relaxed);
    if cap == 0 {
        // SAFETY: `cpuid` 在所有受支持的 x86/x86_64 目标处理器上均可安全执行，不访问内存。
        // 兼容 MSRV 1.92（unsafe fn）与新版 stdarch（safe fn）。
        #[allow(unused_unsafe)]
        let (has_rdrand, has_rdseed) = unsafe {
            let leaf0 = __cpuid(0);
            let leaf1 = __cpuid(1);
            let rdrand = (leaf1.ecx & (1 << 30)) != 0;
            let rdseed = if leaf0.eax >= 7 {
                let leaf7 = __cpuid_count(7, 0);
                (leaf7.ebx & (1 << 18)) != 0
            } else {
                false
            };
            (rdrand, rdseed)
        };
        cap = if has_rdseed {
            3
        } else if has_rdrand {
            2
        } else {
            1
        };
        CAP.store(cap, Ordering::Relaxed);
    }

    if cap <= 1 {
        return None;
    }

    #[target_feature(enable = "rdseed")]
    unsafe fn try_rdseed() -> Option<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            let mut val = 0u64;
            for _ in 0..16 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDSEED，且 `val` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdseed64_step(&mut val) } == 1 {
                    return Some(val);
                }
            }
            None
        }
        #[cfg(target_arch = "x86")]
        {
            let (mut lo, mut hi) = (0u32, 0u32);
            let mut ok_lo = false;
            for _ in 0..16 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDSEED，且 `lo` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdseed32_step(&mut lo) } == 1 {
                    ok_lo = true;
                    break;
                }
            }
            if !ok_lo {
                return None;
            }
            for _ in 0..16 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDSEED，且 `hi` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdseed32_step(&mut hi) } == 1 {
                    return Some(((hi as u64) << 32) | (lo as u64));
                }
            }
            None
        }
    }

    #[target_feature(enable = "rdrand")]
    unsafe fn try_rdrand() -> Option<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            let mut val = 0u64;
            // Intel SDM 建议 RDRAND 最多重试 10 次
            for _ in 0..10 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDRAND，且 `val` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdrand64_step(&mut val) } == 1 {
                    return Some(val);
                }
            }
            None
        }
        #[cfg(target_arch = "x86")]
        {
            let (mut lo, mut hi) = (0u32, 0u32);
            let mut ok_lo = false;
            for _ in 0..10 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDRAND，且 `lo` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdrand32_step(&mut lo) } == 1 {
                    ok_lo = true;
                    break;
                }
            }
            if !ok_lo {
                return None;
            }
            for _ in 0..10 {
                // SAFETY: 调用方已通过 CPUID 确认支持 RDRAND，且 `hi` 为有效的可变引用。
                #[allow(unused_unsafe)]
                if unsafe { _rdrand32_step(&mut hi) } == 1 {
                    return Some(((hi as u64) << 32) | (lo as u64));
                }
            }
            None
        }
    }

    // 优先读取物理熵调理输出 RDSEED；瞬时耗尽时回退到硬件 DRBG 输出 RDRAND
    if cap == 3 {
        // SAFETY: 已通过 CPUID Leaf 7 EBX[18] 确认支持 RDSEED。
        if let Some(v) = unsafe { try_rdseed() } {
            return Some(v);
        }
    }
    // SAFETY: 已通过 CPUID Leaf 1 ECX[30] 确认支持 RDRAND。
    unsafe { try_rdrand() }
}

#[cfg(target_arch = "aarch64")]
fn aarch64_trng() -> Option<u64> {
    fn has_feat_rng() -> bool {
        #[cfg(target_feature = "rand")]
        {
            true
        }
        #[cfg(not(target_feature = "rand"))]
        {
            // Linux / Android 内核在 EL0 下会捕获并安全模拟 ID_AA64ISAR0_EL1 读取
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                use core::sync::atomic::AtomicU8;
                static CAP: AtomicU8 = AtomicU8::new(0);
                let mut cap = CAP.load(Ordering::Relaxed);
                if cap == 0 {
                    let isar0: u64;
                    // SAFETY: Linux/Android 内核为 EL0 模拟读取处理器特性寄存器。
                    unsafe {
                        core::arch::asm!(
                        "mrs {}, id_aa64isar0_el1",
                        out(reg) isar0,
                        options(nomem, nostack, preserves_flags)
                        );
                    }
                    // bits [63:60] 为 RNDR 字段，非 0 表示硬件支持 FEAT_RNG
                    cap = if ((isar0 >> 60) & 0xF) != 0 { 2 } else { 1 };
                    CAP.store(cap, Ordering::Relaxed);
                }
                cap == 2
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                false
            }
        }
    }

    if !has_feat_rng() {
        return None;
    }

    for _ in 0..10 {
        let val: u64;
        let ok: u64;
        // SAFETY: 使用系统寄存器编码 s3_3_c2_c4_0 (RNDR) 避免编译期强制要求 `+rand`；
        // 成功时 PSTATE.Z=0，失败时 Z=1，注意不可标注 `preserves_flags`。
        unsafe {
            core::arch::asm!(
            "mrs {val}, s3_3_c2_c4_0",
            "cset {ok}, ne",
            val = out(reg) val,
            ok = out(reg) ok,
            options(nomem, nostack)
            );
        }
        if ok != 0 {
            return Some(val);
        }
    }
    None
}

#[cfg(all(
    any(target_arch = "riscv64", target_arch = "riscv32"),
    target_feature = "zkr"
))]
fn riscv_trng() -> Option<u64> {
    let mut acc = 0u64;
    let mut collected = 0u32;
    // Zkr 每次成功返回 16-bit 有效熵 (ES16)，需拼接 4 次构成 64 位
    for _ in 0..64 {
        let raw: usize;
        // SAFETY: RISC-V Crypto 规范要求使用 csrrw 对 seed CSR (0x015) 写 x0 触发采样。
        unsafe {
            core::arch::asm!(
            "csrrw {rd}, 0x015, x0",
            rd = out(reg) raw,
            options(nomem, nostack, preserves_flags)
            );
        }
        match (raw >> 30) & 0b11 {
            0b10 => {
                // ES16: 低 16 位为有效物理熵
                acc = (acc << 16) | ((raw & 0xFFFF) as u64);
                collected += 1;
                if collected == 4 {
                    return Some(acc);
                }
            }
            0b00 | 0b01 => continue, // BIST 或 WAIT，继续轮询
            _ => return None,        // DEAD (0b11)，硬件熵源故障
        }
    }
    None
}

#[cfg(all(target_arch = "powerpc64", target_feature = "power9-vector"))]
fn ppc64_trng() -> Option<u64> {
    for _ in 0..10 {
        let val: u64;
        // SAFETY: POWER9+ `darn` 指令，L=1 返回 64 位已调理随机数，失败返回全 1。
        unsafe {
            core::arch::asm!(
            "darn {val}, 1",
            val = out(reg) val,
            options(nomem, nostack, preserves_flags)
            );
        }
        if val != u64::MAX {
            return Some(val);
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn wasm_trng() -> Option<u64> {
    #[cfg(target_os = "wasi")]
    {
        Some(wasi::random::random::get_random_u64())
    }
    #[cfg(not(target_os = "wasi"))]
    {
        None
    }
}

/// 混合多熵源（硬件计数器、ASLR 寄存器特征、L1 缓存与分支预测抖动）生成 64 位种子。
pub fn seed() -> u64 {
    use core::hint::black_box;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    const SCRATCH_LEN: usize = 16;
    static CONFIG: (u8, u8, u8) = (8, 4, 4);

    let c = COUNTER.fetch_add(1, Ordering::Relaxed) as u64;
    let x = probe();
    let y = reg_sig();
    let z = trng().unwrap_or(0);

    let mut scratch = [0u64; SCRATCH_LEN];
    let sp_addr = scratch.as_mut_ptr() as usize as u64;
    for (idx, slot) in scratch.iter_mut().enumerate() {
        *slot = x
            .wrapping_add(y.rotate_left((idx as u32) * 4))
            .wrapping_add(sp_addr ^ ((idx as u64).wrapping_mul(0x9E3779B97F4A7C15)))
            ^ !z;
    }
    let sp = black_box(scratch.as_mut_ptr());

    // SAFETY: `sp` 指向已完全初始化的 `scratch` 数组，`masked` 由
    // `& (SCRATCH_LEN - 1)` 严格限幅，读写均落在数组范围内且元素为 `u64`。
    let rs = |idx: usize, salt: u64| unsafe {
        let masked = idx & (SCRATCH_LEN - 1);
        let ptr = sp.add(masked);
        let val = core::ptr::read_volatile(ptr);
        let next = val.rotate_left(13).wrapping_add(salt | 1);
        core::ptr::write_volatile(ptr, next);
        ((val >> 24) as u8).saturating_add(2)
    };

    let t2s = |t: u64, mut s: u64, i: u8| {
        if t.rotate_left(i as u32) as u8 & 1 == 0 {
            let idx = (t as usize) ^ (i as usize);
            s = s.rotate_left((rs(idx, t) & 63) as u32);
            let idx2 = (s >> 16) as usize;
            s = s.wrapping_mul((rs(idx2, s) as u64) | 1);
        } else {
            let idx = ((t >> 16) as usize) ^ (i as usize);
            s = s.rotate_right((rs(idx, t) & 63) as u32);
            let idx2 = s as usize;
            s = s.wrapping_mul((rs(idx2, s) as u64) | 1);
        }

        s = match s.rotate_left(i as u32) as u8 {
            0 => s ^ 3862943611198905,
            255 => s.wrapping_mul(probe().saturating_add(2) | 1),
            127 => s.rotate_right(7),
            3 => s.wrapping_mul(767),
            7 => s.wrapping_add(945910149055313305),
            32 => s.wrapping_mul(302585092994045),
            64 => s ^ trng().unwrap_or(0),
            _ => s ^ (t.rotate_right((i & 63) as u32)),
        };
        s
    };

    let mut s = (x ^ y ^ z ^ !c).rotate_right((c & 63) as u32);

    for i in 0..black_box(CONFIG.0) {
        if ((s >> 8) as u8 ^ i.wrapping_mul(0x9D)) > 127 {
            s = s
                .rotate_left(5)
                .wrapping_add((i as u64) ^ 0x9E3779B97F4A7C15);
            continue;
        }
        let t1 = probe().wrapping_mul(((i as u64 ^ s) | 1).wrapping_mul(302585092994045));
        s ^= t1;
        s = t2s(t1, s, i);

        for j in 0..black_box(CONFIG.1) {
            let ij = i ^ j.rotate_left(2);
            if ((s >> 16) as u8 ^ ij.wrapping_mul(0x5B)) <= 127 {
                s = s
                    .rotate_right(11)
                    .wrapping_add((ij as u64) ^ 0xBF58476D1CE4E5B9);
                continue;
            }
            let t2 = probe().wrapping_mul(((ij as u64 ^ s) | 1).wrapping_mul(302585092994045));
            s ^= t2;
            s = t2s(t2, s, ij);

            for k in 0..black_box(CONFIG.2) {
                let ijk = ij ^ k.rotate_left(4);
                if ((s >> 32) as u8 ^ ijk.wrapping_mul(0xB5)) > 127 {
                    s = s
                        .rotate_left(17)
                        .wrapping_add((ijk as u64) ^ 0x94D049BB133111EB);
                    continue;
                }
                let t3 = probe().wrapping_mul(((ijk as u64 ^ s) | 1).wrapping_mul(302585092994045));
                s ^= t3;
                s = t2s(t3, s, ijk);
            }
        }
    }

    s = (s ^ (s >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    s = (s ^ (s >> 27)).wrapping_mul(0x94D049BB133111EB);
    s ^ (s >> 31)
}

/// 组合两次 [`seed`] 采样为 128 位运行时种子（低 64 位 + 高 64 位）。
pub fn seed128() -> u128 {
    let lo = seed() as u128;
    let hi = seed() as u128;
    lo | (hi << 64)
}
