//! Resource bounds on untrusted CML (INV-016), from security review cr-35xovl (bn-ybq).
//!
//! Every input here is built **linearly**: one line per link of a chain, never by
//! doubling a string. The stack tests run the whole pipeline — elaborate, dump,
//! identity, lower — in a thread with a quarter of the default 2 MiB stack
//! ([`SMALL_STACK`]), so a regression that brings back deep recursion fails as a stack
//! overflow here, long before it can reach a default-sized thread. The memory test
//! runs in a child process under an address-space limit, so it cannot exhaust the host.

use std::thread;

use continuum_cml_elab::budget::MAX_NODES;
use continuum_cml_elab::budget::{MAX_TYPE_DEPTH, MAX_WORK};
use continuum_cml_elab::config::{ConfigErrorKind, MAX_BOUND_VALUES, MAX_SORT_ELEMENTS, RunConfig};
use continuum_cml_elab::elab::{
    MAX_INLINE_DEPTH, MAX_RECURSION_DEPTH, MAX_TREE_DEPTH, MAX_UNFOLD_NESTING,
};
use continuum_cml_elab::lower::MAX_INIT_BINDINGS;
use continuum_cml_elab::lower::{MAX_RELATIONAL_CANDIDATES, init_work, successor_work};
use continuum_cml_elab::lower::{identity_encodings_on_this_thread, lower_configured};
use continuum_cml_elab::{
    ElabError, ElabErrorKind, Limits, LowerErrorKind, NormModel, Unlowerable, Unsupported,
    elaborate_source, elaborate_source_with, lower, lower_with,
};
use continuum_model_core::ident::MAX_IDENT_BYTES;
use continuum_model_core::model::{MAX_ACTIONS, MAX_VARIABLES};

/// A quarter of the default 2 MiB thread stack.
const SMALL_STACK: usize = 512 * 1024;

/// Run the whole pipeline on `src` in a thread with [`SMALL_STACK`]. Returns the
/// elaboration result and, when it succeeds, the lowering error code (if any).
fn pipeline(src: String) -> (Result<NormModel, ElabError>, Option<&'static str>) {
    thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(move || {
            let elaborated = elaborate_source(&src);
            let lowered = elaborated.as_ref().ok().and_then(|m| {
                let _ = m.dump();
                let _ = m.identity();
                lower(m).err().map(|e| e.code())
            });
            (elaborated, lowered)
        })
        .expect("spawn")
        .join()
        .expect("the pipeline does not overflow a quarter of the default stack")
}

const HEADER: &str =
    "module T\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A { unchanged x }\n";

// ---------------------------------------------------------------------------
// finding 1: dependency chains are resolved without recursion
// ---------------------------------------------------------------------------

const CHAIN: usize = 5000;

/// `type T4999 = T4998`, …, `type T0 = Nat`, declared dependents-first so that no
/// declaration-order pass can resolve them from a cache: the chain is followed, and
/// it is followed iteratively.
#[test]
fn a_long_alias_chain_resolves_in_a_small_stack() {
    let mut src = String::from("module T\n");
    for i in (1..CHAIN).rev() {
        src.push_str(&format!("type T{i} = T{}\n", i - 1));
    }
    src.push_str("type T0 = Nat\n");
    src.push_str(&format!(
        "state {{ x: T{} where x <= 3 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\n",
        CHAIN - 1
    ));
    let (elaborated, lowered) = pipeline(src);
    let model = elaborated.expect("a long acyclic alias chain elaborates");
    assert!(model.dump().contains("(var x Nat"));
    assert_eq!(lowered, None, "and lowers");
}

#[test]
fn a_long_alias_cycle_is_refused_in_a_small_stack() {
    let mut src = String::from("module T\n");
    for i in (1..CHAIN).rev() {
        src.push_str(&format!("type T{i} = T{}\n", i - 1));
    }
    src.push_str(&format!("type T0 = T{}\n", CHAIN - 1));
    src.push_str("state { x: T0 }\ninit { true }\naction A { unchanged x }\n");
    let (elaborated, _) = pipeline(src);
    let e = elaborated.expect_err("the chain closes on itself");
    assert!(matches!(e.kind, ElabErrorKind::CyclicTypeAlias(_)), "{e}");
}

/// `def f4999(n: Nat): Nat = f4998(n)`, …, `def f0(n: Nat): Nat = n`, dependents
/// first. Each body is elaborated after the defs it calls, so no def is ever elaborated
/// inside another.
#[test]
fn a_long_def_chain_elaborates_in_a_small_stack() {
    let mut src = String::from(HEADER);
    for i in (1..CHAIN).rev() {
        src.push_str(&format!("def f{i}(n: Nat): Nat = f{}(n)\n", i - 1));
    }
    src.push_str("def f0(n: Nat): Nat = n\n");
    src.push_str(&format!("invariant I {{ f{}(x) <= 3 }}\n", CHAIN - 1));
    let (elaborated, lowered) = pipeline(src);
    let model = elaborated.expect("a long acyclic def chain elaborates");
    assert!(
        model.dump().contains("(le (state x) 3)"),
        "{}",
        model.dump()
    );
    assert_eq!(lowered, None, "and lowers");
}

#[test]
fn a_long_def_cycle_is_typed_unsupported_in_a_small_stack() {
    let mut src = String::from(HEADER);
    for i in (1..CHAIN).rev() {
        src.push_str(&format!("def f{i}(n: Nat): Nat = f{}(n)\n", i - 1));
    }
    src.push_str(&format!("def f0(n: Nat): Nat = f{}(n)\n", CHAIN - 1));
    let (elaborated, _) = pipeline(src);
    let e = elaborated.expect_err("the chain is one mutually recursive cycle");
    assert_eq!(
        e.kind,
        ElabErrorKind::Unsupported(Unsupported::MutualRecursion)
    );
}

/// `f0` is `n` under `a` additions and `f1` is `f0(n)` under `b` more, so an inlined
/// call of `f1` is `a + b + 1` deep.
fn inline_source(a: usize, b: usize) -> String {
    format!(
        "{HEADER}def f0(n: Nat): Nat = n{}\ndef f1(n: Nat): Nat = f0(n){}\ninvariant I {{ f1(x) >= 0 }}\n",
        " + 0".repeat(a),
        " + 0".repeat(b)
    )
}

/// Near and over the inlining depth bound, which is what bounds the depth of every
/// tree later passes recurse over.
#[test]
fn inlined_depth_is_bounded_near_and_over_the_limit() {
    let at = MAX_INLINE_DEPTH - 1;
    let (elaborated, _) = pipeline(inline_source(30, at - 30));
    elaborated.expect("an inlined call exactly at the depth bound elaborates");

    let (elaborated, _) = pipeline(inline_source(30, at - 29));
    let e = elaborated.expect_err("one level more is refused");
    assert_eq!(e.kind, ElabErrorKind::TooLarge);
}

/// The deepest tree elaboration accepts: an expression at the parser's nesting bound
/// around an inlined call at the inlining bound. It elaborates, dumps, and hashes in a
/// small stack; lowering refuses it by depth before recursing.
#[test]
fn the_deepest_accepted_tree_fits_a_small_stack() {
    let src = format!(
        "{HEADER}def f0(n: Nat): Nat = n{}\ninvariant I {{ f0(x){} >= 0 }}\n",
        " + 0".repeat(MAX_INLINE_DEPTH - 1),
        " + 0".repeat(60)
    );
    let (elaborated, lowered) = pipeline(src);
    elaborated.expect("elaborates");
    assert_eq!(lowered, Some("cml.lower.expression_too_deep"));
}

// ---------------------------------------------------------------------------
// finding 2: init enumeration is memory-bounded
// ---------------------------------------------------------------------------

/// Twenty-two binary variables with `init { true }`: 4,194,304 initial states, within
/// the enumeration bound. Before the fix every one became 22 owned bindings (gigabytes)
/// before the builder refused the list. Now the lowering stops at [`MAX_INIT_BINDINGS`].
fn wide_init_source() -> String {
    let mut src = String::from("module Wide\nstate {\n");
    for i in 0..22 {
        src.push_str(&format!("  b{i:02}: Nat where b{i:02} <= 1\n"));
    }
    src.push_str("}\ninit { true }\naction A {\n");
    for i in 0..22 {
        src.push_str(&format!("  unchanged b{i:02}\n"));
    }
    src.push_str("}\n");
    src
}

// The bound is below the adversarial product, so the test exercises it.
const _: () = assert!(MAX_INIT_BINDINGS < 22 << 22);

