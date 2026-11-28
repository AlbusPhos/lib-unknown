// SPDX-License-Identifier: MPL-2.0

//! 编译期多态与可复现随机过程宏。
//!
//! - `poly`：快捷属性宏，按概率控制函数内联（默认 `0.5`）与 `#[cold]`（默认 `0.0`）。
//! - `poly_attr`：按权重或独立概率为 item 随机选择任意属性组合。
//! - `poly_branch!`：按权重选择一个表达式分支；未选中的分支进入 `else if false`，
//!   完整参与类型、生命周期与借用检查，但会被编译器优化器彻底删除。
//! - `poly_select!`：等概率选择一个表达式。
//! - `const_random!`：在编译期生成随机常量。
//! - `poly_track_env!`：植入 `POLY_SEED` 环境依赖，保证 Cargo 正确重编译。
//!
//! 随机源为内部实现的 Xoshiro256++，不依赖任何第三方 RNG。
//!
//! # 可复现
//!
//! - 设置环境变量 `POLY_SEED=<任意字符串>` 可获得确定性展开。
//! - 或启用本 crate 的 `reproducible` feature，使用固定内部种子。
//! - 每次宏调用还会混入 crate 名、归一化输入 token 与进程内调用序号，
//!   因此相同代码中的不同调用点相互独立。

#![cfg_attr(docsrs, feature(doc_cfg))]

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use syn::{
    Expr, Ident, Lit, Meta, Token, Type, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// `reproducible` feature 固定种子。
const FIXED_SEED: u64 = 0x1234_5678_9ABC_DEF0;

/// 按 `(kind, crate_name, normalized_context)` 的 64 位哈希分片的局部调用计数器。
///
/// 不同函数/代码块拥有不同的哈希 Key，其计数恒为 `0`，免疫编译顺序与 `#[cfg]` 变动；
/// 仅当同一 crate 内出现完全相同的宏调用时，对应桶才会递增。
static CONTEXT_COUNTERS: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();

fn next_occurrence_for(context_hash: u64) -> u64 {
    let map_mutex = CONTEXT_COUNTERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map_mutex.lock().unwrap_or_else(|e| e.into_inner());
    let count = map.entry(context_hash).or_insert(0);
    let current = *count;
    *count = current.wrapping_add(1);
    current
}

// ============================================================================
// 内部 RNG：Xoshiro256++
// ============================================================================
struct Xoshiro256PlusPlus {
    s: [u64; 4],
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Xoshiro256PlusPlus {
    fn from_seed_u64(seed: u64) -> Self {
        let mut x = seed;
        Self {
            s: [
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
                splitmix64(&mut x),
            ],
        }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Lemire 加权选择（调用方需保证 `weights` 总和 > 0 且不溢出）。
    fn choose_weighted(&mut self, weights: &[u64]) -> usize {
        let total: u64 = weights.iter().copied().sum();
        assert!(total > 0, "poly: total weight must be greater than zero");
        let r = ((self.next_u64() as u128 * total as u128) >> 64) as u64;
        let mut acc = 0u64;
        for (idx, &w) in weights.iter().enumerate() {
            acc += w;
            if r < acc {
                return idx;
            }
        }
        weights.len() - 1
    }

    /// 基于 IEEE-754 53-bit 尾数的精确概率判定。
    fn chance(&mut self, p: f64) -> bool {
        if p <= 0.0 || p.is_nan() {
            return false;
        }
        if p >= 1.0 {
            return true;
        }
        let uniform = (self.next_u64() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64));
        uniform < p
    }
}

fn fnv1a(mut h: u64, bytes: &[u8]) -> u64 {
    const PRIME: u64 = 0x100_0000_01B3;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// 忽略所有空白字符以及跨平台 CRLF 转义序列（`\r`）的 FNV-1a 哈希。
fn fnv1a_normalized(mut h: u64, s: &str) -> u64 {
    const PRIME: u64 = 0x100_0000_01B3;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'\\' && bytes.get(i + 1) == Some(&b'r') {
            i += 2;
            continue;
        }
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
        i += 1;
    }
    h
}

fn base_seed() -> u64 {
    if let Some(value) = std::env::var_os("POLY_SEED") {
        return fnv1a(
            FIXED_SEED ^ 0xA5A5_5A5A_1234_5678,
            value.to_string_lossy().as_bytes(),
        );
    }
    if cfg!(feature = "reproducible") {
        return FIXED_SEED;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    let stack_probe = 0u8;
    (&stack_probe as *const u8 as usize).hash(&mut hasher);
    hasher.finish()
}

fn rng_for(kind: &str, context: &str) -> Xoshiro256PlusPlus {
    let crate_name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();

    // 先计算与调用顺序无关的上下文指纹。
    let mut ctx_hash = FIXED_SEED;
    ctx_hash = fnv1a(ctx_hash, kind.as_bytes());
    ctx_hash = fnv1a(ctx_hash, crate_name.as_bytes());
    ctx_hash = fnv1a_normalized(ctx_hash, context);

    // 仅针对完全相同的上下文指纹递增序号。
    let occurrence = next_occurrence_for(ctx_hash);

    // 混入种子与局部序号生成最终 RNG。
    let mut seed = base_seed();
    seed = fnv1a(seed, &ctx_hash.to_le_bytes());
    seed = fnv1a(seed, &occurrence.to_le_bytes());
    Xoshiro256PlusPlus::from_seed_u64(seed)
}

fn parse_prob_lit(lit: &Lit) -> syn::Result<f64> {
    let p: f64 = match lit {
        Lit::Float(f) => f.base10_parse()?,
        Lit::Int(i) => i.base10_parse()?,
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "expected a float or integer probability",
            ));
        }
    };
    if !(0.0..=1.0).contains(&p) {
        return Err(syn::Error::new_spanned(
            lit,
            "probability must be between 0.0 and 1.0",
        ));
    }
    Ok(p)
}

