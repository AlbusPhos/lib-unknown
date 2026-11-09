// build.rs

//! 构建脚本：采样运行时熵生成 `SEED*` 环境变量，并据此发射
//! `coin0..coin127` 与 `dice0..dice9` 编译期 cfg。
#![allow(unexpected_cfgs)]
include!("src/rand/seed.rs");

fn main() {
    println!("cargo::rustc-env=SEED64={}", seed());
    println!("cargo::rustc-env=SEED128={}", seed128());
    println!("cargo::rustc-env=SEED={}_{}", seed128(), seed128());

    let s: u128 = seed128();

    for i in 0..128 {
        println!("cargo::rustc-check-cfg=cfg(coin{i})");
        if (s >> i) & 1 == 1 {
            println!("cargo::rustc-cfg=coin{i}");
        }
    }

    for i in 0..10 {
        println!(
            "cargo::rustc-check-cfg=cfg(dice{i}, values(\"1\", \"2\", \"3\", \"4\", \"5\", \"6\"))"
        );
        println!("cargo::rustc-cfg=dice{i}=\"{}\"", (seed() % 6) + 1);
    }
}
