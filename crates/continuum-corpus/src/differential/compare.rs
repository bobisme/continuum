//! Comparison of two normalized answers about one model.
//!
//! The rules, field by field:
//!
//! - **Inconclusive is not a semantic disagreement.** When either side of a field is
//!   typed inconclusive, the field is recorded with both sides' INV-008 reasons. A
//!   budget stop or an unsupported feature decides nothing, so it cannot contradict
//!   anything (RFC 0030's `budget-sensitive` class: "never quarantines on budget …
//!   grounds").
//! - **An engine error is an engine fault, never a typed inconclusive.** RFC 0026
//!   requires an engine failure to stay inconclusive *and* to enter the defect
//!   lifecycle. The two classes are kept apart: a budget stop (`ResourceExhausted`),
//!   unsupported semantics (`Unsupported`), and INV-008's other typed reasons are
//!   legitimately undecided, on one side or both, and are counted per lane. An
//!   `EngineError` is a fault. On one side it is [`Disagreement::EngineFailure`]; on
//!   both sides of one field it is [`Disagreement::EngineFaultBoth`], because two
//!   engines failing alike is still two faults and says nothing about the model's
//!   semantics. Either way the field is minimized, reported with a `defect_*` handle,
//!   and halts the lane's claims. Adapters therefore map only faults to
//!   `EngineError` (an evaluation failure, an internal inconsistency, an adapter
//!   mismatch) and every legitimate inconclusive to its own typed reason. A panic is
//!   caught by the driver and is always a defect ([`Disagreement::EnginePanicked`]).
//! - **Coverage comes from the model.** Each engine declares its [`Fields`]; an engine
//!   that judges invariants must judge exactly the model's predicates, and one that
//!   reports a projection must report one. An empty answer is not an opt-out.
//! - **Verdicts** disagree exactly when one side says `Holds` and the other says
//!   `Violated` or `NotEstablished`. A decisive answer is checked against the other
//!   decisive answer, so an engine that turns a budget stop into `Holds` is caught
//!   whenever the other side refutes the invariant.
//! - **Each side on its own**: a `Holds` beside the same engine's `ResourceExhausted`
//!   reachable-state projection is laundered inconclusiveness and is a disagreement
//!   even when the other side is also stopped.
//! - **Undefined reads** (bn-24a5c, RFC 0003 "Definedness") are their own category,
//!   never a verdict and never mapped to undecided. Per invariant, `Undefined(kind)` is
//!   a decided class: equal to the same kind, a disagreement against another kind or a
//!   verdict, and undecided only against the other side's typed inconclusive. At model
//!   level, an undefined action read is compared whenever both engines judge
//!   invariants: found on both sides, decided absent on both sides, or a disagreement.
//!   Which reached state and subject each side names is its own choice, so each claim is
//!   checked against the model ([`super::replay::check_undefined`]), and a claim that
//!   does not hold is a defect. Reachability is proven, by replaying the claim's path
//!   or by the side's exact reachable set; an unproven claim is never agreement (the
//!   field is `undefined-unproven`). The model-level field accepts only action reads. Deadlocks of a model with an undefined action read are
//!   not a CML answer and are recorded as `deadlocks-under-undefined-action`, not
//!   compared.
//! - **Counterexamples** are replayed against the model ([`super::replay`]), each on
//!   its own. A path that does not replay is a disagreement between that engine and
//!   the semantics, whatever the other side says.
//! - **Reachable states**: two exact sets must be equal, and so must their deadlocked
//!   subsets; a count is compared with a count or with an exact set's size; a checker
//!   that rejects the closed set disagrees with any side that reports one.

use std::collections::BTreeSet;
use std::fmt;

use continuum_model_core::model::Model;
use continuum_value::assurance::InconclusiveReason;

use super::engine::Fields;
use super::normal::{InvariantVerdict, Normalized, Projection, UndefinedKind, UndefinedRead};
use super::replay::{ReplayFault, UndefinedFault, check_undefined, replay};

