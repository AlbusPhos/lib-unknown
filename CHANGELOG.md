# Changelog

本文件记录本项目所有值得关注的变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)。

> **版本号说明（0.x 阶段）**：在 1.0.0 发布之前，次版本号（`0.MINOR.0`）的变更
> 也可能包含破坏性改动，请留意标注为 **[BREAKING]** 的条目。


---

## [Unreleased]

### Changed
- 仓库改为 Cargo Workspace，按职责拆分出 `lib-unknown-polymorphism`、`lib-unknown-seed`、
  `lib-unknown-lazy`、`lib-unknown-errors`、`lib-unknown-hide`、
  `lib-unknown-crypto`、`lib-unknown-rand`、`lib-unknown-types`、`lib-unknown-sys`、
  `lib-unknown-hash`、`lib-unknown-encoding` 与 `lib-unknown-dyntest`；`lib-unknown`
  作为门面保持原有公开模块路径与 Feature 名称。
- 原 `lib-unknown-core` 拆分为 `lib-unknown-errors` 与 `lib-unknown-hide`；`cfg_if!` /
  `cfg_match!` 并入门面 `src/utils/cfg.rs`，保留 `lib_unknown::utils` 模块路径及根宏路径，
  不再发布独立的 `lib-unknown-cfg` crate。`hide` 依赖 `lib-unknown-polymorphism`
  与可选的 `lib-unknown-lazy`。
- 新增 `lib-unknown-polymorphism` 过程宏 crate（仅依赖 `syn` / `quote` / `proc-macro2`），
  提供 `poly` / `poly_attr!` / `poly_branch!` / `poly_select!` / `const_random!`；内部使用
  自实现 Xoshiro256++，支持 `POLY_SEED` 与 `reproducible` feature。随机流按
  `(kind, crate, normalized_context)` 哈希分片计数，并归一化空白与 CRLF 转义，
  局部代码或 `#[cfg]` 变化不会影响无关调用点；各多态 crate 通过 `poly_track_env!`
  植入 `POLY_SEED` 环境依赖，种子变化可触发 Cargo 正确重编译。
- **[BREAKING]** 移除全部 crate 的 `build.rs` 以及 `coin0..coin127` / `dice0..dice9`
  cfg 体系；原手写 `cfg_attr` 多态逻辑统一改为 `poly_attr!`，结构化分支改为
  `poly_branch!` / `poly_select!`。`const_random!` 从 `lib-unknown-hide` 迁入
  `lib-unknown-polymorphism`，并由 hide / 门面重导出。
- 仅控制内联 / `#[cold]` 的 356 处 `poly_attr` 进一步收敛为 `#[poly(...)]`：
  热路径小函数使用高内联概率且 `cold = 0`，函数体越大内联概率越低；初始化、生成与
  动态解析路径使用较低内联概率和 `0.05..0.15` 冷路径概率。含中性 `inline` 或空属性臂、
  无法用 `#[poly]` 等价表达的组合保留为 `poly_attr`。
- `lib-unknown-seed` 收敛为纯运行时熵源，移除构建脚本、`__private` 常量与 `build` feature。
- `OnceLock` / `LazyLock` 独立为 `lib-unknown-lazy`。
- `lib-unknown-hash` 的 `seed` 直接依赖 `lib-unknown-seed`，不再拉入
  `lib-unknown-rand` / `lib-unknown-crypto`；`lib-unknown-dyntest` 精简为仅依赖
  `lib-unknown-errors`；`lib-unknown-encoding` 的 `hb2` 依赖 `lib-unknown-crypto`
  的 `crypto-core`，hex / base64 保持独立。
- `lib-unknown-seed` 移除构建脚本与 feature 门控，`probe` / `seed` / `seed128`
  始终可用；`lib-unknown-rand` 的 `rand-seed` 仅控制是否重导出运行时熵源。
- crypto / rand / hash / sys / encoding 的宏与内部依赖改为按 feature 可选，未启用
  功能时不再产生 unused dependency 警告，也避免无 feature 构建拉入空依赖。


---

## [0.5.0] - 2026-11-17