fn assert_wide_init_is_refused() {
    let model = elaborate_source(&wide_init_source()).expect("elaborates");
    let e = lower(&model).expect_err("too many initial states");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::TooManyInitialStates)
    );
}

const MEMORY_CHILD: &str = "CML_ELAB_MEMORY_CHILD";

/// Address-space limit for the child, in KiB: far below the gigabytes the unbounded
/// accumulation needed, well above what the bounded lowering and the test harness use.
const MEMORY_LIMIT_KIB: usize = 512 * 1024;

/// Run `body` in a child process under `ulimit -v` [`MEMORY_LIMIT_KIB`], so an
/// unbounded allocation fails that process rather than the host. The child is this same
/// test binary, filtered to `test` (the calling test's name), with [`MEMORY_CHILD`] set;
/// in the child, `body` runs in-process in a [`SMALL_STACK`] thread.
fn under_memory_limit(test: &str, body: fn()) {
    let in_small_stack = || {
        thread::Builder::new()
            .stack_size(SMALL_STACK)
            .spawn(body)
            .expect("spawn")
            .join()
            .expect("the body passes in a small stack");
    };
    if std::env::var_os(MEMORY_CHILD).is_some() || !cfg!(target_os = "linux") {
        // In the child — or where the address-space limit (a Linux mechanism) is not
        // available — run in-process.
        in_small_stack();
        return;
    }
    let exe = std::env::current_exe().expect("the test binary");
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -v {MEMORY_LIMIT_KIB} && exec \"$0\" --exact {test} --test-threads 1"
        ))
        .arg(exe)
        .env(MEMORY_CHILD, "1")
        .status()
        .expect("spawn the child");
    assert!(
        status.success(),
        "{test} failed or exceeded {MEMORY_LIMIT_KIB} KiB of address space"
    );
}

#[test]
fn a_wide_init_is_refused_within_a_memory_limit() {
    under_memory_limit(
        "a_wide_init_is_refused_within_a_memory_limit",
        assert_wide_init_is_refused,
    );
}

// ---------------------------------------------------------------------------
// round 2 (cr-35xovl): one output budget, charged before every amplifying allocation
// ---------------------------------------------------------------------------

/// `type A0 = Option[Nat]` and `type A{i} = Option[A{i-1}]`: each alias one level
/// deeper than the last, so alias `A{k}` is `k + 2` deep.
fn wrapper_aliases(k: usize) -> String {
    let mut src = String::from("module T\ntype A0 = Option[Nat]\n");
    for i in 1..=k {
        src.push_str(&format!("type A{i} = Option[A{}]\n", i - 1));
    }
    src.push_str(&format!(
        "state {{ x: Nat where x <= 1\n y: A{k} }}\ninit {{ x == 0 }}\naction A {{ unchanged x, y }}\n"
    ));
    src
}

/// Near and over [`MAX_TYPE_DEPTH`] with wrapper aliases: at the bound the model
/// elaborates, one level more is refused before the type is built.
#[test]
fn wrapper_alias_depth_is_bounded_near_and_over_the_limit() {
    let (elaborated, _) = pipeline(wrapper_aliases(MAX_TYPE_DEPTH - 2));
    elaborated.expect("an alias exactly at the type depth bound elaborates");
    let (elaborated, _) = pipeline(wrapper_aliases(MAX_TYPE_DEPTH - 1));
    assert_eq!(
        elaborated.expect_err("one level more is refused").kind,
        ElabErrorKind::TooLarge
    );
}

/// The reviewer's case: a 5000-link wrapper alias chain, built linearly, run under the
/// memory limit and a small stack. It is refused with a typed error, not a crash.
#[test]
fn a_long_wrapper_alias_chain_is_refused_within_limits() {
    fn body() {
        let e = elaborate_source(&wrapper_aliases(5000)).expect_err("too deep");
        assert_eq!(e.kind, ElabErrorKind::TooLarge);
    }
    under_memory_limit("a_long_wrapper_alias_chain_is_refused_within_limits", body);
}

/// Inference can nest a type through variables without any deep source: 5000 binders,
/// each constrained to be the singleton set of the one before, in one flat set literal.
/// The resolved type of the last binder is 5000 deep; unification and the occurs check
/// are iterative, and the type is measured before it is materialized.
#[test]
fn an_inferred_type_chain_is_refused_within_limits() {
    fn body() {
        let n = 5000;
        let names: Vec<String> = (1..=n).map(|i| format!("a{i}")).collect();
        let clauses: Vec<String> = (1..=n).map(|i| format!("a{i} == {{a{}}}", i - 1)).collect();
        let src = format!(
            "{HEADER}invariant I {{ forall a0 in {{1}}, {}: {{{}}} subseteq {{true}} }}\n",
            names.join(", "),
            clauses.join(", ")
        );
        let Err(e) = elaborate_source(&src) else {
            panic!("the inferred type is too deep");
        };
        assert_eq!(e.kind, ElabErrorKind::TooLarge);
    }
    under_memory_limit("an_inferred_type_chain_is_refused_within_limits", body);
}

/// `def f(s: Set[Nat]): Seq[Set[Nat]] = [s, s, …]` with `refs` references (a sequence,
/// so nothing is deduplicated), called on a literal set of `width` elements: the
/// substituted body is `refs * width` nodes.
fn fanout_source(refs: usize, width: usize) -> String {
    let body = vec!["s"; refs].join(", ");
    let arg: Vec<String> = (0..width).map(|i| i.to_string()).collect();
    format!(
        "{HEADER}def f(s: Set[Nat]): Seq[Set[Nat]] = [{body}]\ninvariant I {{ f({{{}}}) != [] }}\n",
        arg.join(", ")
    )
}

/// Near and over the budget for multiplicative substitution. The over case is
/// `4000 * 4000` nodes — gigabytes if materialized — so it runs under the memory limit;
/// the charge is computed from the body and the argument before anything is copied.
#[test]
fn def_substitution_is_charged_before_it_is_built() {
    let (elaborated, _) = pipeline(fanout_source(300, 300));
    elaborated.expect("a 300 x 300 substitution fits the budget");
    fn body() {
        let Err(e) = elaborate_source(&fanout_source(4000, 4000)) else {
            panic!("a 4000 x 4000 substitution must be refused");
        };
        assert_eq!(e.kind, ElabErrorKind::TooLarge);
    }
    under_memory_limit("def_substitution_is_charged_before_it_is_built", body);
}

/// An equivalence nest `k` deep over a wide leaf: `a15` is `x` doubled fifteen times
/// by `let`s (2^16 nodes, 16 levels), so the nest stays within the model's expression
/// depth while lowering, which writes each equivalence with both operands twice,
/// doubles the output per level.
fn iff_nest(k: usize, eq: bool) -> String {
    let mut body = String::from("  let a0 = x\n");
    for i in 1..=15 {
        body.push_str(&format!("  let a{i} = a{} + a{}\n", i - 1, i - 1));
    }
    let mut e = String::from("a15 >= 0");
    for _ in 0..k {
        e = if eq {
            format!("(({e}) == (x == 0))")
        } else {
            format!("(({e}) <=> (x == 0))")
        };
    }
    format!("{HEADER}invariant I {{\n{body}  {e}\n}}\n")
}

/// Near: 3 levels lower within the budget. Over: 7 levels stay within the model's depth
/// bound but would be about 8 million nodes; the copy that does not fit is refused before
/// it is made. Both spellings, `<=>` and Boolean `==`, run under the memory limit.
#[test]
fn equivalence_lowering_is_charged_before_each_copy() {
    fn body() {
        for eq in [false, true] {
            let m = elaborate_source(&iff_nest(3, eq)).expect("elaborates");
            lower(&m).expect("3 nested equivalences lower within the budget");
            let m = elaborate_source(&iff_nest(7, eq)).expect("elaborates");
            let e = lower(&m).expect_err("over budget");
            assert_eq!(
                e.kind,
                LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge)
            );
        }
    }
    under_memory_limit("equivalence_lowering_is_charged_before_each_copy", body);
}

/// A plain equivalence nest too deep for the model is refused by depth, before the
/// first node over the bound is built.
#[test]
fn a_deep_equivalence_nest_is_refused_by_depth() {
    let mut e = String::from("x == 0");
    for _ in 0..22 {
        e = format!("(({e}) <=> (x == 0))");
    }
    let m = elaborate_source(&format!("{HEADER}invariant I {{ {e} }}\n")).expect("elaborates");
    let err = lower(&m).expect_err("too deep");
    assert_eq!(
        err.kind,
        LowerErrorKind::Unlowerable(Unlowerable::ExpressionTooDeep)
    );
}