/// Which side of a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// The engine under test.
    Subject,
    /// The engine it is measured against.
    Oracle,
}

impl Side {
    /// `subject` or `oracle`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Oracle => "oracle",
        }
    }
}

/// A decisive verdict class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Decided {
    /// Every reachable state satisfies the invariant.
    Holds,
    /// Some reachable state falsifies it, or a checker refused to establish it.
    NotHolds,
    /// No verdict: an undefined read of this kind (RFC 0003). Equal only to the same
    /// kind; the subject and state each side names are checked against the model.
    Undefined(UndefinedKind),
}

/// One field on which two engines disagree. Every arm names the field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Disagreement {
    /// The engine panicked. Caught by the driver; always a defect. First in the
    /// order, so it is the disagreement a panicking lane is reported and minimized
    /// under.
    EnginePanicked {
        /// The side that panicked.
        side: Side,
        /// The panic payload, when it is text.
        detail: String,
    },
    /// The exact reachable sets differ.
    ReachableStates {
        /// States only the subject reports.
        only_subject: usize,
        /// States only the oracle reports.
        only_oracle: usize,
        /// The least state in the symmetric difference.
        first: Vec<i64>,
    },
    /// The deadlocked subsets differ (over equal reachable sets).
    Deadlocks {
        /// Deadlocks only the subject reports.
        only_subject: usize,
        /// Deadlocks only the oracle reports.
        only_oracle: usize,
        /// The least state in the symmetric difference.
        first: Vec<i64>,
    },
    /// The reachable-state counts differ.
    StateCount {
        /// The subject's count.
        subject: usize,
        /// The oracle's count.
        oracle: usize,
    },
    /// A checker rejected the closed set the other side reports.
    ClosureRejected {
        /// The side that rejected.
        side: Side,
        /// The checker's reason.
        detail: String,
    },
    /// Decisive verdicts differ.
    Verdict {
        /// The invariant.
        invariant: String,
        /// The subject's verdict.
        subject: Decided,
        /// The oracle's verdict.
        oracle: Decided,
    },
    /// An engine that judges invariants did not judge exactly the model's predicates:
    /// a declared predicate is missing, or a name the model does not declare is
    /// judged. An omitted invariant would otherwise agree vacuously.
    InvariantCoverage {
        /// The side whose coverage is wrong.
        side: Side,
        /// The least name missing or extra.
        name: String,
    },
    /// An engine that declares a projection reported none, or one that declares none
    /// reported one.
    ProjectionCoverage {
        /// The side whose coverage is wrong.
        side: Side,
    },
    /// One side reports `EngineError` on a field: an engine fault (RFC 0026, plan
    /// §4.7).
    EngineFailure {
        /// The side that failed.
        side: Side,
        /// The field (`reachable-states` or `verdict:<invariant>`).
        field: String,
        /// The engine's detail.
        detail: String,
    },
    /// Both sides report `EngineError` on one field: two engine faults. Still a
    /// defect, never undecided.
    EngineFaultBoth {
        /// The field.
        field: String,
        /// The subject's detail.
        subject: String,
        /// The oracle's detail.
        oracle: String,
    },
    /// An engine says `Holds` while its own reachable-state projection is a budget
    /// stop: it has not explored the reachable set, so the `Holds` is laundered
    /// inconclusiveness (INV-008), whatever the other side says.
    UnexploredHolds {
        /// The side that said it.
        side: Side,
        /// The invariant.
        invariant: String,
    },
    /// One side found an undefined action read and the other, having decided the
    /// reachable set, found none: they disagree on whether the model is defined.
    UndefinedAction {
        /// The subject's `kind:subject`, if it found one.
        subject: Option<String>,
        /// The oracle's `kind:subject`, if it found one.
        oracle: Option<String>,
    },
    /// An undefined-read claim does not hold at its state.
    InvalidUndefined {
        /// The side that made it.
        side: Side,
        /// `undefined-action` or `verdict:<invariant>`.
        field: String,
        /// Why it does not hold.
        fault: UndefinedFault,
    },
    /// A counterexample does not replay.
    InvalidWitness {
        /// The side that offered it.
        side: Side,
        /// The invariant it is offered against.
        invariant: String,
        /// Why it does not replay.
        fault: ReplayFault,
    },
}