### Added
- CI：新增 `features` 作业（cargo-hack），逐 feature 执行 clippy / test，并运行
  depth 2 的 feature powerset 检查（跳过聚合 feature）与 `wasm32-unknown-unknown` /
  `wasm32-wasip1` 两个 WASM 目标的逐 feature 检查；发布流程同步引入同款
  feature 组合检查（clippy / test / powerset）。
- 新增 `crypto::core::mix2` 模块（`crypto-core`）：8 / 16 / 32 / 64 / 128 位可逆雪崩置换族
  `mix8_2` / `mix16_2` / `mix32_2` / `mix64_2` / `mix128_2` 及对应逆函数 `inv_mix*_2`，
  全部为 `const fn`；附带全空间双射、短周期环与雪崩统计（SAC 矩阵、汉明重量均值 / 方差）
  测试。
- `hash`：新增 `LightHasher::hash` / `LightHasher::hash_with_seed` 便捷函数，以及实现
  `core::hash::BuildHasher` 的 `LightBuildHasher`（可经 `hash_one` 直接求任意
  `Hash + ?Sized` 值）。
- 新增 `reproducible` feature：启用后构建脚本回退为固定种子，`SEED64` / `SEED128`、
  `coin0..coin127`、`dice0..dice9` 与 `const_random!` 输出在不同构建间保持一致，
  用于可复现构建（默认关闭，不影响逐构建差异化行为）。

### Changed
- **[BREAKING]** 移除 `encoding::hb3` 模块与 `encoding-hb3` feature。
- **[BREAKING]** `hash-base` feature 更名为 `hash-core`（现依赖 `lazy`）。`LightHasher`
  的轮函数与 `finish` 混合显著强化，内部常量改为经 `hide_array_lazy!` 在编译期隐藏；
  新版生成的哈希值与旧版不一致。
- **[BREAKING]** 移除 `hide!` 与 `hide_lazy!` 宏：单个混淆常量在 `black_box` 下仍可能以
  明文形态出现在符号级，统一改用数组版本 `hide_array!` / `hide_array_lazy!`；
  `const_random!` 的构建环境熵源同步精简为 `SCCACHE_CACHE_SIZE` / `RUSTC_WRAPPER` /
  `LIB_SEED`。
- **[BREAKING]** `encoding::hb2` 编码输出不再与旧版逐位一致：编码前对每个字节施加基于
  **逻辑字节索引**的扰动（移位 + 异或白化 + `mix8_2` 双射置换），相同字节在不同位置、
  重复的输入块不再产生相同的符号序列；解码宽松模式按输出索引同步解扰，混入表外脏字节
  仍可正确解码。`encoding-hb2` feature 现传递依赖 `crypto-core`。
- `crypto::core::inv_mul` 全系列函数（`inv_mul8` / `inv_mul16` / `inv_mul32` /
  `inv_mul64` / `inv_mul128`) 现为 `const fn`。
- 文档标注 `crypto::core::mix` 可逆雪崩映射为不再推荐使用（`mix2` 为其后继），模块
  行为未变。

### Fixed
- 修复仅启用 `rand-splitmix64`（不含 `rand-core`）的构建失败：`SplitMix64Atomic` 的
  `next_u128_relaxed` / `fill_bytes_relaxed` 依赖的 `next_u128` / `fill_bytes` 此前仅为
  `RngCore` trait 方法；现于 `SplitMix64` 提供语义一致的固有实现，确定性序列不变。
- 修复 CI 逐 feature 检查暴露的 `build.rs` 与 `mix2` 测试中的
  `needless_range_loop` 告警（改用迭代器遍历，行为不变）。

---

## [0.4.2] - 2026-11-14

### Added
- `rand::core::Rng` 新增默认方法 `random_bytes` / `random_vec` / `random_bool` / `choose` /
  `choose_mut`，并在 `rand::core` 提供同名的进程级全局便捷函数（`random_vec` 需启用
  `alloc` feature）。
- 新增 `Random for char`：在全部 Unicode 标量值上均匀取值，自动跳过
  `0xD800..=0xDFFF` 代理区。
