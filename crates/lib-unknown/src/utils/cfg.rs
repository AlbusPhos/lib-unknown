// src/utils/cfg.rs

#[macro_export]
macro_rules! cfg_if {
    ($cond:expr, $b1:block) => {
        $crate::cfg_if!($cond, $b1, { () })
    };
    ($cond:expr, $b1:block, $b2:block) => {{
        let __idx = ($cond) as usize;

        #[inline(always)]
        fn __infer_ret<R>() -> R {
            unreachable!()
        }

        let mut __branch_true = Some(move || {
            #[allow(unreachable_code)]
            if false {
                __infer_ret()
            } else {
                $b1
            }
        });
        let mut __branch_false = Some(move || {
            #[allow(unreachable_code)]
            if false {
                __infer_ret()
            } else {
                $b2
            }
        });

        #[inline(always)]
        fn __make_trampoline<F: FnOnce() -> R, R>(_: &Option<F>) -> unsafe fn(*mut ()) -> R {
            unsafe fn __trampoline<F: FnOnce() -> R, R>(env: *mut ()) -> R {
                unsafe {
                    let opt = &mut *(env as *mut Option<F>);
                    (opt.take().unwrap_unchecked())()
                }
            }
            __trampoline::<F, R>
        }

        #[inline(always)]
        fn __dispatch<R>(
            idx: usize,
            fn_table: &[unsafe fn(*mut ()) -> R; 2],
            env_table: &[*mut (); 2],
        ) -> R {
            let selected_fn: unsafe fn(*mut ()) -> R;
            let selected_env: *mut ();
            unsafe {
                #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
                {
                    core::arch::asm!(
                        "mov {fns}, [{fns} + {idx} * 8]",
                        "mov {envs}, [{envs} + {idx} * 8]",
                        idx = in(reg) idx,
                        fns = inout(reg) fn_table.as_ptr() => selected_fn,
                        envs = inout(reg) env_table.as_ptr() => selected_env,
                        options(nostack, pure, readonly)
                    );
                }
                #[cfg(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_pointer_width = "32"
                ))]
                {
                    core::arch::asm!(
                        "mov {fns}, [{fns} + {idx} * 4]",
                        "mov {envs}, [{envs} + {idx} * 4]",
                        idx = in(reg) idx,
                        fns = inout(reg) fn_table.as_ptr() => selected_fn,
                        envs = inout(reg) env_table.as_ptr() => selected_env,
                        options(nostack, pure, readonly)
                    );
                }
                #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
                {
                    let blind_idx = core::hint::black_box(idx);
                    let fns_ptr = core::hint::black_box(fn_table.as_ptr());
                    let envs_ptr = core::hint::black_box(env_table.as_ptr());
                    selected_fn = core::ptr::read_volatile(fns_ptr.add(blind_idx));
                    selected_env = core::ptr::read_volatile(envs_ptr.add(blind_idx));
                }
                selected_fn(selected_env)
            }
        }

        let __fn_table = [
            __make_trampoline(&__branch_false),
            __make_trampoline(&__branch_true),
        ];
        let __env_table = [
            &mut __branch_false as *mut _ as *mut (),
            &mut __branch_true as *mut _ as *mut (),
        ];

        __dispatch(__idx, &__fn_table, &__env_table)
    }};
}