// ---------------------------------------------------------------------------
// round 3 (cr-8zf003): flat clause lists conjoin to a bounded depth
// ---------------------------------------------------------------------------

/// A model whose init, one action guard, or one invariant carries `n` separate clauses,
/// each two levels deep. Built one line per clause.
fn flat_clauses(n: usize, site: &str) -> String {
    let mut src = String::from("module T\nstate { x: Nat where x <= 3 }\n");
    let clauses = "  x <= 3\n".repeat(n);
    match site {
        "init" => src.push_str(&format!(
            "init {{\n{clauses}}}\naction A {{ unchanged x }}\n"
        )),
        "guard" => {
            let requires = "  require x <= 3\n".repeat(n);
            src.push_str(&format!(
                "init {{ x == 0 }}\naction A {{\n{requires}  unchanged x\n}}\n"
            ));
        }
        _ => src.push_str(&format!(
            "init {{ x == 0 }}\naction A {{ unchanged x }}\ninvariant I {{\n{clauses}}}\n"
        )),
    }
    src
}

/// The clause list conjoins as a balanced tree, so 100 000 two-level clauses make a
/// 19-level conjunction: within the model's expression depth, and a valid model.
/// 400 000 clauses exceed the output budget and are refused, typed. Neither aborts,
/// and nothing deeper than the model's depth bound is ever built or dropped.
fn flat_clauses_lower_or_refuse(site: &str) {
    let near = elaborate_source(&flat_clauses(100_000, site)).expect("elaborates");
    let model = lower(&near).expect("100 000 flat clauses lower to a bounded model");
    let explored = model.initial_states().len();
    assert!(explored >= 1);

    match elaborate_source(&flat_clauses(400_000, site)) {
        Err(e) => assert_eq!(e.kind, ElabErrorKind::TooLarge),
        Ok(m) => {
            let e = lower(&m).expect_err("over budget");
            assert!(
                matches!(
                    e.kind,
                    LowerErrorKind::Unlowerable(
                        Unlowerable::OutputTooLarge | Unlowerable::ExpressionTooDeep
                    )
                ),
                "{e}"
            );
        }
    }
}

#[test]
fn a_flat_init_clause_list_lowers_or_refuses_within_limits() {
    fn body() {
        flat_clauses_lower_or_refuse("init");
    }
    under_memory_limit(
        "a_flat_init_clause_list_lowers_or_refuses_within_limits",
        body,
    );
}

#[test]
fn a_flat_guard_clause_list_lowers_or_refuses_within_limits() {
    fn body() {
        flat_clauses_lower_or_refuse("guard");
    }
    under_memory_limit(
        "a_flat_guard_clause_list_lowers_or_refuses_within_limits",
        body,
    );
}

#[test]
fn a_flat_invariant_clause_list_lowers_or_refuses_within_limits() {
    fn body() {
        flat_clauses_lower_or_refuse("invariant");
    }
    under_memory_limit(
        "a_flat_invariant_clause_list_lowers_or_refuses_within_limits",
        body,
    );
}

/// The combined depth is checked, not only each clause: clauses already at the model's
/// depth bound cannot be conjoined at all, and the refusal is typed.
#[test]
fn a_conjunction_over_the_depth_bound_is_refused_before_it_is_built() {
    // `x + 0 + … <= 3` with 30 additions is 32 levels deep: at the bound on its own.
    let deep = format!("x{} <= 3", " + 0".repeat(30));
    let one = format!(
        "module T\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\ninvariant I {{ {deep} }}\n"
    );
    lower(&elaborate_source(&one).expect("elaborates")).expect("one clause at the bound lowers");
    let two = one.replace(
        &format!("{{ {deep} }}"),
        &format!("{{\n  {deep}\n  {deep}\n}}"),
    );
    let e = lower(&elaborate_source(&two).expect("elaborates")).expect_err("one level over");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::ExpressionTooDeep)
    );
}

// ---------------------------------------------------------------------------
// round 4 (cr-8zf003): a work budget; linear name checks; init work pre-charged
// ---------------------------------------------------------------------------

/// The work one elaboration of `src` spends, from the crate's own fuel meter.
fn elaboration_work(src: &str) -> u64 {
    let (result, usage) = elaborate_source_with(src, Limits::default());
    result.expect("elaborates");
    usage.work
}

/// `forall b1 in {1}, …, bn in {1}: true` — one flat binder list.
fn flat_binders(n: usize) -> String {
    let binders: Vec<String> = (1..=n).map(|i| format!("b{i} in {{1}}")).collect();
    format!(
        "{HEADER}invariant I {{ forall {}: true }}\n",
        binders.join(", ")
    )
}

/// `{f1: 1, …, fn: 1} == {f1: 1, …, fn: 1}` — two flat record literals.
fn flat_record(n: usize) -> String {
    let fields: Vec<String> = (1..=n).map(|i| format!("f{i}: 1")).collect();
    let r = format!("{{{}}}", fields.join(", "));
    format!("{HEADER}invariant I {{ {r} == {r} }}\n")
}

/// Doubling the input may cost at most this factor more work: 2 for the linear part,
/// with headroom for the logarithm of ordered-table lookups. A quadratic scan doubles
/// its share four-fold and fails it.
const LINEAR_RATIO: f64 = 2.4;

fn assert_linear(what: &str, small: u64, large: u64) {
    let ratio = large as f64 / small as f64;
    assert!(
        ratio <= LINEAR_RATIO,
        "{what}: work grew {ratio:.2}x when the input doubled ({small} -> {large})"
    );
}

/// Freshness checks over a flat binder list cost `O(n log n)` work, not `O(n²)`.
#[test]
fn binder_name_checks_grow_linearly() {
    let n = 4000;
    assert_linear(
        "binders",
        elaboration_work(&flat_binders(n)),
        elaboration_work(&flat_binders(2 * n)),
    );
}

/// Record-field uniqueness costs `O(n log n)` work, not `O(n²)`.
#[test]
fn record_field_checks_grow_linearly() {
    let n = 4000;
    assert_linear(
        "record fields",
        elaboration_work(&flat_record(n)),
        elaboration_work(&flat_record(2 * n)),
    );
}

/// The reviewer's case: 100 000 flat binders, run under the memory limit and a small
/// stack. The binder list exceeds the output budget (each binder owns a name, a domain,
/// and a type), so it is refused, typed — having spent work linear in the list, far
/// below the work bound, rather than the five billion comparisons of a scan.
#[test]
fn a_hundred_thousand_binders_elaborate_within_limits() {
    fn body() {
        let (result, usage) = elaborate_source_with(&flat_binders(100_000), Limits::default());
        if let Err(e) = result {
            assert_eq!(e.kind, ElabErrorKind::TooLarge);
        }
        assert!(usage.work < MAX_WORK / 64, "{usage:?}");
    }
    under_memory_limit("a_hundred_thousand_binders_elaborate_within_limits", body);
}

/// `n` binary variables, an init that fixes every one to 0, and `pad` further
/// clauses: one accepted state out of `2^n` candidates, each evaluating the predicate.
fn wide_init(n: usize, pad: usize) -> String {
    let mut src = String::from("module W\nstate {\n");
    for i in 0..n {
        src.push_str(&format!("  b{i:02}: Nat where b{i:02} <= 1\n"));
    }
    src.push_str("}\ninit {\n");
    for i in 0..n {
        src.push_str(&format!("  b{i:02} == 0\n"));
    }
    src.push_str(&"  b00 <= 1\n".repeat(pad));
    src.push_str("}\naction A {\n");
    for i in 0..n {
        src.push_str(&format!("  unchanged b{i:02}\n"));
    }
    src.push_str("}\n");
    src
}

/// A wide domain times a wide predicate is refused as `WorkLimitExceeded` from the
/// predicted product, before any candidate is evaluated: the work spent is a small
/// fraction of what the enumeration would have cost.
#[test]
fn wide_domain_times_wide_predicate_is_refused_before_enumeration() {
    fn body() {
        let n = 16;
        let pad = 2000;
        let model = elaborate_source(&wide_init(n, pad)).expect("elaborates");
        let (result, usage) = lower_with(&model, Limits::default());
        let e = result.expect_err("too much init work");
        assert_eq!(
            e.kind,
            LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
        );
        // The predicate has more than `pad` nodes, so enumerating would cost more than
        // `2^n * pad` visits; the refusal spent less than one pass over the domain.
        let enumeration = init_work(1 << n, pad, n);
        assert!(enumeration > MAX_WORK, "the case is over the bound");
        assert!(usage.work < 1 << n, "refused before enumerating: {usage:?}");
    }
    under_memory_limit(
        "wide_domain_times_wide_predicate_is_refused_before_enumeration",
        body,
    );
}

