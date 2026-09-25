//! C005 mutation evidence: seeded faults in the reducer, each caught by the
//! differential.
//!
//! Each [`Mutant`] removes one insertion the sound algorithm makes (a dependence edge,
//! a necessary-enabling insertion, the proviso's full expansion, the sleep-set wakeup,
//! the wait for a component to close before donating a sleeping label). The mutants
//! exist only under `#[cfg(test)]` ([`engine::Knobs`]); no shipped code path can
//! select one.
//!
//! The campaign runs the same generated corpus and the same oracle comparison as
//! `tests/c005_differential.rs` (both files include `tests/support/`), under every
//! mutant, and requires that the **differential** — verdicts, visible reachable
//! states, terminal states, counterexample replay — detects each mutant on at least
//! one model. It also records, per mutant, how often the independent witness checker
//! rejects the mutant's witness: the checker is the second, local line of defence.
//!
//! The retained ledger is `tests/golden/c005_mutation.evidence.txt`, compared byte
//! for byte. On drift the test writes the new ledger to
//! `tests/golden/c005_mutation.evidence.txt.new`; review the diff and move it over.
//!
//! Evidence for RFC 0004 correction 1 and claim C005.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use crate::test_support::{corpus, differential, handmade};

use core::fmt::Write as _;

use crate::engine::{self, Knobs, Mutant};
use crate::report::{DeadlockPolicy, Obligations};

/// Seeds of the generated corpus the campaign runs: the same range as the
/// differential's (`tests/c005_differential.rs`, `CORPUS_SEEDS`).
const CORPUS_SEEDS: u64 = 1000;

struct Tally {
    models: usize,
    detected: usize,
    checker_rejected: usize,
    first: Option<String>,
    adversarial: Vec<&'static str>,
    /// Detections by comparison category, over both scopes.
    by_category: std::collections::BTreeMap<&'static str, usize>,
    /// Detections under the narrow scope (first invariant, terminal states allowed).
    narrow: usize,
    /// Detections that only the terminal-state comparison made.
    terminal_only: usize,
}

fn run_mutant(mutant: Mutant) -> Tally {
    let mut tally = Tally {
        models: 0,
        detected: 0,
        checker_rejected: 0,
        first: None,
        adversarial: Vec::new(),
        by_category: std::collections::BTreeMap::new(),
        narrow: 0,
        terminal_only: 0,
    };
    let knobs = Knobs {
        mutant: Some(mutant),
        ..Knobs::default()
    };
    let mut cases: Vec<(
        String,
        Option<&'static str>,
        continuum_model_core::model::Model,
    )> = handmade::all()
        .into_iter()
        .map(|(id, model)| (id.to_owned(), Some(id), model))
        .collect();
    for seed in 0..CORPUS_SEEDS {
        cases.push((format!("seed {seed}"), None, corpus::generate(seed).model));
    }
    for (id, adversarial, model) in cases {
        let wide = differential::Scope::every(&model);
        let narrow = differential::Scope {
            invariants: (0..model.predicates().len().min(1)).collect(),
            defect: false,
        };
        let mut comparisons = Vec::new();
        for scope in [&wide, &narrow] {
            let oracle = differential::oracle_of(&model, scope);
            let report = engine::run(&model, &scope.dpor(), differential::dpor_bounds(), knobs)
                .expect("obligations are declared");
            comparisons.push(differential::compare_of(&model, scope, &oracle, &report));
        }
        let narrow_comparison = comparisons.pop().expect("two scopes");
        let comparison = comparisons.pop().expect("two scopes");
        tally.models += 1;
        if narrow_comparison.differential_detects() {
            tally.narrow += 1;
        }
        let mut failed = comparison.failed.clone();
        failed.extend(narrow_comparison.failed.iter().copied());
        for category in &failed {
            *tally.by_category.entry(*category).or_default() += 1;
        }
        if failed.len() == 1 && failed.contains("terminals") {
            tally.terminal_only += 1;
        }
        if comparison.differential_detects() || narrow_comparison.differential_detects() {
            tally.detected += 1;
            if tally.first.is_none() {
                tally.first = Some(id.clone());
            }
            if let Some(name) = adversarial {
                tally.adversarial.push(name);
            }
        }
        if comparison.checker_rejected.is_some() || narrow_comparison.checker_rejected.is_some() {
            tally.checker_rejected += 1;
        }
    }
    tally
}

