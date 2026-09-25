//! C005 evidence: baseline DPOR preserves finite safety verdicts
//! (`notes/plan/docs/18_CLAIMS_MATRIX.md`, row C005; required evidence "exhaustive
//! differential corpus").
//!
//! The reduced engine ([`dpor::check`]) is run against the unreduced oracle
//! (`continuum-engine-reference`: breadth-first exploration of every reachable state
//! and its own invariant and deadlock check) over three corpora, and each model is
//! compared on four counts (`tests/support/differential.rs`): verdicts, the visible
//! projection of the reachable states and the terminal states, counterexample
//! replay, and acceptance of the reduction witness by the independent checker
//! ([`dpor::check_witness`]).
//!
//! 1. **Dossier models** (`c005-dos-*`): every model under `notes/plan/examples/`
//!    with a committed run configuration, lowered through `continuum-cml-elab`, plus
//!    the Die Hard corpus port (TV-009). A dossier model the front end refuses is
//!    recorded with its typed refusal code rather than skipped silently.
//! 2. **Adversarial models** (`c005-adv-*`, `tests/support/handmade.rs`): one model
//!    per naive shortcut a reducer could take.
//! 3. **Generated corpus** (`c005-gen-*`, `tests/support/corpus.rs`): seeds
//!    `0..CORPUS_SEEDS` of a deterministic generator of small concurrent models, one
//!    ledger line per seed and a summary.
//!
//! Every line renders into the committed ledger
//! `tests/golden/c005_differential.evidence.txt`, compared byte for byte, so a change
//! in any count is a visible diff. Regenerate with `DPOR_BLESS=1 cargo test -p
//! continuum-engine-dpor --test c005_differential`, and review the diff.
//!
//! The boundary evidence (`c005-bnd-*`), the determinism check, and the metamorphic
//! relation (alpha-renaming, docs/19 §3) are separate tests below. Mutation evidence
//! is `src/mutation.rs` with its own ledger.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use core::fmt::Write as _;
use std::collections::BTreeMap;

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{InitPath, lower_configured_with_init};
use continuum_cml_elab::{Limits, elaborate_source, lower};
use continuum_engine_dpor as dpor;
use continuum_engine_reference as reference;
use continuum_model_core::model::Model;

#[path = "support/corpus.rs"]
mod corpus;
#[path = "support/differential.rs"]
mod differential;
#[path = "support/handmade.rs"]
mod handmade;

/// The generated corpus: seeds `0..CORPUS_SEEDS`.
const CORPUS_SEEDS: u64 = 1000;