impl Disagreement {
    /// The field, as a stable key. Minimization keeps a candidate only when a
    /// disagreement with the same key persists, so a shrink cannot trade one defect
    /// for another.
    #[must_use]
    ///
    /// The key carries the direction as well as the field: which side is missing
    /// states, which side says `Holds`, and the replay fault's class. Every variant has
    /// its own prefix, and the side, direction or fault precedes the free-text name, so
    /// no two distinct fields share a key.
    pub fn key(&self) -> String {
        let direction =
            |only_subject: usize, only_oracle: usize| match (only_subject > 0, only_oracle > 0) {
                (true, true) => "both",
                (true, false) => "subject-extra",
                _ => "subject-missing",
            };
        match self {
            Self::ReachableStates {
                only_subject,
                only_oracle,
                ..
            } => format!(
                "reachable-states:{}",
                direction(*only_subject, *only_oracle)
            ),
            Self::Deadlocks {
                only_subject,
                only_oracle,
                ..
            } => format!("deadlocks:{}", direction(*only_subject, *only_oracle)),
            Self::StateCount { subject, oracle } => format!(
                "state-count:{}",
                if subject > oracle {
                    "subject-extra"
                } else {
                    "subject-missing"
                }
            ),
            Self::ClosureRejected { side, .. } => format!("closure-rejected:{}", side.as_str()),
            Self::Verdict {
                invariant, subject, ..
            } => format!("verdict:subject-{subject:?}:{invariant}"),
            Self::InvariantCoverage { side, name } => {
                format!("invariant-coverage:{}:{name}", side.as_str())
            }
            Self::UnexploredHolds { side, invariant } => {
                format!("unexplored-holds:{}:{invariant}", side.as_str())
            }
            Self::ProjectionCoverage { side } => format!("projection-coverage:{}", side.as_str()),
            Self::UndefinedAction { subject, .. } => format!(
                "undefined-action:subject-{}",
                if subject.is_some() { "found" } else { "none" }
            ),
            Self::InvalidUndefined { side, field, fault } => {
                format!(
                    "invalid-undefined:{}:{}:{field}",
                    side.as_str(),
                    fault.class()
                )
            }
            Self::EngineFailure { side, field, .. } => {
                format!("engine-failure:{}:{field}", side.as_str())
            }
            Self::EnginePanicked { side, .. } => format!("engine-panicked:{}", side.as_str()),
            Self::EngineFaultBoth { field, .. } => format!("engine-fault-both:{field}"),
            Self::InvalidWitness {
                side,
                invariant,
                fault,
            } => format!(
                "invalid-witness:{}:{}:{invariant}",
                side.as_str(),
                fault.class()
            ),
        }
    }
}

