// build.rs

//! 构建脚本：采样运行时熵生成 `SEED*` 环境变量，并据此发射
//! `coin0..coin127` 与 `dice0..dice9` 编译期 cfg。
//! 当启用 `reproducible` 特性时，所有输出均回退为固定值。
#![allow(unexpected_cfgs)]
include!("src/rand/seed.rs");

const FIXED_SEED64: u64 = 0x1234_5678_9ABC_DEF0;
const FIXED_SEED128: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;
const FIXED_COINS: u128 = 0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA;
const FIXED_DICE: [u64; 10] = [1, 4, 2, 6, 3, 5, 1, 6, 2, 3];

fn main() {
    let is_reproducible = std::env::var("CARGO_FEATURE_REPRODUCIBLE").is_ok();

    let (s64, s128, s_combined, coin_bits) = if is_reproducible {
        println!("cargo::warning=已启用可复现构建 (reproducible)，使用固定随机种子。");
        (
            FIXED_SEED64,
            FIXED_SEED128,
            format!("{}_{}", FIXED_SEED128, FIXED_SEED128),
            FIXED_COINS,
        )
    } else {
        (
            seed(),
            seed128(),
            format!("{}_{}", seed128(), seed128()),
            seed128(),
        )
    };

    println!("cargo::rustc-env=SEED64={}", s64);
    println!("cargo::rustc-env=SEED128={}", s128);
    println!("cargo::rustc-env=SEED={}", s_combined);

    for i in 0..128 {
        println!("cargo::rustc-check-cfg=cfg(coin{i})");
        if (coin_bits >> i) & 1 == 1 {
            println!("cargo::rustc-cfg=coin{i}");
        }
    }

    for (i, fixed) in FIXED_DICE.iter().enumerate() {
        println!(
            "cargo::rustc-check-cfg=cfg(dice{i}, values(\"1\", \"2\", \"3\", \"4\", \"5\", \"6\"))"
        );

        let dice_val = if is_reproducible {
            *fixed
        } else {
            (seed() % 6) + 1
        };

        println!("cargo::rustc-cfg=dice{i}=\"{}\"", dice_val);
    }
}
