//! The harness driver: a corpus, a set of engines, the lane table, one run.
//!
//! For each fixture the driver builds the model, asks every plugged-in engine once,
//! and compares each lane's subject with its oracle ([`super::compare`]). A lane with
//! a disagreement:
//!
//! 1. **minimizes** the fixture: delta debugging over its declarations keeps a subset
//!    only while the same field still disagrees ([`super::minimize`]);
//! 2. **emits** a defect report pinning the engines, epochs, budget, the original and
//!    the minimized model by content identity ([`super::defect`]);
//! 3. **halts** the lane's claims (and, for an engine fault, every claim of every lane
//!    the faulting slot serves): each enters the run's [`QuarantineLedger`], which
//!    [`super::claims::halts`] then holds against the committed register and the
//!    claims registry.
//!
//! Everything is ordered (`BTreeMap`, sorted vectors, the corpus order), so one corpus
//! and one engine set give a byte-identical [`RunReport::summary`].
//!
//! Decision records: RFC 0030 ("Quarantine": durable, cleared only by evidence),
//! RFC 0026 (defect emission) and ADR-0029 (oracle tooling does not ship).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::claims::{Lane, Quarantine, QuarantineLedger};
use super::compare::{Comparison, Disagreement, Side, compare_fields};
use super::defect::{ContractDefect, ContractFault, DefectReport, Epochs, Pinned, Reproduction};
use super::engine::{Budget, Engine, EngineIdentity, Fields};
use super::fixture::{Element, Fixture};
use super::minimize::{MinimizeBudget, ddmin};
use super::normal::{Inconclusive, Normalized, Projection};
use continuum_model_core::model::Model;
use continuum_value::assurance::InconclusiveReason;

/// Why a harness cannot be assembled: configuration errors only. Every variant is
/// decided from the lane table and the configured slot names **before any engine code
/// runs**; an engine that panics or declares the wrong contract is not a
/// `HarnessError` but a [`ContractFinding`] with a `defect_*` report and a quarantine
/// (cr-2r0m24 round 3). `tests/cross_engine_differential.rs` pins that no variant is
/// reachable through engine code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    /// Two engines claim one slot, so a lane could not say which it compared.
    DuplicateSlot(String),
    /// A slot name the quarantine register cannot carry
    /// ([`super::claims::is_slot`]).
    BadSlot(String),
    /// A lane compares a slot with itself, which can never disagree.
    SelfLane(String),
    /// A lane halts no claim, or names something that is not a claim id.
    BadClaims(String),
    /// Two lanes compare the same subject with the same oracle, so a report's
    /// (subject, oracle) pair would not name one lane.
    DuplicateLane(String),
}

/// An engine as plugged in: its identity and field contract read once.
#[derive(Clone)]
struct Plugged<'a> {
    engine: &'a dyn Engine,
    identity: EngineIdentity,
    fields: Fields,
}

/// An engine whose contract could not be read, or did not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractFinding {
    /// Every lane the slot serves, in table order.
    pub lanes: Vec<Lane>,
    /// The report.
    pub report: ContractDefect,
    /// Its handle.
    pub handle: String,
}

/// A configured harness.
pub struct Harness<'a> {
    engines: BTreeMap<String, Plugged<'a>>,
    faulted: BTreeMap<String, ContractFinding>,
    lanes: Vec<Lane>,
    budget: Budget,
    epochs: Epochs,
    minimize: MinimizeBudget,
}

/// Per-lane counts over one run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaneCounts {
    /// Fixtures compared.
    pub fixtures: usize,
    /// Fields decided alike.
    pub agreed: usize,
    /// Counterexamples that replayed.
    pub witnesses_replayed: usize,
    /// Fields left undecided, by the pair of INV-008 reasons (`subject/oracle`, `-`
    /// for a side that decided).
    pub undecided: BTreeMap<String, usize>,
    /// Fixtures with at least one disagreement.
    pub disagreeing_fixtures: usize,
    /// Disagreeing fields.
    pub disagreements: usize,
    /// Verdicts compared with nothing, because the lane's other engine does not judge
    /// invariants by declaration (see [`super::compare::Comparison::unjudged`]).
    pub unjudged: usize,
    /// Undefined-read claims that held at their state when checked against the model.
    pub undefined_checked: usize,
    /// Fixtures whose model declares no predicate: the verdict fields are empty by
    /// the model's own shape, counted here so an all-empty run is visible rather than
    /// read as agreement.
    pub no_invariant_fixtures: usize,
}