impl fmt::Display for Disagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReachableStates {
                only_subject,
                only_oracle,
                first,
            } => write!(
                f,
                "reachable-states only-subject={only_subject} only-oracle={only_oracle} first={first:?}"
            ),
            Self::Deadlocks {
                only_subject,
                only_oracle,
                first,
            } => write!(
                f,
                "deadlocks only-subject={only_subject} only-oracle={only_oracle} first={first:?}"
            ),
            Self::StateCount { subject, oracle } => {
                write!(f, "state-count subject={subject} oracle={oracle}")
            }
            Self::ClosureRejected { side, detail } => {
                write!(f, "closure-rejected side={} detail={detail}", side.as_str())
            }
            Self::Verdict {
                invariant,
                subject,
                oracle,
            } => write!(
                f,
                "verdict invariant={invariant} subject={subject:?} oracle={oracle:?}"
            ),
            Self::InvariantCoverage { side, name } => {
                write!(f, "invariant-coverage side={} name={name}", side.as_str())
            }
            Self::ProjectionCoverage { side } => {
                write!(f, "projection-coverage side={}", side.as_str())
            }
            Self::UndefinedAction { subject, oracle } => write!(
                f,
                "undefined-action subject={} oracle={}",
                subject.as_deref().unwrap_or("none"),
                oracle.as_deref().unwrap_or("none")
            ),
            Self::InvalidUndefined { side, field, fault } => write!(
                f,
                "invalid-undefined side={} field={field} fault={}",
                side.as_str(),
                fault.class()
            ),
            Self::EngineFailure {
                side,
                field,
                detail,
            } => write!(
                f,
                "engine-failure side={} field={field} detail={detail}",
                side.as_str()
            ),
            Self::EngineFaultBoth {
                field,
                subject,
                oracle,
            } => write!(
                f,
                "engine-fault-both field={field} subject={subject} oracle={oracle}"
            ),
            Self::EnginePanicked { side, detail } => {
                write!(f, "engine-panicked side={} detail={detail}", side.as_str())
            }
            Self::UnexploredHolds { side, invariant } => write!(
                f,
                "unexplored-holds side={} invariant={invariant}",
                side.as_str()
            ),
            Self::InvalidWitness {
                side,
                invariant,
                fault,
            } => write!(
                f,
                "invalid-witness side={} invariant={invariant} fault={fault:?}",
                side.as_str()
            ),
        }
    }
}

/// A field left undecided, with each side's typed reason (`None`: that side decided).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undecided {
    /// The field key (`reachable-states` or `verdict:<invariant>`).
    pub field: String,
    /// The subject's reason, if it was inconclusive.
    pub subject: Option<InconclusiveReason>,
    /// The oracle's reason, if it was inconclusive.
    pub oracle: Option<InconclusiveReason>,
}

/// The result of comparing two answers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comparison {
    /// Fields both sides decided, and decided alike.
    pub agreed: usize,
    /// Counterexamples that replayed.
    pub witnesses_replayed: usize,
    /// Undefined-read claims that held at their state when checked against the model.
    pub undefined_checked: usize,
    /// Verdicts one side gave on an invariant the other side's engine does not judge
    /// by declaration: compared with nothing, and counted so that is visible.
    pub unjudged: usize,
    /// Fields at least one side left undecided.
    pub undecided: Vec<Undecided>,
    /// Fields on which the sides disagree, in a fixed order.
    pub disagreements: Vec<Disagreement>,
}

fn difference(
    subject: &BTreeSet<Vec<i64>>,
    oracle: &BTreeSet<Vec<i64>>,
) -> Option<(usize, usize, Vec<i64>)> {
    if subject == oracle {
        return None;
    }
    let only_subject: Vec<&Vec<i64>> = subject.difference(oracle).collect();
    let only_oracle: Vec<&Vec<i64>> = oracle.difference(subject).collect();
    let first = only_subject
        .first()
        .copied()
        .into_iter()
        .chain(only_oracle.first().copied())
        .min()
        .cloned()
        .unwrap_or_default();
    Some((only_subject.len(), only_oracle.len(), first))
}

fn count(projection: &Projection) -> Option<usize> {
    match projection {
        Projection::Exact { states, .. } => Some(states.len()),
        Projection::Cardinality { states } => Some(*states),
        Projection::CheckerRejected { .. }
        | Projection::NotProvided
        | Projection::Inconclusive(_) => None,
    }
}

/// Whether a side has decided the undefined-action question: it found one (a reached
/// state, as a violation is), or it decided the whole reachable set and found none.
fn decides_undefined_action(answer: &Normalized) -> bool {
    answer.undefined_action.is_some()
        || matches!(
            answer.projection,
            Projection::Exact { .. } | Projection::Cardinality { .. }
        )
}

fn render_undefined(read: &UndefinedRead) -> String {
    format!("{}:{}", read.kind.as_str(), read.subject)
}