/// The boundary, with the work limit set from a measured run: a limit equal to what
/// lowering spends is accepted, one unit less is refused, typed. No wall clock.
#[test]
fn the_work_boundary_is_exact() {
    let model = elaborate_source(&wide_init(10, 50)).expect("elaborates");
    let (result, usage) = lower_with(&model, Limits::default());
    result.expect("lowers within the default budget");
    let at = Limits {
        work: usage.work,
        ..Limits::default()
    };
    lower_with(&model, at).0.expect("at the limit it lowers");
    let under = Limits {
        work: usage.work - 1,
        ..Limits::default()
    };
    let e = lower_with(&model, under)
        .0
        .expect_err("one unit less is refused");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    // The same for elaboration.
    let src = flat_binders(200);
    let (_, used) = elaborate_source_with(&src, Limits::default());
    let tight = Limits {
        work: used.work - 1,
        ..Limits::default()
    };
    let e = elaborate_source_with(&src, tight).0.expect_err("refused");
    assert_eq!(e.kind, ElabErrorKind::WorkLimitExceeded);
}

// ---------------------------------------------------------------------------
// finding 3: strict bounds at the i64 edges are empty, not saturated
// ---------------------------------------------------------------------------

fn refinement(r: &str, init: &str) -> Result<continuum_model_core::Model, Unlowerable> {
    let src = format!(
        "module T\nstate {{ x: Int where {r} }}\ninit {{ {init} }}\naction A {{ unchanged x }}\n"
    );
    let m = elaborate_source(&src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    lower(&m).map_err(|e| match e.kind {
        LowerErrorKind::Unlowerable(u) => u,
        other => panic!("expected an Unlowerable reason, got {other:?}"),
    })
}

#[test]
fn a_strict_bound_past_i64_max_is_an_empty_refinement() {
    assert_eq!(
        refinement(
            "x > 9223372036854775807 && x <= 9223372036854775807",
            "x == 9223372036854775807"
        )
        .err(),
        Some(Unlowerable::EmptyRefinement)
    );
}

#[test]
fn a_strict_bound_past_i64_min_is_an_empty_refinement() {
    assert_eq!(
        refinement(
            "x < -9223372036854775807 - 1 && x >= -9223372036854775807 - 1",
            "x == -9223372036854775807 - 1"
        )
        .err(),
        Some(Unlowerable::EmptyRefinement)
    );
}

#[test]
fn crossing_bounds_are_an_empty_refinement() {
    assert_eq!(
        refinement("x > 5 && x < 3", "x == 4").err(),
        Some(Unlowerable::EmptyRefinement)
    );
}

/// The edges themselves are still representable: a satisfiable interval ending at
/// `i64::MAX` or starting at `i64::MIN` lowers to exactly that domain.
#[test]
fn satisfiable_bounds_at_the_i64_edges_lower_exactly() {
    let top = refinement(
        "x > 9223372036854775805 && x <= 9223372036854775807",
        "x == 9223372036854775807",
    )
    .expect("lowers");
    let d = top.variables().first().expect("one variable").domain();
    assert_eq!((d.lo(), d.hi()), (i64::MAX - 1, i64::MAX));

    let bottom = refinement(
        "x < -9223372036854775807 + 1 && x >= -9223372036854775807 - 1",
        "x == -9223372036854775807 - 1",
    )
    .expect("lowers");
    let d = bottom.variables().first().expect("one variable").domain();
    assert_eq!((d.lo(), d.hi()), (i64::MIN, i64::MIN + 1));
}

// ---------------------------------------------------------------------------
// bn-36x3b: unfolding recursive defs
// ---------------------------------------------------------------------------

/// A tail-recursive def called at the constant `k`: `k` unfoldings, one node of output.
fn tail_recursion(k: u64) -> String {
    format!(
        "{HEADER}def f(k: Nat): Bool = if k == 0 then true else f(k - 1)\ninvariant I {{ f({k}) }}\n"
    )
}

/// Deep recursion: the chain bound is checked from the constant measure before anything
/// is planned, so a call one past [`MAX_RECURSION_DEPTH`] and a call at `10^18` are
/// refused with the same work. Up to the bound, the work grows linearly with the chain.
#[test]
fn deep_recursion_is_refused_before_unfolding_and_counted_below_the_bound() {
    let (elaborated, lowered) = pipeline(tail_recursion(MAX_RECURSION_DEPTH as u64));
    let model = elaborated.expect("a chain at the bound elaborates in a small stack");
    assert!(
        model.dump().contains("(invariant I\n    true\n"),
        "{}",
        model.dump()
    );
    assert_eq!(lowered, None, "and lowers");

    assert_linear(
        "tail recursion",
        elaboration_work(&tail_recursion(16)),
        elaboration_work(&tail_recursion(32)),
    );

    let over = elaborate_source_with(
        &tail_recursion(MAX_RECURSION_DEPTH as u64 + 1),
        Limits::default(),
    );
    let huge = elaborate_source_with(
        &tail_recursion(1_000_000_000_000_000_000),
        Limits::default(),
    );
    for (result, _) in [&over, &huge] {
        let e = result.as_ref().expect_err("past the chain bound");
        assert_eq!(e.kind, ElabErrorKind::TooLarge);
    }
    assert_eq!(over.1, huge.1, "refused before any unfolding: {:?}", over.1);
}

/// `n` measure tests before each recursive call: every one is decided (and so visited)
/// in every unfolding, so the build's recursion is `(n + 1) * k + 2` deep. At
/// [`MAX_UNFOLD_NESTING`] the whole pipeline runs in a small stack; one unfolding more
/// is refused by the plan, before anything is built.
///
/// With `deep`, each unfolding also adds one `+ 1` level of output, so the build is
/// `(n + 2) * k + 2` deep and the output `k + 1`: the deepest build over a deep output.
fn nested_tests(n: usize, k: usize, deep: bool) -> String {
    let (ty, base, other, call, check) = if deep {
        ("Int", "0", "0", "f(k - 1) + 1", ">= 0")
    } else {
        ("Bool", "true", "false", "f(k - 1)", "")
    };
    let mut body = format!("if k == 0 then {base}");
    for i in 1..n {
        body.push_str(&format!(" else if k == {} then {other}", 1000 + i));
    }
    body.push_str(&format!(" else {call}"));
    format!("{HEADER}def f(k: Nat): {ty} = {body}\ninvariant I {{ f({k}) {check} }}\n")
}

#[test]
fn unfolding_nesting_is_bounded_near_and_over_the_limit() {
    const _: () = assert!(10 * 51 + 2 == MAX_UNFOLD_NESTING);
    for (n, deep) in [(9, false), (8, true)] {
        let (elaborated, _) = pipeline(nested_tests(n, 51, deep));
        elaborated.expect("nesting exactly at the bound elaborates in a small stack");
        let (elaborated, _) = pipeline(nested_tests(n, 52, deep));
        assert_eq!(
            elaborated.expect_err("one unfolding more").kind,
            ElabErrorKind::TooLarge
        );
    }
    let (_, lowered) = pipeline(nested_tests(9, 51, false));
    assert_eq!(lowered, None, "the shallow output lowers");
}

/// `f(k) = f(k - 1) + 1` is `k + 1` deep: the output depth is bounded by
/// [`MAX_INLINE_DEPTH`] exactly as for a non-recursive def.
#[test]
fn unfolded_depth_is_bounded_near_and_over_the_limit() {
    let src = |k: usize| {
        format!(
            "{HEADER}def f(k: Nat): Int = if k == 0 then 0 else f(k - 1) + 1\ninvariant I {{ f({k}) >= 0 }}\n"
        )
    };
    let (elaborated, _) = pipeline(src(MAX_INLINE_DEPTH - 1));
    elaborated.expect("an unfolding exactly at the depth bound elaborates");
    let (elaborated, _) = pipeline(src(MAX_INLINE_DEPTH));
    assert_eq!(
        elaborated.expect_err("one level more").kind,
        ElabErrorKind::TooLarge
    );
}

/// Wide fan-out: two recursive calls per unfolding, so `f(k)` is `2^(k+1) - 1` nodes
/// (two budget units each, the node and its `Int` type).
fn fan_out(k: u32) -> String {
    format!(
        "{HEADER}def f(k: Nat): Int = if k == 0 then 1 else f(k - 1) + f(k - 1)\ninvariant I {{ f({k}) >= 0 }}\n"
    )
}

fn fan_out_nodes(k: u32) -> usize {
    2 * ((1 << (k + 1)) - 1)
}

/// The charge covers the output (anti-vacuity: an uncharged unfolding would use fewer
/// nodes than it builds), the work grows with the output and not faster, and an
/// unfolding far past the budget is refused before it is built: the refusal spends a
/// bounded amount of work and allocates nothing.
#[test]
fn wide_fan_out_is_charged_and_refused_before_it_is_built() {
    for k in [8, 10] {
        let (result, usage) = elaborate_source_with(&fan_out(k), Limits::default());
        let m = result.expect("elaborates");
        assert!(usage.nodes >= fan_out_nodes(k), "k = {k}: {usage:?}");
        lower(&m).expect("lowers");
    }
    assert_linear(
        "fan-out (the output doubles)",
        elaboration_work(&fan_out(10)),
        elaboration_work(&fan_out(11)),
    );
    fn body() {
        let (result, usage) = elaborate_source_with(&fan_out(40), Limits::default());
        assert_eq!(
            result.expect_err("2^41 nodes").kind,
            ElabErrorKind::TooLarge
        );
        assert!(
            usage.nodes < 10_000,
            "nothing of the unfolding is built: {usage:?}"
        );
        assert!(usage.work < 8 * MAX_NODES as u64, "{usage:?}");
    }
    under_memory_limit(
        "wide_fan_out_is_charged_and_refused_before_it_is_built",
        body,
    );
}

/// The unfolding's boundaries are exact: at the measured node and work usage it
/// elaborates, one unit less of either is refused, typed.
#[test]
fn the_unfolding_boundary_is_exact() {
    let src = fan_out(9);
    let (result, used) = elaborate_source_with(&src, Limits::default());
    result.expect("elaborates");
    elaborate_source_with(&src, used_limits(used.nodes, used.work))
        .0
        .expect("at the limits it elaborates");
    let e = elaborate_source_with(&src, used_limits(used.nodes - 1, used.work))
        .0
        .expect_err("one node less");
    assert_eq!(e.kind, ElabErrorKind::TooLarge);
    let e = elaborate_source_with(&src, used_limits(used.nodes, used.work - 1))
        .0
        .expect_err("one unit of work less");
    assert_eq!(e.kind, ElabErrorKind::WorkLimitExceeded);
}

fn used_limits(nodes: usize, work: u64) -> Limits {
    Limits { nodes, work }
}

/// An argument that doubles at every unfolding is charged when it is built, even where
/// the body never uses it: `g({1}, 40)` is refused before its arguments are built.
#[test]
fn an_unused_growing_argument_is_charged() {
    let src = |k: u32| {
        format!(
            "{HEADER}def g(r: Set[Int], k: Nat): Bool = if k == 0 then true else g(r union r, k - 1)\ninvariant I {{ g({{1}}, {k}) }}\n"
        )
    };
    let (result, small) = elaborate_source_with(&src(10), Limits::default());
    result.expect("2^10 elaborates");
    assert!(
        small.nodes >= 1 << 11,
        "the arguments are charged: {small:?}"
    );
    fn body() {
        let src = "module T\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A { unchanged x }\ndef g(r: Set[Int], k: Nat): Bool = if k == 0 then true else g(r union r, k - 1)\ninvariant I { g({1}, 40) }\n";
        let (result, usage) = elaborate_source_with(src, Limits::default());
        assert_eq!(result.expect_err("2^40").kind, ElabErrorKind::TooLarge);
        assert!(usage.nodes < 10_000, "{usage:?}");
    }
    under_memory_limit("an_unused_growing_argument_is_charged", body);
}

// ---------------------------------------------------------------------------
// bn-2ouro: relational actions
// ---------------------------------------------------------------------------

/// `x` and `y` in `0..n-1`, both relational in one action whose postcondition has
/// `pad` extra conjuncts: `n * n` candidates, each a copy of the template.
fn relational_source(n: u64, pad: usize) -> String {
    let extra = " && x' <= y' + 1000".repeat(pad);
    format!(
        "module R\nstate {{ x: Nat where x < {n}\n y: Nat where y < {n} }}\ninit {{ x == 0 && y == 0 }}\naction A {{ x' + y' == x + y{extra} }}\n"
    )
}

fn lowering_usage(
    src: &str,
) -> (
    Result<continuum_model_core::Model, continuum_cml_elab::LowerError>,
    continuum_cml_elab::Usage,
) {
    let m = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    lower_with(&m, Limits::default())
}

/// The candidate enumeration is charged before it starts: the work of a lowering
/// covers `successor_work` for its candidates (anti-vacuity: an uncharged enumeration
/// would spend less), and doubling the candidates doubles the work, not more.
#[test]
fn relational_enumeration_is_charged_and_grows_linearly() {
    let (result, small) = lowering_usage(&relational_source(16, 0));
    let model = result.expect("256 candidates lower");
    assert_eq!(model.actions().len(), 256);
    assert!(
        small.work >= successor_work(256, 1, 1),
        "the enumeration is charged: {small:?}"
    );
    assert!(
        small.nodes >= 256 * 8,
        "each candidate's copy is charged: {small:?}"
    );
    let (_, large) = lowering_usage(&relational_source(16, 1));
    let (_, doubled) = lowering_usage(&relational_source(23, 0));
    // 23 * 23 = 529 candidates, about twice 256.
    assert_linear("relational candidates", small.work, doubled.work);
    assert!(
        large.work > small.work,
        "a larger template costs more per candidate"
    );
}

/// Over the candidate bound, and over the budget with a candidate count under the
/// bound: both are refused before the first candidate is built, so the refusal spends
/// less work and allocates fewer nodes than one pass over the candidates would.
#[test]
fn relational_enumeration_is_refused_before_it_is_built() {
    fn body() {
        // 65 * 64 candidates: past MAX_RELATIONAL_CANDIDATES.
        let src = "module R\nstate { x: Nat where x < 65\n y: Nat where y < 64 }\ninit { x == 0 && y == 0 }\naction A { x' + y' == x + y }\n";
        let (result, usage) = lowering_usage(src);
        assert_eq!(
            result.expect_err("too many candidates").kind,
            LowerErrorKind::Unlowerable(Unlowerable::SuccessorDomainTooLarge)
        );
        // The same model with a stuttering action spends the init enumeration and the
        // template; the refusal adds a constant to that, not a pass over the candidates.
        let (_, baseline) = lowering_usage(&src.replace("x' + y' == x + y", "unchanged x, y"));
        assert!(
            usage.work < baseline.work + 1000,
            "{usage:?} against {baseline:?}"
        );

        // 64 * 64 candidates (at the bound) with a 40-conjunct template: millions of
        // nodes, refused by the up-front charge.
        const _: () = assert!(64 * 64 == MAX_RELATIONAL_CANDIDATES);
        let (result, usage) = lowering_usage(&relational_source(64, 40));
        let e = result.expect_err("over budget");
        assert!(
            matches!(
                e.kind,
                LowerErrorKind::Unlowerable(
                    Unlowerable::WorkLimitExceeded | Unlowerable::OutputTooLarge
                )
            ),
            "{e}"
        );
        assert!(usage.nodes < 10_000, "no candidate was built: {usage:?}");
        // The predicted enumeration work is charged (it fits the work budget); the
        // predicted output does not fit, so nothing is built.
    }
    under_memory_limit("relational_enumeration_is_refused_before_it_is_built", body);
}

/// The boundary is exact: at the measured usage the lowering succeeds, one unit of
/// work or one node less is refused, typed.
#[test]
fn the_relational_boundary_is_exact() {
    let m = elaborate_source(&relational_source(8, 2)).expect("elaborates");
    let (result, used) = lower_with(&m, Limits::default());
    result.expect("lowers");
    lower_with(&m, used_limits(used.nodes, used.work))
        .0
        .expect("at the limits");
    let e = lower_with(&m, used_limits(used.nodes, used.work - 1))
        .0
        .expect_err("one unit less");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    let e = lower_with(&m, used_limits(used.nodes - 1, used.work))
        .0
        .expect_err("one node less");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge)
    );
}

