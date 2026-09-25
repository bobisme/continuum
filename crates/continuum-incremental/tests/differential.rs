//! Differential: the incremental engine against the front end and the reference
//! engine run directly (bn-31vf).
//!
//! - **oracle**: `continuum_cml_elab::elaborate_source` and `lower`, then
//!   `continuum_engine_reference::bfs::explore` and
//!   `continuum_engine_reference::checking::check` — the reference checker's own
//!   verdict code, not the engine's and not the audit's.
//! - **subject**: `continuum_incremental::engine::Engine`, warm from the base model
//!   and revised to each corpus edit, in the promotion lane, so its explorations
//!   and checks are reused through `Exact`, `Validated` and `Conservative` licences
//!   wherever it can.
//!
//! Each compared output is the canonical encoding of `continuum_incremental::output`
//! built from the oracle's values, so the two sides meet on bytes.
//!
//! Definedness (bn-24a5c): the oracle's `CheckOutcome::Undefined` is compared with the
//! engine's typed `InvariantVerdict::UndefinedAction` (an action's whole definedness
//! chain has a false member) or `InvariantVerdict::Undefined` (the invariant's own
//! chain does). Both sides classify a model's `#defined` predicates the same way,
//! `continuum_model_core::definedness::Definedness::of` (bn-24a5c's whole-chain, fail-
//! closed, collision-checked rule): the engine and the independent audit each call it
//! directly, and this differential's comparison is real for whatever chain shape the
//! oracle reports (`undefined.read()`, `undefined.subject()`).
//!
//! Every model here is lowered by the CML front end, so every generated case is a
//! depth-1 chain with no name collision: `#` is not a CML identifier character, so no
//! declared name can itself end in `#defined`, and lowering only ever writes one
//! `X#defined` per subject `X`, never a nested `X#defined#defined` — a structural fact
//! about the front end, not a limitation of this engine (bn-1eoco). Nested chains, a
//! gap, and a name collision — every shape only a hand-built model can carry — are
//! covered where they can actually be built: `continuum-incremental::engine`'s and
//! `::audit`'s own `#[cfg(test)]` modules (`chain_rule`/`tests` there), each checked
//! against this same reference-engine oracle over a hand-built
//! `continuum_engine_reference::ModelBuilder` model, mirroring
//! `continuum-engine-reference/tests/definedness_nested.rs` (cr-pt5h3a).

mod support;

use continuum_engine_reference::Guarded;
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::checking::{self, CheckOutcome, DeadlockPolicy, Obligations};
use continuum_incremental::engine::{Config, Engine};
use continuum_incremental::output::{self, InvariantVerdict};
use continuum_incremental::query::{Label, Stage};
use continuum_incremental::vocab::Lane;

use support::{CORPUS, diehard};

#[test]
fn every_reused_exploration_and_verdict_equals_the_reference_engine_run_directly() {
    let base = diehard();
    let mut compared = 0;
    for mutation in CORPUS {
        let source = mutation.apply(&base);
        compared += compare(&base, &source, mutation.id).0;
    }
    assert!(
        compared >= 30,
        "the differential compared {compared} artifacts"
    );
}

/// Partial-map models, where definedness decides the verdict: an undefined read in an
/// invariant, in an action's guard, both (the action read outranks), and defined
/// variants. Each is reached warm from a base revision, so reuse is exercised too.
#[test]
fn undefined_reads_agree_with_the_reference_engine_run_directly() {
    const INVARIANT: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n}\ninit { m == {} }\naction Stay { unchanged m }\ninvariant Z { m[true] == false }\n";
    const ACTION: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n  b: Nat where b <= 1\n}\ninit { m == {} && b == 0 }\naction Flip {\n  require m[true] == false\n  next b = 1\n  unchanged m\n}\ninvariant NeverB { b == 0 }\n";
    const BOTH: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n  b: Nat where b <= 1\n}\ninit { m == {true -> false} && b == 0 }\naction Drop {\n  require b == 0\n  next m = {}\n  next b = 1\n}\naction Read {\n  require m[true] == false\n  unchanged m, b\n}\ninvariant Other { m[false] == false }\n";
    // bn-1eoco: two actions, each with its own `#defined` predicate, both false at
    // the initial state. The engine now scans every action's whole definedness
    // chain via `Definedness::action_chains()` (ordered by shallowest member)
    // rather than a hand-built `Vec` in predicate order; this pins that the two
    // orders still agree with the reference checker's own first-in-order pick, not
    // just that a lone action does.
    const TWO_ACTIONS: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n  n: Map[Bool, Bool]\n}\ninit { m == {} && n == {} }\naction A {\n  require m[true] == false\n  unchanged m, n\n}\naction B {\n  require n[true] == false\n  unchanged m, n\n}\ninvariant Z { true }\n";
    let cases: [(&str, &str, &str); 7] = [
        ("invariant", INVARIANT, INVARIANT),
        (
            "invariant-defined",
            INVARIANT,
            &INVARIANT.replace("init { m == {} }", "init { m == {true -> false} }"),
        ),
        ("action", ACTION, ACTION),
        (
            "action-defined",
            ACTION,
            &ACTION.replace("require m[true] == false", "require b == 0"),
        ),
        ("both", BOTH, BOTH),
        ("both-from-action", ACTION, BOTH),
        ("two-actions", TWO_ACTIONS, TWO_ACTIONS),
    ];
    let mut kinds = [0_usize; 3];
    for (id, base, source) in cases {
        let (_, seen) = compare(base, source, id);
        for (kind, count) in kinds.iter_mut().zip(seen) {
            *kind += count;
        }
    }
    let [undefined_action, undefined_invariant, decided] = kinds;
    assert!(undefined_action >= 2, "{kinds:?}");
    assert!(undefined_invariant >= 1, "{kinds:?}");
    assert!(decided >= 2, "{kinds:?}");
}