#[test]
fn every_seeded_fault_is_caught_by_the_differential() {
    // Mutant 1 is docs/19 §4's "Semantic engine" mutation kind named below (a
    // write/read conflict dropped from the dependence relation); mutants 2 and 3 drop
    // a backtrack insertion, mutant 4 drops the sleep-set wakeup, and mutant 5
    // donates a sleeping label into an open component.
    let kind = "declare conflicting events independent";
    let mut ledger = String::new();
    ledger.push_str(
        "# C005 mutation evidence (bn-voq4): seeded reducer faults against the unreduced oracle.\n\
         # Corpus: tests/support/handmade.rs plus generated seeds 0..1000 (tests/support/corpus.rs).\n\
         # Regenerate: cargo test -p continuum-engine-dpor --lib mutation, then review the\n\
         # .new file it writes beside this one on drift and move it over.\n",
    );
    writeln!(ledger, "# docs/19 §4 mutation kind of c005-mut-01: {kind}").unwrap();
    // The terminal-state comparison (under both policies) and the narrow scope must
    // each be load-bearing: some mutant is caught by the terminal comparison alone,
    // and some under the narrow scope (cr-1wuzry round 2's false-green finding).
    let mut terminal_only = 0_usize;
    let mut narrow_detected = 0_usize;
    for (position, mutant) in Mutant::ALL.iter().enumerate() {
        let tally = run_mutant(*mutant);
        writeln!(
            ledger,
            "\n[c005-mut-{:02}-{}]\nmodels (each under both scopes): {}\ndifferential detects: {}\n  under the narrow scope: {}\n  by category: {}\n  by the terminal-state comparison alone: {}\nchecker rejects: {}\nfirst detecting model: {}\nadversarial models detecting it: {}",
            position + 1,
            mutant.as_str(),
            tally.models,
            tally.detected,
            tally.narrow,
            tally
                .by_category
                .iter()
                .map(|(c, n)| format!("{c}={n}"))
                .collect::<Vec<_>>()
                .join(" "),
            tally.terminal_only,
            tally.checker_rejected,
            tally.first.as_deref().unwrap_or("none"),
            tally.adversarial.join(","),
        )
        .unwrap();
        assert!(
            tally.detected > 0,
            "mutant {} escaped the differential on every model",
            mutant.as_str()
        );
        if *mutant == Mutant::HideDefinednessReads {
            // Hiding the definedness reads must change an outcome, not only the
            // projection the harness compares: the undefined read is missed.
            assert!(
                tally.by_category.get("verdicts").copied().unwrap_or(0) > 0,
                "hiding definedness reads never changed an undefined-read verdict"
            );
        }
        terminal_only += tally.terminal_only;
        narrow_detected += tally.narrow;
    }
    assert!(
        terminal_only > 0,
        "no mutant is caught by the terminal comparison alone"
    );
    assert!(
        narrow_detected > 0,
        "no mutant is caught under the narrow scope"
    );
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/c005_mutation.evidence.txt"
    );
    // No environment switch here: GOV-1-04 keeps `std::env` out of a core crate's
    // `src/`, tests included. A drifted ledger is written beside the golden as
    // `.new`, to be reviewed and moved over it.
    let golden = std::fs::read_to_string(path).unwrap_or_default();
    if ledger != golden {
        std::fs::write(format!("{path}.new"), &ledger).expect("write the drifted ledger");
    }
    assert_eq!(ledger, golden, "the mutation ledger drifted from {path}");
}

#[test]
fn the_sound_reducer_passes_the_same_campaign() {
    // Anti-vacuity for the campaign itself: with no mutant, nothing is detected.
    for (id, model) in handmade::all() {
        let oracle = differential::oracle(&model);
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
        let report = engine::run(
            &model,
            &obligations,
            differential::dpor_bounds(),
            Knobs::default(),
        )
        .expect("obligations are declared");
        let comparison = differential::compare(&model, &oracle, &report);
        assert!(
            !comparison.differential_detects(),
            "{id}: {:?}",
            comparison.disagreements
        );
        assert_eq!(comparison.checker_rejected, None, "{id}");
    }
}

