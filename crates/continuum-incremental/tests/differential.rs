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
//! engine's typed `InvariantVerdict::UndefinedAction` (an action's `A#defined` is
//! false) or `InvariantVerdict::Undefined` (the invariant's `I#defined` is false).
//! Scope: every model here is lowered by the CML front end, so each definedness chain
//! has depth 1 and no name collides with an action (CML declarations share one
//! namespace). The reference engine's rule for nested chains and name collisions,
//! which only a hand-built model can carry, is not yet the incremental engine's
//! (bn-1eoco), so no such model is generated here.

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
    let cases: [(&str, &str, &str); 6] = [
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