/// 在 crate 根部植入 `POLY_SEED` 环境依赖，确保仅使用 `poly_attr` / `poly` 的 crate
/// 也会在环境变量变化时被 Cargo 重新编译。
///
/// 用法（模块级）：
///
/// ```rust,ignore
/// lib_unknown_polymorphism::poly_track_env!();
/// ```
#[proc_macro]
pub fn poly_track_env(_input: TokenStream) -> TokenStream {
    quote! {
        const _: usize = match ::core::option_env!("POLY_SEED") {
            ::core::option::Option::Some(s) => s.len(),
            ::core::option::Option::None => 0,
        };
    }
    .into()
}

// ============================================================================
// #[poly(...)] 快捷内联与冷函数控制宏
// ============================================================================
struct PolyArgs {
    inline_prob: f64,
    cold_prob: f64,
}

impl Parse for PolyArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut inline_prob: Option<f64> = None;
        let mut cold_prob: Option<f64> = None;
        let mut used_named = false;
        let mut positional_idx = 0usize;

        while !input.is_empty() {
            if input.peek(Ident) {
                if positional_idx > 0 {
                    return Err(input.error("cannot mix positional and named arguments in #[poly]"));
                }
                used_named = true;
                let key: Ident = input.parse()?;

                // 同时支持 `inline = 0.5` 和 `inline(0.5)` 两种语法
                let lit: Lit = if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    input.parse()?
                } else if input.peek(syn::token::Paren) {
                    let content;
                    parenthesized!(content in input);
                    content.parse()?
                } else {
                    return Err(syn::Error::new_spanned(
                        &key,
                        "expected `=` or `(...)` after parameter name",
                    ));
                };

                let p = parse_prob_lit(&lit)?;
                match key.to_string().as_str() {
                    "inline" => {
                        if inline_prob.replace(p).is_some() {
                            return Err(syn::Error::new_spanned(
                                key,
                                "duplicate `inline` argument",
                            ));
                        }
                    }
                    "cold" => {
                        if cold_prob.replace(p).is_some() {
                            return Err(syn::Error::new_spanned(key, "duplicate `cold` argument"));
                        }
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            key,
                            "unknown argument, expected `inline` or `cold`",
                        ));
                    }
                }
            } else {
                if used_named {
                    return Err(input.error("cannot mix positional and named arguments in #[poly]"));
                }
                let lit: Lit = input.parse()?;
                let p = parse_prob_lit(&lit)?;
                match positional_idx {
                    0 => inline_prob = Some(p),
                    1 => cold_prob = Some(p),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            lit,
                            "#[poly] accepts at most 2 positional arguments: (inline_prob, cold_prob)",
                        ));
                    }
                }
                positional_idx += 1;
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            inline_prob: inline_prob.unwrap_or(0.4),
            cold_prob: cold_prob.unwrap_or(0.15),
        })
    }
}