/// The model-level undefined action read, compared: both found one (each is checked
/// against the model on its own), both decided there is none, or they disagree. A side
/// that did not decide leaves the field undecided, recorded, not skipped.
fn compare_undefined_action(
    subject: &Normalized,
    oracle: &Normalized,
    unproven: bool,
    out: &mut Comparison,
) {
    // A side whose reachable set is an engine fault has no answer to give; the fault
    // is already a defect (`engine_failures`), so the field is not counted again.
    if engine_error_of_projection(&subject.projection).is_some()
        || engine_error_of_projection(&oracle.projection).is_some()
    {
        return;
    }
    let (left, right) = (&subject.undefined_action, &oracle.undefined_action);
    let (left_decides, right_decides) = (
        decides_undefined_action(subject),
        decides_undefined_action(oracle),
    );
    match (left, right) {
        // Agreement only when both are proven action reads. An unproven one is
        // undecided; a read of another kind is already an `InvalidUndefined` defect.
        (Some(a), Some(b))
            if a.kind == UndefinedKind::Action && b.kind == UndefinedKind::Action =>
        {
            if unproven {
                out.undecided.push(Undecided {
                    field: "undefined-unproven:undefined-action".to_owned(),
                    subject: None,
                    oracle: None,
                });
            } else {
                out.agreed = out.agreed.saturating_add(1);
            }
        }
        (Some(_), Some(_)) => {}
        _ if left_decides && right_decides => {
            if left.is_none() && right.is_none() {
                out.agreed = out.agreed.saturating_add(1);
            } else {
                out.disagreements.push(Disagreement::UndefinedAction {
                    subject: left.as_ref().map(render_undefined),
                    oracle: right.as_ref().map(render_undefined),
                });
            }
        }
        _ => {
            let reason = |answer: &Normalized| match &answer.projection {
                Projection::Inconclusive(why) => Some(why.reason),
                _ => None,
            };
            out.undecided.push(Undecided {
                field: "undefined-action".to_owned(),
                subject: reason(subject),
                oracle: reason(oracle),
            });
        }
    }
}

fn compare_projection(
    subject: &Projection,
    oracle: &Projection,
    under_undefined: bool,
    out: &mut Comparison,
) {
    let reason = |projection: &Projection| match projection {
        Projection::Inconclusive(why) => Some(why.reason),
        _ => None,
    };
    match (subject, oracle) {
        (Projection::NotProvided, _) | (_, Projection::NotProvided) => {}
        // A field with an engine fault on either side is a defect
        // (`engine_failures`), not an undecided field: agreed, undecided, and
        // disagreeing fields partition the compared fields.
        _ if engine_error_of_projection(subject).is_some()
            || engine_error_of_projection(oracle).is_some() => {}
        (Projection::Inconclusive(_), _) | (_, Projection::Inconclusive(_)) => {
            out.undecided.push(Undecided {
                field: "reachable-states".to_owned(),
                subject: reason(subject),
                oracle: reason(oracle),
            });
        }
        (Projection::CheckerRejected { .. }, Projection::CheckerRejected { .. }) => {
            out.agreed = out.agreed.saturating_add(1);
        }
        (Projection::CheckerRejected { detail }, _) => {
            out.disagreements.push(Disagreement::ClosureRejected {
                side: Side::Subject,
                detail: detail.clone(),
            });
        }
        (_, Projection::CheckerRejected { detail }) => {
            out.disagreements.push(Disagreement::ClosureRejected {
                side: Side::Oracle,
                detail: detail.clone(),
            });
        }
        (
            Projection::Exact {
                states: subject_states,
                deadlocks: subject_deadlocks,
            },
            Projection::Exact {
                states: oracle_states,
                deadlocks: oracle_deadlocks,
            },
        ) => {
            if let Some((only_subject, only_oracle, first)) =
                difference(subject_states, oracle_states)
            {
                out.disagreements.push(Disagreement::ReachableStates {
                    only_subject,
                    only_oracle,
                    first,
                });
            } else {
                out.agreed = out.agreed.saturating_add(1);
                // Under an undefined action read the deadlocked states are not a CML
                // answer (the lowered guard is false only to stop a successor), so they
                // are not compared; the undefined read itself is (`undefined-action`).
                if under_undefined {
                    out.undecided.push(Undecided {
                        field: "deadlocks-under-undefined-action".to_owned(),
                        subject: None,
                        oracle: None,
                    });
                    return;
                }
                // Deadlocks are compared only over one reachable set: over two
                // different sets the reachable-state disagreement already names the
                // defect, and a second finding would be its echo.
                match difference(subject_deadlocks, oracle_deadlocks) {
                    Some((only_subject, only_oracle, first)) => {
                        out.disagreements.push(Disagreement::Deadlocks {
                            only_subject,
                            only_oracle,
                            first,
                        });
                    }
                    None => out.agreed = out.agreed.saturating_add(1),
                }
            }
        }
        _ => match (count(subject), count(oracle)) {
            (Some(left), Some(right)) if left == right => out.agreed = out.agreed.saturating_add(1),
            (Some(left), Some(right)) => out.disagreements.push(Disagreement::StateCount {
                subject: left,
                oracle: right,
            }),
            _ => {}
        },
    }
}