fn dossier(rel: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan/").to_owned() + rel;
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Elaborate and lower one dossier model: the model, or the front end's typed
/// refusal code.
fn lower_dossier(ctm: &str, config: Option<&str>, one_epoch: bool) -> Result<Model, String> {
    let norm = elaborate_source(&dossier(ctm))
        .map_err(|e| format!("elaboration refused: {}", e.code()))?;
    match config {
        None => lower(&norm).map_err(|e| format!("lowering refused: {}", e.code())),
        Some(config) => {
            let mut text = dossier(config);
            if one_epoch {
                // The committed configuration with `Nat` bounded to 0: one epoch, one
                // independent component of claim A (the scope
                // `pr16-impl02-pos-02-one-epoch` of continuum-cml-elab checks).
                assert!(
                    text.contains("\"max\": 1"),
                    "the committed configuration bounds Nat to 1"
                );
                text = text.replacen("\"max\": 1", "\"max\": 0", 1);
            }
            let config = RunConfig::parse(text.as_bytes()).expect("committed configuration reads");
            lower_configured_with_init(&norm, &config, Limits::default(), InitPath::Pinned)
                .0
                .map(|configured| configured.model().clone())
                .map_err(|e| format!("lowering refused: {}", e.code()))
        }
    }
}

/// The dossier models: every `notes/plan/examples/*.ctm`, the durable register also
/// at one epoch, and the TV-009 port.
fn dossier_models() -> Vec<(&'static str, String, Result<Model, String>)> {
    let entries: [(&str, &str, Option<&str>, bool); 6] = [
        (
            "c005-dos-01-abstract-register",
            "examples/abstract_register.ctm",
            Some("examples/abstract_register.run-config.json"),
            false,
        ),
        (
            "c005-dos-02-durable-register-claim-a",
            "examples/durable_register.ctm",
            Some("examples/durable_register.run-config.json"),
            false,
        ),
        (
            "c005-dos-03-durable-register-one-epoch",
            "examples/durable_register.ctm",
            Some("examples/durable_register.run-config.json"),
            true,
        ),
        (
            "c005-dos-04-replicated-register",
            "examples/replicated_register.ctm",
            None,
            false,
        ),
        (
            "c005-dos-05-forge-ack-protocol",
            "examples/forge_ack_protocol.ctm",
            None,
            false,
        ),
        (
            "c005-dos-06-diehard",
            "corpus/tla-examples/ports/TV-009/DieHard.ctm",
            None,
            false,
        ),
    ];
    entries
        .into_iter()
        .map(|(id, ctm, config, one_epoch)| {
            let source = match (config, one_epoch) {
                (Some(config), false) => format!("notes/plan/{ctm} with notes/plan/{config}"),
                (Some(config), true) => {
                    format!("notes/plan/{ctm} with notes/plan/{config}, Nat max 0 (one epoch)")
                }
                (None, _) => format!("notes/plan/{ctm}"),
            };
            let lowered = lower_dossier(ctm, config, one_epoch)
                .and_then(|model| differential::fits(&model).map(|()| model));
            (id, source, lowered)
        })
        .collect()
}

/// Everything one model contributes to the ledger and to the totals.
struct Row {
    agrees: bool,
    engine_error: bool,
    obligations: usize,
    unreduced: usize,
    reduced: usize,
    visits: usize,
    unreduced_transitions: u64,
    reduced_transitions: u64,
    traces: usize,
    persistence: usize,
    sleep: usize,
    narrow_unreduced: usize,
    narrow_reduced: usize,
    made: BTreeMap<&'static str, usize>,
    inconclusive: bool,
    text: String,
}

/// One reduced check against the oracle under `scope`.
fn one_scope(
    model: &Model,
    scope: &differential::Scope,
) -> (
    dpor::Report,
    differential::Oracle,
    differential::Comparison,
    Option<dpor::CheckedWitness>,
) {
    let oracle = differential::oracle_of(model, scope);
    let obligations = scope.dpor();
    let report = dpor::check(model, &obligations, differential::dpor_bounds())
        .expect("obligations are declared");
    let comparison = differential::compare_of(model, scope, &oracle, &report);
    let checked = dpor::check_witness(model, &obligations, report.witness(), 1 << 40).ok();
    (report, oracle, comparison, checked)
}

fn agrees(comparison: &differential::Comparison) -> bool {
    !comparison.differential_detects() && comparison.checker_rejected.is_none()
}

/// Run one model twice: under every predicate with deadlock a defect (the corpus
/// default), and under its first predicate alone with terminal states allowed (a
/// smaller visible set, and the other completion policy).
fn run_one(model: &Model) -> Row {
    let (report, oracle, comparison, checked) =
        one_scope(model, &differential::Scope::every(model));
    let narrow = differential::Scope {
        invariants: (0..model.predicates().len().min(1)).collect(),
        defect: false,
    };
    let (narrow_report, narrow_oracle, narrow_comparison, _) = one_scope(model, &narrow);
    let wide_agrees = agrees(&comparison);
    let narrow_agrees = agrees(&narrow_comparison);
    let stats = report.stats();
    let verdicts = differential::dpor_verdicts(&report)
        .map_or_else(|| "engine-error".to_owned(), |list| list.join(","));
    let narrow_verdicts = differential::dpor_verdicts(&narrow_report)
        .map_or_else(|| "engine-error".to_owned(), |list| list.join(","));
    let text = format!(
        "verdicts={verdicts} agree={} states={}/{} visits={} transitions={}/{} traces={} declined={}p+{}s | first-invariant,allowed: verdicts={narrow_verdicts} agree={} states={}/{}",
        if wide_agrees { "yes" } else { "NO" },
        stats.states,
        oracle.states,
        stats.visits,
        stats.transitions,
        oracle.transitions,
        comparison.traces_replayed,
        checked.as_ref().map_or(0, |c| c.declined_by_persistence),
        checked.as_ref().map_or(0, |c| c.declined_by_sleep),
        if narrow_agrees { "yes" } else { "NO" },
        narrow_report.stats().states,
        narrow_oracle.states,
    );
    let all_agree = wide_agrees && narrow_agrees;
    let mut made: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (category, count) in comparison.made.iter().chain(narrow_comparison.made.iter()) {
        *made.entry(*category).or_default() += count;
    }
    // A faulted model is compared on the fault only: both engines halt at the first
    // fault they reach, so neither state count means anything, and neither enters a
    // state total.
    let faulted = comparison.engine_error;
    let text = if faulted {
        format!(
            "faulted: both engines report an engine error; the reduced fault is one of {} reachable faults agree={}",
            oracle.faults.len(),
            if all_agree { "yes" } else { "NO" }
        )
    } else {
        text
    };
    let zero_if_faulted = |count: usize| if faulted { 0 } else { count };
    Row {
        made,
        inconclusive: comparison.inconclusive || narrow_comparison.inconclusive,
        agrees: all_agree,
        engine_error: comparison.engine_error,
        // A faulted model compares only the typed engine error, never obligations.
        obligations: if comparison.engine_error {
            0
        } else {
            model.predicates().len() + 1 + narrow.invariants.len() + 1
        },
        unreduced: zero_if_faulted(oracle.states),
        reduced: zero_if_faulted(stats.states),
        visits: zero_if_faulted(stats.visits),
        unreduced_transitions: if faulted { 0 } else { oracle.transitions },
        reduced_transitions: if faulted { 0 } else { stats.transitions },
        traces: comparison.traces_replayed + narrow_comparison.traces_replayed,
        persistence: checked.as_ref().map_or(0, |c| c.declined_by_persistence),
        sleep: checked.as_ref().map_or(0, |c| c.declined_by_sleep),
        narrow_unreduced: zero_if_faulted(narrow_oracle.states),
        narrow_reduced: zero_if_faulted(narrow_report.stats().states),
        text: if all_agree {
            text
        } else {
            format!(
                "{text}\n  disagreements: {:?} {:?} checker: {:?} {:?}",
                comparison.disagreements,
                narrow_comparison.disagreements,
                comparison.checker_rejected,
                narrow_comparison.checker_rejected
            )
        },
    }
}

#[derive(Default)]
struct Totals {
    models: usize,
    agreements: usize,
    engine_errors: usize,
    obligations: usize,
    unreduced: usize,
    reduced: usize,
    visits: usize,
    unreduced_transitions: u64,
    reduced_transitions: u64,
    fewer_states: usize,
    traces: usize,
    persistence: usize,
    sleep: usize,
    narrow_unreduced: usize,
    narrow_reduced: usize,
    made: BTreeMap<&'static str, usize>,
    inconclusive: usize,
}

impl Totals {
    fn add(&mut self, row: &Row) {
        self.models += 1;
        self.agreements += usize::from(row.agrees);
        self.engine_errors += usize::from(row.engine_error);
        self.obligations += row.obligations;
        self.unreduced += row.unreduced;
        self.reduced += row.reduced;
        self.visits += row.visits;
        self.unreduced_transitions += row.unreduced_transitions;
        self.reduced_transitions += row.reduced_transitions;
        self.fewer_states += usize::from(row.reduced < row.unreduced);
        self.traces += row.traces;
        self.persistence += row.persistence;
        self.sleep += row.sleep;
        self.narrow_unreduced += row.narrow_unreduced;
        self.narrow_reduced += row.narrow_reduced;
        self.inconclusive += usize::from(row.inconclusive);
        for (category, count) in &row.made {
            *self.made.entry(*category).or_default() += count;
        }
    }

    /// No category of comparison is vacuous, and faulted and inconclusive models are
    /// counted and stay a minority: a corpus that compared nothing, or whose models
    /// mostly fault, is not evidence and fails here.
    fn assert_not_vacuous(&self, id: &str, categories: &[&str]) {
        for category in categories {
            assert!(
                self.made.get(category).copied().unwrap_or(0) > 0,
                "{id}: no {category} comparison was made"
            );
        }
        assert_eq!(self.agreements, self.models, "{id}: every model agrees");
        // At most a tenth (and at most one in a group of up to ten, the adversarial
        // group's one designed fault).
        let bound = self.models.max(10);
        assert!(
            self.engine_errors * 10 <= bound,
            "{id}: too many faulted models"
        );
        assert!(
            self.inconclusive * 10 <= bound,
            "{id}: too many inconclusive models"
        );
    }

    fn render(&self, id: &str, ledger: &mut String) {
        writeln!(
            ledger,
            "\n[{id}]\nmodels: {}\nagreements under both scopes (verdicts, visible projections, terminal states, trace replay, witness): {}/{}\n\
             comparisons made: {}\n\
             models with an inconclusive verdict (both engines agree on it): {}\n\
             engine-error models (both engines fault; only the fault's reachability is compared; excluded from state totals): {}\n\
             obligations compared on the other models (invariants + deadlock, both scopes): {}\n\
             witnesses accepted by the checker: {}\n\
             states (every invariant, deadlock a defect): unreduced={} reduced={} visits={}\n\
             states (first invariant, terminal states allowed): unreduced={} reduced={}\n\
             transitions: unreduced state-changing (action, target) steps={} reduced label firings={}\n\
             models with strictly fewer stored states: {}\ncounterexamples replayed: {}\n\
             declined enabled labels: by persistence={} by sleep={}",
            self.models,
            self.agreements,
            self.models,
            differential::CATEGORIES
                .iter()
                .map(|c| format!("{c}={}", self.made.get(c).copied().unwrap_or(0)))
                .collect::<Vec<_>>()
                .join(" "),
            self.inconclusive,
            self.engine_errors,
            self.obligations,
            self.models - self.engine_errors,
            self.unreduced,
            self.reduced,
            self.visits,
            self.narrow_unreduced,
            self.narrow_reduced,
            self.unreduced_transitions,
            self.reduced_transitions,
            self.fewer_states,
            self.traces,
            self.persistence,
            self.sleep,
        )
        .unwrap();
    }
}

#[test]
fn c005_exhaustive_differential_corpus() {
    let mut ledger = String::from(
        "# C005 exhaustive differential corpus (bn-voq4): continuum-engine-dpor against the\n\
         # unreduced continuum-engine-reference. Per model: verdicts (invariants then deadlock),\n\
         # stored states reduced/unreduced, visits, transitions reduced (label firings) /\n\
         # unreduced (state-changing (action, target) steps), replayed counterexamples, and\n\
         # enabled labels the checker accepted as declined; then the same model under its\n\
         # first invariant alone with terminal states allowed.\n\
         # Regenerate: DPOR_BLESS=1 cargo test -p continuum-engine-dpor --test c005_differential\n",
    );
    let mut all = Totals::default();

    let mut dossier_totals = Totals::default();
    for (id, source, lowered) in dossier_models() {
        write!(ledger, "\n[{id}]\nsource: {source}\n").unwrap();
        match lowered {
            Err(refusal) => writeln!(ledger, "not checked: {refusal}").unwrap(),
            Ok(model) => {
                let row = run_one(&model);
                writeln!(
                    ledger,
                    "model: variables={} actions={} predicates={}\n{}",
                    model.arity(),
                    model.actions().len(),
                    model.predicates().len(),
                    row.text
                )
                .unwrap();
                assert!(row.agrees, "{id}: {}", row.text);
                dossier_totals.add(&row);
                all.add(&row);
            }
        }
    }
    dossier_totals.render("c005-dos-summary", &mut ledger);

    let mut adversarial = Totals::default();
    for (id, model) in handmade::all() {
        let row = run_one(&model);
        write!(ledger, "\n[c005-adv-{id}]\n{}\n", row.text).unwrap();
        assert!(row.agrees, "{id}: {}", row.text);
        adversarial.add(&row);
        all.add(&row);
    }
    adversarial.render("c005-adv-summary", &mut ledger);

    let mut generated = Totals::default();
    let mut lines = String::new();
    for seed in 0..CORPUS_SEEDS {
        let model = corpus::generate(seed);
        let row = run_one(&model.model);
        writeln!(
            lines,
            "seed={seed} family={} processes={} wraps={} {}",
            if model.loose { "loose" } else { "tangled" },
            model.processes,
            u8::from(model.wraps),
            row.text
        )
        .unwrap();
        assert!(row.agrees, "seed {seed}: {}", row.text);
        generated.add(&row);
        all.add(&row);
    }
    generated.render("c005-gen-summary", &mut ledger);
    write!(ledger, "\n[c005-gen-models]\n{lines}").unwrap();
    all.render("c005-all-summary", &mut ledger);

    // Loud on vacuity: every category is compared in the corpus as a whole and in
    // the generated part; the dossier and adversarial parts compare what they hold.
    all.assert_not_vacuous("c005-all", &differential::CATEGORIES);
    generated.assert_not_vacuous("c005-gen", &differential::CATEGORIES);
    dossier_totals.assert_not_vacuous(
        "c005-dos",
        &["verdicts", "projections", "traces", "witness"],
    );
    adversarial.assert_not_vacuous(
        "c005-adv",
        &[
            "verdicts",
            "projections",
            "terminals",
            "traces",
            "witness",
            "fault",
        ],
    );

    // The corpus must exercise the reduction, not only agree with the oracle.
    assert!(
        generated.fewer_states * 4 >= generated.models,
        "the reduction is too rarely active"
    );
    assert!(generated.persistence > 0 && generated.sleep > 0);

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/c005_differential.evidence.txt"
    );
    if std::env::var_os("DPOR_BLESS").is_some() {
        std::fs::write(path, &ledger).expect("write golden");
    }
    let golden = std::fs::read_to_string(path).expect("read golden");
    assert_eq!(ledger, golden, "the C005 ledger drifted from {path}");
}

