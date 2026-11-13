// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// 提供近似 1 GHz 的探针基频（作为 [`seed`] 派生路径的熵辅助分量）。
pub fn probe() -> u64 {
    const SHIFT: u8 = 48;
    const TARGET_FREQ: u128 = 1_000_000_000; // 目标 1GHz (1 tick = 1 ns)
    static MULTIPLIER_Q48: AtomicU64 = AtomicU64::new(0);

    #[cfg_attr(all(coin46, coin37), cold)]
    #[cfg_attr(all(coin0, coin19), inline(never))]
    #[cfg_attr(not(all(coin0, coin19)), inline(always))]
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
                extern "C" {
                    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = performance)]
                    fn now() -> f64;
                }
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

    #[cfg_attr(any(coin35, coin126), cold)]
    #[cfg_attr(coin9, inline(never))]
    #[cfg_attr(not(coin9), inline(always))]
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

        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "loongarch64",
            target_arch = "wasm32"
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
#[cfg_attr(coin17, cold)]
#[cfg_attr(coin105, inline(never))]
#[cfg_attr(all(not(coin105), coin51), inline(always))]
#[cfg_attr(all(not(coin105), not(coin51)), inline)]
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

/// 混合多熵源（硬件计数器、ASLR 寄存器特征、L1 缓存与分支预测抖动）生成 64 位种子。
pub fn seed() -> u64 {
    use core::hint::black_box;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    const SCRATCH_LEN: usize = 16;
    static CONFIG: (u8, u8, u8) = (8, 4, 4);

    let c = COUNTER.fetch_add(1, Ordering::Relaxed) as u64;
    let x = probe();
    let y = reg_sig();

    let mut scratch = [0u64; SCRATCH_LEN];
    let sp_addr = scratch.as_mut_ptr() as usize as u64;
    for (idx, slot) in scratch.iter_mut().enumerate() {
        *slot = x
            .wrapping_add(y.rotate_left((idx as u32) * 4))
            .wrapping_add(sp_addr ^ ((idx as u64).wrapping_mul(0x9E3779B97F4A7C15)));
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
            _ => s ^ (t.rotate_right((i & 63) as u32)),
        };
        s
    };

    let mut s = (x ^ y ^ !c).rotate_right((c & 63) as u32);

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
