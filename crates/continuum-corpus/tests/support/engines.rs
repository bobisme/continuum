//! Engine adapters for the engines on trunk, and the planted mutants.
//!
//! Each adapter projects one engine's own answer onto `Normalized` and decides
//! nothing itself: the reference adapter reads `checking::check`'s report, the kernel
//! adapter reads `check_certificate`'s verdict on bytes, and the semantic-oracle
//! adapter reads `oracle::run`'s artifact. The mutants are test-only engines in the
//! explicit-engine slot, each a known way for an optimized engine to be wrong.

use std::collections::{BTreeMap, BTreeSet};

use continuum_corpus::differential::engine::{DPOR, EXPLICIT, KERNEL, REFERENCE, SEMANTIC_ORACLE};
use continuum_corpus::differential::{
    Budget, Engine, EngineIdentity, Fields, Inconclusive, InvariantVerdict, Normalized, Projection,
    Trace, UndefinedKind, UndefinedRead,
};
use continuum_engine_dpor as dpor;
use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{
    self, ClaimEnvelope, ClosedSet, EmissionError, PRODUCER,
};
use continuum_engine_reference::checking::{
    self, CheckOutcome, DeadlockOutcome, DeadlockPolicy, Obligations, Unresolved,
};
use continuum_engine_reference::model::{Model, State};
use continuum_engine_reference::semantic::oracle::{self, Limits, Refusal};
use continuum_engine_reference::semantic::system::{
    ActionMeta, Fairness, Footprint, Role, System, SystemError, SystemParts, VarKind,
};
use continuum_engine_reference::{Definedness, Guarded, Undefined};
use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::{CheckedClaim, PropertyClass, Rejection, Verdict};
use continuum_model_core::definedness::definedness_base;
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

fn names(model: &Model) -> Vec<&str> {
    model
        .predicates()
        .iter()
        .map(|p| p.name().as_str())
        .collect()
}

fn bounds(budget: Budget) -> Bounds {
    Bounds::new(budget.states, budget.depth, budget.transitions)
}

fn vector(state: &State) -> Vec<i64> {
    state.as_slice().to_vec()
}

fn unresolved(reason: &Unresolved) -> Inconclusive {
    let typed = match reason {
        Unresolved::ResourceExhausted { .. } => InconclusiveReason::ResourceExhausted,
        Unresolved::EngineError { .. } => InconclusiveReason::EngineError,
    };
    Inconclusive::new(typed, reason.to_string())
}

fn undefined_read(undefined: &Undefined) -> UndefinedRead {
    UndefinedRead {
        kind: match undefined.read() {
            Guarded::Action => UndefinedKind::Action,
            Guarded::Predicate(_) => UndefinedKind::Invariant,
        },
        subject: undefined.subject().to_owned(),
        state: Some(vector(undefined.state())),
        path: undefined.evidence().witness().map(|found| Trace {
            start: vector(found.start()),
            steps: found
                .steps()
                .iter()
                .map(|step| (step.name().as_str().to_owned(), vector(step.target())))
                .collect(),
        }),
    }
}

// ---------------------------------------------------------------------------
// the reference path
// ---------------------------------------------------------------------------

/// `bfs::explore` + `checking::check` (every predicate, deadlock as a defect) +
/// `witness::shortest` through the report's evidence.
pub struct ReferenceEngine;