// ---------------------------------------------------------------------------
// boundary evidence
// ---------------------------------------------------------------------------

fn obligations_of(model: &Model) -> dpor::Obligations {
    dpor::Obligations::every_predicate(model, dpor::DeadlockPolicy::Defect)
}

/// `c005-bnd-01`: a bound that stops the search before it settles an invariant that
/// holds is `Inconclusive(ResourceExhausted)`, never `Holds` — for every bound kind.
#[test]
fn c005_bnd_01_a_bound_is_never_a_pass() {
    let (_, model) = handmade::all()
        .into_iter()
        .find(|(id, _)| *id == "independent")
        .unwrap();
    // Terminal states are allowed here: the model's one invariant holds, so every
    // obligation is established by the complete run.
    let obligations = dpor::Obligations::every_predicate(&model, dpor::DeadlockPolicy::Allowed);
    let full = dpor::check(&model, &obligations, differential::dpor_bounds()).unwrap();
    assert_eq!(full.verdict(), dpor::Verdict::Established);
    for (bounds, tripped) in [
        (
            dpor::Bounds::new(3, 1 << 20, 1 << 20, 1 << 40),
            dpor::Bound::States,
        ),
        (
            dpor::Bounds::new(1 << 20, 2, 1 << 20, 1 << 40),
            dpor::Bound::Transitions,
        ),
        (
            dpor::Bounds::new(1 << 20, 1 << 20, 2, 1 << 40),
            dpor::Bound::Depth,
        ),
        (
            dpor::Bounds::new(1 << 20, 1 << 20, 1 << 20, dpor::OBLIGATION_UNITS + 200),
            dpor::Bound::Work,
        ),
    ] {
        let report = dpor::check(&model, &obligations, bounds).unwrap();
        assert_eq!(
            *report.completeness(),
            dpor::Completeness::Exhausted(tripped)
        );
        for result in report.invariants() {
            assert!(
                matches!(
                    result.outcome(),
                    dpor::InvariantOutcome::Inconclusive(dpor::Unresolved::ResourceExhausted { tripped: t, .. }) if *t == tripped
                ),
                "{tripped:?}: {:?}",
                result.outcome()
            );
        }
        assert_eq!(report.verdict(), dpor::Verdict::Inconclusive);
        // An incomplete witness justifies nothing.
        assert_eq!(
            dpor::check_witness(&model, &obligations, report.witness(), 1 << 40),
            Err(dpor::WitnessDefect::Incomplete)
        );
    }
    // A work bound below the obligations' reserve is refused before any is read; one
    // that covers the reserve but not the footprints refuses before any state.
    assert_eq!(
        dpor::check(
            &model,
            &obligations,
            dpor::Bounds::new(1 << 20, 1 << 20, 1 << 20, 1)
        ),
        Err(dpor::CheckError::WorkBelowObligations {
            invariants: 1,
            work: 1
        })
    );
    let report = dpor::check(
        &model,
        &obligations,
        dpor::Bounds::new(1 << 20, 1 << 20, 1 << 20, dpor::OBLIGATION_UNITS),
    )
    .unwrap();
    assert_eq!(
        *report.completeness(),
        dpor::Completeness::Exhausted(dpor::Bound::Work)
    );
    assert_eq!(report.stats().states, 0);
}