// ---------------------------------------------------------------------------
// cr-1wuzry round 2: the reducer's charge sites, paid per site
// ---------------------------------------------------------------------------

mod charges {
    use continuum_model_core::expr::{BoolExpr, CmpOp, IntExpr};
    use continuum_model_core::model::{ActionDecl, Model, ModelBuilder};

    use crate::budget::bits;
    use crate::engine::{self, Knobs, Omission};
    use crate::report::{Bound, Bounds, CheckError, Completeness, DeadlockPolicy, Obligations};
    use crate::test_support::handmade;

    const WORK: u64 = 1 << 40;

    fn generous() -> Bounds {
        Bounds::new(1 << 20, 1 << 24, 1 << 20, WORK)
    }

    fn panics_unpaid(run: impl FnOnce()) -> bool {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
            Ok(()) => false,
            Err(payload) => payload
                .downcast_ref::<String>()
                .is_some_and(|message| message.contains("unpaid")),
        }
    }

    fn omitting(site: Omission) -> Knobs {
        Knobs {
            omit: Some(site),
            ..Knobs::default()
        }
    }

    /// `n` constant predicates over one variable.
    fn many_predicates(n: usize) -> Model {
        let mut builder = ModelBuilder::new()
            .variable("x", 0, 1)
            .action(ActionDecl::deterministic(
                "Set",
                BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
                vec![("x", IntExpr::constant(1))],
            ))
            .initial_state(&[("x", 0)]);
        for index in 0..n {
            builder = builder.predicate(&format!("P{index}"), BoolExpr::constant(true));
        }
        builder.build().unwrap()
    }

    #[test]
    fn obligations_are_reserved_before_any_is_read() {
        // 100 000 constant predicates. One unit below the reserve is a typed refusal
        // before the obligation set is walked (4c281661 walked and collected it, then
        // built one refused result per invariant, all uncharged); at the reserve the
        // check proceeds and the footprint derivation is what runs out.
        let n = 100_000;
        let model = many_predicates(n);
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Allowed);
        let reserve = n as u64 * crate::OBLIGATION_UNITS;
        let short = Bounds::new(1 << 20, 1 << 24, 1 << 20, reserve - 1);
        assert_eq!(
            engine::run(&model, &obligations, short, Knobs::default()),
            Err(CheckError::WorkBelowObligations {
                invariants: n,
                work: reserve - 1
            })
        );
        let exact = Bounds::new(1 << 20, 1 << 24, 1 << 20, reserve);
        let report = engine::run(&model, &obligations, exact, Knobs::default()).unwrap();
        assert_eq!(*report.completeness(), Completeness::Exhausted(Bound::Work));
        assert_eq!(report.invariants().len(), n);
        assert!(panics_unpaid(|| {
            let _ = engine::run(
                &model,
                &obligations,
                generous(),
                omitting(Omission::Obligations),
            );
        }));
    }

    /// `k` binary variables, each toggled by its own action, `padding` constant
    /// variables no action reads or writes, and an invariant over the toggles: a
    /// cyclic model with `2^k` states and no reduction, so almost every transition
    /// lands on a state already in a large index.
    fn toggles(k: usize, padding: usize) -> Model {
        let mut builder = ModelBuilder::new();
        let mut everything = BoolExpr::constant(true);
        let mut names: Vec<String> = Vec::new();
        for index in 0..k {
            let name = format!("b{index}");
            builder = builder
                .variable(&name, 0, 1)
                .action(ActionDecl::deterministic(
                    &format!("T{index}"),
                    BoolExpr::constant(true),
                    vec![(
                        name.as_str(),
                        IntExpr::minus(IntExpr::constant(1), IntExpr::var(&name)),
                    )],
                ));
            everything = BoolExpr::and(everything, BoolExpr::in_range(IntExpr::var(&name), 0, 1));
            names.push(name);
        }
        for index in 0..padding {
            let name = format!("pad{index}");
            builder = builder.variable(&name, 0, 0);
            names.push(name);
        }
        let bindings: Vec<(&str, i64)> = names.iter().map(|name| (name.as_str(), 0)).collect();
        builder
            .predicate("Binary", everything)
            .initial_state(&bindings)
            .build()
            .unwrap()
    }

    #[test]
    fn every_state_index_search_is_paid() {
        // Padding the state with variables no action touches changes one thing about
        // the work: every state comparison gets longer. Each transition's index
        // lookup compares states along a path of the index, so the work the padding
        // adds per padded variable is at least one comparison unit per level per
        // transition. 4c281661 never charged a lookup that hit, and its added work
        // per padded variable is about four label copies per state — below this floor.
        let k = 10;
        let padding = 40;
        let obligations = Obligations::new(DeadlockPolicy::Allowed).invariant(0);
        let run = |padding| {
            engine::run(
                &toggles(k, padding),
                &obligations,
                generous(),
                Knobs::default(),
            )
            .unwrap()
            .stats()
        };
        let (plain, padded) = (run(0), run(padding));
        assert_eq!((plain.states, padded.states), (1 << k, 1 << k));
        let slope = (padded.work - plain.work) / padding as u64;
        let floor = plain.transitions * (bits(plain.states) as u64 - 1);
        assert!(
            slope >= floor,
            "work per padded variable {slope} below {floor}"
        );

        let model = toggles(4, 0);
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Allowed);
        assert!(panics_unpaid(|| {
            let _ = engine::run(
                &model,
                &obligations,
                generous(),
                omitting(Omission::IndexSearch),
            );
        }));
    }

    #[test]
    fn the_proviso_collection_is_paid_before_it_is_built() {
        let (_, model) = handmade::all()
            .into_iter()
            .find(|(id, _)| *id == "ignoring")
            .unwrap();
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
        let report = engine::run(&model, &obligations, generous(), Knobs::default()).unwrap();
        assert!(
            report.stats().full_proviso > 0,
            "the model needs the proviso"
        );
        assert!(panics_unpaid(|| {
            let _ = engine::run(
                &model,
                &obligations,
                generous(),
                omitting(Omission::ProvisoCollect),
            );
        }));
        // Every budget below the run's own spend is refused typed.
        let spend = report.stats().work;
        for work in 0..spend {
            let bounded = Bounds::new(1 << 20, 1 << 24, 1 << 20, work);
            match engine::run(&model, &obligations, bounded, Knobs::default()) {
                Ok(report) => {
                    assert_eq!(*report.completeness(), Completeness::Exhausted(Bound::Work))
                }
                Err(CheckError::WorkBelowObligations { .. }) => {}
                Err(other) => panic!("work {work}: {other:?}"),
            }
        }
    }

    #[test]
    fn finalization_is_paid_before_the_report_is_assembled() {
        // cr-1wuzry: a budget that covers the whole search but not the report's
        // assembly (witness records, projections, the terminal sort) gives a typed
        // `ResourceExhausted`, never a report built past the bound. One unit below the
        // unbounded run's spend is exactly such a budget: the search completes (every
        // state is stored), and finalization is refused.
        for id in ["philosophers", "independent", "undefined-invariant"] {
            let (_, model) = handmade::all()
                .into_iter()
                .find(|(name, _)| *name == id)
                .unwrap();
            let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
            let full = engine::run(&model, &obligations, generous(), Knobs::default()).unwrap();
            assert_eq!(*full.completeness(), Completeness::Complete, "{id}");
            let spend = full.stats().work;
            let short = Bounds::new(1 << 20, 1 << 24, 1 << 20, spend - 1);
            let report = engine::run(&model, &obligations, short, Knobs::default()).unwrap();
            assert_eq!(
                *report.completeness(),
                Completeness::Exhausted(Bound::Work),
                "{id}"
            );
            assert_eq!(
                report.stats().states,
                full.stats().states,
                "{id}: the search completed"
            );
            assert!(
                report.stats().work < spend,
                "{id}: nothing ran past the bound"
            );
            assert!(report.projections().is_empty() && report.witness().nodes().is_empty());
            assert!(!report.witness().is_complete());
            for result in report.invariants() {
                assert!(matches!(
                    result.outcome(),
                    crate::report::InvariantOutcome::Inconclusive(
                        crate::report::Unresolved::ResourceExhausted {
                            tripped: Bound::Work,
                            ..
                        }
                    )
                ));
            }
            assert!(matches!(
                report.deadlock(),
                crate::report::DeadlockOutcome::Inconclusive(_)
            ));
        }
    }
}
