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

mod support;

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
            continue;
        };
        let Ok(model) = continuum_cml_elab::lower(&norm) else {
            assert!(
                revision.get(&Label::of(Stage::Explore)).is_none(),
                "{}",
                mutation.id
            );
            continue;
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

        let Ok(exploration) = &explored else { continue };
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
            };
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
    assert!(
        compared >= 30,
        "the differential compared {compared} artifacts"
    );
}