impl Engine for ReferenceEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: REFERENCE.to_owned(),
            build: PRODUCER.to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let exploration = match bfs::explore(model, bounds(budget)) {
            Ok(exploration) => exploration,
            Err(error) => {
                return Normalized::inconclusive(
                    &Inconclusive::new(InconclusiveReason::EngineError, error.to_string()),
                    names(model),
                );
            }
        };
        let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
        let report = match checking::check(model, &exploration, &obligations) {
            Ok(report) => report,
            Err(error) => {
                return Normalized::inconclusive(
                    &Inconclusive::new(InconclusiveReason::EngineError, error.to_string()),
                    names(model),
                );
            }
        };
        let mut invariants = BTreeMap::new();
        for result in report.invariants() {
            let verdict = match result.outcome() {
                CheckOutcome::Holds { .. } => InvariantVerdict::Holds,
                CheckOutcome::Violated { evidence, .. } => InvariantVerdict::Violated {
                    witness: evidence.witness().map(|found| Trace {
                        start: vector(found.start()),
                        steps: found
                            .steps()
                            .iter()
                            .map(|step| (step.name().as_str().to_owned(), vector(step.target())))
                            .collect(),
                    }),
                },
                CheckOutcome::Undefined(undefined) => {
                    InvariantVerdict::Undefined(undefined_read(undefined))
                }
                CheckOutcome::Inconclusive(reason) => {
                    InvariantVerdict::Inconclusive(unresolved(reason))
                }
            };
            invariants.insert(result.name().as_str().to_owned(), verdict);
        }
        // The deadlock question is where the reference reports an undefined action read
        // at model level (RFC 0003: it invalidates the model); its deadlocked states are
        // then not a CML answer, so none are reported and the harness does not compare
        // them.
        let undefined_action = match report.deadlock() {
            DeadlockOutcome::Undefined(undefined) => Some(undefined_read(undefined)),
            _ => None,
        };
        let projection = match (&exploration, report.deadlock()) {
            (Exploration::Complete(reachable), DeadlockOutcome::Undefined(_)) => {
                Projection::Exact {
                    states: reachable.states().iter().map(vector).collect(),
                    deadlocks: BTreeSet::new(),
                }
            }
            (
                Exploration::Complete(reachable),
                DeadlockOutcome::Free { .. } | DeadlockOutcome::Deadlocked { .. },
            ) => Projection::Exact {
                states: reachable.states().iter().map(vector).collect(),
                deadlocks: report
                    .deadlock()
                    .deadlocks()
                    .iter()
                    .map(|d| vector(d.state()))
                    .collect(),
            },
            (_, DeadlockOutcome::Inconclusive(reason)) => {
                Projection::Inconclusive(unresolved(reason))
            }
            (Exploration::Exhausted(partial), _) => Projection::Inconclusive(Inconclusive::new(
                InconclusiveReason::ResourceExhausted,
                format!("{} bound tripped", partial.tripped()),
            )),
            (Exploration::Complete(_), DeadlockOutcome::NotJudged { .. }) => {
                Projection::Inconclusive(Inconclusive::new(
                    InconclusiveReason::EngineError,
                    "deadlock not judged under the defect policy",
                ))
            }
        };
        Normalized {
            projection,
            invariants,
            undefined_action,
        }
    }
}

// ---------------------------------------------------------------------------
// the kernel, through wire-epoch-2 certificates
// ---------------------------------------------------------------------------

/// The reference path's closed set, written as certificates by the reference crate's
/// emitter and **decided by `continuum-kernel-core`**: its own decoder and evaluator
/// re-derive every successor row from the model the certificate carries.
pub struct KernelEngine;