/// `c005-bnd-02`: a violation found inside a bounded run is a genuine refutation, and
/// its trace replays in the reference engine's model.
#[test]
fn c005_bnd_02_a_violation_inside_a_bound_is_genuine() {
    let (_, model) = handmade::all()
        .into_iter()
        .find(|(id, _)| *id == "wakeup")
        .unwrap();
    let obligations = obligations_of(&model);
    let oracle = differential::oracle(&model);
    let mut found = false;
    for states in 1..=8 {
        let report = dpor::check(
            &model,
            &obligations,
            dpor::Bounds::new(states, 1 << 20, 1 << 20, 1 << 40),
        )
        .unwrap();
        for result in report.invariants() {
            match result.outcome() {
                dpor::InvariantOutcome::Violated { trace, .. } => {
                    found = true;
                    let trace = trace.as_ref().expect("the work bound pays for the trace");
                    assert_eq!(
                        model.evaluate_predicate(result.index(), trace.end()),
                        Ok(false)
                    );
                    assert!(oracle.reachable.contains(trace.end()));
                    let mut here = trace.start().clone();
                    for step in trace.steps() {
                        let row = model.successors(&here).unwrap();
                        assert!(row.iter().any(|s| s.action() == step.action() && s.target() == step.target()));
                        here = step.target().clone();
                    }
                }
                dpor::InvariantOutcome::Holds { .. } => {
                    assert_eq!(*report.completeness(), dpor::Completeness::Complete);
                }
                dpor::InvariantOutcome::Undefined(_) | dpor::InvariantOutcome::Inconclusive(_) => {}
            }
        }
    }
    assert!(found, "some bound finds the violation");
}