/// 快捷多态函数属性宏：按概率控制内联（`#[inline(always)]` vs `#[inline(never)]`）与冷路径（`#[cold]`）。
///
/// - 默认参数：`inline = 0.5`，`cold = 0.15`。
/// - 示例：
///   - `#[poly]`
///   - `#[poly(cold = 0.25)]`
///   - `#[poly(inline = 0.25, cold = 0.75)]`
///   - `#[poly(0.8, 0.1)]`
#[proc_macro_attribute]
pub fn poly(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_text = attr.to_string();
    let item2 = TokenStream2::from(item);
    let args = parse_macro_input!(attr as PolyArgs);
    let mut rng = rng_for("poly", &format!("{}::{}", attr_text, item2));

    let inline_attr = if rng.chance(args.inline_prob) {
        quote! { #[inline(always)] }
    } else {
        quote! { #[inline(never)] }
    };

    let cold_attr = if rng.chance(args.cold_prob) {
        quote! { #[cold] }
    } else {
        TokenStream2::new()
    };

    quote! {
        #cold_attr
        #inline_attr
        #item2
    }
    .into()
}

// ============================================================================
// #[poly_attr(...)]
// ============================================================================
enum AttrRule {
    Weight { weight: u64, attrs: Vec<Meta> },
    Prob { p: f64, attrs: Vec<Meta> },
}

struct AttrRules {
    rules: Vec<AttrRule>,
}

impl Parse for AttrRules {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut rules = Vec::new();
        let mut total_weight = 0u64;
        let mut has_weight_rule = false;

        while !input.is_empty() {
            let kw: Ident = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: Lit = content.parse()?;
            input.parse::<Token![=>]>()?;
            let attr_content;
            bracketed!(attr_content in input);
            let attrs = Punctuated::<Meta, Token![,]>::parse_terminated(&attr_content)?
                .into_iter()
                .collect::<Vec<_>>();
            match kw.to_string().as_str() {
                "weight" => {
                    let weight = match &lit {
                        Lit::Int(i) => i.base10_parse::<u64>()?,
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "weight expects an unsigned integer",
                            ));
                        }
                    };
                    total_weight = total_weight.checked_add(weight).ok_or_else(|| {
                        syn::Error::new_spanned(&lit, "total weight overflowed u64")
                    })?;
                    has_weight_rule = true;
                    rules.push(AttrRule::Weight { weight, attrs });
                }
                "prob" => rules.push(AttrRule::Prob {
                    p: parse_prob_lit(&lit)?,
                    attrs,
                }),
                _ => {
                    return Err(syn::Error::new_spanned(kw, "expected `weight` or `prob`"));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        if rules.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "poly_attr requires at least one rule",
            ));
        }
        if has_weight_rule && total_weight == 0 {
            return Err(syn::Error::new(
                Span::call_site(),
                "total weight of `weight(...)` rules in poly_attr must be greater than zero",
            ));
        }
        Ok(Self { rules })
    }
}

#[proc_macro_attribute]
pub fn poly_attr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_text = attr.to_string();
    let item2 = TokenStream2::from(item);
    let rules = parse_macro_input!(attr as AttrRules);
    let mut rng = rng_for("poly_attr", &format!("{}::{}", attr_text, item2));

    let weights: Vec<u64> = rules
        .rules
        .iter()
        .filter_map(|r| match r {
            AttrRule::Weight { weight, .. } => Some(*weight),
            AttrRule::Prob { .. } => None,
        })
        .collect();

    let selected_weight = if weights.is_empty() {
        None
    } else {
        let idx = rng.choose_weighted(&weights);
        let mut seen = 0usize;
        let mut selected = None;
        for (i, rule) in rules.rules.iter().enumerate() {
            if matches!(rule, AttrRule::Weight { .. }) {
                if seen == idx {
                    selected = Some(i);
                    break;
                }
                seen += 1;
            }
        }
        selected
    };

    let mut chosen_attrs: Vec<&Meta> = Vec::new();
    for (idx, rule) in rules.rules.iter().enumerate() {
        match rule {
            AttrRule::Weight { attrs, .. } => {
                if selected_weight == Some(idx) {
                    chosen_attrs.extend(attrs.iter());
                }
            }
            AttrRule::Prob { p, attrs } => {
                if rng.chance(*p) {
                    chosen_attrs.extend(attrs.iter());
                }
            }
        }
    }

    quote! {
        #( #[#chosen_attrs] )*
        #item2
    }
    .into()
}