/// A lane's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneStatus {
    /// Both engines were plugged in and the lane ran.
    Ran(LaneCounts),
    /// An engine of the lane faulted at assembly; the lane did not run, and its claims
    /// are quarantined by that engine's contract defect.
    Faulted {
        /// The faulted slots.
        slots: Vec<String>,
    },
    /// A slot has no engine; the lane decided nothing and passed nothing.
    Absent {
        /// The unplugged slots.
        missing: Vec<String>,
    },
}

/// One disagreement, handled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The lane.
    pub lane: Lane,
    /// The fixture's label.
    pub fixture: String,
    /// Every disagreement on the original fixture.
    pub disagreements: Vec<Disagreement>,
    /// The minimized fixture.
    pub minimized: Fixture,
    /// The defect report.
    pub report: DefectReport,
    /// Its handle.
    pub handle: String,
    /// `None` for a disagreement found on a corpus fixture; the parent finding's
    /// handle for an engine fault first seen while minimizing that finding.
    pub found_while_minimizing: Option<String>,
    /// Every slot this finding's own evidence shows faulting: its retained
    /// disagreement and the faults on its fixture. Each one's lanes are halted under
    /// this finding's handle, whether or not the fault reproduced later. A fault met on
    /// a minimization candidate under another key has a finding of its own.
    pub faulted_slots: BTreeSet<String>,
}

/// One run's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// Fixtures in the corpus.
    pub fixtures: usize,
    /// Fixtures whose model did not build, with the builder's reason.
    pub unbuildable: Vec<(String, String)>,
    /// Every lane and what became of it, in table order.
    pub lanes: Vec<(Lane, LaneStatus)>,
    /// Every handled disagreement, in corpus then lane order.
    pub findings: Vec<Finding>,
    /// Every engine that faulted at assembly, in slot order.
    pub contract_findings: Vec<ContractFinding>,
    /// Every slot seen to fault anywhere in the run. A slot is entered at its first
    /// observed fault and never removed; every lane of each is quarantined for the
    /// run, under the handle of a report that records that fault.
    pub faulted_slots: BTreeSet<String>,
    /// The claims this run halted.
    pub ledger: QuarantineLedger,
}

impl RunReport {
    /// A deterministic text summary: counts per lane, then one line per finding.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "fixtures={} unbuildable={}",
            self.fixtures,
            self.unbuildable.len()
        );
        for (lane, status) in &self.lanes {
            match status {
                LaneStatus::Ran(counts) => {
                    let undecided: Vec<String> = counts
                        .undecided
                        .iter()
                        .map(|(reasons, count)| format!("{reasons}:{count}"))
                        .collect();
                    let _ = writeln!(
                        out,
                        "lane {} vs {} fixtures={} no-invariants={} agreed={} unjudged={} witnesses-replayed={} undefined-checked={} undecided=[{}] disagreeing-fixtures={} disagreements={}",
                        lane.subject,
                        lane.oracle,
                        counts.fixtures,
                        counts.no_invariant_fixtures,
                        counts.agreed,
                        counts.unjudged,
                        counts.witnesses_replayed,
                        counts.undefined_checked,
                        undecided.join(","),
                        counts.disagreeing_fixtures,
                        counts.disagreements
                    );
                }
                LaneStatus::Faulted { slots } => {
                    let _ = writeln!(
                        out,
                        "lane {} vs {} faulted slots={}",
                        lane.subject,
                        lane.oracle,
                        slots.join(",")
                    );
                }
                LaneStatus::Absent { missing } => {
                    let _ = writeln!(
                        out,
                        "lane {} vs {} absent missing={}",
                        lane.subject,
                        lane.oracle,
                        missing.join(",")
                    );
                }
            }
        }
        let faulted: Vec<&str> = self.faulted_slots.iter().map(String::as_str).collect();
        let _ = writeln!(out, "faulted-slots=[{}]", faulted.join(","));
        for finding in &self.contract_findings {
            let _ = writeln!(
                out,
                "contract-finding {} {} {}",
                finding.report.slot,
                finding.report.fault.as_str(),
                finding.handle
            );
        }
        for finding in &self.findings {
            let _ = writeln!(
                out,
                "finding {} lane {} vs {} {} -> size {} {}",
                finding.fixture,
                finding.lane.subject,
                finding.lane.oracle,
                finding.handle,
                finding.report.reproduction.model.size,
                finding.report.reproduction.minimality.as_str()
            );
        }
        out
    }
}

