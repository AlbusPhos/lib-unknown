# Changelog

本文件记录本项目所有值得关注的变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)。

> **版本号说明（0.x 阶段）**：在 1.0.0 发布之前，次版本号（`0.MINOR.0`）的变更
> 也可能包含破坏性改动，请留意标注为 **[BREAKING]** 的条目。

---

## [Unreleased]

### Added
-

### Changed
-

### Deprecated
-

### Removed
-

### Fixed
-

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
- `dyntest`：开发期动态编译运行 harness，包含 `DnyRun`、`DnyTask`、`BatchRunner`、
  `DnyResult`，支持超时、批量执行与缓存；需要同时启用 `std`。
- 平台与目标支持：完整测试覆盖 `x86_64-unknown-linux-gnu`；多个 Linux / Android /
  Windows 目标通过 `cargo check --all-features`；WASM 目标在非 `sys` 模块下可按需使用
  `wasm-bindgen` 或 WASI preview1 绑定。

---

[Unreleased]: https://github.com/AlbusPhos/lib-unknown/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/AlbusPhos/lib-unknown/releases/tag/v0.3.1