#[macro_export]
macro_rules! cfg_match {
    // 语法支持 1：带花括号包裹的风格 cfg_match!(val, { 1 => { ... }, _ => { ... } })
    ($val:expr, { $( $tt:tt )* }) => {
        $crate::cfg_match!($val, $($tt)*)
    };
    // 语法支持 2：扁平参数风格 cfg_match!(val, 1 => { ... }, _ => { ... })
    (
        $val:expr,
        $( $pat:pat $(if $guard:expr)? => $body:expr ),+ $(,)?
    ) => {{
        let __idx: usize = {
            let __val = &($val);
            #[allow(unreachable_code, unused_variables, clippy::diverging_sub_expression)]
            if false {
                match *__val {
                    $( $pat $(if $guard)? => {}, )+
                }
            }
            let mut __acc_idx: usize = 0;
            let mut __matched: usize = 0; // 0: 尚未匹配, 1: 已命中过前置分支
            let mut __cur_idx: usize = 0;
            $(
                {
                    #[allow(unreachable_patterns, unused_variables)]
                    let __hit = (match *__val {
                        $pat $(if $guard)? => true,
                        _ => false,
                    }) as usize;
                    let __take = core::hint::black_box(__hit & (1usize.wrapping_sub(__matched)));
                    __acc_idx |= __take.wrapping_mul(__cur_idx);
                    __matched |= __take;
                    __cur_idx += 1;
                }
            )+
            __acc_idx
        };

        #[repr(C)]
        struct __CfgEntry<R> {
            f: unsafe fn(*mut ()) -> R,
            e: *mut (),
        }

        #[inline(always)]
        fn __infer_ret<R>() -> R {
            unreachable!()
        }

        #[inline(always)]
        fn __make_entry<F: FnOnce() -> R, R>(branch: &mut Option<F>) -> __CfgEntry<R> {
            unsafe fn __trampoline<F: FnOnce() -> R, R>(env: *mut ()) -> R {
                unsafe {
                    let opt = &mut *(env as *mut Option<F>);
                    (opt.take().unwrap_unchecked())()
                }
            }
            __CfgEntry {
                f: __trampoline::<F, R>,
                e: branch as *mut Option<F> as *mut (),
            }
        }

        #[inline(always)]
        fn __dispatch<R>(idx: usize, table: &[__CfgEntry<R>]) -> R {
            let selected_fn: unsafe fn(*mut ()) -> R;
            let selected_env: *mut ();
            unsafe {
                #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
                {
                    core::arch::asm!(
                        "mov {f_out}, [{table} + {idx2} * 8]",
                        "mov {table}, [{table} + {idx2} * 8 + 8]",
                        table = inout(reg) table.as_ptr() => selected_env,
                        idx2 = in(reg) idx << 1,
                        f_out = out(reg) selected_fn,
                        options(nostack, pure, readonly)
                    );
                }
                #[cfg(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_pointer_width = "32"
                ))]
                {
                    core::arch::asm!(
                        "mov {f_out}, [{table} + {idx} * 8]",
                        "mov {table}, [{table} + {idx} * 8 + 4]",
                        table = inout(reg) table.as_ptr() => selected_env,
                        idx = in(reg) idx,
                        f_out = out(reg) selected_fn,
                        options(nostack, pure, readonly)
                    );
                }
                #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
                {
                    let blind_idx = core::hint::black_box(idx);
                    let table_ptr = core::hint::black_box(table.as_ptr());
                    let entry = core::ptr::read_volatile(table_ptr.add(blind_idx));
                    selected_fn = entry.f;
                    selected_env = entry.e;
                }
                selected_fn(selected_env)
            }
        }

        __dispatch(
            __idx,
            &[
                $(
                    __make_entry(&mut Some(move || {
                        #[allow(unreachable_code)]
                        if false {
                            __infer_ret()
                        } else {
                            $body
                        }
                    })),
                )+
            ],
        )
    }};
}

#[cfg(test)]
#[allow(clippy::unused_unit, clippy::manual_range_patterns)]
mod tests {
    use core::cell::Cell;