- 新增 `Random for [T; N]`：对定长数组逐元素独立采样。
- `rand::core::GlobalRng` 由内部类型转为公开，并派生 `Clone` / `Copy` / `Default` /
  `Debug`，可作为零大小句柄传给任意 `Rng` 泛型接口。

### Changed
- `rand::core` 整数区间采样改用 Lemire 乘法取高位（nearly divisionless）算法：
  `u8`~`u64` / `usize` 及对应有符号类型的快路径不再执行除法；`u128` / `i128`
  改为惰性阈值的取模拒绝路径。区间分布仍保持无偏。
  注意：拒绝重试的时机随之改变，旧版本产生的确定性序列不再可复现。
- `f32` / `f64` 的 `Random` 实现由除法改为乘以倒数常量，输出与旧版逐位一致。
- 浮点 `Range` / `RangeInclusive` 采样改用 `(1 - r) * start + r * end` 插值，
  避免 `end - start` 溢出（如 `-MAX..MAX`）；`Range` 额外保证不返回右开边界。
- 内部重构：`DYB` 的 `apply_p8` / `apply_inv_p8` 改用迭代器遍历，消除 clippy
  `needless_range_loop` 告警，行为不变。

### Fixed
- 修复 `rand` 线程级 `Cwg128x64` 的 `next_u128` 不推进计数器，导致仅使用 128 位取数的
  场景永不触发周期重播种的问题。
- `rand::core::fill_bytes` 在未启用 `std` 的全局源路径下对空切片直接返回，
  不再为空缓冲区消耗一次随机数。

---

## [0.4.1] - 2026-11-13

### Added
- `Buffer` trait 新增带默认实现的 `new_zeroed(len)`，用于构造“长度为 `len`、内容全零”的容器；
  `StackBuffer` / `HeapBuffer` 提供同名固有实现，直接复用初始化时的全零存储、仅在容量校验
  通过后设置长度（`HeapBuffer` 在 `len > N` 时于堆分配前返回错误）。

### Fixed
- 修复 `rand` 全局随机源在未启用 `rand-seed` 的构建中，`const_random!()` 默认 `u64` 与
  `u128` 种子异或导致的类型不匹配问题（显式改用 `const_random!(u128)`）。
- 修复 `Buffer::wipe_data` 只擦除 `data[..len]`、与其“清零全部底层存储”文档及 `Drop`
  行为不一致的问题；现在显式擦除会覆盖完整容量 `data[..N]`，未使用区域不再可能残留旧数据。
  代价是该操作由 O(len) 变为 O(N)，与对象销毁时的擦除成本一致。
- 修复 macOS 平台支持：补齐 XNU 常量表的 `ACCEPT` / `LSEEK` 调用号，修正
  `OPENAT` / `RENAMEAT` / `READLINKAT` 为正确的 BSD 系统调用号；补齐 `ptrace_req`
  的 `PT_READ_U` / `PT_WRITE_U` / `PT_FORCEQUOTA` / `PT_FIRSTMACH`；将 macOS x86_64
  内联汇编局部标签由 `1`/`0` 改为 `2`；`sys_ptrace` 文档示例按平台选择
  `PT_TRACE_ME` / `PTRACE_TRACEME`，修复 macOS doctest 编译失败；新增 macOS 原生
  syscall smoke test（`getpid` / `gettid` / `open` + `close`）。

### Removed
- **[BREAKING]** 移除 macOS 的 `constants::GETCWD`：macOS 没有 `getcwd` 系统调用
  （libc 通过 `fcntl(F_GETPATH)` 实现），原值 `277` 实际指向 `open_extended`。

---

## [0.4.0] - 2026-11-12

### Added
- 新增 `encoding::hb2`（`encoding-hb2`）与 `encoding::hb3`（`encoding-hb3`）：面向空白字符
  隐写的二值 / 三进制符号编码。提供 `const fn` 定容缓冲接口、宽松模式解码（自动跳过表外
  脏字节）、默认符号表以及 `alloc` 下的 `Vec` 封装。