fn envelope<'a>(model_digest: &'a str, property: &'a str) -> ClaimEnvelope<'a> {
    ClaimEnvelope {
        model_digest,
        semantic_epoch: "continuum-semantics-1",
        property_digest: property,
        scope_digest: "blake3:bn-34mw-differential-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    }
}

fn emission(error: &EmissionError) -> Inconclusive {
    // A count or a certificate size past a wire limit is a format the kernel cannot
    // carry: unsupported, not a verdict and not a fault. Every other emission failure
    // (envelope, evaluation, unknown predicate, not closed) is documented unreachable
    // for a well-formed call, so it is the producer failing: a fault.
    let reason = match error {
        EmissionError::CountOutOfRange { .. } | EmissionError::Oversized { .. } => {
            InconclusiveReason::Unsupported
        }
        _ => InconclusiveReason::EngineError,
    };
    Inconclusive::new(reason, error.to_string())
}

/// A verified claim counts only for the question asked: wire epoch 2, this model's
/// identity, and the requested property (the state domain, or this invariant). A
/// certificate the kernel verifies about another model or another predicate is not an
/// answer here (the binding bn-3hk4v closed for `evidence.verify`).
fn binding(claim: &CheckedClaim, model: &Model, invariant: Option<&str>) -> Result<(), String> {
    if claim.wire_epoch() != 2 {
        return Err(format!(
            "verified at wire epoch {}, not 2",
            claim.wire_epoch()
        ));
    }
    if claim.model_identity() != Some(model.identity().as_bytes()) {
        return Err("verified claim is bound to another model".to_owned());
    }
    match (invariant, claim.property(), claim.invariant()) {
        (None, PropertyClass::StateDomain, None) => Ok(()),
        (Some(name), PropertyClass::Invariant, Some(token))
            if token.as_bytes() == name.as_bytes() =>
        {
            Ok(())
        }
        _ => Err("verified claim is about another property".to_owned()),
    }
}

/// The producer's refusal to certify an undefined read (bn-24a5c), as an undefined
/// read: the kind from the model core's classification of the false predicate, the
/// base of its chain, and the state. The harness checks it against the model.
fn emitted_undefined(model: &Model, predicate: &str, state: &State) -> Option<UndefinedRead> {
    let index = model.predicate_index(predicate)?;
    let kind = match Definedness::of(model).guards(index)? {
        Guarded::Action => UndefinedKind::Action,
        Guarded::Predicate(_) => UndefinedKind::Invariant,
    };
    Some(UndefinedRead {
        kind,
        subject: definedness_base(predicate).unwrap_or(predicate).to_owned(),
        state: Some(vector(state)),
        // The refusal names a state of the producer's closed set and no path; the
        // harness proves nothing from it, so on this lane the claim is unproven.
        path: None,
    })
}

/// The kernel's typed undefined-read rejections (bn-iu8eh), as an undefined read: the
/// table state it names (the certificate's table is the closed set in ascending order)
/// and the base of the false predicate's chain.
fn kernel_undefined(
    rejection: &Rejection,
    model: &Model,
    closed: ClosedSet<'_>,
) -> Option<UndefinedRead> {
    let (kind, state, predicate) = match rejection {
        Rejection::UndefinedActionRead { state, predicate } => {
            (UndefinedKind::Action, *state, *predicate)
        }
        Rejection::UndefinedInvariantRead { state, predicate } => {
            (UndefinedKind::Invariant, *state, *predicate)
        }
        _ => return None,
    };
    let name = model
        .predicates()
        .get(usize::try_from(predicate).ok()?)?
        .name()
        .as_str();
    let mut table: Vec<&State> = closed.reachable().states().iter().collect();
    table.sort();
    Some(UndefinedRead {
        kind,
        subject: definedness_base(name).unwrap_or(name).to_owned(),
        state: table.get(usize::try_from(state).ok()?).map(|s| vector(s)),
        path: None,
    })
}

impl Engine for KernelEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: KERNEL.to_owned(),
            build: format!("continuum-kernel-core wire-epoch-2 via {PRODUCER} certificates"),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let exploration = match bfs::explore(model, bounds(budget)) {
            Ok(exploration) => exploration,
            Err(error) => {
                return Normalized::inconclusive(
                    &Inconclusive::new(InconclusiveReason::EngineError, error.to_string()),
                    names(model),
                );
            }
        };
        let Some(closed) = ClosedSet::of(&exploration) else {
            return Normalized::inconclusive(
                &Inconclusive::new(
                    InconclusiveReason::ResourceExhausted,
                    "the producer's exploration did not close; no certificate exists to check",
                ),
                names(model),
            );
        };
        let digest = format!(
            "blake3:{}",
            Blake3Hasher::hash(model.identity().as_bytes()).to_token()
        );
        let mut undefined_action = None;
        let projection = match certificate::emit_finite_closure(
            model,
            closed,
            &envelope(&digest, "blake3:state-domain"),
        ) {
            // The producer refuses to certify a set with an undefined action read. That
            // refusal, which is the reference's own definedness scan and not the
            // kernel's, is reported as this lane's undefined read and checked against
            // the model by the harness; no certificate exists for the kernel to decide.
            // (The lane basis in `claims::LANES` states this limit.)
            Err(EmissionError::Undefined { predicate, state }) => {
                match emitted_undefined(model, &predicate, &state) {
                    Some(read) if read.kind == UndefinedKind::Action => {
                        undefined_action = Some(read);
                        Projection::Inconclusive(Inconclusive::new(
                            InconclusiveReason::Unsupported,
                            "no certificate over a model with an undefined action read",
                        ))
                    }
                    _ => Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::EngineError,
                        format!("the producer refused `{predicate}` for the state domain"),
                    )),
                }
            }
            Err(error) => Projection::Inconclusive(emission(&error)),
            Ok(bytes) => match check_certificate(&bytes) {
                Verdict::Verified(claim) => match binding(&claim, model, None) {
                    Ok(()) => Projection::Cardinality {
                        states: usize::try_from(claim.states()).unwrap_or(usize::MAX),
                    },
                    Err(detail) => Projection::CheckerRejected { detail },
                },
                // An undefined action read is the kernel's step 4, after closure
                // (steps 1-3) held: the set is closed under the lowered relation, and
                // the model has an undefined read, which is compared as such.
                Verdict::Rejected(rejection) => match kernel_undefined(&rejection, model, closed) {
                    Some(read) if read.kind == UndefinedKind::Action => {
                        undefined_action = Some(read);
                        Projection::Cardinality {
                            states: closed.reachable().len(),
                        }
                    }
                    _ => Projection::CheckerRejected {
                        detail: rejection.reason().to_owned(),
                    },
                },
                Verdict::Unsupported(feature) => Projection::Inconclusive(Inconclusive::new(
                    InconclusiveReason::Unsupported,
                    format!("{feature:?}"),
                )),
            },
        };
        let mut invariants = BTreeMap::new();
        for predicate in model.predicates() {
            let name = predicate.name().as_str();
            let property = format!("blake3:invariant-{name}");
            let verdict = match certificate::emit_invariant_closure(
                model,
                closed,
                &envelope(&digest, &property),
                name,
            ) {
                Err(EmissionError::Undefined { predicate, state }) => {
                    match emitted_undefined(model, &predicate, &state) {
                        Some(read) => InvariantVerdict::Undefined(read),
                        None => InvariantVerdict::Inconclusive(Inconclusive::new(
                            InconclusiveReason::EngineError,
                            format!(
                                "the producer refused `{predicate}`, which is not a definedness predicate"
                            ),
                        )),
                    }
                }
                Err(error) => InvariantVerdict::Inconclusive(emission(&error)),
                Ok(bytes) => match check_certificate(&bytes) {
                    Verdict::Verified(claim) => match binding(&claim, model, Some(name)) {
                        Ok(()) => InvariantVerdict::Holds,
                        Err(detail) => InvariantVerdict::NotEstablished { detail },
                    },
                    Verdict::Rejected(rejection) => {
                        match kernel_undefined(&rejection, model, closed) {
                            Some(read) => InvariantVerdict::Undefined(read),
                            None => InvariantVerdict::NotEstablished {
                                detail: rejection.reason().to_owned(),
                            },
                        }
                    }
                    Verdict::Unsupported(feature) => InvariantVerdict::Inconclusive(
                        Inconclusive::new(InconclusiveReason::Unsupported, format!("{feature:?}")),
                    ),
                },
            };
            invariants.insert(name.to_owned(), verdict);
        }
        Normalized {
            projection,
            invariants,
            undefined_action,
        }
    }
}