/// One engine's answer, with the panic it raised if it raised one.
#[derive(Debug)]
struct Guarded {
    answer: Normalized,
    panicked: Option<String>,
}

/// The text of a panic payload. The payload is then forgotten rather than dropped, so
/// a payload whose `Drop` panics cannot escape the guard.
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    let text = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-text panic payload".to_owned());
    std::mem::forget(payload);
    text
}

/// Ask `engine` inside `catch_unwind`, so a panicking adapter cannot take the run down
/// and cannot skip the defect lifecycle. A panic becomes an `EngineError` answer shaped
/// by the engine's declared `fields` (verdicts only if it judges invariants, a
/// projection only if it reports one), carrying the panic text.
fn guarded(engine: &dyn Engine, fields: Fields, model: &Model, budget: Budget) -> Guarded {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.evaluate(model, budget)
    })) {
        Ok(answer) => Guarded {
            answer,
            panicked: None,
        },
        Err(payload) => {
            let detail = panic_text(payload);
            let why = Inconclusive::new(
                InconclusiveReason::EngineError,
                format!("panicked: {detail}"),
            );
            let names: Vec<&str> = if fields.invariants {
                model
                    .predicates()
                    .iter()
                    .map(|p| p.name().as_str())
                    .collect()
            } else {
                Vec::new()
            };
            let mut answer = Normalized::inconclusive(&why, names);
            if !fields.projection {
                answer.projection = Projection::NotProvided;
            }
            Guarded {
                answer,
                panicked: Some(detail),
            }
        }
    }
}

/// The side a disagreement is attributed to, when it is one side's alone.
fn side_of(disagreement: &Disagreement) -> Option<Side> {
    match disagreement {
        Disagreement::EnginePanicked { side, .. }
        | Disagreement::EngineFailure { side, .. }
        | Disagreement::InvariantCoverage { side, .. }
        | Disagreement::ProjectionCoverage { side }
        | Disagreement::UnexploredHolds { side, .. }
        | Disagreement::InvalidWitness { side, .. }
        | Disagreement::InvalidUndefined { side, .. }
        | Disagreement::ClosureRejected { side, .. } => Some(*side),
        Disagreement::ReachableStates { .. }
        | Disagreement::Deadlocks { .. }
        | Disagreement::StateCount { .. }
        | Disagreement::EngineFaultBoth { .. }
        | Disagreement::UndefinedAction { .. }
        | Disagreement::Verdict { .. } => None,
    }
}

/// Whether `disagreement` is an engine fault of `side`: a panic, a one-sided
/// `EngineError`, or a two-sided one.
fn faults(disagreement: &Disagreement, side: Side) -> bool {
    match disagreement {
        Disagreement::EnginePanicked { side: s, .. }
        | Disagreement::EngineFailure { side: s, .. } => *s == side,
        Disagreement::EngineFaultBoth { .. } => true,
        _ => false,
    }
}