/// A primed read of an updated variable copies the update into the postcondition. The
/// copies are charged before they are made: an update of `2^16` nodes read `k` times
/// fits for small `k` and is refused, typed, well before `k` copies are built.
fn primed_copies(k: usize) -> String {
    let mut body = String::from("  let a0 = x\n");
    for i in 1..=15 {
        body.push_str(&format!("  let a{i} = a{} + a{}\n", i - 1, i - 1));
    }
    let reads = vec!["y'"; k].join(" + ");
    format!(
        "module R\nstate {{ x: Int\n y: Int }}\ninit {{ x == 0 && y == 0 }}\naction A {{\n{body}  next y = a15\n  x' >= {reads}\n}}\n"
    )
}

#[test]
fn primed_read_copies_are_charged_before_they_are_built() {
    let (result, small) = elaborate_source_with(&primed_copies(2), Limits::default());
    result.expect("two copies fit");
    assert!(
        small.nodes >= 2 * (1 << 17),
        "the copies are charged: {small:?}"
    );
    fn body() {
        let (result, usage) = elaborate_source_with(&primed_copies(40), Limits::default());
        assert_eq!(result.expect_err("40 copies").kind, ElabErrorKind::TooLarge);
        assert!(usage.nodes <= MAX_NODES, "{usage:?}");
    }
    under_memory_limit("primed_read_copies_are_charged_before_they_are_built", body);
}

