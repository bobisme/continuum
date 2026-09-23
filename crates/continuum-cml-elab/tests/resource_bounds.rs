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
use continuum_cml_elab::elab::{MAX_INLINE_DEPTH, MAX_RECURSION_DEPTH, MAX_UNFOLD_NESTING};
use continuum_cml_elab::lower::MAX_INIT_BINDINGS;
use continuum_cml_elab::lower::init_work;
use continuum_cml_elab::{
    ElabError, ElabErrorKind, Limits, LowerErrorKind, NormModel, Unlowerable, Unsupported,
    elaborate_source, elaborate_source_with, lower, lower_with,
};

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