/// bn-1eoco: alpha-renaming preserves the whole-chain classifier's outcome. The
/// two-actions case (both `A#defined` and `B#defined` false at the initial state)
/// renamed throughout (`m`→`p`, `n`→`q`, `A`→`First`, `B`→`Second`, `Z`→`Inv`)
/// still agrees with the reference engine, and still reports the same kind of
/// outcome, an undefined action read: `Definedness::of` reads chain shape, not
/// spelling, so renaming every declared name changes no verdict's category.
#[test]
fn alpha_renaming_the_two_actions_case_preserves_its_undefined_outcome() {
    const TWO_ACTIONS: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n  n: Map[Bool, Bool]\n}\ninit { m == {} && n == {} }\naction A {\n  require m[true] == false\n  unchanged m, n\n}\naction B {\n  require n[true] == false\n  unchanged m, n\n}\ninvariant Z { true }\n";
    const RENAMED: &str = "module M\nstate {\n  p: Map[Bool, Bool]\n  q: Map[Bool, Bool]\n}\ninit { p == {} && q == {} }\naction First {\n  require p[true] == false\n  unchanged p, q\n}\naction Second {\n  require q[true] == false\n  unchanged p, q\n}\ninvariant Inv { true }\n";
    let (_, before) = compare(TWO_ACTIONS, TWO_ACTIONS, "two-actions");
    let (_, after) = compare(RENAMED, RENAMED, "two-actions-renamed");
    assert_eq!(before, after, "alpha-renaming changed the outcome kind");
    assert_eq!(before[0], 1, "{before:?}");
}

/// Revise an engine to `base` then `source`, and compare its exploration and every
/// invariant verdict with the oracle. Returns how many artifacts were compared, and
/// how many verdicts were an undefined action read, an undefined invariant read, or
/// decided.
fn compare(base: &str, source: &str, id: &str) -> (usize, [usize; 3]) {
    let mutation = Id { id };
    let mut compared = 0;
    let mut seen = [0_usize; 3];
    {
        let base = base.to_owned();
        let source = source.to_owned();
        let mut engine = Engine::new(Config::default());
        engine
            .revise(&base, Lane::Promotion)
            .expect("the base fits");
        let revision = engine.revise(&source, Lane::Promotion).expect("fits");

        // The oracle: the front end and the reference engine, no engine in between.
        let Ok(norm) = continuum_cml_elab::elaborate_source(&source) else {
            assert!(
                revision.get(&Label::of(Stage::Explore)).is_none(),
                "{}",
                mutation.id
            );
            return (compared, seen);
        };
        let Ok(model) = continuum_cml_elab::lower(&norm) else {
            assert!(
                revision.get(&Label::of(Stage::Explore)).is_none(),
                "{}",
                mutation.id
            );
            return (compared, seen);
        };
        let explored = bfs::explore(&model, Bounds::CERTIFIABLE);
        let subject = revision
            .get(&Label::of(Stage::Explore))
            .expect("a lowered model is explored");
        assert_eq!(
            subject.output.bytes(),
            output::exploration(&explored, &model).as_slice(),
            "{}: exploration",
            mutation.id
        );
        compared += 1;

        let Ok(exploration) = &explored else {
            return (compared, seen);
        };
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Allowed);
        let report = checking::check(&model, exploration, &obligations).expect("declared");
        for invariant in &norm.invariants {
            let index = model.predicate_index(&invariant.name).expect("lowered");
            let expected = match report.invariant(index).expect("checked").outcome() {
                CheckOutcome::Holds { states } => InvariantVerdict::Holds { states: *states },
                CheckOutcome::Violated { state, depth, .. } => InvariantVerdict::Violated {
                    depth: *depth,
                    state: state.clone(),
                },
                CheckOutcome::Inconclusive(why) => panic!("{}: {why:?}", mutation.id),
                // The same typed outcome on both sides: the action read outranks the
                // invariant's own, and each names the least (depth, canonical) state.
                CheckOutcome::Undefined(undefined) => match undefined.read() {
                    Guarded::Action => InvariantVerdict::UndefinedAction {
                        action: undefined.subject().to_owned(),
                        depth: undefined.depth(),
                        state: undefined.state().clone(),
                    },
                    Guarded::Predicate(_) => InvariantVerdict::Undefined {
                        depth: undefined.depth(),
                        state: undefined.state().clone(),
                    },
                },
            };
            match &expected {
                InvariantVerdict::UndefinedAction { .. } => seen[0] += 1,
                InvariantVerdict::Undefined { .. } => seen[1] += 1,
                _ => seen[2] += 1,
            }
            let subject = revision
                .get(&Label::named(Stage::CheckInvariant, &invariant.name))
                .expect("checked");
            assert_eq!(
                subject.output.bytes(),
                output::invariant_verdict(&expected).as_slice(),
                "{}: {}",
                mutation.id,
                invariant.name
            );
            compared += 1;
        }
    }
    (compared, seen)
}

/// A case label, so the loop body keeps its `mutation.id` messages.
struct Id<'a> {
    id: &'a str,
}