/// Compare two guarded answers under each engine's declared fields. A panic on either
/// side is always a disagreement, even when both sides panicked; the entries the
/// panic's stand-in answer would otherwise derive for that side (engine failures,
/// coverage) are dropped, so the finding is the panic and nothing that echoes it.
fn compare_guarded(
    model: &Model,
    subject: Fields,
    left: &Guarded,
    oracle: Fields,
    right: &Guarded,
) -> Comparison {
    let mut comparison = compare_fields(model, &left.answer, subject, &right.answer, oracle);
    let other = |side: Side| match side {
        Side::Subject => Side::Oracle,
        Side::Oracle => Side::Subject,
    };
    for (side, answer) in [(Side::Subject, left), (Side::Oracle, right)] {
        if let Some(detail) = &answer.panicked {
            comparison
                .disagreements
                .retain(|d| side_of(d) != Some(side));
            // A two-sided fault that involves the panic's stand-in is the other side's
            // own fault (or, if the other side panicked too, the echo of a panic).
            let paired = std::mem::take(&mut comparison.disagreements);
            for disagreement in paired {
                match disagreement {
                    Disagreement::EngineFaultBoth {
                        field,
                        subject: left_detail,
                        oracle: right_detail,
                    } => {
                        let survivor = other(side);
                        let survivor_panicked = match survivor {
                            Side::Subject => left.panicked.is_some(),
                            Side::Oracle => right.panicked.is_some(),
                        };
                        if !survivor_panicked {
                            comparison.disagreements.push(Disagreement::EngineFailure {
                                side: survivor,
                                field,
                                detail: match survivor {
                                    Side::Subject => left_detail,
                                    Side::Oracle => right_detail,
                                },
                            });
                        }
                    }
                    kept => comparison.disagreements.push(kept),
                }
            }
            comparison.disagreements.push(Disagreement::EnginePanicked {
                side,
                detail: detail.clone(),
            });
        }
    }
    comparison.disagreements.sort();
    comparison
}

fn reasons(comparison: &Comparison, counts: &mut LaneCounts) {
    for field in &comparison.undecided {
        let side = |reason: Option<continuum_value::assurance::InconclusiveReason>| {
            reason.map_or("-", |r| r.as_str())
        };
        // A field neither side left inconclusive is undecided by rule, not by a
        // reason (the deadlocks of a model with an undefined action read): keyed by
        // the field, so the summary names it.
        let key = if field.subject.is_none() && field.oracle.is_none() {
            // Group per-field names (`undefined-unproven:verdict:<inv>`) by their kind.
            field.field.split(':').next().unwrap_or_default().to_owned()
        } else {
            format!("{}/{}", side(field.subject), side(field.oracle))
        };
        let slot = counts.undecided.entry(key).or_insert(0);
        *slot = slot.saturating_add(1);
    }
}