- 新增 `crypto::core::inv_mul` 模块：`inv_mul8` / `inv_mul16` / `inv_mul32` / `inv_mul64` /
  `inv_mul128` 统一采用 Newton–Raphson 精度倍增实现；MSE2 复用同一实现。
- `Buffer` trait 及其 `StackBytes` / `HeapBytes` 实现新增 `remaining_capacity`、`truncate`、
  `pop`、`push`、`resize`、`resize_zeroed`、`extend_zeroed`，被丢弃或临时副本均 `volatile` 清零。
- `Str` / `CStr` 容器新增全套固有方法，无需导入 trait 即可调用：容量查询、`as_buffer`、
  `ct_eq` / `ct_eq_cstr` / `ct_eq_dyn` 常量时间比较、`wipe_data` / `clear`、原地
  `push_str` / `push_bytes` / `push_cstr` / `extend_and_wipe`、`try_push` / `try_push_and_wipe`、
  `pop`、按 UTF-8 边界安全回退的 `truncate`。
- `StackCStr` / `HeapCStr` 新增与 `StackBytes` / `HeapBytes` 的零拷贝互转
  （`from_inner_with_nul` / `from_inner_payload` / `from_buffer_payload` / `into_bytes` /
  `to_*_bytes`）、`HeapCStr::into_stack`，以及 `TryFrom<StackBytes<M>>` / `TryFrom<HeapBytes<M>>`
  引用与按值转换（`&mut` 版本自动擦除源容器）。
- `StackStr` / `HeapStr` 新增 `impl core::fmt::Write`，可通过 `write!` 原地格式化，
  避免 `format!` 在堆上残留明文。
- `rand`：`std` + `rand-cwg` 下全局随机源改为每线程 `Cwg128x64`（一次 128 位步进拆分为
  两次 `u64`），并新增低频重播种；非 `std` 环境继续使用原子 `SplitMix64`。
- `SplitMix64Atomic` 新增 `next_u128_relaxed` 与 `fill_bytes_relaxed`（单次原子预留 + 本地批量填充）。
- 新增 RNG 契约测试（确定性重放、全零种子抗退化、`fill_bytes` 边界）、`SplitMix64Atomic`
  并发测试，以及默认忽略的五维 `rng_perf` 基准。
- `rand-default` 现在包含 `rand-cwg`；`encoding` 现在包含 `encoding-hb2` / `encoding-hb3`。

### Changed
- **[BREAKING]** `crypto::core::inv_mul8` 迁移至 `crypto::core::inv_mul::inv_mul8`，并扩展为
  8 / 16 / 32 / 64 / 128 位五种宽度；`mse` / `mse2` 的调用点已同步。
- **[BREAKING]** `SplitMix64::next` 不再是 `const fn`（改用共享的 `mix64` 实现），
  依赖 const 求值的下游代码需调整。
- **[BREAKING]** `Buffer` trait 新增必需方法 `truncate` / `pop` / `resize`，`Str` trait 新增
  必需方法 `capacity` / `ct_eq` / `wipe_data`；外部实现者需补齐实现。
- `StackCStr` / `HeapCStr` 内部改为封装 `StackBytes<N>` / `HeapBytes<N>`，清零职责统一委托
  底层容器；公开方法签名保持不变。
- `CStr` trait 的 `len` / `is_empty` 改为直接读取载荷字节；`CStr::ct_eq*` 系列新增。
- `try_grow` / `push_bytes_into` / `push_str_into` / `into_heap` 改为委托底层
  `Buffer::try_grow` / `extend_into`，移除内部 `expect("Capacity already checked")` 路径，
  消除相应的潜在 panic。
- `StackCStr::push_bytes_into` 的 `InteriorNul` 错误下标改为拼接后的绝对下标。
- 全局 `next()` 移除了 `AtomicUsize` 步进计数器与固定周期重播种，改为概率触发重播种。
- `GlobalRng::from` 不再 `unreachable!()`，`reseed_from` 改为空操作，使零大小全局句柄可正常使用。
- 清理 `rand` 各 PRNG 中已失效的 `coin*` 冷热/内联 cfg 属性；`sys::unix` 改用直接导入的
  `size_of` 并移除占位测试。