/// `c005-bnd-03`: an evaluation fault on a reachable interleaving is an engine error
/// for the reduced engine exactly when it is for the oracle — never a verdict.
#[test]
fn c005_bnd_03_a_reachable_fault_is_an_engine_error() {
    let (_, model) = handmade::all()
        .into_iter()
        .find(|(id, _)| *id == "fault")
        .unwrap();
    assert!(matches!(
        reference::explore(&model, differential::reference_bounds()),
        Err(reference::ExplorationError::Evaluation { .. })
    ));
    let report = dpor::check(&model, &obligations_of(&model), differential::dpor_bounds()).unwrap();
    assert!(matches!(
        report.completeness(),
        dpor::Completeness::Faulted(dpor::EngineFault::Evaluation { .. })
    ));
    assert_eq!(report.verdict(), dpor::Verdict::Inconclusive);
    for result in report.invariants() {
        assert!(matches!(
            result.outcome(),
            dpor::InvariantOutcome::Inconclusive(dpor::Unresolved::EngineError(_))
        ));
    }
}

/// `c005-bnd-05`: the work bound is swept one unit at a time. A run the bound stops
/// is never `Holds` or `Free`; the first bound that completes the search is exactly
/// the unbounded run's spend (every unit is charged before its work, so nothing is
/// done unpaid), and it gives the unbounded run's verdicts with every counterexample
/// path present: a path is charged when its state is stored, not built unpaid later.
#[test]
fn c005_bnd_05_every_work_bound_is_typed() {
    let (_, model) = handmade::all()
        .into_iter()
        .find(|(id, _)| *id == "philosophers")
        .unwrap();
    let obligations = obligations_of(&model);
    let full = dpor::check(&model, &obligations, differential::dpor_bounds()).unwrap();
    let full_verdicts = differential::dpor_verdicts(&full);
    let mut work = 0_u64;
    loop {
        let report = match dpor::check(
            &model,
            &obligations,
            dpor::Bounds::new(1 << 18, 1 << 24, 1 << 20, work),
        ) {
            Ok(report) => report,
            Err(dpor::CheckError::WorkBelowObligations { .. }) => {
                assert!(work < dpor::OBLIGATION_UNITS * obligations.invariants().len() as u64);
                work += 1;
                continue;
            }
            Err(other) => panic!("work {work}: {other:?}"),
        };
        match report.completeness() {
            dpor::Completeness::Exhausted(dpor::Bound::Work) => {
                for result in report.invariants() {
                    assert!(!matches!(
                        result.outcome(),
                        dpor::InvariantOutcome::Holds { .. }
                    ));
                }
                assert!(!matches!(
                    report.deadlock(),
                    dpor::DeadlockOutcome::Free { .. }
                ));
                if let dpor::DeadlockOutcome::Deadlocked { states } = report.deadlock() {
                    assert!(states.iter().all(|deadlock| deadlock.trace().is_some()));
                }
            }
            dpor::Completeness::Complete => {
                assert_eq!(
                    differential::dpor_verdicts(&report),
                    full_verdicts,
                    "work {work}"
                );
                let dpor::DeadlockOutcome::Deadlocked { states } = report.deadlock() else {
                    panic!("the circular wait is found");
                };
                assert!(states.iter().all(|deadlock| deadlock.trace().is_some()));
                break;
            }
            other => panic!("work {work}: {other:?}"),
        }
        work += 1;
    }
    assert_eq!(
        report_work(&model, &obligations),
        work,
        "the first sufficient bound is the unbounded run's spend"
    );
}

