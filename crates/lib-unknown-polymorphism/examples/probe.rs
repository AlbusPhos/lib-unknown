// SPDX-License-Identifier: MPL-2.0

//! 打印一次编译期随机结果，便于验证 `POLY_SEED` 与 `reproducible`。

use lib_unknown_polymorphism::{const_random, poly_select};

fn main() {
    let value: u64 = const_random!(u64);
    let choice = poly_select!("a", "b", "c");
    println!("{value} {choice}");
}