// ============================================================================
// poly_branch! / poly_select!
// ============================================================================
struct Branch {
    weight: u64,
    body: Expr,
}

struct Branches {
    branches: Vec<Branch>,
}

impl Parse for Branches {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut branches = Vec::new();
        let mut total_weight = 0u64;

        while !input.is_empty() {
            let kw: Ident = input.parse()?;
            if kw != "weight" {
                return Err(syn::Error::new_spanned(kw, "expected `weight`"));
            }
            let content;
            parenthesized!(content in input);
            let weight_lit: Lit = content.parse()?;
            let weight = match &weight_lit {
                Lit::Int(i) => i.base10_parse::<u64>()?,
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "weight expects an unsigned integer",
                    ));
                }
            };
            total_weight = total_weight.checked_add(weight).ok_or_else(|| {
                syn::Error::new_spanned(&weight_lit, "total weight overflowed u64")
            })?;
            input.parse::<Token![=>]>()?;
            let body: Expr = input.parse()?;
            branches.push(Branch { weight, body });
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        if branches.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "poly_branch requires at least one branch",
            ));
        }
        if total_weight == 0 {
            return Err(syn::Error::new(
                Span::call_site(),
                "total weight in poly_branch must be greater than zero",
            ));
        }
        Ok(Self { branches })
    }
}

fn emit_selected(chosen: &Expr, dead: &[&Expr]) -> TokenStream {
    quote! {
        {
            const _: usize = match ::core::option_env!("POLY_SEED") {
                ::core::option::Option::Some(s) => s.len(),
                ::core::option::Option::None => 0,
            };
            if true {
                #chosen
            } #( else if false {
                #[allow(
                    unreachable_code,
                    unused_variables,
                    unused_assignments,
                    unused_mut,
                    unused_unsafe,
                    dead_code,
                    clippy::all
                )]
                let __poly_dead = { #dead };
                __poly_dead
            } )* else {
                unreachable!()
            }
        }
    }
    .into()
}

#[proc_macro]
pub fn poly_branch(input: TokenStream) -> TokenStream {
    let input2 = TokenStream2::from(input.clone());
    let parsed = parse_macro_input!(input as Branches);
    let mut rng = rng_for("poly_branch", &input2.to_string());
    let weights: Vec<u64> = parsed.branches.iter().map(|b| b.weight).collect();
    let chosen_idx = rng.choose_weighted(&weights);
    let chosen = &parsed.branches[chosen_idx].body;
    let dead: Vec<&Expr> = parsed
        .branches
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != chosen_idx)
        .map(|(_, b)| &b.body)
        .collect();
    emit_selected(chosen, &dead)
}

#[proc_macro]
pub fn poly_select(input: TokenStream) -> TokenStream {
    let input2 = TokenStream2::from(input.clone());
    let exprs = parse_macro_input!(input with Punctuated::<Expr, Token![,]>::parse_terminated);
    let exprs: Vec<Expr> = exprs.into_iter().collect();
    if exprs.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            "poly_select requires at least one expression",
        )
        .to_compile_error()
        .into();
    }
    if exprs.len() == 1 {
        let only = &exprs[0];
        return emit_selected(only, &[]);
    }
    let mut rng = rng_for("poly_select", &input2.to_string());
    let chosen_idx = rng.choose_weighted(&vec![1u64; exprs.len()]);
    let chosen = &exprs[chosen_idx];
    let dead: Vec<&Expr> = exprs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != chosen_idx)
        .map(|(_, e)| e)
        .collect();
    emit_selected(chosen, &dead)
}

// ============================================================================
// const_random!
// ============================================================================
struct ConstRandomArgs {
    ty: Type,
    _salt: Option<Expr>,
}

impl Parse for ConstRandomArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                ty: syn::parse_quote!(u64),
                _salt: None,
            });
        }
        let ty: Type = input.parse()?;
        let salt = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };
        Ok(Self { ty, _salt: salt })
    }
}