/// The precedence rank of a decided class that reports something reached.
const fn rank(decided: Decided) -> u8 {
    match decided {
        Decided::Holds => 0,
        Decided::NotHolds => 1,
        Decided::Undefined(UndefinedKind::Invariant) => 2,
        Decided::Undefined(UndefinedKind::Action) => 3,
    }
}

/// Whether an answer decided the whole reachable set.
fn complete(answer: &Normalized) -> bool {
    matches!(
        answer.projection,
        Projection::Exact { .. } | Projection::Cardinality { .. }
    )
}

fn decided(verdict: &InvariantVerdict) -> Option<Decided> {
    match verdict {
        InvariantVerdict::Holds => Some(Decided::Holds),
        InvariantVerdict::Violated { .. } | InvariantVerdict::NotEstablished { .. } => {
            Some(Decided::NotHolds)
        }
        InvariantVerdict::Undefined(read) => Some(Decided::Undefined(read.kind)),
        InvariantVerdict::Inconclusive(_) => None,
    }
}

/// Check every undefined-read claim of one side; return the fields whose claim did not
/// check: unproven (undefined at the state, but nothing proves the state reached), or
/// a defect. Neither is ever counted as agreement.
fn replay_side(
    model: &Model,
    side: Side,
    answer: &Normalized,
    out: &mut Comparison,
) -> BTreeSet<String> {
    let mut unproven = BTreeSet::new();
    let reached = match &answer.projection {
        Projection::Exact { states, .. } => Some(states),
        _ => None,
    };
    let mut claim = |field: String, read: &UndefinedRead, invariant: Option<&str>| {
        match check_undefined(model, read, invariant, reached) {
            Ok(()) => out.undefined_checked = out.undefined_checked.saturating_add(1),
            Err(UndefinedFault::Unproven) => {
                unproven.insert(field);
            }
            Err(fault) => {
                // Not agreement either: the field is a defect.
                unproven.insert(field.clone());
                out.disagreements
                    .push(Disagreement::InvalidUndefined { side, field, fault });
            }
        }
    };
    if let Some(read) = &answer.undefined_action {
        claim("undefined-action".to_owned(), read, None);
    }
    for (invariant, verdict) in &answer.invariants {
        if let InvariantVerdict::Undefined(read) = verdict {
            claim(format!("verdict:{invariant}"), read, Some(invariant));
        }
    }
    for (invariant, verdict) in &answer.invariants {
        if let InvariantVerdict::Violated {
            witness: Some(trace),
        } = verdict
        {
            match replay(model, invariant, trace) {
                Ok(()) => out.witnesses_replayed = out.witnesses_replayed.saturating_add(1),
                Err(fault) => out.disagreements.push(Disagreement::InvalidWitness {
                    side,
                    invariant: invariant.clone(),
                    fault,
                }),
            }
        }
    }
    unproven
}