fn report_work(model: &Model, obligations: &dpor::Obligations) -> u64 {
    dpor::check(model, obligations, differential::dpor_bounds())
        .unwrap()
        .stats()
        .work
}

/// `c005-bnd-04`: an obligation naming an undeclared predicate is refused before any
/// state is examined.
#[test]
fn c005_bnd_04_an_unknown_predicate_is_refused() {
    let (_, model) = handmade::all().into_iter().next().unwrap();
    let obligations = dpor::Obligations::new(dpor::DeadlockPolicy::Allowed).invariant(7);
    assert_eq!(
        dpor::check(&model, &obligations, differential::dpor_bounds()),
        Err(dpor::CheckError::UnknownPredicate {
            index: 7,
            declared: 1
        })
    );
}

/// `c005-bnd-06`: faults are compared by reachability, not by existence alone. A
/// model with two reachable faults, on two interleavings (`A` then `B`, or `C` then
/// `B`; `B` writes `x + 1` into `z`, whose domain is `0..=1`): the unreduced walk
/// finds both, and the reduced engine halts at one of them, which the comparison
/// accepts. The negative control is a report from a twin model that starts at
/// `z == 1`: its fault state faults in this model too (the check 4c281661 made, "its
/// row fails to evaluate", accepts it) but is not reachable here, so the comparison
/// must refuse it in the `fault` category.
#[test]
fn c005_bnd_06_a_reduced_fault_must_be_one_of_the_reachable_faults() {
    use continuum_model_core::expr::{BoolExpr, CmpOp, IntExpr};
    use continuum_model_core::model::{ActionDecl, ModelBuilder};
    let eq = |name: &str, value: i64| {
        BoolExpr::compare(CmpOp::Eq, IntExpr::var(name), IntExpr::constant(value))
    };
    let build = |z0: i64| {
        ModelBuilder::new()
            .variable("x", 0, 2)
            .variable("z", 0, 1)
            .variable("pb", 0, 1)
            .action(ActionDecl::deterministic(
                "A",
                eq("x", 0),
                vec![("x", IntExpr::constant(1))],
            ))
            .action(ActionDecl::deterministic(
                "C",
                eq("x", 0),
                vec![("x", IntExpr::constant(2))],
            ))
            .action(ActionDecl::deterministic(
                "B",
                BoolExpr::and(
                    eq("pb", 0),
                    BoolExpr::compare(CmpOp::Gt, IntExpr::var("x"), IntExpr::constant(0)),
                ),
                vec![
                    ("pb", IntExpr::constant(1)),
                    ("z", IntExpr::plus(IntExpr::var("x"), IntExpr::constant(1))),
                ],
            ))
            .predicate("Any", BoolExpr::constant(true))
            .initial_state(&[("x", if z0 == 0 { 0 } else { 1 }), ("z", z0), ("pb", 0)])
            .build()
            .unwrap()
    };
    let model = build(0);
    let faults = differential::fault_walk(&model);
    assert_eq!(
        faults.len(),
        2,
        "x == 1 and x == 2 both overflow z: {faults:?}"
    );
    let oracle = differential::oracle(&model);
    assert!(oracle.verdicts.is_none());
    let report = dpor::check(&model, &obligations_of(&model), differential::dpor_bounds()).unwrap();
    let comparison = differential::compare(&model, &oracle, &report);
    assert!(comparison.engine_error && comparison.disagreements.is_empty());
    assert_eq!(comparison.made.get("fault"), Some(&1));

    // Negative control: a fault that faults here but is not reachable here.
    let twin = build(1);
    let foreign = dpor::check(&twin, &obligations_of(&twin), differential::dpor_bounds()).unwrap();
    let dpor::Completeness::Faulted(dpor::EngineFault::Evaluation { state, .. }) =
        foreign.completeness()
    else {
        panic!("the twin faults at once");
    };
    assert!(
        model.successors(state).is_err(),
        "the old check would accept it"
    );
    assert!(!faults.contains(state), "it is not reachable in the model");
    let refused = differential::compare(&model, &oracle, &foreign);
    assert!(
        refused.failed.contains("fault"),
        "{:?}",
        refused.disagreements
    );
    // And the initial state is a reachable state that does not fault.
    let benign = model.initial_states().first().unwrap();
    assert!(model.successors(benign).is_ok() && !faults.contains(benign));
}

