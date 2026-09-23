//! Acceptance fixtures for CML elaboration (PR 15a, bn-ybq).
//!
//! # What is pinned
//!
//! | Fixture | Source | Expectation |
//! |---|---|---|
//! | replicated register | `notes/plan/examples/replicated_register.ctm` | elaborates; golden normalized AST; lowering refused, typed |
//! | Die Hard (TV-009, Wave 0) | `notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm` | elaborates; golden normalized AST; lowers |
//! | Coffee Can, Tower of Hanoi | `continuum-cml-syntax/tests/fixtures/` (Wave 0 shapes) | elaborate; golden normalized ASTs |
//! | Transitive Closure | same | elaborates (its recursive `def` passes the termination check, bn-36x3b); golden normalized AST; lowering refused, typed |
//! | syntax coverage | same | elaborates, with a relational action (bn-2ouro); golden normalized AST; lowering refused, typed |
//!
//! The dossier and parser fixtures are read in place, so a change to them is a change to
//! this test's input. Every accepted fixture is elaborated twice and must give the same
//! dump and the same identity (determinism).
//!
//! Regenerate the golden dumps with `CML_BLESS=1 cargo test -p continuum-cml-elab --test
//! fixtures`, and review the diff.

use std::path::PathBuf;

use continuum_cml_elab::{
    ElabErrorKind, LowerErrorKind, NormModel, Unlowerable, elaborate_source, lower,
};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dossier(rel: &str) -> PathBuf {
    manifest().join("../../notes/plan").join(rel)
}