### Removed
- **[BREAKING]** 移除旧路径 `crypto::core::inv_mul8`（改用 `crypto::core::inv_mul::inv_mul8`）。
- **[BREAKING]** 移除 `SplitMix64::next` 的 `const` 能力。
- **[BREAKING]** 移除 `StackCStr` / `HeapCStr` 旧的自有 `len` + `data` 字段表示（字段原本私有）。
- 移除 `mse2` 内部重复的私有 `inv_mul64` 实现。

### Fixed
- 修复 `types::cstr` 模块文档中 `StackBytes` / `HeapBytes` / `StackCStr` / `HeapCStr`
  的坏 intra-doc 链接，`cargo doc -D warnings` 恢复通过。
- 修复 `crypto::core` 中已迁移函数的悬空文档注释错误挂到 `Key` trait 的问题。
- 修复 clippy `empty_line_after_doc_comments`、`redundant_slicing`、`needless_borrow` 告警。
- `TryFrom<String>`（`Str` / `CStr`）现在会易失清零原 `String` 的堆缓冲，避免明文残留。
- `StackStr` / `HeapStr` 的 `push_str_into` 增加溢出检查，拼接长度不再可能回绕。

### Security
- `types` 新增 `ct_eq` / `ct_eq_dyn` / `ct_eq_cstr` 常量时间比较，覆盖 `CStr` 与 `Str` 容器，
  降低口令、路径等敏感载荷的时序侧信道风险。
- 原地容器操作（`truncate` / `pop` / `push` / `resize`）对被丢弃字节与入参副本执行
  `volatile` 清零；容器转换路径（含 `TryFrom<String>`）同步擦除源数据。

---

## [0.3.2] - 2026-11-09

### Added
- `Cargo.toml` 补充 `authors`、`repository`、`homepage`、`documentation` 元数据。

### Changed
- `wasi` 依赖由 `0.11.1` 升级至 `0.14`，`rand-seed` 的单调时钟读取迁移到 WASI 0.2
  （preview2）API `wasi::clocks::monotonic_clock::now()`；README 的 WASM 支持说明同步更新。
- `description` 元数据移除反引号；`homepage` 指向维护者主页。
- `rand::Seed128` 文档明确整数种子扩展规则（无符号零扩展、有符号符号扩展）。

### Deprecated
-

### Removed
-

### Fixed
- 修复 `wasm32-wasip1` + `rand-seed` 构建失败（`wasi 0.14` 已移除
  `clock_time_get` / `CLOCKID_MONOTONIC`）。
- 修复 `encoding` 在 `no_std + alloc` 下缺少 `vec!` 宏导入导致的构建失败。
- 修复 rustdoc 文档构建告警：统一 `errors` / `types` / `sys::unix::syscall`
  的 intra-doc 链接在 stable 与 nightly 下的解析方式，并避免关闭 `alloc` 时
  `String` 链接失效。

### Security
-

---

## [0.3.1] - 2026-11-09

### Added
- 初始化变更日志基线，对应当前 `Cargo.toml` 中的版本 `0.3.1`。
- 核心定位：默认 `no_std`、非 WASM 目标零第三方依赖；支持按需启用 `std` / `alloc`；
  使用 Rust 2024 edition，MSRV 为 `1.92`，许可证为 MPL-2.0。
- 细粒度 Cargo features：`rand`、`sys`、`types`、`crypto`、`hash`、`lazy`、`encoding`
  及各自子特性，另有仅开发期使用的 `dyntest`。
- 构建期随机化：`build.rs` 采样运行时熵生成 `SEED` / `SEED64` / `SEED128` 环境常量，
  并派生 `coin0..coin127` 与 `dice0..dice9` 编译期 cfg，用于轮转等价实现、内联策略、
  冷热路径、常量掩码与算法形态。注意 Cargo 会缓存构建脚本输出，源码未变时不会重新采样。