/// `c005-bnd-07`: terminal states are compared under the terminal-allowed policy too.
/// The reference engine reports only a count there, so the oracle's terminal set is
/// taken from the raw exploration and compared with the checker's own list of stored
/// terminal states; a reduced run that lost a terminal state whose projection some
/// live state shares would fail this even though no verdict reads it.
#[test]
fn c005_bnd_07_terminal_states_are_compared_when_they_are_allowed() {
    let (_, model) = handmade::all()
        .into_iter()
        .find(|(id, _)| *id == "philosophers")
        .unwrap();
    let narrow = differential::Scope {
        invariants: Vec::new(),
        defect: false,
    };
    let oracle = differential::oracle_of(&model, &narrow);
    assert!(
        !oracle.terminals.is_empty(),
        "the circular wait is terminal"
    );
    assert!(
        oracle.deadlocks.is_empty(),
        "the reference reports only a count"
    );
    let report = dpor::check(&model, &narrow.dpor(), differential::dpor_bounds()).unwrap();
    let comparison = differential::compare_of(&model, &narrow, &oracle, &report);
    assert!(
        comparison.disagreements.is_empty(),
        "{:?}",
        comparison.disagreements
    );
    assert!(comparison.made.get("terminals").copied().unwrap_or(0) > 0);
    let checked = dpor::check_witness(&model, &narrow.dpor(), report.witness(), 1 << 40).unwrap();
    assert_eq!(
        checked
            .deadlocks
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        oracle.terminals
    );
}