// ---------------------------------------------------------------------------
// the tiny exhaustive oracle
// ---------------------------------------------------------------------------

/// `semantic::oracle::run` over the model as a plain system: every variable data,
/// every action an unfair work step of one process with its syntactic footprint, no
/// conflicts, obligations or phases. Reports the reachable set and its quiescent
/// states; judges no invariant.
pub struct SemanticOracleEngine;

fn refusal(refused: &Refusal) -> Inconclusive {
    let reason = match refused {
        Refusal::TooLarge { .. } => InconclusiveReason::ResourceExhausted,
        Refusal::NondeterministicAction { .. } => InconclusiveReason::Unsupported,
        Refusal::Exploration(_) | Refusal::Evaluation { .. } | Refusal::Inconsistent { .. } => {
            InconclusiveReason::EngineError
        }
    };
    Inconclusive::new(reason, format!("{refused:?}"))
}

impl Engine for SemanticOracleEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: SEMANTIC_ORACLE.to_owned(),
            build: PRODUCER.to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::PROJECTION_ONLY
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let parts = SystemParts {
            model: model.clone(),
            kinds: model
                .variables()
                .iter()
                .map(|v| (v.name().as_str().to_owned(), VarKind::Data))
                .collect(),
            meta: model
                .actions()
                .iter()
                .map(|action| {
                    (
                        action.name().as_str().to_owned(),
                        ActionMeta {
                            process: 0,
                            role: Role::Work,
                            footprint: Footprint::syntactic(action),
                            fairness: Fairness::Unfair,
                        },
                    )
                })
                .collect(),
            conflicts: BTreeSet::new(),
            obligations: Vec::new(),
            phases: Vec::new(),
            lineage: Vec::new(),
        };
        let system = match System::new(parts) {
            Ok(system) => system,
            Err(SystemError::ModelFairness) => {
                return Normalized {
                    projection: Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::Unsupported,
                        "the tiny oracle's systems carry no model-level fairness",
                    )),
                    invariants: BTreeMap::new(),
                    undefined_action: None,
                };
            }
            Err(error) => {
                return Normalized {
                    projection: Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::EngineError,
                        format!("{error:?}"),
                    )),
                    invariants: BTreeMap::new(),
                    undefined_action: None,
                };
            }
        };
        let limits = Limits {
            max_variables: 16,
            max_actions: 32,
            max_states: budget.states,
            max_depth: 3,
            max_interleavings: 1 << 16,
        };
        let projection = match oracle::run(&system, &limits) {
            Ok(artifact) => Projection::Exact {
                states: artifact.states().iter().map(vector).collect(),
                deadlocks: artifact
                    .quiescent()
                    .iter()
                    .filter_map(|index| artifact.states().get(*index))
                    .map(vector)
                    .collect(),
            },
            Err(refused) => Projection::Inconclusive(refusal(&refused)),
        };
        // Projection only: the tiny oracle judges neither invariants nor definedness.
        Normalized {
            projection,
            invariants: BTreeMap::new(),
            undefined_action: None,
        }
    }
}

// ---------------------------------------------------------------------------
// the partial-order reduction engine (bn-3vhzw, C005)
// ---------------------------------------------------------------------------

/// The `work` bound `dpor::Bounds` needs beyond the corpus's `Budget`: generous
/// enough that a check never refuses for lack of a work reserve
/// (`dpor::CheckError::WorkBelowObligations`), while `states`/`transitions`/`depth`
/// still do the actual bounding (mirrors bn-voq4's own `tests/support/differential.rs`
/// `dpor_bounds`).
const DPOR_WORK: u64 = 1 << 40;