/// A copy is refused before it would make a postcondition deeper than
/// [`MAX_TREE_DEPTH`]. The update is an inlined call at [`MAX_INLINE_DEPTH`] under a
/// 60-level chain (`60 + 64` deep); the read `y'` sits under `>=` and `k` additions, so
/// the clause is `k + 1 + 124` deep.
fn primed_depth(k: usize) -> String {
    format!(
        "module R\nstate {{ x: Int\n y: Int }}\ninit {{ x == 0 && y == 0 }}\ndef f0(n: Int): Int = n{}\naction A {{\n  next y = f0(x){}\n  x' >= y'{}\n}}\n",
        " + 0".repeat(MAX_INLINE_DEPTH - 1),
        " + 0".repeat(60),
        " + 0".repeat(k)
    )
}

#[test]
fn primed_read_copies_are_depth_bounded_in_a_small_stack() {
    let at = MAX_TREE_DEPTH - 125;
    let (elaborated, _) = pipeline(primed_depth(at));
    elaborated.expect("a clause exactly at the depth bound elaborates in a small stack");
    let (elaborated, _) = pipeline(primed_depth(at + 1));
    assert_eq!(
        elaborated.expect_err("one level more").kind,
        ElabErrorKind::TooLarge
    );
}

// ---------------------------------------------------------------------------
// cr-22lrdf: substitution cost, and model-core's limits preflighted
// ---------------------------------------------------------------------------

/// `n` relational variables `a`, `b`, … with one-value domains (so one candidate) and
/// `reads` separate postconditions that read the last of them. One-letter names keep
/// the generated action name (`A[a=0,…]`) within the model's name limit.
fn late_reads(n: usize, reads: usize) -> String {
    let names: Vec<char> = ('a'..='z').take(n).collect();
    let mut src = String::from("module L\nstate {\n");
    for v in &names {
        src.push_str(&format!("  {v}: Nat where {v} <= 0\n"));
    }
    src.push_str("}\ninit { true }\naction A {\n");
    for v in &names {
        src.push_str(&format!("  {v}' >= 0\n"));
    }
    let last = names[n - 1];
    src.push_str(&format!("  {last}' >= 0\n").repeat(reads));
    src.push_str("}\n");
    src
}

fn lowering_work(src: &str) -> u64 {
    let (result, usage) = lowering_usage(src);
    result.unwrap_or_else(|e| panic!("lowers: {e}"));
    usage.work
}

/// Finding a placeholder's value is constant work per occurrence (th-2s0fp8): with the
/// reads fixed, going from 6 to 24 relational variables adds work for the variables
/// themselves, not for every read times every variable. Anti-vacuity: a lookup that
/// scans the relational variables per read would add at least `reads * 18` units here
/// (checked by restoring the linear lookup, which fails this test).
#[test]
fn a_late_primed_read_costs_constant_work_per_occurrence() {
    let reads = 2000;
    let few = lowering_work(&late_reads(6, reads));
    let many = lowering_work(&late_reads(24, reads));
    let extra = many.saturating_sub(few);
    assert!(
        extra < (reads as u64) * 18 / 4,
        "work grew by {extra} ({few} -> {many}) for {reads} reads"
    );
}

/// With a work limit below the enumeration's prediction, the lowering is refused at
/// the prediction, before any candidate is substituted or charged as output.
#[test]
fn a_low_work_limit_is_refused_before_substitution() {
    let src = late_reads(24, 2000);
    let m = elaborate_source(&src).expect("elaborates");
    let (result, full) = lower_with(&m, Limits::default());
    result.expect("lowers within the default budget");
    let (result, usage) = lower_with(&m, used_limits(full.nodes, full.work - 1));
    assert_eq!(
        result.expect_err("one unit less").kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    assert!(
        usage.nodes + 2000 < full.nodes,
        "no candidate was charged: {usage:?} against {full:?}"
    );
}

/// `n` variables with one-value domains and one action that keeps them all.
fn wide_state(n: usize) -> String {
    let mut src = String::from("module W\nstate {\n");
    let mut keep = Vec::new();
    for i in 0..n {
        src.push_str(&format!("  v{i:02}: Nat where v{i:02} <= 0\n"));
        keep.push(format!("v{i:02}"));
    }
    src.push_str(&format!(
        "}}\ninit {{ true }}\naction A {{ unchanged {} }}\n",
        keep.join(", ")
    ));
    src
}

#[test]
fn model_core_variable_limit_is_preflighted() {
    lower(&elaborate_source(&wide_state(MAX_VARIABLES)).expect("elaborates"))
        .expect("at the limit it lowers");
    let (result, usage) = lowering_usage(&wide_state(MAX_VARIABLES + 1));
    assert_eq!(
        result.expect_err("one variable more").kind,
        LowerErrorKind::Unlowerable(Unlowerable::TooManyVariables)
    );
    assert_eq!(
        usage.nodes, 0,
        "refused before anything is built: {usage:?}"
    );
}

/// `k` relational actions over `x` in `0..63` (64 candidates each) and `plain` more
/// that keep `x`.
fn many_actions(k: usize, plain: usize) -> String {
    let mut src = String::from("module M\nstate { x: Nat where x <= 63 }\ninit { x == 0 }\n");
    for i in 0..k {
        src.push_str(&format!("action R{i:02} {{ x' >= 0 }}\n"));
    }
    for i in 0..plain {
        src.push_str(&format!("action P{i:02} {{ unchanged x }}\n"));
    }
    src
}

/// The total over all actions, candidates included, is model-core's `MAX_ACTIONS`
/// (th-l7e98m): exactly at it the model lowers, one more is refused before any action
/// is built.
#[test]
fn model_core_action_limit_is_preflighted_in_aggregate() {
    const _: () = assert!(64 * 64 == MAX_ACTIONS);
    let model = lower(&elaborate_source(&many_actions(64, 0)).expect("elaborates"))
        .expect("4096 actions lower");
    assert_eq!(model.actions().len(), MAX_ACTIONS);
    let (result, usage) = lowering_usage(&many_actions(64, 1));
    assert_eq!(
        result.expect_err("4097 actions").kind,
        LowerErrorKind::Unlowerable(Unlowerable::TooManyActions)
    );
    assert!(usage.nodes < 100, "no action was built: {usage:?}");
}

/// A relational action named with `len` bytes over one variable `x` in `0..0`:
/// candidate name `<name>[x=0]`, `len + 5` bytes.
fn labelled(len: usize) -> String {
    let name = format!("A{}", "b".repeat(len - 1));
    format!(
        "module N\nstate {{ x: Nat where x <= 0 }}\ninit {{ x == 0 }}\naction {name} {{ x' >= 0 }}\n"
    )
}

#[test]
fn generated_and_declared_names_are_preflighted() {
    let model = lower(&elaborate_source(&labelled(MAX_IDENT_BYTES - 5)).expect("elaborates"))
        .expect("a 128-byte generated name lowers");
    let name = model.actions()[0].name().to_string();
    assert_eq!(name.len(), MAX_IDENT_BYTES, "{name}");
    let (result, usage) = lowering_usage(&labelled(MAX_IDENT_BYTES - 4));
    assert_eq!(
        result.expect_err("a 129-byte generated name").kind,
        LowerErrorKind::Unlowerable(Unlowerable::NameTooLong)
    );
    assert!(usage.nodes < 100, "no action was built: {usage:?}");
    // A declared name past the limit is refused the same way.
    let long = "a".repeat(MAX_IDENT_BYTES + 1);
    let src = format!(
        "module N\nstate {{ x: Nat where x <= 0 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\ninvariant {long} {{ x == 0 }}\n"
    );
    assert_eq!(
        lower(&elaborate_source(&src).expect("elaborates"))
            .expect_err("a 129-byte invariant name")
            .kind,
        LowerErrorKind::Unlowerable(Unlowerable::NameTooLong)
    );
}

// ---------------------------------------------------------------------------
// bn-3a9sr: run configurations
// ---------------------------------------------------------------------------

/// A model with one sort `S` instantiated with `elements` names, a state variable of
/// that sort, and a constant `C: Set[Nat]` bound to a set of `members` integers. Built
/// one element at a time.
fn sized_run(elements: usize, members: usize) -> (NormModel, String) {
    let model = elaborate_source(
        "module Z\ntype S\nconst C: Set[Nat]\nstate { s: S }\ninit { true }\naction A { unchanged s }\n",
    )
    .expect("elaborates");
    let names: Vec<String> = (0..elements).map(|i| format!("\"e{i}\"")).collect();
    let set: Vec<String> = (0..members).map(|i| format!("{{\"int\":{i}}}")).collect();
    let doc = format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"Z","sorts":{{"S":{{"elements":[{}]}}}},"constants":{{"C":{{"set":[{}]}}}}}}"#,
        names.join(","),
        set.join(",")
    );
    (model, doc)
}

