# lib-unknown

> **一个支持 `no_std`、默认零第三方依赖的库，用于实现编译期随机化的系统调用、随机数生成、密码学与编码。**

[![Type](https://img.shields.io/badge/Project%20Type-Library-green.svg)](#快速开始-quick-start)
[![License](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)
[![Documentation](https://docs.rs/lib-unknown/badge.svg)](https://docs.rs/lib-unknown)
[![MSRV](https://img.shields.io/badge/MSRV-1.92-blue.svg)](#平台与环境支持-platform-support)

---

`lib-unknown` 是一个纯 Rust 实现的底层工具库，不依赖标准库与 libc，可完整在 `no_std`
环境下工作。它把裸系统调用、多源熵随机数、ChaCha20 / MSE 混淆原语、擦除式内存容器、
编译期数据混淆与轻量编码集中在同一套细粒度 Feature 之下。库的默认构建不引入任何
`crates.io` 依赖，面向对二进制体积、运行环境与静态特征敏感的场景。所有敏感容器在销毁时
都会对整块存储执行 `volatile` 清零。

## 目录

- [快速开始](#快速开始-quick-start)
- [设计哲学](#设计哲学-design-philosophy)
- [API 预览](#api-预览-api-overview)
- [适用场景](#适用场景-when-to-use)
- [特性标志](#特性标志-feature-flags)
- [平台与环境支持](#平台与环境支持-platform-support)
- [安全性](#安全性-security)
- [贡献](#贡献-contributing)
- [变更日志](#变更日志-changelog)
- [开源协议](#开源协议-license)

---

## 快速开始 (Quick Start)

### 作为库使用 (Library)

添加依赖：

```bash
cargo add lib-unknown
```

或在 `Cargo.toml` 中手动添加：

```toml
[dependencies]
lib-unknown = "0.3"
```

基本用法（需启用 `"types"` Feature）：

```rust
#[cfg(feature = "types")]
fn main() {
    use lib_unknown::types::buffer::StackBytes;

    let mut b = StackBytes::<32>::new();
    b.extend_from_slice(b"sensitive payload").unwrap();
    assert!(b.ct_eq(b"sensitive payload"));

    // 作用域结束或显式清零：整块存储 volatile 清零
    b.wipe_data();
    assert!(b.is_empty());
}

#[cfg(not(feature = "types"))]
fn main() {}
```

更多场景：

#### 编译期数据混淆

```rust
fn main() {
    let hidden = lib_unknown::hide_array!(
    u64, [
        0x1122_3344_5566_7788,
        0x9900_aabb_ccdd_eeff,
    ]
);
    assert_eq!(hidden[0], 0x1122_3344_5566_7788);
}
```

#### ChaCha20（RFC 8439 兼容，需 `"crypto"` Feature）

```rust
#[cfg(feature = "crypto")]
fn main() {
    use lib_unknown::crypto::chacha20::chacha20_0;

    let mut buf = *b"hello world";
    let key: &[u8; 32] = &[0u8; 32];
    let nonce: &[u8; 12] = &[0u8; 12];
    chacha20_0(&mut buf, key, nonce).unwrap();
}

#[cfg(not(feature = "crypto"))]
fn main() {}
```

#### 随机数（需 `"rand"` Feature）

```rust
#[cfg(feature = "rand")]
fn main() {
    let x: u64 = lib_unknown::rand::core::random();
    let y: f64 = lib_unknown::rand::core::random_range(0.0..1.0);
    let _ = (x, y);
}

#[cfg(not(feature = "rand"))]
fn main() {}
```

#### 裸系统调用（以 Linux 为例，需 `"sys"` + `"sys-unix"`）

```rust
#[cfg(all(feature = "sys", feature = "sys-unix", target_os = "linux"))]
fn main() {
    use lib_unknown::sys::unix::syscall::sys_getpid;

    let pid = sys_getpid();
    assert!(pid > 0);
}

#[cfg(not(all(feature = "sys", feature = "sys-unix", target_os = "linux")))]
fn main() {}
```

---

## 设计哲学 (Design Philosophy)

### 核心原则

1. **常规目标零第三方依赖，体积最小化。** 非 WASM 目标上的 `no_std` 构建不引入任何
   `crates.io` 依赖；`wasm32-unknown-unknown` 仅在 `rand-seed` 下可选 `wasm-bindgen`，
   `wasm32-wasip1` 通过 `wasi` 绑定读取单调时钟。
2. **编译期多态，逐次构建差异化。** 构建脚本根据运行时熵生成种子，逐位派生
   `coin0..coin127`，并依据 `dice1` / `dice2` / `dice6` / `dice8` 在等价实现间轮转选择，
   改变内联策略、冷热路径、常量掩码乃至算法形态。注意：Cargo 会缓存构建脚本输出，
   源码未变时不会重新采样。
3. **抗符号与特征分析的数据处理。** 常量通过 XOR 掩码在编译期加密、运行时经
   `black_box` 还原；部分位运算提供基于 IEEE-754 浮点代数的等价实现，避免
   `rol` / `xor` 等典型指令模式直出。
4. **安全的内存生命周期。** 敏感容器在 `Drop` 与 `wipe_data` 时对整块存储执行
   `volatile` 清零，跨类型搬运、未对齐拷贝与 `TryFrom` 转换均会清零源数据。

### 权衡取舍 (Trade-offs)

| 我们选择了           | 而不是         | 原因                                                                 |
|:---------------------|:---------------|:---------------------------------------------------------------------|
| 零默认依赖与自研实现 | 复用生态 crate | 减少特征、降低供应链面、便于审计，并保证无 libc 的 `no_std` 构建     |
| 构建期随机化         | 可复现构建     | 换取函数布局、常量与实现形态的逐构建差异，用于干扰静态分析与对抗tlsh |
| `no_std` 优先        | 默认启用 `std` | 适配嵌入式、受控客户端与无 libc 环境；`std` / `alloc` 需显式开启     |

---

## API 预览 (API Overview)

完整 API 文档请参阅 [docs.rs/lib-unknown](https://docs.rs/lib-unknown)。

### 核心组件速查

| 核心入口 / 类型 | 角色定位       | 职责说明                                                                       |
|:----------------|:---------------|:-------------------------------------------------------------------------------|
| **`sys`**       | 系统接口       | 裸系统调用（多架构内联汇编）与 `dlopen` / `dlsym`、Windows DLL 动态符号解析    |
| **`rand`**      | 随机源         | 多源熵混合与 SplitMix64 / Xoshiro / CWG / ChaCha 四族 PRNG                     |
| **`crypto`**    | 密码与混淆     | RFC 8439 ChaCha20、MSE / MSE2 混淆密码、常数时间 PKCS#7 / ISO-7816 填充        |
| **`types`**     | 安全内存       | `StackBytes` / `StackStr` / `StackCStr` 等擦除式定容容器，`ct_eq` 与跨位宽视图 |
| **`hide`**      | 编译期混淆     | `const_random!` / `hide!` / `hide_array!` 及惰性变体                           |
| **`lazy`**      | 惰性初始化     | 手写 `OnceLock` / `LazyLock`，支持失败重试与 panic 中毒恢复                    |
| **`hash`**      | 轻量哈希       | 多泳道乘法链 `LightHasher` 与无 std 定容 `SHashMap`                            |
| **`encoding`**  | 编码           | hex / base64 编解码辅助                                                        |
| **`errors`**    | 统一错误       | `Error` / `ErrType` 与 `ensure!` / `bail!` / `err!` 宏                         |
| **`dyntest`**   | 开发期 Harness | 将代码片段生成临时 Cargo 工程并编译执行，支持超时与缓存                        |

---

## 适用场景 (When to Use)

**适合：**

- **安全研究与授权渗透测试**：需要规避静态特征识别、支持动态 API 解析的对抗性评估框架；
- **受控客户端 / 载荷开发**：希望在无 libc 环境下运行、且对二进制体积与特征敏感的可执行组件；
- **场景受限的嵌入式 / 无标准库环境**：需要轻量随机数、编码与缓冲容器的普通 `no_std` 开发。

---

## 特性标志 (Feature Flags)

本项目支持以下 [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)：

| Feature           | 默认启用 | 依赖                                                          | 说明                                                               |
|:------------------|:--------:|:--------------------------------------------------------------|:-------------------------------------------------------------------|
| `default`         |    ✅    | `rand`, `sys`, `types`, `crypto`, `hash`, `lazy`, `encoding`  | 默认启用的标准特性集合                                             |
| `std`             |    ❌    | `alloc`                                                       | 启用标准库；关闭 `no_std`                                          |
| `alloc`           |    ❌    | —                                                             | 启用堆分配支持（`Box` / `Vec` 等）                                 |
| `rand`            |    ✅    | `rand-default`, `rand-chacha`, `rand-xoshiro`, `rand-cwg`     | 全部随机数能力                                                     |
| `rand-default`    |    ✅    | `rand-seed`, `rand-core`                                      | `rand` 的默认源组合（熵源 + 核心 trait）                           |
| `rand-seed`       |    ✅    | `dep:wasm-bindgen`, `dep:wasi`                                | 微架构熵源（TSC / 寄存器签名 / 栈混合）；WASM 下按目标引入对应绑定 |
| `rand-core`       |    ✅    | `rand-splitmix64`, `crypto-core`                              | `RngCore` / `Rng` 等核心 trait 与全局便捷接口                      |
| `rand-splitmix64` |    ✅    | `crypto-core`                                                 | SplitMix64 生成器（含原子共享变体）                                |
| `rand-cwg`        |    ✅    | `crypto-core`                                                 | CWG 64 / 128 位生成器                                              |
| `rand-chacha`     |    ✅    | `crypto-core`                                                 | ChaCha 8 / 12 / 14 / 20 轮生成器                                   |
| `rand-xoshiro`    |    ✅    | `crypto-core`                                                 | xoshiro256 / xoroshiro128 四种变体                                 |
| `sys`             |    ✅    | `sys-win`, `sys-unix`, `sys-syscall`                          | 全部系统接口                                                       |
| `sys-syscall`     |    ✅    | —                                                             | 裸 `syscall0`–`syscall6` 封装                                      |
| `sys-win`         |    ✅    | —                                                             | Windows DLL 动态符号解析（仅 Windows 目标编译）                    |
| `sys-unix`        |    ✅    | —                                                             | Unix 调用号、flags 与 `dlopen` / `dlsym` 解析（仅 Unix 目标编译）  |
| `types`           |    ✅    | `types-str`, `types-buffer`, `types-cstr`                     | 全部擦除式安全容器                                                 |
| `types-buffer`    |    ✅    | —                                                             | 定容字节 / 宽位宽容器（`StackBytes` / `HeapBytes` 等）             |
| `types-str`       |    ✅    | `types-buffer`                                                | UTF-8 安全字符串容器                                               |
| `types-cstr`      |    ✅    | `types-buffer`                                                | NUL 结尾 C 字符串容器                                              |
| `crypto`          |    ✅    | `crypto-core`, `crypto-mse`, `crypto-chacha20`, `crypto-mse2` | 全部密码与混淆能力                                                 |
| `crypto-core`     |    ✅    | `lazy`                                                        | S/P 盒、填充、置换与混合原语                                       |
| `crypto-mse`      |    ✅    | `crypto-core`                                                 | MSE 第一代字节级混淆                                               |
| `crypto-mse2`     |    ✅    | `crypto-core`                                                 | MSE2 64 字节分组混淆                                               |
| `crypto-chacha20` |    ✅    | `crypto-core`                                                 | RFC 8439 ChaCha20                                                  |
| `hash`            |    ✅    | `hash-base`, `hash-hashmap`                                   | 轻量哈希与定容哈希表                                               |
| `hash-base`       |    ✅    | —                                                             | `LightHasher` 多泳道乘法链哈希                                     |
| `hash-hashmap`    |    ✅    | `hash-base`                                                   | 无 std 定容 `SHashMap`                                             |
| `lazy`            |    ✅    | —                                                             | 手写 `OnceLock` / `LazyLock`                                       |
| `encoding`        |    ✅    | `encoding-hex`, `encoding-base64`                             | hex / base64 编解码                                                |
| `encoding-hex`    |    ✅    | —                                                             | 十六进制编解码                                                     |
| `encoding-base64` |    ✅    | —                                                             | Base64 编解码（含 URL-safe 字母表）                                |
| `dyntest`         |    ❌    | `std`, `rand`                                                 | 动态编译运行 Harness（仅开发期）                                   |

最小化构建示例：

```bash
cargo build --release --no-default-features \
  --features rand,types
```

---

## 平台与环境支持 (Platform Support)

- **MSRV**：`1.92`（`Cargo.toml` 的 `rust-version`；提升 MSRV 视为次版本号变更）。
- **支持操作系统**：
    - Linux / Android：`x86_64`、`x86`、`AArch64`、`RISC-V64`；
    - macOS：`x86_64`、`AArch64`；
    - Windows：`x86`、`x86_64`、`AArch64`（裸 syscall 与 DLL 解析）。
    - 验证级别：仅对 `x86_64-unknown-linux-gnu` 执行完整测试；其余目标的验证状态见下方矩阵。
- **`no_std` 支持**：是。默认不启用 `std`（`#![no_std]`）；开启 `std` 会传递启用 `alloc`，
  也可单独开启 `alloc`。`types` 的 `HeapBuffer` / `HeapStr` / `HeapCStr`、`dyntest` 与
  `std` 特性需要堆/标准库支持；`no_std` 环境不链接 libc。
- **WebAssembly**：
    - `wasm32-unknown-unknown`：支持非 `sys` 模块；需使用 `--no-default-features`，
      启用 `rand` 时通过 `wasm-bindgen` 获取浏览器 `performance.now()` 熵源。
    - `wasm32-wasip1`：支持非 `sys` 模块；需使用 `--no-default-features`，
      `rand-seed` 通过 `wasi 0.11`（WASI preview1）读取单调时钟。
    - `wasm32-wasip2`：非 `sys` 模块可编译；当前 WASI 绑定面向 preview1，
      Component Model 运行期兼容性尚未验证。
- **系统与外部依赖**：非 WASM 目标为纯 Rust 实现，默认零第三方依赖、不链接 libc，
  无需 C 工具链或 `pkg-config`；`sys` 的裸系统调用依赖目标架构内联汇编。
  WASM 目标按需引入 `wasm-bindgen`（非 WASI）或 `wasi`（WASI preview1）。

<details>
<summary><b>目标三元组支持矩阵 (Target Support Matrix)</b></summary>

<br>

| 目标三元组 (Target Triple)    | 架构     | 等级 (Tier) | 备注                                      |
|:------------------------------|:---------|:-----------:|:------------------------------------------|
| `x86_64-unknown-linux-gnu`    | x86_64   | **Tier 1**  | `cargo test --all-features` 通过          |
| `x86_64-unknown-linux-musl`   | x86_64   | **Tier 1**  | `cargo check --all-features` 通过         |
| `i686-unknown-linux-gnu`      | x86      | **Tier 2**  | `cargo check --all-features` 通过         |
| `aarch64-unknown-linux-gnu`   | ARM64    | **Tier 2**  | `cargo check --all-features` 通过         |
| `riscv64gc-unknown-linux-gnu` | RISC-V64 | **Tier 2**  | `cargo check --all-features` 通过         |
| `x86_64-linux-android`        | x86_64   | **Tier 2**  | `cargo check --all-features` 通过         |
| `aarch64-linux-android`       | ARM64    | **Tier 2**  | `cargo check --all-features` 通过         |
| `i686-linux-android`          | x86      | **Tier 2**  | `cargo check --all-features` 通过         |
| `armv7-linux-androideabi`     | ARM      | **Tier 3**  | `sys` 不支持该架构；非 `sys` 模块检查通过 |
| `x86_64-pc-windows-gnu`       | x86_64   | **Tier 2**  | `cargo check --all-features` 通过         |
| `x86_64-pc-windows-gnullvm`   | x86_64   | **Tier 2**  | `cargo check --all-features` 通过         |
| `x86_64-apple-darwin`         | x86_64   | **Tier 3**  |                                           |
| `aarch64-apple-darwin`        | ARM64    | **Tier 3**  |                                           |
| `x86_64-pc-windows-msvc`      | x86_64   | **Tier 3**  |                                           |
| `aarch64-pc-windows-msvc`     | ARM64    | **Tier 3**  |                                           |
| `i686-pc-windows-msvc`        | x86      | **Tier 3**  |                                           |
| `x86_64-unknown-none`         | x86_64   | **Tier 3**  |                                           |
| `wasm32-unknown-unknown`      | Wasm     | **Tier 2**  |                                           |
| `wasm32-wasip1`               | Wasm     | **Tier 2**  |                                           |
| `wasm32-wasip2`               | Wasm     | **Tier 3**  |                                           |

**等级定义说明**：

- **Tier 1**：核心支持目标。本机完成完整测试或编译验证；发布前应纳入 CI 阻断。
- **Tier 2**：次级支持目标。已完成交叉编译 `cargo check`，不承诺覆盖全部运行期用例。
- **Tier 3**：兼容/未验证目标。平台代码路径存在，但本机未验证，不纳入发布阻断。

</details>

---

## 安全性 (Security)

- **Unsafe**：全库在 syscall 内联汇编、擦除式容器、惰性初始化（`lazy`）、
  哈希未对齐读取、编码指针运算、MSE 混淆原语及 `hide!` 展开等受控边界使用 `unsafe`；
  绝大多数 `unsafe` 块附 `SAFETY` 前置条件注释。
- **敏感数据擦除**：`types` 模块容器在 `Drop` 与显式 `wipe_data` 时对整块存储执行
  `volatile` 清零，并覆盖未使用区域（详见[设计哲学](#设计哲学-design-philosophy)）。
- **漏洞披露**：提交 Issue 前请勿公开可利用细节，优先通过维护者私信协商披露流程。

---

## 贡献 (Contributing)

欢迎提交 Issue 报告问题或发起 Pull Request 贡献代码！

- 提交新功能 PR 前，请先阅读本项目的[设计哲学](#设计哲学-design-philosophy)；
  与核心定位冲突的功能特性可能不会被合入，建议先通过 Issue 讨论。
- 提交 PR 前请确保在本地通过以下检查：

  ```bash
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-features
  ```

- 改动涉及 `feature` 或 `pub` API 时，请同步更新 README 与 CHANGELOG。

---

## 变更日志 (Changelog)

本项目遵循 [语义化版本 2.0.0 (SemVer)](https://semver.org/lang/zh-CN/) 规范。
各版本的详细变更记录请查阅 [CHANGELOG.md](CHANGELOG.md)。

---

## 开源协议 (License)

本项目基于 [MPL-2.0](./LICENSE) 协议开源。除非另有明确声明，您提交的任何贡献默认按同等协议条款授权。