fn dpor_bounds(budget: Budget) -> dpor::Bounds {
    dpor::Bounds::new(budget.states, budget.transitions, budget.depth, DPOR_WORK)
}

/// A bitmask with one bit set per declared variable: the mask a reduction would need
/// to cover to make its visible projection the model's full state domain.
fn full_mask(model: &Model) -> u64 {
    let bits = model.variables().len();
    if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

fn dpor_unresolved(reason: &dpor::Unresolved) -> Inconclusive {
    match reason {
        dpor::Unresolved::ResourceExhausted { tripped, stored } => Inconclusive::new(
            InconclusiveReason::ResourceExhausted,
            format!("{tripped} bound tripped after {stored} states stored"),
        ),
        dpor::Unresolved::EngineError(fault) => {
            Inconclusive::new(InconclusiveReason::EngineError, fault.to_string())
        }
    }
}

fn dpor_trace(model: &Model, trace: &dpor::Trace) -> Trace {
    Trace {
        start: vector(trace.start()),
        steps: trace
            .steps()
            .iter()
            .map(|step| {
                let name = model
                    .actions()
                    .get(step.action())
                    .expect("a dpor trace names a declared action")
                    .name()
                    .as_str()
                    .to_owned();
                (name, vector(step.target()))
            })
            .collect(),
    }
}

fn dpor_undefined(model: &Model, read: &dpor::UndefinedRead) -> UndefinedRead {
    UndefinedRead {
        kind: match read.read() {
            Guarded::Action => UndefinedKind::Action,
            Guarded::Predicate(_) => UndefinedKind::Invariant,
        },
        subject: read.subject().to_owned(),
        state: Some(vector(read.state())),
        path: read.trace().map(|trace| dpor_trace(model, trace)),
    }
}

fn dpor_invariant(model: &Model, outcome: &dpor::InvariantOutcome) -> InvariantVerdict {
    match outcome {
        dpor::InvariantOutcome::Holds { .. } => InvariantVerdict::Holds,
        dpor::InvariantOutcome::Violated { trace, .. } => InvariantVerdict::Violated {
            witness: trace.as_ref().map(|found| dpor_trace(model, found)),
        },
        dpor::InvariantOutcome::Undefined(read) => {
            InvariantVerdict::Undefined(dpor_undefined(model, read))
        }
        dpor::InvariantOutcome::Inconclusive(reason) => {
            InvariantVerdict::Inconclusive(dpor_unresolved(reason))
        }
    }
}

/// `dpor::check` over every model predicate as an invariant, deadlock a defect
/// (`Obligations::every_predicate`, `DeadlockPolicy::Defect`): the RFC 0004 engine
/// that fills the DPOR slot.
///
/// Its reachable-state answer is the reduction's *visible* projection (INV-013: the
/// variables the invariants and every action's definedness chain read;
/// `ReductionWitness::visible`), which docs/33's soundness argument proves
/// exhaustive over the visible valuations — not necessarily over the full state
/// domain the tiny exhaustive oracle enumerates. Claiming `Projection::Exact` (a
/// genuine full reachable set, comparable to the oracle's) is honest only when the
/// visible mask happens to cover every declared variable and the search completed;
/// otherwise the field is typed `Unsupported` (a question this engine's answer does
/// not decide at the full-state grain) or `ResourceExhausted`, never a laundered
/// claim. A full mask is provably the one case where the reduction is a no-op: every
/// progressing label then writes a visible variable, so no stubborn set without an
/// enabled visible label can ever exist and every state expands in full
/// (`witness::FullReason::Exhaustive`) — checked directly over `Stats` by
/// `the_dpor_lane_s_exact_answers_are_provably_unreduced_full_mask_fixtures`, which
/// finds `declined_persistent == declined_sleep == 0` on every full-mask corpus
/// fixture. So agreement on those fixtures is evidence that DPOR's search machinery
/// (visits, the cycle proviso, discovery and replay) agrees with a second,
/// differently-coded exhaustive explorer, never evidence about the persistent-set or
/// sleep-set reduction, which only narrows the search on the fixtures this field
/// cannot claim exact — those still compare deadlocks and verdicts nowhere in this
/// lane (the oracle judges neither), a residual gap the docs/19 §5 basis for this
/// lane accepts because `continuum-engine-dpor`'s own `tests/c005_differential.rs`
/// (bn-voq4) already compares verdicts, visible states and terminal states against
/// the unreduced reference engine exhaustively, including on masked fixtures, where
/// the reduction is genuinely active. The
/// deadlock question and the model-level undefined-action read are exact regardless
/// of the mask (RFC 0004's cycle proviso, and an action's own definedness chain is
/// always visible), but this adapter reports deadlocks only inside the `Exact`
/// answer, since `Projection` has no shape for "deadlocks without a full reachable
/// set" — deliberately conservative, never a claim past what the field can carry. An
/// engine fault the deadlock question itself carries is reported as `EngineError`
/// ahead of completeness or the mask, so it is never relabelled a typed inconclusive
/// nothing quarantines it for. Every trace this adapter offers is the reducer's own
/// discovery path, and the harness replays it against the model independently —
/// nothing here is taken on the reducer's word.
pub struct DporEngine;

impl Engine for DporEngine {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: DPOR.to_owned(),
            build: dpor::ALGORITHM.to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let obligations = dpor::Obligations::every_predicate(model, dpor::DeadlockPolicy::Defect);
        let report = match dpor::check(model, &obligations, dpor_bounds(budget)) {
            Ok(report) => report,
            Err(error) => {
                return Normalized::inconclusive(
                    &Inconclusive::new(InconclusiveReason::EngineError, error.to_string()),
                    names(model),
                );
            }
        };

        let mut invariants = BTreeMap::new();
        for result in report.invariants() {
            invariants.insert(
                result.name().as_str().to_owned(),
                dpor_invariant(model, result.outcome()),
            );
        }

        // The deadlock question's own undefined read is the successor computation's
        // (an action read, RFC 0003), reported as-is like the reference and kernel
        // adapters.
        let undefined_action = match report.deadlock() {
            dpor::DeadlockOutcome::Undefined(read) => Some(dpor_undefined(model, read)),
            _ => None,
        };

        // An engine fault the deadlock question itself carries (an internal
        // invariant broken while reconstructing a terminal state's discovery trace,
        // say) is checked first, ahead of completeness and the mask: an `EngineError`
        // must always enter the defect lifecycle (compare.rs's
        // `engine_error_of_projection`), and relabelling it `ResourceExhausted` (the
        // completeness arm below) or `Unsupported` (the mask arm below) would launder
        // a fault into a typed inconclusive nothing ever quarantines it for. Likewise
        // `NotJudged`: every obligation this adapter asks is `DeadlockPolicy::Defect`,
        // so an answer under the policy never requested is the engine's own contract
        // broken, not this fixture's semantics.
        let fault = match report.deadlock() {
            dpor::DeadlockOutcome::Inconclusive(dpor::Unresolved::EngineError(deadlock_fault)) => {
                Some(deadlock_fault.to_string())
            }
            dpor::DeadlockOutcome::NotJudged { .. } => {
                Some("deadlock not judged although DeadlockPolicy::Defect was requested".to_owned())
            }
            _ => None,
        };
        let projection = if let Some(detail) = fault {
            Projection::Inconclusive(Inconclusive::new(InconclusiveReason::EngineError, detail))
        } else {
            match report.completeness() {
                dpor::Completeness::Exhausted(bound) => {
                    Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::ResourceExhausted,
                        format!("{bound} bound tripped"),
                    ))
                }
                dpor::Completeness::Faulted(fault) => Projection::Inconclusive(Inconclusive::new(
                    InconclusiveReason::EngineError,
                    fault.to_string(),
                )),
                dpor::Completeness::Complete if report.witness().visible() != full_mask(model) => {
                    Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::Unsupported,
                        format!(
                            "the reduced search's exact reachable set is the visible projection \
                             (mask {:#x} of {:#x}); the full state domain is not reported",
                            report.witness().visible(),
                            full_mask(model),
                        ),
                    ))
                }
                dpor::Completeness::Complete => match report.deadlock() {
                    dpor::DeadlockOutcome::Deadlocked { states } => Projection::Exact {
                        states: report.projections().clone(),
                        deadlocks: states.iter().map(|d| vector(d.state())).collect(),
                    },
                    dpor::DeadlockOutcome::Free { .. } | dpor::DeadlockOutcome::Undefined(_) => {
                        Projection::Exact {
                            states: report.projections().clone(),
                            deadlocks: BTreeSet::new(),
                        }
                    }
                    dpor::DeadlockOutcome::Inconclusive(reason) => {
                        Projection::Inconclusive(dpor_unresolved(reason))
                    }
                    // Excluded above: `fault` catches every `EngineError` and every
                    // `NotJudged` before this arm is reached.
                    dpor::DeadlockOutcome::NotJudged { .. } => {
                        Projection::Inconclusive(Inconclusive::new(
                            InconclusiveReason::EngineError,
                            "unreachable: excluded above",
                        ))
                    }
                },
            }
        };

        Normalized {
            projection,
            invariants,
            undefined_action,
        }
    }
}