fn configured_usage(
    elements: usize,
    members: usize,
    limits: Limits,
) -> (bool, continuum_cml_elab::Usage) {
    let (model, doc) = sized_run(elements, members);
    let config = RunConfig::parse(doc.as_bytes()).expect("reads");
    let (result, usage) = lower_configured(&model, &config, limits);
    (result.is_ok(), usage)
}

/// Sort elements and value nodes are charged to the output budget (anti-vacuity: the
/// usage covers them), and the work grows linearly with the configuration.
#[test]
fn configuration_sizes_are_charged_and_grow_linearly() {
    let (ok, small) = configured_usage(1000, 4000, Limits::default());
    assert!(ok, "lowers");
    assert!(
        small.nodes >= 1000 + 4000,
        "elements and members are charged: {small:?}"
    );
    let (_, large) = configured_usage(2000, 8000, Limits::default());
    assert_linear("configuration size", small.work, large.work);
}

/// At the sort bound the configuration reads and lowers (a 2^16-value domain); one
/// element more is refused by the reader. A value past the output budget is refused
/// typed before it is used, within the memory limit.
#[test]
fn configuration_bounds_are_enforced_before_use() {
    fn body() {
        let (ok, usage) = configured_usage(MAX_SORT_ELEMENTS, 1, Limits::default());
        assert!(ok, "a sort at the bound lowers: {usage:?}");
        let (_, doc) = sized_run(MAX_SORT_ELEMENTS + 1, 1);
        assert_eq!(
            RunConfig::parse(doc.as_bytes()).expect_err("too many").kind,
            ConfigErrorKind::SortTooLarge
        );
        let (model, doc) = sized_run(3, 200_000);
        let config = RunConfig::parse(doc.as_bytes()).expect("reads");
        let tight = Limits {
            nodes: 100_000,
            ..Limits::default()
        };
        let (result, usage) = lower_configured(&model, &config, tight);
        assert_eq!(
            result.expect_err("over the output budget").kind,
            LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge)
        );
        assert!(usage.nodes <= 100_000, "{usage:?}");
    }
    under_memory_limit("configuration_bounds_are_enforced_before_use", body);
}

/// The configuration's domains meet model-core's limits like any other: a sort-typed
/// relational action over a 4097-element sort is refused by the action preflight.
#[test]
fn configured_domains_are_preflighted_against_model_core() {
    let model =
        elaborate_source("module Y\ntype S\nstate { s: S }\ninit { true }\naction A { s' != s }\n")
            .expect("elaborates");
    let names: Vec<String> = (0..MAX_ACTIONS + 1).map(|i| format!("\"e{i}\"")).collect();
    let doc = format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"Y","sorts":{{"S":{{"elements":[{}]}}}},"constants":{{}}}}"#,
        names.join(",")
    );
    let config = RunConfig::parse(doc.as_bytes()).expect("reads");
    let e = lower_configured(&model, &config, Limits::default())
        .0
        .expect_err("4097 candidates");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::SuccessorDomainTooLarge)
    );
}

// ---------------------------------------------------------------------------
// cr-1sxoia: element lookups by table, text charged by bytes, identities shared
// ---------------------------------------------------------------------------

/// `k` constants of a sort of `e` elements, each bound to the last element (the
/// worst case for a scan of the element list).
fn element_constants(k: usize, e: usize) -> (NormModel, RunConfig) {
    let mut src = String::from("module K\ntype S\n");
    for i in 0..k {
        src.push_str(&format!("const c{i}: S\n"));
    }
    src.push_str("state { x: Nat where x <= 0 }\ninit { x == 0 }\naction A { unchanged x }\n");
    let model = elaborate_source(&src).expect("elaborates");
    let names: Vec<String> = (0..e).map(|i| format!("\"e{i}\"")).collect();
    let last = e - 1;
    let consts: Vec<String> = (0..k)
        .map(|i| format!(r#""c{i}":{{"elem":{{"sort":"S","name":"e{last}"}}}}"#))
        .collect();
    let doc = format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"K","sorts":{{"S":{{"elements":[{}]}}}},"constants":{{{}}}}}"#,
        names.join(","),
        consts.join(",")
    );
    (model, RunConfig::parse(doc.as_bytes()).expect("reads"))
}

fn element_work(k: usize, e: usize) -> u64 {
    let (model, config) = element_constants(k, e);
    let (result, usage) = lower_configured(&model, &config, Limits::default());
    result.unwrap_or_else(|err| panic!("lowers: {err}"));
    usage.work
}

/// Two axes (constants × elements), operation-counted: the marginal work of one more
/// element constant grows with the logarithm of the sort, not with its size. With a
/// scan of the element list per constant (the defect cr-1sxoia found, checked by
/// restoring it), doubling the sort doubles the marginal cost and this fails.
#[test]
fn element_constants_cost_a_table_lookup_not_a_scan() {
    let (k, e) = (500, 4096);
    let marginal = |e: usize| {
        let one = element_work(k, e);
        let two = element_work(2 * k, e);
        (two - one) as f64 / k as f64
    };
    let small = marginal(e);
    let large = marginal(2 * e);
    assert!(
        large / small < 1.5,
        "per-constant work grew {:.2}x when the sort doubled ({small:.1} -> {large:.1})",
        large / small
    );
}

/// A work limit below the element table's own charge is refused before the table is
/// built, so no element is ever looked up.
#[test]
fn a_tight_work_limit_is_refused_before_any_element_lookup() {
    let (model, config) = element_constants(1, MAX_SORT_ELEMENTS);
    let tight = Limits {
        work: 50_000,
        ..Limits::default()
    };
    let (result, usage) = lower_configured(&model, &config, tight);
    assert_eq!(
        result.expect_err("under the table's charge").kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    assert!(usage.work <= 50_000, "{usage:?}");
    // The element table alone charges more than the limit: the refusal came first.
    assert!(MAX_SORT_ELEMENTS as u64 > 50_000);
}

/// A string constant is charged by its bytes (cr-1sxoia): a 1 MiB string under a
/// 10 000-node limit is refused, typed, where a one-node count would admit it.
#[test]
fn long_string_constants_are_charged_by_their_bytes() {
    let model = elaborate_source(
        "module L\nconst Text: String\nstate { x: Nat where x <= 0 }\ninit { x == 0 }\naction A { unchanged x }\n",
    )
    .expect("elaborates");
    let long = "a".repeat(1 << 20);
    let doc = format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"L","sorts":{{}},"constants":{{"Text":{{"str":"{long}"}}}}}}"#
    );
    let config = RunConfig::parse(doc.as_bytes()).expect("reads");
    let (result, usage) = lower_configured(&model, &config, Limits::default());
    result.expect("fits the default budget");
    assert!(usage.nodes >= (1 << 20) / 32, "charged by bytes: {usage:?}");
    let tight = Limits {
        nodes: 10_000,
        ..Limits::default()
    };
    let (result, _) = lower_configured(&model, &config, tight);
    assert_eq!(
        result.expect_err("over the node limit").kind,
        LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge)
    );
}