/// Each side on its own: coverage against the engine's declared [`Fields`], with the
/// required invariant set taken from the model's predicates, and no `Holds` beside
/// the same engine's budget stop on the reachable set.
fn check_side(
    model: &Model,
    side: Side,
    answer: &Normalized,
    fields: Fields,
    out: &mut Comparison,
) {
    let required: BTreeSet<&str> = if fields.invariants {
        model
            .predicates()
            .iter()
            .map(|p| p.name().as_str())
            .collect()
    } else {
        BTreeSet::new()
    };
    let judged: BTreeSet<&str> = answer.invariants.keys().map(String::as_str).collect();
    if let Some(name) = required.symmetric_difference(&judged).min() {
        out.disagreements.push(Disagreement::InvariantCoverage {
            side,
            name: (*name).to_owned(),
        });
    }
    if fields.projection == matches!(answer.projection, Projection::NotProvided) {
        out.disagreements
            .push(Disagreement::ProjectionCoverage { side });
    }
    if let Projection::Inconclusive(why) = &answer.projection {
        // A budget stop or an engine error on the reachable set: either way the
        // engine did not finish exploring, so it cannot know an invariant holds.
        if matches!(
            why.reason,
            InconclusiveReason::ResourceExhausted | InconclusiveReason::EngineError
        ) {
            for (invariant, verdict) in &answer.invariants {
                if *verdict == InvariantVerdict::Holds {
                    out.disagreements.push(Disagreement::UnexploredHolds {
                        side,
                        invariant: invariant.clone(),
                    });
                }
            }
        }
    }
}

fn engine_error_of_projection(projection: &Projection) -> Option<&str> {
    match projection {
        Projection::Inconclusive(why) if why.reason == InconclusiveReason::EngineError => {
            Some(why.detail.as_str())
        }
        _ => None,
    }
}

fn engine_error_of_verdict(verdict: Option<&InvariantVerdict>) -> Option<&str> {
    match verdict {
        Some(InvariantVerdict::Inconclusive(why))
            if why.reason == InconclusiveReason::EngineError =>
        {
            Some(why.detail.as_str())
        }
        _ => None,
    }
}

/// Engine faults, field by field, on either side or both (see the module rules).
fn engine_failures(subject: &Normalized, oracle: &Normalized, out: &mut Comparison) {
    let mut push = |field: String, left: Option<&str>, right: Option<&str>| match (left, right) {
        (Some(detail), None) => out.disagreements.push(Disagreement::EngineFailure {
            side: Side::Subject,
            field,
            detail: detail.to_owned(),
        }),
        (None, Some(detail)) => out.disagreements.push(Disagreement::EngineFailure {
            side: Side::Oracle,
            field,
            detail: detail.to_owned(),
        }),
        (Some(subject), Some(oracle)) => out.disagreements.push(Disagreement::EngineFaultBoth {
            field,
            subject: subject.to_owned(),
            oracle: oracle.to_owned(),
        }),
        (None, None) => {}
    };
    push(
        "reachable-states".to_owned(),
        engine_error_of_projection(&subject.projection),
        engine_error_of_projection(&oracle.projection),
    );
    let names: BTreeSet<&String> = subject
        .invariants
        .keys()
        .chain(oracle.invariants.keys())
        .collect();
    for name in names {
        push(
            format!("verdict:{name}"),
            engine_error_of_verdict(subject.invariants.get(name)),
            engine_error_of_verdict(oracle.invariants.get(name)),
        );
    }
}

/// Compare `subject` with `oracle`, both answers about `model`, both engines
/// declaring [`Fields::ALL`].
#[must_use]
pub fn compare(model: &Model, subject: &Normalized, oracle: &Normalized) -> Comparison {
    compare_fields(model, subject, Fields::ALL, oracle, Fields::ALL)
}

