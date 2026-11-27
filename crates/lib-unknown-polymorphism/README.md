# lib-unknown-polymorphism

编译期多态与可复现随机的过程宏 crate。

```rust
use lib_unknown_polymorphism::{poly, poly_attr, poly_branch, poly_select, const_random, poly_track_env};

#[poly]                       // 50% inline(always) / 50% inline(never)
#[poly(cold = 0.25)]          // 另加 25% 概率 #[cold]
#[poly_attr(
    weight(2) => [inline(always)],
    weight(1) => [cold, inline(never)],
    weight(1) => [inline],
)]
fn example() {}

poly_track_env!();

let x = poly_branch! {
    weight(2) => 1u32,
    weight(1) => 2u32,
};
let mask: u8 = const_random!(u8);
```

- `#[poly]` 快捷控制内联/冷路径：默认 `inline = 0.5`、`cold = 0.0`；支持
  `#[poly]`、`#[poly(cold = 0.25)]`、`#[poly(0.8, 0.1)]`。
- 随机源：内部 Xoshiro256++，无第三方 RNG 依赖。
- 可复现：`POLY_SEED=12345 cargo build`，或启用 `reproducible` feature；`const_random!` / `poly_branch!` / `poly_select!` 自带环境追踪，其他 crate 可在模块级调用 `poly_track_env!()` 获得同样的重编译行为。
- 随机流按 `(kind, crate, normalized_context)` 哈希分片计数，局部增删或 `#[cfg]` 变化不会影响无关调用点。
- 上下文哈希忽略空白与 CRLF 转义差异，跨平台源码检出保持一致。
- `poly_branch!` / `poly_select!` 的未选中分支保留在 `if false` 中，仍参与类型与借用检查。

## 仓库内概率策略

- 热路径 / 高频基础操作：`cold = 0`；小函数内联概率 `0.85..0.9`，中等函数 `0.6`。
- 大函数：内联概率降至 `0.2` 或 `0.05`，避免代码膨胀。
- 初始化、生成、解析与动态符号解析：内联概率较低，`cold` 取 `0.05..0.15`。
- `syscall0..6` 的源码行数被多目标 `cfg` 放大，统一按热路径覆盖为 `inline = 0.9`。
- 含中性 `inline` 或空属性臂的组合无法用 `#[poly]` 等价表达，保留为 `poly_attr`。

本 crate 仅依赖 `proc-macro2`、`quote`、`syn`。