impl<'a> Harness<'a> {
    /// Assemble a harness from `(slot, engine)` pairs. The slot is the
    /// configuration's name for the engine, so a fault is attributed to it even when
    /// the engine cannot say its own name.
    ///
    /// Configuration is validated first, with no engine code: the lane table, the
    /// slot names, and duplicate slots. Only then is each engine's `identity` and
    /// `fields` read, each under its own guard. A panic there, an identity naming
    /// another slot, or fields another lane requires differently is a contract fault:
    /// the engine is not run, and [`Harness::run`] reports it with a `defect_*` handle
    /// and quarantines every claim of every lane the slot serves.
    ///
    /// # Errors
    ///
    /// Every arm of [`HarnessError`], all engine-free: a malformed or duplicate slot
    /// name, a lane that compares a slot with itself, or a lane without well-formed
    /// claims.
    pub fn new<S: AsRef<str>>(
        engines: &[(S, &'a dyn Engine)],
        lanes: &[Lane],
        budget: Budget,
        epochs: Epochs,
        minimize: MinimizeBudget,
    ) -> Result<Self, HarnessError> {
        for lane in lanes {
            for slot in [lane.subject, lane.oracle] {
                if !super::claims::is_slot(slot) {
                    return Err(HarnessError::BadSlot(slot.to_owned()));
                }
            }
            if lane.subject == lane.oracle {
                return Err(HarnessError::SelfLane(lane.subject.to_owned()));
            }
            if lane.claims.is_empty() || !lane.claims.iter().all(|c| super::claims::is_claim_id(c))
            {
                return Err(HarnessError::BadClaims(format!(
                    "{} vs {}",
                    lane.subject, lane.oracle
                )));
            }
        }
        let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
        for lane in lanes {
            if !pairs.insert((lane.subject, lane.oracle)) {
                return Err(HarnessError::DuplicateLane(format!(
                    "{} vs {}",
                    lane.subject, lane.oracle
                )));
            }
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (slot, _) in engines {
            let slot = slot.as_ref();
            if !super::claims::is_slot(slot) {
                return Err(HarnessError::BadSlot(slot.to_owned()));
            }
            if !seen.insert(slot) {
                return Err(HarnessError::DuplicateSlot(slot.to_owned()));
            }
        }

        let mut map: BTreeMap<String, Plugged<'a>> = BTreeMap::new();
        let mut faulted: BTreeMap<String, ContractFinding> = BTreeMap::new();
        for (slot, engine) in engines {
            let slot = slot.as_ref();
            let served: Vec<Lane> = lanes
                .iter()
                .filter(|lane| lane.subject == slot || lane.oracle == slot)
                .copied()
                .collect();
            let fault = |fault: ContractFault, detail: String, build: Option<String>| {
                let mut claims: Vec<String> = served
                    .iter()
                    .flat_map(|lane| lane.claims.iter().map(|c| (*c).to_owned()))
                    .collect();
                claims.sort();
                claims.dedup();
                let report = ContractDefect {
                    slot: slot.to_owned(),
                    fault,
                    detail,
                    build,
                    claims,
                    epochs: epochs.clone(),
                };
                let handle = report.handle();
                ContractFinding {
                    lanes: served.clone(),
                    report,
                    handle,
                }
            };
            let identity = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.identity()
            })) {
                Ok(identity) => identity,
                Err(payload) => {
                    faulted.insert(
                        slot.to_owned(),
                        fault(ContractFault::IdentityPanicked, panic_text(payload), None),
                    );
                    continue;
                }
            };
            let fields =
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.fields())) {
                    Ok(fields) => fields,
                    Err(payload) => {
                        faulted.insert(
                            slot.to_owned(),
                            fault(
                                ContractFault::FieldsPanicked,
                                panic_text(payload),
                                Some(identity.build.clone()),
                            ),
                        );
                        continue;
                    }
                };
            if identity.slot != slot {
                faulted.insert(
                    slot.to_owned(),
                    fault(
                        ContractFault::SlotMismatch,
                        format!("names itself {}", identity.slot),
                        Some(identity.build.clone()),
                    ),
                );
                continue;
            }
            let mismatch = served.iter().find(|lane| {
                (lane.subject == slot && lane.subject_fields != fields)
                    || (lane.oracle == slot && lane.oracle_fields != fields)
            });
            if let Some(lane) = mismatch {
                faulted.insert(
                    slot.to_owned(),
                    fault(
                        ContractFault::FieldsMismatch,
                        format!(
                            "declares {fields:?}; lane {} vs {} requires otherwise",
                            lane.subject, lane.oracle
                        ),
                        Some(identity.build.clone()),
                    ),
                );
                continue;
            }
            map.insert(
                slot.to_owned(),
                Plugged {
                    engine: *engine,
                    identity,
                    fields,
                },
            );
        }
        Ok(Self {
            engines: map,
            faulted,
            lanes: lanes.to_vec(),
            budget,
            epochs,
            minimize,
        })
    }

    /// Every claim a finding halts: the lane's, and every claim of every lane a
    /// faulted slot serves. Sorted, without repeats; the report lists exactly these.
    fn halted_claims(&self, lane: &Lane, slots: &BTreeSet<&str>) -> Vec<String> {
        let mut claims: BTreeSet<&str> = lane.claims.iter().copied().collect();
        for other in self
            .lanes
            .iter()
            .filter(|l| slots.contains(l.subject) || slots.contains(l.oracle))
        {
            claims.extend(other.claims.iter().copied());
        }
        claims.into_iter().map(str::to_owned).collect()
    }

    /// Enter a finding's quarantines, read **only** from its encoded report: the claims
    /// of the lane its subject and oracle slots name (the report records their
    /// disagreement), and every claim of every lane of each slot whose fault the report
    /// encodes, all under the report's own handle. So every ledger entry names a report
    /// whose encoded evidence shows that lane's disagreement or that slot's fault.
    fn enter(&self, report: &DefectReport, ledger: &mut QuarantineLedger) {
        let handle = report.handle();
        for lane in self
            .lanes
            .iter()
            .filter(|l| l.subject == report.subject.slot && l.oracle == report.oracle.slot)
        {
            for claim in lane.claims {
                ledger.insert(Quarantine {
                    claim: (*claim).to_owned(),
                    subject: lane.subject.to_owned(),
                    oracle: lane.oracle.to_owned(),
                    defect: handle.clone(),
                });
            }
        }
        for slot in report.faulted_slots() {
            self.quarantine_slot(slot, &handle, ledger);
        }
    }

    /// Quarantine every claim of every lane that `slot` serves, under `defect`.
    fn quarantine_slot(&self, slot: &str, defect: &str, ledger: &mut QuarantineLedger) {
        for lane in self
            .lanes
            .iter()
            .filter(|l| l.subject == slot || l.oracle == slot)
        {
            for claim in lane.claims {
                ledger.insert(Quarantine {
                    claim: (*claim).to_owned(),
                    subject: lane.subject.to_owned(),
                    oracle: lane.oracle.to_owned(),
                    defect: defect.to_owned(),
                });
            }
        }
    }

    fn engine(&self, slot: &str) -> Option<&Plugged<'a>> {
        self.engines.get(slot)
    }

    /// Run every lane over `corpus`.
    #[must_use]
    pub fn run(&self, corpus: &[Fixture]) -> RunReport {
        let mut unbuildable = Vec::new();
        let mut counts: Vec<LaneCounts> = vec![LaneCounts::default(); self.lanes.len()];
        let mut findings = Vec::new();
        let mut ledger = QuarantineLedger::new();
        // Contract faults first: their quarantines come from the lane table alone.
        let contract_findings: Vec<ContractFinding> = self.faulted.values().cloned().collect();
        // Append-only for the run: a slot enters on its first observed fault
        // (assembly, evaluation, a minimization candidate, or a recheck) and never
        // leaves, whatever later reruns show.
        let mut faulted_slots: BTreeSet<String> = BTreeSet::new();
        for finding in &contract_findings {
            self.quarantine_slot(&finding.report.slot, &finding.report.handle(), &mut ledger);
            faulted_slots.insert(finding.report.slot.clone());
        }

        for fixture in corpus {
            let model = match fixture.build() {
                Ok(model) => model,
                Err(error) => {
                    unbuildable.push((fixture.label.clone(), error.to_string()));
                    continue;
                }
            };
            let mut answers: BTreeMap<String, Guarded> = BTreeMap::new();
            for (index, lane) in self.lanes.iter().enumerate() {
                let (Some(subject), Some(oracle)) =
                    (self.engine(lane.subject), self.engine(lane.oracle))
                else {
                    continue;
                };
                // Keyed by the lane's slot strings, which are the map keys the engines
                // were plugged in under, never by a fresh `identity()` call.
                for (slot, plugged) in [(lane.subject, subject), (lane.oracle, oracle)] {
                    answers.entry(slot.to_owned()).or_insert_with(|| {
                        guarded(plugged.engine, plugged.fields, &model, self.budget)
                    });
                }
                let (Some(left), Some(right)) =
                    (answers.get(lane.subject), answers.get(lane.oracle))
                else {
                    continue;
                };
                let comparison =
                    compare_guarded(&model, subject.fields, left, oracle.fields, right);
                let Some(tally) = counts.get_mut(index) else {
                    continue;
                };
                tally.fixtures = tally.fixtures.saturating_add(1);
                if model.predicates().is_empty() {
                    tally.no_invariant_fixtures = tally.no_invariant_fixtures.saturating_add(1);
                }
                tally.agreed = tally.agreed.saturating_add(comparison.agreed);
                tally.witnesses_replayed = tally
                    .witnesses_replayed
                    .saturating_add(comparison.witnesses_replayed);
                tally.unjudged = tally.unjudged.saturating_add(comparison.unjudged);
                tally.undefined_checked = tally
                    .undefined_checked
                    .saturating_add(comparison.undefined_checked);
                reasons(&comparison, tally);
                // An engine fault is what the report is about when there is one, so
                // the report and its minimization name the fault and its slot.
                let Some(first) = comparison
                    .disagreements
                    .iter()
                    .find(|d| faults(d, Side::Subject) || faults(d, Side::Oracle))
                    .or_else(|| comparison.disagreements.first())
                else {
                    continue;
                };
                tally.disagreeing_fixtures = tally.disagreeing_fixtures.saturating_add(1);
                tally.disagreements = tally
                    .disagreements
                    .saturating_add(comparison.disagreements.len());
                // Every fault met on a minimization candidate under another key gets
                // its own finding, whose `first` is that fault; its own minimization
                // may meet further faults, so this is a worklist. Keys are deduplicated
                // per lane and fixture, and a lane has finitely many (fields times
                // sides), so it ends.
                let mut pending: Vec<(Fixture, Disagreement, String)> = Vec::new();
                let mut reported: BTreeSet<String> = BTreeSet::new();
                reported.insert(first.key());
                let finding = self.handle(
                    lane,
                    subject,
                    oracle,
                    fixture,
                    &model,
                    first,
                    &comparison,
                    &mut |candidate, fault| pending.push((candidate, fault, String::new())),
                    None,
                );
                self.enter(&finding.report, &mut ledger);
                faulted_slots.extend(finding.faulted_slots.iter().cloned());
                let parent = finding.handle.clone();
                for entry in &mut pending {
                    entry.2.clone_from(&parent);
                }
                findings.push(finding);
                while let Some((candidate, fault, parent)) = pending.pop() {
                    if !reported.insert(fault.key()) {
                        continue;
                    }
                    let Ok(candidate_model) = candidate.build() else {
                        continue;
                    };
                    let left = guarded(
                        subject.engine,
                        subject.fields,
                        &candidate_model,
                        self.budget,
                    );
                    let right =
                        guarded(oracle.engine, oracle.fields, &candidate_model, self.budget);
                    let again = compare_guarded(
                        &candidate_model,
                        subject.fields,
                        &left,
                        oracle.fields,
                        &right,
                    );
                    let mut more: Vec<(Fixture, Disagreement)> = Vec::new();
                    let finding = self.handle(
                        lane,
                        subject,
                        oracle,
                        &candidate,
                        &candidate_model,
                        &fault,
                        &again,
                        &mut |c, f| more.push((c, f)),
                        Some(parent),
                    );
                    self.enter(&finding.report, &mut ledger);
                    faulted_slots.extend(finding.faulted_slots.iter().cloned());
                    for (c, f) in more {
                        pending.push((c, f, finding.handle.clone()));
                    }
                    findings.push(finding);
                }
            }
        }

        let lanes = self
            .lanes
            .iter()
            .zip(counts)
            .map(|(lane, tally)| {
                let missing: Vec<String> = [lane.subject, lane.oracle]
                    .iter()
                    .filter(|slot| {
                        self.engine(slot).is_none() && !self.faulted.contains_key(**slot)
                    })
                    .map(|slot| (*slot).to_owned())
                    .collect();
                let faulted: Vec<String> = [lane.subject, lane.oracle]
                    .iter()
                    .filter(|slot| self.faulted.contains_key(**slot))
                    .map(|slot| (*slot).to_owned())
                    .collect();
                let status = if !faulted.is_empty() {
                    LaneStatus::Faulted { slots: faulted }
                } else if missing.is_empty() {
                    LaneStatus::Ran(tally)
                } else {
                    LaneStatus::Absent { missing }
                };
                (*lane, status)
            })
            .collect();
        RunReport {
            fixtures: corpus.len(),
            unbuildable,
            lanes,
            findings,
            contract_findings,
            faulted_slots,
            ledger,
        }
    }

    /// Minimize, then report.
    #[allow(clippy::too_many_arguments)]
    fn handle(
        &self,
        lane: &Lane,
        subject: &Plugged<'_>,
        oracle: &Plugged<'_>,
        fixture: &Fixture,
        model: &Model,
        first: &Disagreement,
        comparison: &Comparison,
        side_fault: &mut dyn FnMut(Fixture, Disagreement),
        found_while_minimizing: Option<String>,
    ) -> Finding {
        let key = first.key();
        let elements = fixture.elements();
        let budget = self.budget;
        // Engine faults under another key, met on a candidate, are kept (the first
        // candidate per key) so the caller reports them instead of ddmin reading them
        // as "does not reproduce" and dropping them.
        let met: std::cell::RefCell<BTreeMap<String, (Fixture, Disagreement)>> =
            std::cell::RefCell::new(BTreeMap::new());
        let found = |keep: &[Element]| -> Option<Disagreement> {
            let keep: BTreeSet<Element> = keep.iter().copied().collect();
            let restricted = fixture.restrict(&keep);
            let candidate = restricted.build().ok()?;
            let left = guarded(subject.engine, subject.fields, &candidate, budget);
            let right = guarded(oracle.engine, oracle.fields, &candidate, budget);
            let mut hit = None;
            let seen = compare_guarded(&candidate, subject.fields, &left, oracle.fields, &right)
                .disagreements;
            for d in seen {
                if d.key() == key {
                    if hit.is_none() {
                        hit = Some(d);
                    }
                } else if faults(&d, Side::Subject) || faults(&d, Side::Oracle) {
                    met.borrow_mut()
                        .entry(d.key())
                        .or_insert_with(|| (restricted.clone(), d));
                }
            }
            hit
        };
        let minimized = ddmin(elements, self.minimize, |keep| found(keep).is_some());
        let keep: BTreeSet<Element> = minimized.kept.iter().copied().collect();
        let small = fixture.restrict(&keep);
        let size = small.size();
        // The kept subset reproduced when it was tested; rebuilding it is the same
        // pure function of the same declarations. Should it not rebuild, the report
        // falls back to the original, which is a reproduction by construction.
        let (reproduction, minimized_fixture) = match (small.build(), found(&minimized.kept)) {
            (Ok(small_model), Some(disagreement)) => (
                Reproduction::of(
                    &small.label,
                    &small_model,
                    size,
                    disagreement.to_string(),
                    minimized.minimality,
                    // The re-run for the report is one more test.
                    minimized.tests.saturating_add(1),
                ),
                small,
            ),
            _ => (
                Reproduction::of(
                    &fixture.label,
                    model,
                    fixture.size(),
                    first.to_string(),
                    super::minimize::Minimality::NotReproduced,
                    minimized.tests.saturating_add(1),
                ),
                fixture.clone(),
            ),
        };
        // This finding's own faults, attributed to their slots: the retained
        // disagreement and the faults on the comparison it was built on (a recheck, for
        // a side finding). The report encodes them, and quarantine reads them back
        // from the report and nothing else. Faults met on candidates under other keys
        // are the caller's side findings, each with that fault as its `first`.
        let mut encoded: BTreeSet<(String, String)> = BTreeSet::new();
        for d in std::iter::once(first).chain(comparison.disagreements.iter()) {
            for (side, slot) in [(Side::Subject, lane.subject), (Side::Oracle, lane.oracle)] {
                if faults(d, side) {
                    encoded.insert((slot.to_owned(), d.to_string()));
                }
            }
        }
        let slots: BTreeSet<&str> = encoded.iter().map(|(slot, _)| slot.as_str()).collect();
        let claims = self.halted_claims(lane, &slots);
        let report = DefectReport {
            subject: subject.identity.clone(),
            oracle: oracle.identity.clone(),
            claims,
            epochs: self.epochs.clone(),
            budget: self.budget,
            original: Pinned::of(&fixture.label, model, fixture.size()),
            disagreement: first.to_string(),
            faults: encoded.into_iter().collect(),
            reproduction,
        };
        let handle = report.handle();
        for (candidate, fault) in met.into_inner().into_values() {
            side_fault(candidate, fault);
        }
        let faulted_slots = report
            .faulted_slots()
            .into_iter()
            .map(str::to_owned)
            .collect();
        Finding {
            lane: *lane,
            fixture: fixture.label.clone(),
            disagreements: comparison.disagreements.clone(),
            minimized: minimized_fixture,
            report,
            handle,
            found_while_minimizing,
            faulted_slots,
        }
    }
}