/// A DPOR that has lost one discovered state: a stand-in for a reduction defect that
/// drops a transition or wakes a sleeping label too late. `continuum-engine-dpor`'s
/// own seeded mutants (`Mutant::NoSleepWakeup`, `Mutant::DropWriteReadDependence`,
/// …) are `#[cfg(test)]`-private to that crate (`src/mutation.rs`) and unreachable
/// from here, so this wraps the honest engine and perturbs its answer instead: the
/// least state (by canonical order) is dropped from the reachable set whenever the
/// wrapped engine reports one exactly. The oracle still reports the true set, so
/// the lane's own comparison catches it — nothing here is told what to expect.
pub struct DporLostState;

impl Engine for DporLostState {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: DPOR.to_owned(),
            build: "planted-mutant/dpor-lost-state".to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        let mut answer = DporEngine.evaluate(model, budget);
        if let Projection::Exact { states, .. } = &mut answer.projection
            && let Some(least) = states.iter().next().cloned()
        {
            states.remove(&least);
        }
        answer
    }
}

// ---------------------------------------------------------------------------
// planted mutants, in the explicit-engine slot
// ---------------------------------------------------------------------------

/// The mutant classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    /// An "optimized" explorer that drops every successor reached by the last
    /// declared action from a non-initial state: a lost-transition bug.
    DropSuccessor,
    /// The reference answer with every counterexample's last state forged.
    ForgeWitness,
    /// The reference answer under a budget, with each invariant's budget stop
    /// laundered into `Holds` (the projection stays the reference's own budget stop).
    LaunderBudget,
    /// The reference answer, except a typed `EngineError` on the reachable-state
    /// projection of every model with two or more variables.
    EngineErrorOnWide,
    /// The reference answer, except a panic on every model with a nondeterministic
    /// action.
    PanicOnNondeterministic,
    /// The reference answer with every invariant verdict dropped, while still
    /// declaring that it judges invariants.
    DropAllVerdicts,
    /// The lost-transition mutant, which also panics on any model with exactly one
    /// action: minimization of its lost transitions meets the panic on candidates.
    DropSuccessorPanicOnSingleAction,
    /// The reference answer with definedness ignored (the pre-bn-24a5c reading): every
    /// undefined read reported as `Holds`, and no model-level undefined action read.
    IgnoreDefinedness,
    /// The reference answer with every undefined read's kind swapped.
    SwapUndefinedKind,
    /// The reference answer with only the model-level undefined action read dropped:
    /// every verdict is kept.
    DropUndefinedAction,
    /// The reference answer with every undefined read moved to an initial state where
    /// its chain may hold: a claim the model does not support.
    ForgeUndefinedState,
}

