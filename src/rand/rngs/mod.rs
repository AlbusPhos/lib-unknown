/// ChaCha 系（RFC 8439 核心，8/12/14/20 轮类型别名）。
#[cfg(feature = "rand-chacha")]
pub mod chacha;
/// Cwg 系（64/128 位状态非线性步进，暖启动后输出）。
#[cfg(feature = "rand-cwg")]
pub mod cwg;
/// SplitMix64 单线程与原子共享两种变体。
#[cfg(feature = "rand-splitmix64")]
pub mod splitmix64;
/// xoshiro256/xoroshiro128 的 `++` 与 `**` 四种变体。
#[cfg(feature = "rand-xoshiro")]
pub mod xoshiro;

#[cfg(all(test, feature = "rand-core", feature = "std"))]
mod tests {
    use crate::rand::core::{self, Rng, RngCore};
    use crate::rand::seed::seed128;
    use std::hint::black_box;
    use std::time::Instant;

    #[allow(dead_code)]
    fn verify_rng_contract<R: RngCore>(name: &str) {
        let seed: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;

        // (a) 验证 From<u128>、Seed128 元组构造、以及 reseed 确定性重放
        let mut r1 = R::from(seed);
        let mut r2 = R::new(((seed >> 64) as u64, seed as u64));
        for step in 0..32 {
            assert_eq!(
                r1.next_u64(),
                r2.next_u64(),
                "{name}: u64 步进不一致 @{step}"
            );
            assert_eq!(
                r1.next_u32(),
                r2.next_u32(),
                "{name}: u32 步进不一致 @{step}"
            );
            assert_eq!(
                r1.next_u128(),
                r2.next_u128(),
                "{name}: u128 步进不一致 @{step}"
            );
        }

        let expected_first = R::from(seed).next_u64();
        r1.reseed(seed);
        assert_eq!(
            r1.next_u64(),
            expected_first,
            "{name}: reseed 后状态未能精确重置"
        );

        // (b) 验证全零种子抗退化（尤其针对 xoshiro/xoroshiro 全零死锁防御）与不同低位种子区分度
        let mut rz = R::from(0u128);
        let any_nonzero = (0..8).any(|_| rz.next_u64() != 0);
        assert!(any_nonzero, "{name}: seed=0 发生全零状态退化");

        let out_s0 = R::from(0u128).next_u128();
        let out_s1 = R::from(1u128).next_u128();
        assert_ne!(
            out_s0, out_s1,
            "{name}: 不同种子产出了完全相同的首帧 128 位输出"
        );

        // (c) 验证 fill_bytes 在各种对齐/非对齐尾部长度下“无漏写”且“无越界污染”
        for len in [0usize, 1, 7, 8, 9, 15, 16, 17, 35, 64, 131] {
            let mut ra = R::from(seed);
            let mut rb = R::from(seed);
            let mut buf_zero = [0x00u8; 136];
            let mut buf_ones = [0xFFu8; 136];

            ra.fill_bytes(&mut buf_zero[..len]);
            rb.fill_bytes(&mut buf_ones[..len]);

            assert_eq!(
                &buf_zero[..len],
                &buf_ones[..len],
                "{name}: fill_bytes 存在未覆盖字节 (len={len})"
            );
            assert!(
                buf_zero[len..].iter().all(|&b| b == 0x00)
                    && buf_ones[len..].iter().all(|&b| b == 0xFF),
                "{name}: fill_bytes 发生越界覆写 (len={len})"
            );
        }

        // (d) 验证上层 Rng 扩展接口（Random / SampleRange / shuffle）
        let mut rng = R::from(seed);
        for _ in 0..64 {
            let _: bool = rng.random();
            let v_i32 = rng.random_range(-50i32..50);
            assert!((-50..50).contains(&v_i32), "{name}: i32 范围越界");

            let v_single = rng.random_range(42u64..=42);
            assert_eq!(v_single, 42, "{name}: 单点闭区间采样错误");

            let _: u8 = rng.random_range(u8::MIN..=u8::MAX);

            let f: f64 = rng.random();
            assert!((0.0..1.0).contains(&f), "{name}: f64 单位区间越界: {f}");

            let fr = rng.random_range(-2.5f32..3.5f32);
            assert!((-2.5..3.5).contains(&fr), "{name}: f32 范围越界: {fr}");
        }

        let mut shuffled = rng.shuffle([1u8, 2, 3, 4, 5, 6, 7, 8]);
        shuffled.sort_unstable();
        assert_eq!(
            shuffled,
            [1, 2, 3, 4, 5, 6, 7, 8],
            "{name}: shuffle 破坏了元素集合"
        );
    }