/// Compare `subject` with `oracle`, each checked against the fields its engine
/// declares. Verdicts are compared on the invariants both sides judge.
#[must_use]
pub fn compare_fields(
    model: &Model,
    subject: &Normalized,
    subject_fields: Fields,
    oracle: &Normalized,
    oracle_fields: Fields,
) -> Comparison {
    let mut out = Comparison::default();
    check_side(model, Side::Subject, subject, subject_fields, &mut out);
    check_side(model, Side::Oracle, oracle, oracle_fields, &mut out);
    engine_failures(subject, oracle, &mut out);

    // Only an action read suspends the deadlock comparison; a read of another kind in
    // that field is a defect, not a reason to stop comparing.
    let is_action = |answer: &Normalized| {
        answer
            .undefined_action
            .as_ref()
            .is_some_and(|read| read.kind == UndefinedKind::Action)
    };
    let under_undefined = is_action(subject) || is_action(oracle);
    compare_projection(
        &subject.projection,
        &oracle.projection,
        under_undefined,
        &mut out,
    );
    // Every undefined-read claim is checked first, so agreement below can require it.
    let left_unproven = replay_side(model, Side::Subject, subject, &mut out);
    let right_unproven = replay_side(model, Side::Oracle, oracle, &mut out);
    let unproven = |field: &str| left_unproven.contains(field) || right_unproven.contains(field);
    if subject_fields.invariants && oracle_fields.invariants {
        compare_undefined_action(subject, oracle, unproven("undefined-action"), &mut out);
    }
    if subject_fields.invariants != oracle_fields.invariants {
        let judged = if subject_fields.invariants {
            subject.invariants.len()
        } else {
            oracle.invariants.len()
        };
        out.unjudged = out.unjudged.saturating_add(judged);
        // The judging side's model-level undefined action read is compared with
        // nothing too; it is still checked against the model.
        let judging = if subject_fields.invariants {
            subject
        } else {
            oracle
        };
        if judging.undefined_action.is_some() {
            out.unjudged = out.unjudged.saturating_add(1);
        }
    }
    for (invariant, left) in &subject.invariants {
        let Some(right) = oracle.invariants.get(invariant) else {
            continue;
        };
        match (decided(left), decided(right)) {
            // Equal undefined reads agree only when both claims are proven reached.
            (Some(a @ Decided::Undefined(_)), Some(b))
                if a == b && unproven(&format!("verdict:{invariant}")) =>
            {
                out.undecided.push(Undecided {
                    field: format!("undefined-unproven:verdict:{invariant}"),
                    subject: None,
                    oracle: None,
                });
            }
            (Some(a), Some(b)) if a == b => out.agreed = out.agreed.saturating_add(1),
            // Both sides found something reached (neither says `Holds`), in different
            // precedence classes: undefined action read > undefined invariant read >
            // violation. A side that did not explore everything may simply not have
            // reached the higher-precedence state, so its lower answer contradicts
            // nothing: the field is undecided, recorded with its budget reason.
            (Some(a), Some(b))
                if a != Decided::Holds
                    && b != Decided::Holds
                    && !complete(if rank(a) < rank(b) { subject } else { oracle }) =>
            {
                let reason = |answer: &Normalized| match &answer.projection {
                    Projection::Inconclusive(why) => Some(why.reason),
                    _ => None,
                };
                out.undecided.push(Undecided {
                    field: format!("verdict:{invariant}"),
                    subject: reason(subject),
                    oracle: reason(oracle),
                });
            }
            (Some(a), Some(b)) => out.disagreements.push(Disagreement::Verdict {
                invariant: invariant.clone(),
                subject: a,
                oracle: b,
            }),
            _ if engine_error_of_verdict(Some(left)).is_some()
                || engine_error_of_verdict(Some(right)).is_some() => {}
            _ => {
                let reason = |verdict: &InvariantVerdict| match verdict {
                    InvariantVerdict::Inconclusive(why) => Some(why.reason),
                    _ => None,
                };
                out.undecided.push(Undecided {
                    field: format!("verdict:{invariant}"),
                    subject: reason(left),
                    oracle: reason(right),
                });
            }
        }
    }
    out.disagreements.sort();
    out
}
