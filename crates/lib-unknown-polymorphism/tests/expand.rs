// SPDX-License-Identifier: MPL-2.0

//! 过程宏展开冒烟测试。

use lib_unknown_polymorphism::{const_random, poly, poly_attr, poly_branch, poly_select};

#[poly_attr(
    weight(1) => [inline(always)],
    weight(1) => [inline(never)],
    weight(1) => [cold],
)]
fn attributed_function() -> u32 {
    7
}

#[poly_attr(prob(0.5) => [inline])]
fn probabilistic_function() -> u32 {
    9
}

#[poly]
fn default_poly_function() -> u32 {
    13
}

#[poly(cold = 0.25, inline = 0.75)]
fn named_poly_function() -> u32 {
    17
}

#[poly(0.8, 0.1)]
fn positional_poly_function() -> u32 {
    19
}

trait DefaultMethod {
    #[poly_attr(weight(1) => [inline(always)], weight(1) => [inline(never)])]
    fn value(&self) -> u32 {
        11
    }
}

impl DefaultMethod for () {}

const CONST_RANDOM_U64: u64 = const_random!(u64);
const CONST_RANDOM_DEFAULT: u64 = const_random!();
const CONST_RANDOM_U8: u8 = const_random!(u8, "salt");
const CONST_RANDOM_BOOL: bool = const_random!(bool);

#[test]
fn attribute_macro_expands() {
    assert_eq!(attributed_function(), 7);
    assert_eq!(probabilistic_function(), 9);
    assert_eq!(().value(), 11);
    assert_eq!(default_poly_function(), 13);
    assert_eq!(named_poly_function(), 17);
    assert_eq!(positional_poly_function(), 19);
    let _ = (
        CONST_RANDOM_U64,
        CONST_RANDOM_DEFAULT,
        CONST_RANDOM_U8,
        CONST_RANDOM_BOOL,
    );
}

#[test]
fn branch_and_select_expand() {
    let branch = poly_branch! {
        weight(2) => 1u32,
        weight(1) => 2u32,
        weight(1) => 3u32,
    };
    assert!((1..=3).contains(&branch));

    let selected = poly_select!(10u32, 20u32, 30u32);
    assert!([10, 20, 30].contains(&selected));

    let one = poly_select!(42u32);
    assert_eq!(one, 42);
}