    #[test]
    fn test_all_rngs() {
        #[cfg(feature = "rand-splitmix64")]
        {
            verify_rng_contract::<super::splitmix64::SplitMix64>("SplitMix64");
            verify_rng_contract::<super::splitmix64::SplitMix64Atomic>("SplitMix64Atomic");
        }
        #[cfg(feature = "rand-xoshiro")]
        {
            verify_rng_contract::<super::xoshiro::Xoroshiro128PlusPlus>("Xoroshiro128++");
            verify_rng_contract::<super::xoshiro::Xoroshiro128StarStar>("Xoroshiro128**");
            verify_rng_contract::<super::xoshiro::Xoshiro256PlusPlus>("Xoshiro256++");
            verify_rng_contract::<super::xoshiro::Xoshiro256StarStar>("Xoshiro256**");
        }
        #[cfg(feature = "rand-cwg")]
        {
            verify_rng_contract::<super::cwg::Cwg64>("Cwg64");
            verify_rng_contract::<super::cwg::Cwg128x64>("Cwg128x64");
            verify_rng_contract::<super::cwg::Cwg128>("Cwg128");
        }
        #[cfg(feature = "rand-chacha")]
        {
            verify_rng_contract::<super::chacha::ChaCha8>("ChaCha8");
            verify_rng_contract::<super::chacha::ChaCha12>("ChaCha12");
            verify_rng_contract::<super::chacha::ChaCha14>("ChaCha14");
            verify_rng_contract::<super::chacha::ChaCha20>("ChaCha20");
        }

        // 验证全局 PRNG 门面接口
        let mut gbuf = [0u8; 37];
        crate::rand::core::fill_bytes(&mut gbuf);
        assert!(gbuf.iter().any(|&b| b != 0));
        let gv = crate::rand::core::random_range(10u32..20);
        assert!((10..20).contains(&gv));
    }