/// A planted mutant, filling the explicit-engine slot.
pub struct Mutant(pub Mutation);

impl Mutant {
    fn label(&self) -> &'static str {
        match self.0 {
            Mutation::DropSuccessor => "planted-mutant/drop-successor",
            Mutation::ForgeWitness => "planted-mutant/forge-witness",
            Mutation::LaunderBudget => "planted-mutant/launder-budget",
            Mutation::EngineErrorOnWide => "planted-mutant/engine-error-on-wide",
            Mutation::PanicOnNondeterministic => "planted-mutant/panic-on-nondeterministic",
            Mutation::DropAllVerdicts => "planted-mutant/drop-all-verdicts",
            Mutation::IgnoreDefinedness => "planted-mutant/ignore-definedness",
            Mutation::SwapUndefinedKind => "planted-mutant/swap-undefined-kind",
            Mutation::DropUndefinedAction => "planted-mutant/drop-undefined-action",
            Mutation::ForgeUndefinedState => "planted-mutant/forge-undefined-state",
            Mutation::DropSuccessorPanicOnSingleAction => {
                "planted-mutant/drop-successor-panic-single"
            }
        }
    }
}

/// Breadth-first search that skips the last action's successors out of non-initial
/// states. Its own code, so the mutant is a whole wrong engine, not a patched answer.
type Parents = BTreeMap<State, Option<(State, usize)>>;

fn lossy_explore(model: &Model, budget: Budget) -> Option<(Parents, BTreeSet<State>)> {
    let last = model.actions().len().checked_sub(1)?;
    let mut parents: Parents = BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();
    for initial in model.initial_states() {
        parents.insert(initial.clone(), None);
        queue.push_back(initial.clone());
    }
    let mut deadlocks = BTreeSet::new();
    while let Some(state) = queue.pop_front() {
        let mut steps = model.successors(&state).ok()?;
        if !model.initial_states().contains(&state) {
            steps.retain(|step| step.action() != last);
        }
        if steps.is_empty() {
            deadlocks.insert(state.clone());
        }
        for step in steps {
            if parents.contains_key(step.target()) {
                continue;
            }
            if parents.len() >= budget.states {
                return None;
            }
            parents.insert(step.target().clone(), Some((state.clone(), step.action())));
            queue.push_back(step.target().clone());
        }
    }
    Some((parents, deadlocks))
}

impl Engine for Mutant {
    fn identity(&self) -> EngineIdentity {
        EngineIdentity {
            slot: EXPLICIT.to_owned(),
            build: self.label().to_owned(),
        }
    }

    fn fields(&self) -> Fields {
        Fields::ALL
    }