fn parser_fixture(name: &str) -> PathBuf {
    manifest()
        .join("../continuum-cml-syntax/tests/fixtures")
        .join(format!("{name}.ctm"))
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Elaborate `path`, compare its dump with `tests/golden/<name>.norm`, and check
/// determinism.
fn accept(name: &str, path: &PathBuf) -> NormModel {
    let src = read(path);
    let model = elaborate_source(&src).unwrap_or_else(|e| panic!("{name} must elaborate: {e}"));
    let dump = model.dump();

    let golden = manifest().join("tests/golden").join(format!("{name}.norm"));
    if std::env::var_os("CML_BLESS").is_some() {
        std::fs::write(&golden, &dump).expect("write golden");
    }
    assert_eq!(
        dump,
        read(&golden),
        "{name}: dump differs from {}",
        golden.display()
    );

    let again = elaborate_source(&src).expect("second elaboration");
    assert_eq!(again, model, "{name}: elaboration is not deterministic");
    assert_eq!(again.identity(), model.identity());
    model
}

#[test]
fn die_hard_elaborates_and_lowers() {
    let model = accept(
        "DieHard",
        &dossier("corpus/tla-examples/ports/TV-009/DieHard.ctm"),
    );
    let lowered = lower(&model).expect("Die Hard lowers to the programmatic model");
    assert_eq!(lowered.arity(), 2);
    assert_eq!(lowered.actions().len(), 6);
    assert_eq!(lowered.predicates().len(), 2);
    assert_eq!(lowered.initial_states().len(), 1);
}

#[test]
fn die_hard_header_no_longer_claims_it_is_unparsed() {
    let src = read(&dossier("corpus/tla-examples/ports/TV-009/DieHard.ctm"));
    let header = src.lines().next().unwrap_or_default();
    assert!(!header.contains("not yet parsed"), "stale header: {header}");
    assert!(header.contains("continuum-cml-elab"), "header: {header}");
}

/// The replicated register is well-formed CML with sets, maps, options, sorts,
/// constants, parameterized actions, domainless binders, and a fairness assumption.
/// It elaborates. It does not lower, because the programmatic model has no fairness —
/// and it says so with a typed reason at the declaration, rather than dropping it.
#[test]
fn replicated_register_elaborates_and_refuses_to_lower_with_a_reason() {
    let model = accept(
        "replicated_register",
        &dossier("examples/replicated_register.ctm"),
    );
    // `forall epoch, v1, v2:` has no domains: each binder carries its inferred type.
    let dump = model.dump();
    assert!(
        dump.contains("(forall ((epoch Nat) (v1 Value) (v2 Value))"),
        "{dump}"
    );

    let err = lower(&model).expect_err("the register does not lower");
    assert_eq!(err.kind, LowerErrorKind::Unlowerable(Unlowerable::Fairness));
    assert_eq!((err.span.line, err.span.col), (63, 15));

    // Without the fairness line, the next reason is the first non-integer state
    // variable, `alive: Set[Node]`, in canonical (name) order.
    let src =
        read(&dossier("examples/replicated_register.ctm")).replace("fairness weak Recover", "");
    let model = elaborate_source(&src).expect("still elaborates");
    let err = lower(&model).expect_err("still does not lower");
    assert_eq!(
        err.kind,
        LowerErrorKind::Unlowerable(Unlowerable::NonIntegerState)
    );
    assert_eq!(err.span.line, 12);
}

#[test]
fn coffee_can_elaborates_and_refuses_an_unbounded_nat() {
    let model = accept("CoffeeCan", &parser_fixture("CoffeeCan"));
    let err = lower(&model).expect_err("`black: Nat` has no upper bound");
    assert_eq!(
        err.kind,
        LowerErrorKind::Unlowerable(Unlowerable::UnboundedDomain)
    );
}

#[test]
fn tower_of_hanoi_elaborates_with_its_def_inlined() {
    let model = accept("TowerOfHanoi", &parser_fixture("TowerOfHanoi"));
    assert!(!model.dump().contains("top"), "`top` is inlined");
}

/// `closure(r, k)` recurses on `k: Nat` under the guard `k == 0`, passing `k - 1`: it
/// passes the termination check, and the fixture elaborates. It does not lower: its
/// state is a set of pairs, and the programmatic model has only integer variables.
/// `tests/recursion.rs` checks the unfolded `closure` against an independent closure.
#[test]
fn transitive_closure_elaborates_and_refuses_to_lower_with_a_reason() {
    let model = accept("TransitiveClosure", &parser_fixture("TransitiveClosure"));
    let err = lower(&model).expect_err("`reach` is a set of pairs");
    assert_eq!(
        err.kind,
        LowerErrorKind::Unlowerable(Unlowerable::NonIntegerState)
    );
}

/// `Receive` primes `phase` in `phase'[m.src] == Done` and updates no `phase`: `phase`
/// is relational (RFC 0003 "Relational actions", bn-2ouro), and the fixture elaborates.
/// It does not lower: the first reason, in the lowering's fixed order, is its
/// `fairness` line; without it, the maps and sequences of its state.
#[test]
fn syntax_coverage_elaborates_with_a_relational_action() {
    let model = accept("SyntaxCoverage", &parser_fixture("SyntaxCoverage"));
    let dump = model.dump();
    assert!(dump.contains("(relational phase)"), "{dump}");
    assert!(dump.contains("(post (eq (index (primed phase)"), "{dump}");
    let err = lower(&model).expect_err("fairness, maps, sequences");
    assert_eq!(err.kind, LowerErrorKind::Unlowerable(Unlowerable::Fairness));
    let src = read(&parser_fixture("SyntaxCoverage")).replace("fairness strong Receive, Reset", "");
    let err = lower(&elaborate_source(&src).expect("elaborates")).expect_err("maps");
    assert_eq!(
        err.kind,
        LowerErrorKind::Unlowerable(Unlowerable::NonStandardBehavior)
    );
}

#[test]
fn an_out_of_fragment_parse_error_stays_typed_unsupported() {
    let err = elaborate_source(&read(&dossier(
        "corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm",
    )))
    .expect_err("parameterized models are outside the parser's fragment");
    assert!(matches!(err.kind, ElabErrorKind::Parse(_)));
    assert!(err.is_unsupported());
}