- `rand`：微架构熵源 `probe()`、运行时种子派生 `seed()` / `seed128()`；
  `Seed128`、`RngCore`、`Rng`、`Random`、`SampleRange` trait；全局便捷接口
  `next()` / `random()` / `fill_bytes()` / `random_range()` / `shuffle()`；
  以及 `SplitMix64` / `SplitMix64Atomic`、`Cwg64` / `Cwg128x64` / `Cwg128`、
  `ChaCha8` / `ChaCha12` / `ChaCha14` / `ChaCha20`、
  `Xoshiro256PlusPlus` / `Xoshiro256StarStar` /
  `Xoroshiro128PlusPlus` / `Xoroshiro128StarStar`。
- `crypto`：核心置换与混合原语（`shuffle_with` / `unshuffle_with`、`Key` trait、
  `DYB` S/P 盒、`mix*`、`rp*`、浮点代数重构的 `f_rotate_*` / `f_xor_*`）；
  常数时间 PKCS#7 与 ISO/IEC 7816-4 填充/去填充；MSE 字节级混淆
  （`mse` / `imse` 及无 S/P 盒变体）；MSE2 64 字节分组混淆
  （`mse2` / `imse2`）；RFC 8439 兼容 ChaCha20
  （`chacha20` / `chacha20_0` / `chacha20_1`）。
- `types`：`Buffer` / `Str` / `CStr` 安全容器抽象；
  `StackBuffer` / `HeapBuffer` 与 `StackBytes`、`StackU16`、`StackU32`、`StackU64` 等别名；
  `StackStr` / `HeapStr`、`StackCStr` / `HeapCStr`；
  提供 `ct_eq`、按小端/大端跨位宽追加、`try_grow`、`push_*_into` / `concat_into`、
  `into_heap`；容器在 `Drop` 与 `wipe_data` 时对整块存储执行 `volatile` 清零。
- `sys`：裸系统调用 `syscall0`–`syscall6`，覆盖 Linux / Android 的 x86_64、x86、
  AArch64、RISC-V64，macOS 的 x86_64、AArch64，以及 Windows 的 x86、x86_64、AArch64；
  提供各平台系统调用号与常用 flags 常量；`sys::unix::syscall` 高层封装
  （进程、文件、内存、socket、ptrace 等）；Unix `dlopen` / `dlsym` 动态符号解析；
  Windows `resolve()` 通过 `GetModuleHandleA` / `LoadLibraryA` / `GetProcAddress`
  动态解析 DLL 导出。
- `hide`：编译期数据混淆宏 `const_random!`、`hide!`、`hide_array!`，
  以及依赖 `lazy` 的 `hide_lazy!`、`hide_array_lazy!`。
- `lazy`：手写无 std `OnceLock` / `LazyLock`，支持初始化失败后重试与 panic 中毒恢复。
- `hash`：32 字节分块、多泳道乘法链的 `LightHasher`，以及无 std 定容开放寻址
  `SHashMap`（容量须为非零 2 的幂）。
- `encoding`：hex 编解码；标准与 URL-safe Base64 编解码；
  均提供 `const fn` 定容缓冲接口及 `alloc` 下的 `Vec` 封装。
- `errors`：统一 `Error` / `ErrType` / `Result` / `ErrorMessage`，
  以及 `err!` / `bail!` / `ensure!` 宏。
- `dyntest`：开发期动态编译运行 harness，包含 `Runner`、`Task`、`BatchRunner`、
  `RunnerResult`，支持超时、批量执行与缓存；需要同时启用 `std`。
- 平台与目标支持：完整测试覆盖 `x86_64-unknown-linux-gnu`；多个 Linux / Android /
  Windows 目标通过 `cargo check --all-features`；WASM 目标在非 `sys` 模块下可按需使用
  `wasm-bindgen` 或 WASI preview1 绑定。

---

[Unreleased]: https://github.com/AlbusPhos/lib-unknown/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/AlbusPhos/lib-unknown/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/AlbusPhos/lib-unknown/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/AlbusPhos/lib-unknown/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/AlbusPhos/lib-unknown/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/AlbusPhos/lib-unknown/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/AlbusPhos/lib-unknown/releases/tag/v0.3.1