    /// 辅助工具：用于追踪闭包捕获变量的析构（Drop）次数，验证无内存泄漏或 Double-Free
    struct DropTracker<'a>(&'a Cell<usize>);
    impl<'a> Drop for DropTracker<'a> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn test_cfg() {
        let x = 10;
        let res = crate::cfg_if!(
            x > 5,
            {
                crate::cfg_match!(x, {
                    0..=5 => "small",
                    6..=10 => "medium",
                    _ => "large",
                })
            },
            { "none" }
        );
        assert_eq!(res, "medium");
    }

    #[test]
    fn test_cfg_if_basic_and_lazy_eval() {
        let effect = Cell::new(0);
        let effect_ref = &effect;

        let res_true = crate::cfg_if!(
            true,
            {
                effect_ref.set(effect_ref.get() + 10);
                "branch_true"
            },
            {
                effect_ref.set(effect_ref.get() + 100);
                "branch_false"
            }
        );
        assert_eq!(res_true, "branch_true");
        assert_eq!(effect.get(), 10);

        let res_false = crate::cfg_if!(
            false,
            {
                effect_ref.set(effect_ref.get() + 10);
                "branch_true"
            },
            {
                effect_ref.set(effect_ref.get() + 100);
                "branch_false"
            }
        );
        assert_eq!(res_false, "branch_false");
        assert_eq!(effect.get(), 110);

        // 单分支语法（返回 ()）
        crate::cfg_if!(true, {
            effect_ref.set(effect_ref.get() + 1);
        });
        assert_eq!(effect.get(), 111);

        crate::cfg_if!(false, {
            effect_ref.set(effect_ref.get() + 1000);
        });
        assert_eq!(effect.get(), 111);
    }

    #[test]
    fn test_cfg_match_patterns_and_syntaxes() {
        #[derive(Clone, Copy)]
        enum Action {
            Stop,
            Move(i32, i32),
            Jump(u8),
        }

        let eval_action = |act: Action| -> i32 {
            crate::cfg_match!(act, {
                Action::Stop => 0,
                Action::Move(0, 0) => 1,
                Action::Move(x, y) if x == y => 2,
                Action::Move(x, _) if x < 0 => -1,
                Action::Move(_, _) => 3,
                Action::Jump(1 | 2 | 3) => 10,
                Action::Jump(4..=10) => 20,
                Action::Jump(_) => 30,
            })
        };

        assert_eq!(eval_action(Action::Stop), 0);
        assert_eq!(eval_action(Action::Move(0, 0)), 1);
        assert_eq!(eval_action(Action::Move(5, 5)), 2);
        assert_eq!(eval_action(Action::Move(-3, 8)), -1);
        assert_eq!(eval_action(Action::Move(3, 8)), 3);
        assert_eq!(eval_action(Action::Jump(2)), 10);
        assert_eq!(eval_action(Action::Jump(7)), 20);
        assert_eq!(eval_action(Action::Jump(99)), 30);

        let flat_res = crate::cfg_match!(
            "rust",
            "c" | "cpp" => 1,
            "rust" => 2,
            _ => 3,
        );
        assert_eq!(flat_res, 2);
    }

    #[test]
    fn test_cfg_match_overlapping_first_match_priority() {
        let check_priority = |val: i32| -> usize {
            let side_effect = Cell::new(0);
            let se = &side_effect;

            let res = crate::cfg_match!(val, {
                0..=10 if val % 2 == 0 => { se.set(se.get() + 1); 100 },
                0..=5 => { se.set(se.get() + 1); 200 },
                0..=10 => { se.set(se.get() + 1); 300 },
                _ => { se.set(se.get() + 1); 400 },
            });

            assert_eq!(side_effect.get(), 1);
            res
        };

        assert_eq!(check_priority(4), 100);
        assert_eq!(check_priority(3), 200);
        assert_eq!(check_priority(7), 300);
        assert_eq!(check_priority(99), 400);
    }

    #[test]
    fn test_drop_and_memory_safety() {
        let drop_count = Cell::new(0);

        {
            let t1 = DropTracker(&drop_count);
            let t2 = DropTracker(&drop_count);

            let res = crate::cfg_if!(
                true,
                {
                    let _moved = t1;
                    10
                },
                {
                    let _moved = t2;
                    20
                }
            );
            assert_eq!(res, 10);
        }
        assert_eq!(drop_count.get(), 2);

        drop_count.set(0);
        {
            let t0 = DropTracker(&drop_count);
            let t1 = DropTracker(&drop_count);
            let t2 = DropTracker(&drop_count);
            let t3 = DropTracker(&drop_count);

            let res = crate::cfg_match!(2, {
                0 => { let _m = t0; "zero" },
                1 => { let _m = t1; "one" },
                2 => { let _m = t2; "two" },
                _ => { let _m = t3; "other" },
            });
            assert_eq!(res, "two");
        }
        assert_eq!(drop_count.get(), 4);
    }

    #[test]
    fn test_large_return_value_and_register_pressure() {
        #[derive(Debug, PartialEq, Eq)]
        struct HeavyPayload {
            data: [u64; 16],
            tag: u32,
        }

        let selector = 3;
        let payload = crate::cfg_match!(selector, {
            0 => HeavyPayload { data: [0; 16], tag: 10 },
            1 => HeavyPayload { data: [1; 16], tag: 20 },
            2 => HeavyPayload { data: [2; 16], tag: 30 },
            3 => HeavyPayload { data: [0xDEAD_BEEF; 16], tag: 40 },
            _ => HeavyPayload { data: [u64::MAX; 16], tag: 50 },
        });

        assert_eq!(payload.tag, 40);
        assert_eq!(payload.data, [0xDEAD_BEEF; 16]);
    }

    #[test]
    fn test_borrow_release_before_dispatch() {
        let mut state = 1;
        let state_ptr = &mut state;

        crate::cfg_match!(*state_ptr, {
            1 => { *state_ptr = 42; },
            _ => {},
        });

        assert_eq!(state, 42);
    }

    #[test]
    #[should_panic(expected = "expected panic in branch 1")]
    fn test_panic_unwind_safety() {
        let val = 1;
        crate::cfg_match!(val, {
            0 => 100,
            1 => panic!("expected panic in branch 1"),
            _ => 200,
        });
    }
}