    fn evaluate(&self, model: &Model, budget: Budget) -> Normalized {
        match self.0 {
            Mutation::DropSuccessor => {
                let Some((parents, deadlocks)) = lossy_explore(model, budget) else {
                    return Normalized::inconclusive(
                        &Inconclusive::new(InconclusiveReason::ResourceExhausted, "mutant budget"),
                        names(model),
                    );
                };
                let mut invariants = BTreeMap::new();
                for (index, predicate) in model.predicates().iter().enumerate() {
                    let violated = parents
                        .keys()
                        .any(|state| matches!(model.evaluate_predicate(index, state), Ok(false)));
                    let verdict = if violated {
                        InvariantVerdict::Violated { witness: None }
                    } else {
                        InvariantVerdict::Holds
                    };
                    invariants.insert(predicate.name().as_str().to_owned(), verdict);
                }
                Normalized {
                    projection: Projection::Exact {
                        states: parents.keys().map(vector).collect(),
                        deadlocks: deadlocks.iter().map(vector).collect(),
                    },
                    invariants,
                    undefined_action: None,
                }
            }
            Mutation::ForgeWitness => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                for verdict in answer.invariants.values_mut() {
                    if let InvariantVerdict::Violated {
                        witness: Some(trace),
                    } = verdict
                    {
                        // Forge the end: claim the path stops where it started.
                        let start = trace.start.clone();
                        match trace.steps.last_mut() {
                            Some((_, end)) => *end = start,
                            None => trace.steps.push(("forged".to_owned(), start)),
                        }
                    }
                }
                answer
            }
            Mutation::LaunderBudget => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                for verdict in answer.invariants.values_mut() {
                    if matches!(verdict, InvariantVerdict::Inconclusive(_)) {
                        *verdict = InvariantVerdict::Holds;
                    }
                }
                answer
            }
            Mutation::EngineErrorOnWide => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                if model.variables().len() >= 2 {
                    answer.projection = Projection::Inconclusive(Inconclusive::new(
                        InconclusiveReason::EngineError,
                        "planted engine error",
                    ));
                }
                answer
            }
            Mutation::PanicOnNondeterministic => {
                assert!(
                    model.actions().iter().all(|a| a.is_deterministic()),
                    "planted panic on a nondeterministic action"
                );
                ReferenceEngine.evaluate(model, budget)
            }
            Mutation::IgnoreDefinedness => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                for verdict in answer.invariants.values_mut() {
                    if matches!(verdict, InvariantVerdict::Undefined(_)) {
                        *verdict = InvariantVerdict::Holds;
                    }
                }
                answer.undefined_action = None;
                answer
            }
            Mutation::DropUndefinedAction => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                answer.undefined_action = None;
                answer
            }
            Mutation::SwapUndefinedKind => {
                let swap = |read: &mut UndefinedRead| {
                    read.kind = match read.kind {
                        UndefinedKind::Action => UndefinedKind::Invariant,
                        UndefinedKind::Invariant => UndefinedKind::Action,
                    };
                };
                let mut answer = ReferenceEngine.evaluate(model, budget);
                for verdict in answer.invariants.values_mut() {
                    if let InvariantVerdict::Undefined(read) = verdict {
                        swap(read);
                    }
                }
                answer
            }
            Mutation::ForgeUndefinedState => {
                // Move each claim to the reached state where the claimed chain holds, if
                // there is one: the claim then names a state where the read is defined.
                let mut answer = ReferenceEngine.evaluate(model, budget);
                let reached: Option<BTreeSet<Vec<i64>>> = match &answer.projection {
                    Projection::Exact { states, .. } => Some(states.clone()),
                    _ => None,
                };
                let states: Vec<Vec<i64>> = reached.iter().flatten().cloned().collect();
                let forge = |read: &mut UndefinedRead, invariant: Option<&str>| {
                    for state in &states {
                        let mut moved = read.clone();
                        moved.state = Some(state.clone());
                        if continuum_corpus::differential::check_undefined(
                            model,
                            &moved,
                            invariant,
                            reached.as_ref(),
                        )
                        .is_err()
                        {
                            *read = moved;
                            return;
                        }
                    }
                };
                for (name, verdict) in &mut answer.invariants {
                    if let InvariantVerdict::Undefined(read) = verdict {
                        forge(read, Some(name));
                    }
                }
                if let Some(read) = answer.undefined_action.as_mut() {
                    forge(read, None);
                }
                answer
            }
            Mutation::DropSuccessorPanicOnSingleAction => {
                assert!(model.actions().len() != 1, "planted single-action panic");
                Mutant(Mutation::DropSuccessor).evaluate(model, budget)
            }
            Mutation::DropAllVerdicts => {
                let mut answer = ReferenceEngine.evaluate(model, budget);
                answer.invariants.clear();
                answer
            }
        }
    }
}