#[proc_macro]
pub fn const_random(input: TokenStream) -> TokenStream {
    let input2 = TokenStream2::from(input.clone());
    let args = match syn::parse2::<ConstRandomArgs>(input2.clone()) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };
    let ty = args.ty;
    let mut rng = rng_for("const_random", &input2.to_string());
    let lo = rng.next_u64() as u128;
    let hi = rng.next_u64() as u128;
    let value = (hi << 64) | lo;

    let is_bool = matches!(
        &ty,
        Type::Path(tp) if tp.qself.is_none() && tp.path.is_ident("bool")
    );

    if is_bool {
        let b = (lo & 1) == 1;
        return quote! {
            {
                const _: usize = match ::core::option_env!("POLY_SEED") {
                    ::core::option::Option::Some(s) => s.len(),
                    ::core::option::Option::None => 0,
                };
                #b
            }
        }
        .into();
    }

    let lit = proc_macro2::Literal::u128_suffixed(value);
    quote! {
        {
            const _: usize = match ::core::option_env!("POLY_SEED") {
                ::core::option::Option::Some(s) => s.len(),
                ::core::option::Option::None => 0,
            };
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_possible_wrap,
                clippy::cast_lossless,
                clippy::unnecessary_cast
            )]
            let __poly_rand: #ty = #lit as #ty;
            __poly_rand
        }
    }
    .into()
}

// ============================================================================
// 内部测试
// ============================================================================
#[cfg(test)]
mod tests {
    use super::{PolyArgs, Xoshiro256PlusPlus, fnv1a_normalized, next_occurrence_for};
    use quote::quote;

    #[test]
    fn xoshiro_is_deterministic() {
        let mut a = Xoshiro256PlusPlus::from_seed_u64(42);
        let mut b = Xoshiro256PlusPlus::from_seed_u64(42);
        for _ in 0..32 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn weighted_selection_respects_zero_weights() {
        let mut rng = Xoshiro256PlusPlus::from_seed_u64(7);
        for _ in 0..64 {
            assert_eq!(rng.choose_weighted(&[0, 1, 0]), 1);
        }
    }

    #[test]
    fn normalized_hash_ignores_whitespace() {
        let h1 = fnv1a_normalized(0, "core::ptr::write_bytes(ptr, 0, len)");
        let h2 = fnv1a_normalized(0, "core :: ptr :: write_bytes ( ptr , 0 , len )");
        assert_eq!(h1, h2);
    }

    #[test]
    fn normalized_hash_ignores_crlf_and_escaped_cr() {
        let lf = fnv1a_normalized(0, "a\nb");
        let crlf = fnv1a_normalized(0, "a\r\nb");
        let escaped = fnv1a_normalized(0, r"a\rb");
        let plain = fnv1a_normalized(0, "ab");
        assert_eq!(lf, crlf);
        assert_eq!(escaped, plain);
    }

    #[test]
    fn per_context_counter_only_advances_on_collisions() {
        // 使用不太可能与真实宏上下文碰撞的测试专用哈希。
        assert_eq!(next_occurrence_for(0xDEAD_BEEF_0000_0001), 0);
        assert_eq!(next_occurrence_for(0xDEAD_BEEF_0000_0002), 0);
        assert_eq!(next_occurrence_for(0xDEAD_BEEF_0000_0001), 1);
        assert_eq!(next_occurrence_for(0xDEAD_BEEF_0000_0002), 1);
    }

    #[test]
    fn poly_args_parsing() {
        let default_args: PolyArgs = syn::parse2(quote!()).unwrap();
        assert_eq!(default_args.inline_prob, 0.4);
        assert_eq!(default_args.cold_prob, 0.15);

        let named_args: PolyArgs = syn::parse2(quote!(cold = 0.25, inline = 0.75)).unwrap();
        assert_eq!(named_args.inline_prob, 0.75);
        assert_eq!(named_args.cold_prob, 0.25);

        let pos_args: PolyArgs = syn::parse2(quote!(0.8, 0.1)).unwrap();
        assert_eq!(pos_args.inline_prob, 0.8);
        assert_eq!(pos_args.cold_prob, 0.1);

        assert!(syn::parse2::<PolyArgs>(quote!(inline = 1.5)).is_err());
        assert!(syn::parse2::<PolyArgs>(quote!(0.5, cold = 0.2)).is_err());
    }
}