// ---------------------------------------------------------------------------
// determinism and the metamorphic relation
// ---------------------------------------------------------------------------

/// Two runs over one model are equal, witness included (INV-005).
#[test]
fn c005_reduced_runs_are_deterministic() {
    for seed in 0..50 {
        let model = corpus::generate(seed).model;
        let obligations = obligations_of(&model);
        let first = dpor::check(&model, &obligations, differential::dpor_bounds()).unwrap();
        let second = dpor::check(&model, &obligations, differential::dpor_bounds()).unwrap();
        assert_eq!(first, second, "seed {seed}");
    }
}

/// Rebuild `model` with every action renamed so that canonical action order is
/// reversed: labels, seeds, and exploration order all change.
fn renamed(model: &Model) -> Model {
    use continuum_model_core::model::{ActionDecl, ModelBuilder};
    let count = model.actions().len();
    let mut builder = ModelBuilder::new();
    for variable in model.variables() {
        builder = builder.variable(
            variable.name().as_str(),
            variable.domain().lo(),
            variable.domain().hi(),
        );
    }
    for (index, action) in model.actions().iter().enumerate() {
        let name = format!("r{:04}_{}", count - index, action.name());
        let outcomes: Vec<Vec<(&str, continuum_model_core::expr::IntExpr)>> = action
            .outcomes()
            .iter()
            .map(|outcome| {
                outcome
                    .assignments()
                    .iter()
                    .map(|a| (a.variable().as_str(), a.value().clone()))
                    .collect()
            })
            .collect();
        builder = builder.action(ActionDecl::enumerated(
            &name,
            action.guard().clone(),
            outcomes,
        ));
    }
    for predicate in model.predicates() {
        builder = builder.predicate(predicate.name().as_str(), predicate.body().clone());
    }
    for state in model.initial_states() {
        let bindings: Vec<(&str, i64)> = model
            .variables()
            .iter()
            .zip(state.as_slice())
            .map(|(variable, value)| (variable.name().as_str(), *value))
            .collect();
        builder = builder.initial_state(&bindings);
    }
    builder.build().unwrap()
}

/// Metamorphic relation "alpha-renaming" (docs/19 §3): renaming every action so
/// that the canonical action order reverses changes the reducer's label table,
/// its seed choice, and its exploration order, and must change no verdict, no
/// visible reachable projection, and no terminal state.
#[test]
fn c005_alpha_renaming_preserves_every_verdict_and_projection() {
    let mut models: Vec<Model> = handmade::all()
        .into_iter()
        .map(|(_, model)| model)
        .collect();
    models.extend((0..200).map(|seed| corpus::generate(seed).model));
    for model in models {
        let twin = renamed(&model);
        let obligations = obligations_of(&model);
        let left = dpor::check(&model, &obligations, differential::dpor_bounds()).unwrap();
        let right = dpor::check(&twin, &obligations, differential::dpor_bounds()).unwrap();
        assert_eq!(
            differential::dpor_verdicts(&left),
            differential::dpor_verdicts(&right)
        );
        if differential::dpor_verdicts(&left).is_none() {
            // A faulted run stops at the first fault it meets, which depends on the
            // order it explores in: only the typed engine error is comparable.
            continue;
        }
        assert_eq!(left.projections(), right.projections());
        let terminal = |report: &dpor::Report| -> Vec<continuum_model_core::model::State> {
            match report.deadlock() {
                dpor::DeadlockOutcome::Deadlocked { states } => {
                    states.iter().map(|d| d.state().clone()).collect()
                }
                _ => Vec::new(),
            }
        };
        assert_eq!(terminal(&left), terminal(&right));
        assert!(dpor::check_witness(&twin, &obligations, right.witness(), 1 << 40).is_ok());
    }
}