    #[test]
    #[cfg(all(feature = "rand-splitmix64", feature = "std"))]
    fn test_splitmix64_atomic_concurrent() {
        use super::splitmix64::SplitMix64Atomic;
        const THREADS: u64 = 4;
        const STEPS_PER_THREAD: u64 = 25_000;
        const GOLDEN_GAMMA: u64 = 0x9E3779B97F4A7C15;

        let shared = SplitMix64Atomic::from_seed(0);
        std::thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|| {
                    for _ in 0..STEPS_PER_THREAD {
                        black_box(shared.next_relaxed());
                    }
                });
            }
        });

        let expected_state = (THREADS * STEPS_PER_THREAD).wrapping_mul(GOLDEN_GAMMA);
        let expected_next = SplitMix64Atomic::from_seed(expected_state).next_relaxed();
        assert_eq!(shared.next_relaxed(), expected_next);
    }

    /// 预热 15ms 后运行 `rounds` 轮，取最快一轮计算 GiB/s（过滤 OS 线程调度毛刺）
    fn bench_gib_s<F: FnMut()>(bytes_per_round: usize, rounds: usize, mut f: F) -> f64 {
        let warmup = Instant::now();
        while warmup.elapsed().as_millis() < 15 {
            f();
        }

        let mut best_sec = f64::MAX;
        for _ in 0..rounds {
            let t0 = Instant::now();
            f();
            let dt = t0.elapsed().as_secs_f64();
            if dt < best_sec {
                best_sec = dt;
            }
        }
        (bytes_per_round as f64 / (1024.0 * 1024.0 * 1024.0)) / best_sec
    }

    /// 针对单个实现了 RngCore 的类型执行 5 维基准测试
    fn run_suite<R: RngCore>(name: &str, l1_buf: &mut [u8], dram_buf: &mut [u8]) {
        let s0 = seed128();
        let mut rng = R::new(s0);

        // 1. L1 Cache 填充吞吐: 32 KiB × 8192 次 = 256 MiB / 轮 (纯 CPU + L1D 速度)
        const L1_ITERS: usize = 8192;
        let l1_bytes = l1_buf.len() * L1_ITERS;
        let l1_gbs = bench_gib_s(l1_bytes, 4, || {
            for _ in 0..L1_ITERS {
                rng.fill_bytes(black_box(l1_buf));
            }
            black_box(l1_buf[l1_buf.len() - 1]);
        });

        // 2. DRAM 大内存填充吞吐: 512 MiB / 轮 (受限于主存写入带宽)
        let dram_bytes = dram_buf.len();
        let dram_gbs = bench_gib_s(dram_bytes, 3, || {
            rng.fill_bytes(black_box(dram_buf));
            black_box(dram_buf[dram_bytes - 1]);
        });

        // 3. 纯寄存器 next_u64 吞吐: 32M 次 × 8B = 256 MiB / 轮
        const U64_ITERS: usize = 32 * 1024 * 1024;
        let u64_gbs = bench_gib_s(U64_ITERS * 8, 4, || {
            let mut acc = 0u64;
            for _ in 0..U64_ITERS {
                acc = acc.wrapping_add(rng.next_u64());
            }
            black_box(acc);
        });

        // 4. 纯寄存器 next_u128 吞吐: 16M 次 × 16B = 256 MiB / 轮 (利好原生 128位 RNG)
        const U128_ITERS: usize = 16 * 1024 * 1024;
        let u128_gbs = bench_gib_s(U128_ITERS * 16, 4, || {
            let mut acc = 0u128;
            for _ in 0..U128_ITERS {
                acc ^= rng.next_u128();
            }
            black_box(acc);
        });

        // 5. 种子初始化 + 首字节延迟 (ns / op)
        const INIT_ITERS: usize = 500_000;
        let t_init = Instant::now();
        let mut acc = 0u64;
        for i in 0..INIT_ITERS {
            let mut tmp = R::new(black_box(s0.wrapping_add(i as u128)));
            acc ^= tmp.next_u64();
        }
        black_box(acc);
        let init_ns = t_init.elapsed().as_nanos() as f64 / INIT_ITERS as f64;

        println!(
            "| {:<22} | {:>9.2} GiB/s | {:>9.2} GiB/s | {:>9.2} GiB/s | {:>9.2} GiB/s | {:>9.1} ns |",
            name, l1_gbs, dram_gbs, u64_gbs, u128_gbs, init_ns
        );
    }

    /// 包装 core.rs 中的全局 RNG 以便放入同一套测试对比
    struct GlobalRngBench;
    impl From<u128> for GlobalRngBench {
        fn from(_: u128) -> Self {
            Self
        }
    }
    impl RngCore for GlobalRngBench {
        #[inline(always)]
        fn next_u64(&mut self) -> u64 {
            core::next()
        }
        #[inline(always)]
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            core::fill_bytes(dest)
        }
    }

    #[test]
    #[ignore]
    fn rng_perf() {
        if cfg!(debug_assertions) {
            return;
        }

        // 一次性预分配并触发物理页映射 (Page Fault)，后续所有 RNG 复用该内存块
        let mut l1_buf = vec![1u8; black_box(32 * 1024)]; // 32 KiB (L1D Cache)
        let mut dram_buf = vec![1u8; black_box(512 * 1024 * 1024)]; // 512 MiB (DRAM)

        println!(
            "\n========================================================================================================="
        );
        println!(
            "| PRNG 算法名称          | L1 填充 (32KiB) | DRAM 填充(512M) | 寄存器 next_u64 | 寄存器next_u128 | 初始化+首取  |"
        );
        println!(
            "|------------------------|-----------------|-----------------|-----------------|-----------------|--------------|"
        );

        #[cfg(feature = "rand-splitmix64")]
        {
            use super::splitmix64::{SplitMix64, SplitMix64Atomic};
            run_suite::<SplitMix64>("SplitMix64", &mut l1_buf, &mut dram_buf);
            run_suite::<SplitMix64Atomic>("SplitMix64Atomic", &mut l1_buf, &mut dram_buf);
        }

        #[cfg(feature = "rand-xoshiro")]
        {
            use super::xoshiro::{
                Xoroshiro128PlusPlus, Xoroshiro128StarStar, Xoshiro256PlusPlus, Xoshiro256StarStar,
            };
            run_suite::<Xoroshiro128PlusPlus>("Xoroshiro128++", &mut l1_buf, &mut dram_buf);
            run_suite::<Xoroshiro128StarStar>("Xoroshiro128**", &mut l1_buf, &mut dram_buf);
            run_suite::<Xoshiro256PlusPlus>("Xoshiro256++", &mut l1_buf, &mut dram_buf);
            run_suite::<Xoshiro256StarStar>("Xoshiro256**", &mut l1_buf, &mut dram_buf);
        }

        #[cfg(feature = "rand-cwg")]
        {
            use super::cwg::{Cwg64, Cwg128, Cwg128x64};
            run_suite::<Cwg64>("Cwg64", &mut l1_buf, &mut dram_buf);
            run_suite::<Cwg128x64>("Cwg128x64", &mut l1_buf, &mut dram_buf);
            run_suite::<Cwg128>("Cwg128", &mut l1_buf, &mut dram_buf);
        }

        #[cfg(feature = "rand-chacha")]
        {
            use super::chacha::{ChaCha8, ChaCha12, ChaCha14, ChaCha20};
            run_suite::<ChaCha8>("ChaCha8", &mut l1_buf, &mut dram_buf);
            run_suite::<ChaCha12>("ChaCha12", &mut l1_buf, &mut dram_buf);
            run_suite::<ChaCha14>("ChaCha14", &mut l1_buf, &mut dram_buf);
            run_suite::<ChaCha20>("ChaCha20", &mut l1_buf, &mut dram_buf);
        }

        run_suite::<GlobalRngBench>("GlobalRng (core::next)", &mut l1_buf, &mut dram_buf);

        println!(
            "=========================================================================================================\n"
        );
    }
}