/// Every allocation of a configured lowering, the identities included, is inside the
/// budget: at the measured usage it lowers, one node or one unit of work less is
/// refused, typed.
#[test]
fn the_configured_boundary_is_exact() {
    let (model, config) = element_constants(20, 300);
    let (result, used) = lower_configured(&model, &config, Limits::default());
    result.expect("lowers");
    lower_configured(&model, &config, used_limits(used.nodes, used.work))
        .0
        .expect("at the limits");
    let e = lower_configured(&model, &config, used_limits(used.nodes - 1, used.work))
        .0
        .expect_err("one node less");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge)
    );
    let e = lower_configured(&model, &config, used_limits(used.nodes, used.work - 1))
        .0
        .expect_err("one unit less");
    assert_eq!(
        e.kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
}

// ---------------------------------------------------------------------------
// cr-1sxoia round 3: the model identity is precharged; value sizes are stored
// ---------------------------------------------------------------------------

/// The model identity's length bound is charged, as output and as work, before the
/// identity is encoded: one node or one unit of work short of the full usage is
/// refused at that charge, and the thread's encoding counter shows the identity was
/// never built. At the full usage it is built exactly once.
#[test]
fn the_model_identity_is_charged_before_it_is_built() {
    let (model, config) = element_constants(20, 300);
    let (result, used) = lower_configured(&model, &config, Limits::default());
    result.expect("lowers");
    for limits in [
        used_limits(used.nodes - 1, used.work),
        used_limits(used.nodes, used.work - 1),
    ] {
        let before = identity_encodings_on_this_thread();
        let e = lower_configured(&model, &config, limits)
            .0
            .expect_err("one short");
        assert!(
            matches!(
                e.kind,
                LowerErrorKind::Unlowerable(
                    Unlowerable::OutputTooLarge | Unlowerable::WorkLimitExceeded
                )
            ),
            "{e}"
        );
        assert_eq!(
            identity_encodings_on_this_thread(),
            before,
            "the identity was not built"
        );
    }
    let before = identity_encodings_on_this_thread();
    lower_configured(&model, &config, used_limits(used.nodes, used.work))
        .0
        .expect("at the limits");
    assert_eq!(identity_encodings_on_this_thread(), before + 1);
}

/// A value's size is computed once while the document is read and stored: the
/// lowering reads it by a charged lookup, not a walk. It counts nodes and text bytes.
#[test]
fn constant_sizes_are_stored_at_read_time() {
    let doc = r#"{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"M","sorts":{},"constants":{"A":{"str":"0123456789012345678901234567890123456789"},"B":{"set":[{"int":1},{"int":2},{"int":3}]}}}"#;
    let config = RunConfig::parse(doc.as_bytes()).expect("reads");
    // A: one node and 40 bytes of text (two 32-byte nodes).
    assert_eq!(config.constant_size("A"), Some(3));
    // B: the set and its three members.
    assert_eq!(config.constant_size("B"), Some(4));
    assert_eq!(config.constant_size("C"), None);
}

// ---------------------------------------------------------------------------
// bn-15zfa: explicit `Nat` and `Int` bounds (RFC 0003 correction 4)
// ---------------------------------------------------------------------------

/// A model of `k` unrefined `Nat` variables with init `init`, and a configuration
/// that bounds `Nat` to `0..=max`. Built one variable at a time.
fn bounded_nats(k: usize, max: u64, init: &str) -> (NormModel, RunConfig) {
    let mut src = String::from("module B\nstate {\n");
    for i in 0..k {
        src.push_str(&format!("  n{i}: Nat\n"));
    }
    src.push_str(&format!("}}\ninit {{ {init} }}\naction A {{\n"));
    for i in 0..k {
        src.push_str(&format!("  unchanged n{i}\n"));
    }
    src.push_str("}\n");
    let model = elaborate_source(&src).expect("elaborates");
    let doc = format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"B","bounds":{{"Nat":{{"max":{max}}}}},"sorts":{{}},"constants":{{}}}}"#
    );
    (model, RunConfig::parse(doc.as_bytes()).expect("reads"))
}

/// The widest bound times several variables is refused by the domain preflight
/// before any candidate is enumerated, within the memory limit, having spent little
/// work. Anti-vacuity: the same model under a small bound lowers.
#[test]
fn bounded_domains_are_preflighted_before_enumeration() {
    fn body() {
        let widest = MAX_BOUND_VALUES - 1;
        let (model, config) = bounded_nats(2, widest, "true");
        let (result, usage) = lower_configured(&model, &config, Limits::default());
        assert_eq!(
            result.expect_err("2^32 candidates").kind,
            LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge)
        );
        assert!(usage.work < 10_000, "refused before enumerating: {usage:?}");
        let (model, config) = bounded_nats(2, 1, "true");
        let lowered = lower_configured(&model, &config, Limits::default())
            .0
            .expect("a small bound lowers");
        assert_eq!(lowered.model().initial_states().len(), 4);
    }
    under_memory_limit("bounded_domains_are_preflighted_before_enumeration", body);
}

/// A bounded domain feeds the same precharged init enumeration as a refinement: the
/// work covers one predicate evaluation per candidate (operation counting), and it
/// grows linearly in the bound.
#[test]
fn a_bounded_enumeration_is_charged_and_grows_linearly() {
    let usage = |max: u64| {
        let (model, config) = bounded_nats(1, max, "n0 == 0");
        let (result, usage) = lower_configured(&model, &config, Limits::default());
        result.expect("lowers");
        usage
    };
    let large = usage(MAX_BOUND_VALUES - 1);
    assert!(
        large.work >= init_work(u128::from(MAX_BOUND_VALUES), 1, 1),
        "every candidate is charged: {large:?}"
    );
    let small = usage(MAX_BOUND_VALUES / 2 - 1);
    assert_linear("bounded enumeration", small.work, large.work);
}

/// With a work limit under the predicted enumeration, a bounded domain is refused from
/// the prediction, before any candidate is evaluated; at the measured work it lowers
/// and one unit less is refused.
#[test]
fn the_bounded_enumeration_boundary_is_exact() {
    let (model, config) = bounded_nats(1, MAX_BOUND_VALUES - 1, "n0 == 0");
    let (result, used) = lower_configured(&model, &config, Limits::default());
    result.expect("lowers");
    let half = Limits {
        work: init_work(u128::from(MAX_BOUND_VALUES), 1, 1) / 2,
        ..Limits::default()
    };
    let (result, spent) = lower_configured(&model, &config, half);
    assert_eq!(
        result.expect_err("under the prediction").kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    assert!(
        spent.work < MAX_BOUND_VALUES,
        "less than one unit per candidate: refused before enumerating ({spent:?})"
    );
    lower_configured(&model, &config, used_limits(used.nodes, used.work))
        .0
        .expect("at the measured limits");
    let under = Limits {
        work: used.work - 1,
        ..Limits::default()
    };
    assert_eq!(
        lower_configured(&model, &config, under)
            .0
            .expect_err("one unit less")
            .kind,
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
}

/// An `Int` bound at the top of `i64`: the domain and its cardinality are exact, with
/// no overflow, and the reader's value count does not wrap on the whole range.
#[test]
fn an_int_bound_at_the_i64_edge_is_exact() {
    let model = elaborate_source(&format!(
        "module E\nstate {{ x: Int }}\ninit {{ x == {} }}\naction A {{ unchanged x }}\n",
        i64::MAX
    ))
    .expect("elaborates");
    let lo = i64::MAX - (MAX_BOUND_VALUES as i64 - 1);
    let doc = |lo: i64, hi: i64| {
        format!(
            r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"E","bounds":{{"Int":{{"min":{lo},"max":{hi}}}}},"sorts":{{}},"constants":{{}}}}"#
        )
    };
    let config = RunConfig::parse(doc(lo, i64::MAX).as_bytes()).expect("reads");
    let lowered = lower_configured(&model, &config, Limits::default())
        .0
        .expect("lowers");
    let x = lowered.model().variables().first().expect("x");
    assert_eq!((x.domain().lo(), x.domain().hi()), (lo, i64::MAX));
    assert_eq!(lowered.model().initial_states().len(), 1);
    assert_eq!(
        RunConfig::parse(doc(i64::MIN, i64::MAX).as_bytes())
            .expect_err("2^64 values")
            .kind,
        ConfigErrorKind::BoundTooLarge
    );
}
