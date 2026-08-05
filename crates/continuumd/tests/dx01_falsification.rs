//! G0-DX-01 falsification campaign: *can an agent diagnose from a bounded artifact
//! rather than raw logs?* (bn-37gu)
//!
//! # Why this file exists
//!
//! `notes/plan/notes/G0_SPIKE_MATRIX.md` states the G0-DX-01 experiment as **"200+ event
//! noisy durability failure → Context Pack"** with pass condition **"causal core
//! replay-preserving; ≥10× context reduction; exact expansion handles"** and failure
//! consequence **"redesign evidence model"**. The Phase A evidence for this row was a
//! finite Python spike (`spikes/R3_SPIKE_REPORT.md` §1: 200 events, 4-event core, 747-byte
//! pack, 23.62×) that validated the artifact *shape* only — docs/53 is explicit that it
//! proved no engine, and the matrix's promotion rule refuses to turn a spike into
//! architecture without adversarial mutations. The production Context Pack surface has
//! since landed (PR-11, all six IMPL bullets: `continuum-context`'s typed references,
//! omission manifest, closed accounting, content-derived expansion handles, byte-budget
//! packer, and `continuumd`'s live `context.expand`). This file executes the row's
//! experiment against that production surface, at the deepest grain the landed code
//! supports, and it is deliberately hostile to it.
//!
//! # Honest scope: each leg's grain, stated before any number
//!
//! RFC 0028's **ten-stage compiler is not wired anywhere in this workspace** —
//! `context.compile` is refused `UnsupportedSemanticFeature` by
//! `daemon::context::ContextFamily`, and `continuum-context::pack`'s own documentation
//! declines whole-pack compilation by name. A campaign that hid that behind a hand-built
//! pack and called it production would be a finite artifact-shape spike wearing a costume.
//! Instead, each leg runs at its deepest honestly available grain, declared here:
//!
//! | leg | grain | what is production | what is harness, and why |
//! |---|---|---|---|
//! | causal core replay-preserving | **partial** | the replay *check*: `continuum-engine-reference`'s own `Model::action_successors` decides whether the core replays step for step, and its own `checking::check`/`evaluate_predicate` decides whether the failure reproduces — RFC 0028 "Validation" makes every guarantee checker independent of the compiler by design, so the check side is exactly what production runs | the core *selection*: stage 2 (backward causal slicing) belongs to the unbuilt compiler, so [`causal_core`] implements dynamic dependence slicing in harness code over the model's own declared read/write sets (the production expression AST, `BoolExpr::variables`/`IntExpr::variables`) |
//! | ≥10× context reduction | **partial** | the counting rule: measured bytes = the whole published canonical encoding (RFC 0028 correction 17, `continuum-context::pack`), checked here via `pack::budget_bytes_of` against the document's own length; and every fragment of the pack — `SelectedItem`, `Manifest`, `Target`, `Verdict`, `Assurance`, `ReplayRef` canonical JSON — is the production rendering | root-pack *assembly*: no landed bullet claims the 17-key root document writer, so [`assemble_root_pack`] joins the production fragments and applies the packer's own least-fixed-point measurement reading |
//! | exact expansion handles | **production, end to end** | the parent pack is registered through `DaemonState::put_context_pack` (the same out-of-band surface a deployment uses), every expansion runs through `Daemon::dispatch`, the promise is derived from the published omission record alone, and the answer is the daemon's own | nothing |
//!
//! **Missing producers, named and routed** (the residual of this row): (1) the ten-stage
//! compiler — `context.compile`'s producer; it must provide candidate-set construction
//! from the evidence graph and stage-2 backward causal slicing licensing
//! `CausallyClosed`/`ReplayPreserving`, with auditable intermediates (RFC 0030, compared
//! artifact 5); (2) whole-pack root assembly — the writer that reconciles the `ctx_*`
//! question identity with the store's content identity (`daemon::context`'s declined
//! publication decision); (3) the crashpack producer — "nothing in this workspace builds
//! a crashpack" (`daemon::task`), so the pack's `replay` names a class-checked `crash_*`
//! handle derived from the core replay recipe with no artifact behind it, and the
//! `in_*` intent handle names the contract a PR-5 registry would resolve, which none
//! does. All three route to bn-3m65 (the PR-11 exit bone) and the compiler's own bone.
//! Two typed-vocabulary absences recorded the same way: `EventRole` is docs/38's
//! two-member *causal-core* split, so a slice-irrelevant trace event has no truthful
//! `event`-kind spelling — the omitted noise is therefore accounted as its concrete
//! per-variable transitions (`StateDeltaRef`, statements that are true), a shape choice
//! the compiler bone revisits; and no landed producer serializes a trace, so the raw
//! baseline is defined below rather than read off an artifact.
//!
//! # The experiment
//!
//! **The trace producer is the real engine.** [`durability_model`] declares an
//! RFC-0007-vocabulary storage failure as inert data in `continuum-engine-reference`'s
//! own model language: a writer that acknowledges before it flushes (`submit_write` →
//! `ack_before_flush`, with `flush_wal` declared and never scheduled), a `power_loss`
//! that wipes the volatile buffer, and the invariant `AckImpliesDurable`
//! (`client_acked = 1 → wal_durable = 1 ∨ wal_buffered = 1`). Around that core, noise:
//! a benign second writer that does the protocol *correctly* and carries
//! durability-vocabulary names (`submit_write_replica`, `flush_wal_replica`,
//! `ack_after_flush_replica`) as deliberate red herrings, plus telemetry, scrubber,
//! heartbeat, and UPS actors — including `telemetry_flush` and `scrub_write_verify`,
//! noise with durability-sounding names, and `power_loss`, a causal event *without* one.
//! A declared deterministic schedule (the dx10 declared-schedule precedent) drives 200+
//! events through `Model::action_successors` — every step's guard evaluation and update
//! application is the production engine's — and the refuted verdict is the production
//! `checking::check` over a complete exploration, corroborated by `witness::shortest`.
//!
//! # Pre-registered expectations
//!
//! Fixed from the governing texts (RFC 0028, docs/38, the matrix row) before the campaign
//! ran, not fitted to what the code produced:
//!
//! | id | claim | required outcome |
//! |---|---|---|
//! | C01 | the 200+ event schedule replays through the production engine and its final state refutes `AckImpliesDurable` | refuted at the trace's own end state |
//! | C02 | the production `checking::check` refutes the invariant over the complete exploration, and `witness::shortest` finds a 4-step counterexample whose actions are exactly the core's | `Verdict::Refuted`; witness = core |
//! | L1 | the 4-event causal core replays through `Model::action_successors` and reproduces the refutation; noise ∩ core = ∅; the demanded sub-state agrees with the original trace at every core step (causal closure, witnessed) | replay-preserving |
//! | A01 | dropping any single causal event breaks the replay or loses the refutation — the comparison can fail, and each of the four rows fails the pre-registered way | begin_txn → disabled at `submit_write`; submit_write → disabled at `ack_before_flush`; ack_before_flush → completes without refuting; power_loss → completes without refuting |
//! | A02 | a textually assembled core (durability vocabulary: `write`/`flush`/`ack`/`wal`) admits noise, misses `begin_txn` and `power_loss`, is not strictly replayable, and even a forgiving skip-disabled replay of it does not refute | C2's "a set assembled from textual relevance is not accepted as replay-preserving" |
//! | A03 | the core out of order does not replay | disabled at the swapped step |
//! | L2 | the assembled pack, measured under the packer's own counting rule, is ≥10× smaller than the raw trace baseline | ratio ≥ 10.00× |
//! | A04 | the production accounting refuses every undercount on this dataset: a payload whose count disagrees, an undispositioned candidate, a selection from outside the candidate set, a no-change delta, a wrong-class replay handle | five typed refusals |
//! | L3 | the `ctx_*` the published omission record promises — derived from the record alone — is the `ctx_*` `Daemon::dispatch` returns, and the expansion returns exactly the omitted items, under an empty residual manifest (the assertion of completeness); a second call under a different idempotency key returns the same handle and byte-identical pack | exact handles, wire-live |
//! | A05 | a tampered record promises a *different* handle (the promise check can fail); a hostile anchor is `MalformedRequest`; a defined anchor with an unregistered relation is `UnsupportedSemanticFeature`; an unheld pack is `CapabilityDenied` | four typed refusals |
//! | L3b | under a ceiling the full answer exceeds, the packed child's `budget` shortfall is recoverable by the *same* promised handle, conservation holds at every publishing ceiling, and below the minimal child nothing is published (`BudgetExhausted`, typed `non_resumable_reason`) | the production packer keeps the promise |
//! | D | two independent builds of the whole campaign render byte-identical evidence, pinned against a golden artifact | deterministic |
//!
//! # The reduction baseline, defined before the numbers
//!
//! No trace serializer is landed, so the baseline is defined here, conservatively, and
//! its sensitivity is reported rather than hidden (the dx10 precedent: accounting
//! choices decide margins, so every reading is published). The **graded** baseline is
//! the canonical JSON of the engine-grain witness form — the trace as "a sequence of
//! `(action, state)` pairs" (`witness.rs`'s own definition of the artifact you hand
//! somebody), each state a self-describing named record, exactly the shape the Phase A
//! spike's named baseline used (`json.dumps([asdict(e) …])`). Reported beside it,
//! ungraded: the engine's own positional `Display` rendering, a positional-vector JSON,
//! a changed-variables-only delta log, and a bare action-name log, each with its ratio,
//! so an adjudicator sees exactly where the 10× margin's sign lives.
//!
//! # House rules
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited (the
//!   bn-21dd precedent).
//! - Deterministic (INV-005): no clock, no entropy, no float, `BTree`-ordered
//!   everything; the whole campaign renders to one byte-stable evidence artifact pinned
//!   at `tests/golden/dx01_falsification_evidence.txt`.
//! - OOM hygiene: every input is built linearly (a 225-entry schedule, one `map` per
//!   item family); the largest value in this file is the ~25 KiB expansion child.
//!
//! # Result of the campaign
//!
//! **All three legs held at the grain declared above; no attack landed.** Measured
//! (every number pinned in `tests/golden/dx01_falsification_evidence.txt`):
//!
//! - **The trace**: 225 events, 221 noise, through the production engine; the complete
//!   exploration (5,248 reachable states) refutes `AckImpliesDurable`, and the engine's
//!   own `witness::shortest` finds exactly the four core actions, in order — an
//!   independent production corroboration of the harness slice.
//! - **Leg 1**: the 4-event core replays through `Model::action_successors` and
//!   reproduces the refutation (`client_acked=1, wal_durable=0, wal_buffered=0`); every
//!   drop-one mutant fails its pre-registered way (two disabled steps, two lost
//!   refutations — both failure modes exercised, so the comparison can fail); the
//!   reordered core is disabled; the textual-relevance control admits 67 noise events,
//!   misses `begin_txn` and `power_loss`, is not strictly replayable, and does not
//!   refute even forgivingly — C2's exclusion, demonstrated.
//! - **Leg 2**: pack 2,791 bytes (equal to its own published canonical length — the
//!   packer's counting rule, checked), raw trace 48,764 bytes under the graded
//!   self-describing witness-form baseline: **17.47×**. The sensitivity table is the
//!   honest half: under the engine's positional `Display` the ratio is 4.92×, under a
//!   positional-vector JSON 4.50×, under a changed-variables delta log 3.93×, under a
//!   bare action-name log 1.44× — the 10× margin's sign flips below the self-describing
//!   form, none of which any landed producer emits, and the pack's fixed answer header
//!   (1,498 of 2,791 bytes) is where a redesign would look first.
//! - **Leg 3**: promised `ctx_*` = returned `ctx_*` through `Daemon::dispatch`; the
//!   expansion returns exactly the 248 omitted transition items, byte for byte, under
//!   an empty residual manifest; a second idempotency key returns the same handle and
//!   byte-identical pack; the packed branch (ceiling = answer − 400) keeps the promise
//!   in its `recoverable_by` and conserves all 248; below the minimal child, a typed
//!   `BudgetExhausted` publishes nothing.
//!
//! The row's residual is the compiler, not the artifact contract.

use std::collections::{BTreeMap, BTreeSet};

use continuum_context::accounting::{Accounting, CandidateSet, Omitted};
use continuum_context::assurance::Assurance;
use continuum_context::event::{EventRef, EventRole};
use continuum_context::expansion::{
    Depth, ExpansionHandle, ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::model::ModelActionRef;
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::pack;
use continuum_context::replay::{ReplayRef, ReplayRefError};
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::state_delta::{StateDeltaClass, StateDeltaRef, StateDeltaRefError};
use continuum_context::target::{Question, Target};
use continuum_context::verdict::Verdict as PackVerdict;
use continuum_engine_reference::checking::{DeadlockPolicy, Obligations, Verdict as EngineVerdict};
use continuum_engine_reference::witness::Target as WitnessTarget;
use continuum_engine_reference::{
    ActionDecl, BoolExpr, Bounds, CmpOp, Exploration, IntExpr, Model, ModelBuilder, State,
    checking, explore, witness,
};
use continuum_intent::canonical_json::Json;
use continuum_value::assurance::{
    AssuranceDimension, AssuranceEnvelope, AssuranceLevel, DimensionEvidence, UnsupportedReason,
};
use continuum_value::epoch::ProtocolWindow;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_value::value::{Name, Value};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, ContextHandle, EpochIdentity, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, ExpansionRelation, OmissionReason, ResultStatus,
};

// =====================================================================================
// the durability model — inert data in the production engine's own language
// =====================================================================================

/// The model's thirteen variables, in declaration order (the state vector's order).
const VARS: [&str; 13] = [
    "txn",
    "wal_buffered",
    "wal_durable",
    "client_acked",
    "crashed",
    "recovered",
    "rep_buffered",
    "rep_durable",
    "rep_acked",
    "telemetry",
    "scrub",
    "heartbeat",
    "ups",
];

/// The four causal events, in trace order — pre-registered, then recomputed by the slice.
const CORE_ACTIONS: [&str; 4] = [
    "begin_txn",
    "submit_write",
    "ack_before_flush",
    "power_loss",
];

const INVARIANT: &str = "AckImpliesDurable";
const SNAPSHOT: &str = "ws_dx01ackwal";
const SEMANTIC_EPOCH: &str = "sem3-r3-dx01";
const QUESTION: &str = "why did AckImpliesDurable fail on the ackwal durability trace?";
const NOW: &str = "2026-08-01T00:00:00.000Z";

fn eq(variable: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, IntExpr::var(variable), IntExpr::constant(value))
}

fn lt(variable: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Lt, IntExpr::var(variable), IntExpr::constant(value))
}

fn and(clauses: Vec<BoolExpr>) -> BoolExpr {
    clauses
        .into_iter()
        .reduce(BoolExpr::and)
        .expect("at least one clause")
}

fn plus_one(variable: &str) -> IntExpr {
    IntExpr::plus(IntExpr::var(variable), IntExpr::constant(1))
}

/// The RFC-0007-vocabulary durability failure, with its noise, as one declared model.
///
/// The bug is `ack_before_flush`: the acknowledgement's guard reads the volatile buffer,
/// not the durable bit. `flush_wal` is declared — the correct protocol exists in the
/// model — and the schedule below simply loses the race, which is what makes this a
/// schedule-shaped durability failure rather than a malformed model.
fn durability_model() -> Model {
    ModelBuilder::new()
        .variable("txn", 0, 1)
        .variable("wal_buffered", 0, 1)
        .variable("wal_durable", 0, 1)
        .variable("client_acked", 0, 1)
        .variable("crashed", 0, 1)
        .variable("recovered", 0, 1)
        .variable("rep_buffered", 0, 1)
        .variable("rep_durable", 0, 1)
        .variable("rep_acked", 0, 1)
        .variable("telemetry", 0, 3)
        .variable("scrub", 0, 3)
        .variable("heartbeat", 0, 1)
        .variable("ups", 0, 1)
        // --- the failing writer (the causal core's actions) ---
        .action(ActionDecl::deterministic(
            "begin_txn",
            and(vec![eq("crashed", 0), eq("txn", 0)]),
            vec![("txn", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "submit_write",
            and(vec![
                eq("crashed", 0),
                eq("txn", 1),
                eq("wal_buffered", 0),
                eq("wal_durable", 0),
                eq("client_acked", 0),
            ]),
            vec![("wal_buffered", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "ack_before_flush",
            and(vec![
                eq("crashed", 0),
                eq("wal_buffered", 1),
                eq("client_acked", 0),
            ]),
            vec![("client_acked", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "flush_wal",
            and(vec![eq("crashed", 0), eq("wal_buffered", 1)]),
            vec![
                ("wal_durable", IntExpr::constant(1)),
                ("wal_buffered", IntExpr::constant(0)),
            ],
        ))
        .action(ActionDecl::deterministic(
            "power_loss",
            eq("crashed", 0),
            vec![
                ("crashed", IntExpr::constant(1)),
                ("wal_buffered", IntExpr::constant(0)),
                ("rep_buffered", IntExpr::constant(0)),
            ],
        ))
        .action(ActionDecl::deterministic(
            "recover",
            and(vec![eq("crashed", 1), eq("recovered", 0)]),
            vec![
                ("crashed", IntExpr::constant(0)),
                ("recovered", IntExpr::constant(1)),
            ],
        ))
        // --- the benign replica writer: correct order, red-herring names ---
        .action(ActionDecl::deterministic(
            "submit_write_replica",
            and(vec![
                eq("crashed", 0),
                eq("rep_buffered", 0),
                eq("rep_durable", 0),
                eq("rep_acked", 0),
            ]),
            vec![("rep_buffered", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "flush_wal_replica",
            and(vec![eq("crashed", 0), eq("rep_buffered", 1)]),
            vec![
                ("rep_durable", IntExpr::constant(1)),
                ("rep_buffered", IntExpr::constant(0)),
            ],
        ))
        .action(ActionDecl::deterministic(
            "ack_after_flush_replica",
            and(vec![
                eq("crashed", 0),
                eq("rep_durable", 1),
                eq("rep_acked", 0),
            ]),
            vec![("rep_acked", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "retire_txn_replica",
            and(vec![eq("rep_acked", 1)]),
            vec![
                ("rep_acked", IntExpr::constant(0)),
                ("rep_durable", IntExpr::constant(0)),
            ],
        ))
        // --- observer-independent noise ---
        .action(ActionDecl::deterministic(
            "telemetry_tick",
            and(vec![eq("crashed", 0), lt("telemetry", 3)]),
            vec![("telemetry", plus_one("telemetry"))],
        ))
        .action(ActionDecl::deterministic(
            "telemetry_flush",
            and(vec![eq("crashed", 0), eq("telemetry", 3)]),
            vec![("telemetry", IntExpr::constant(0))],
        ))
        .action(ActionDecl::deterministic(
            "scrub_advance",
            lt("scrub", 3),
            vec![("scrub", plus_one("scrub"))],
        ))
        .action(ActionDecl::deterministic(
            "scrub_write_verify",
            eq("scrub", 3),
            vec![("scrub", IntExpr::constant(0))],
        ))
        .action(ActionDecl::deterministic(
            "heartbeat_up",
            and(vec![eq("crashed", 0), eq("heartbeat", 0)]),
            vec![("heartbeat", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "heartbeat_down",
            eq("heartbeat", 1),
            vec![("heartbeat", IntExpr::constant(0))],
        ))
        .action(ActionDecl::deterministic(
            "ups_beep_on",
            eq("ups", 0),
            vec![("ups", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "ups_beep_off",
            eq("ups", 1),
            vec![("ups", IntExpr::constant(0))],
        ))
        .initial_state(&VARS.map(|name| (name, 0i64)))
        .predicate(
            INVARIANT,
            BoolExpr::implies(
                eq("client_acked", 1),
                BoolExpr::or(eq("wal_durable", 1), eq("wal_buffered", 1)),
            ),
        )
        .build()
        .expect("the durability model is well formed")
}

// =====================================================================================
// the declared schedule, and the trace it produces through the production engine
// =====================================================================================

/// One pre-crash noise round: sixteen events, every actor back at its start.
const PRE_ROUND: [&str; 16] = [
    "telemetry_tick",
    "telemetry_tick",
    "telemetry_tick",
    "telemetry_flush",
    "scrub_advance",
    "scrub_advance",
    "scrub_advance",
    "scrub_write_verify",
    "heartbeat_up",
    "heartbeat_down",
    "ups_beep_on",
    "ups_beep_off",
    "submit_write_replica",
    "flush_wal_replica",
    "ack_after_flush_replica",
    "retire_txn_replica",
];

/// One post-crash noise round: what still runs with the machine down.
const POST_ROUND: [&str; 6] = [
    "scrub_advance",
    "scrub_advance",
    "scrub_advance",
    "scrub_write_verify",
    "ups_beep_on",
    "ups_beep_off",
];

/// The declared schedule: 225 events, the four causal ones at declared positions.
fn schedule() -> Vec<&'static str> {
    let mut plan: Vec<&'static str> = Vec::new();
    let pre_rounds = |plan: &mut Vec<&'static str>, rounds: usize| {
        for _ in 0..rounds {
            plan.extend(PRE_ROUND);
        }
    };
    pre_rounds(&mut plan, 4);
    plan.push("begin_txn");
    pre_rounds(&mut plan, 3);
    plan.push("submit_write");
    pre_rounds(&mut plan, 3);
    plan.push("ack_before_flush");
    pre_rounds(&mut plan, 1);
    plan.push("power_loss");
    for _ in 0..2 {
        plan.extend(POST_ROUND);
    }
    plan.push("recover");
    pre_rounds(&mut plan, 2);
    plan
}

/// One event of the trace: the production engine's own step, with its endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceEvent {
    index: usize,
    action: usize,
    name: String,
    before: State,
    after: State,
}

impl TraceEvent {
    fn id(&self) -> Name {
        Name::new(&format!("e_{:04}", self.index)).expect("a canonical identifier")
    }

    /// The variables this event changed, as `(name, before, after)` triples.
    fn changed(&self, model: &Model) -> Vec<(String, i64, i64)> {
        let before = self.before.as_slice();
        let after = self.after.as_slice();
        model
            .variables()
            .iter()
            .enumerate()
            .filter(|(index, _)| before[*index] != after[*index])
            .map(|(index, variable)| {
                (
                    variable.name().as_str().to_owned(),
                    before[index],
                    after[index],
                )
            })
            .collect()
    }
}

/// Drive the declared schedule through `Model::action_successors` — the production
/// engine's own guard evaluation and update application, step for step.
fn drive(model: &Model) -> Vec<TraceEvent> {
    let mut state = model.initial_states()[0].clone();
    let mut trace: Vec<TraceEvent> = Vec::new();
    for (index, name) in schedule().into_iter().enumerate() {
        let action = model
            .action_index(name)
            .unwrap_or_else(|| panic!("the schedule names a declared action: {name}"));
        let successors = model
            .action_successors(action, &state)
            .expect("the model evaluates");
        assert_eq!(
            successors.len(),
            1,
            "the schedule fires only enabled deterministic actions; `{name}` at index \
             {index} had {} successors",
            successors.len()
        );
        let after = successors[0].clone();
        trace.push(TraceEvent {
            index,
            action,
            name: name.to_owned(),
            before: state.clone(),
            after: after.clone(),
        });
        state = after;
    }
    trace
}

// =====================================================================================
// stage-2 slicing (harness grain, declared) and replay (production grain)
// =====================================================================================

/// The variables one action reads: its guard's, plus every update expression's.
fn reads(model: &Model, action: usize) -> BTreeSet<String> {
    let declared = &model.actions()[action];
    let mut names: Vec<String> = Vec::new();
    declared.guard().variables(&mut names);
    for outcome in declared.outcomes() {
        for assignment in outcome.assignments() {
            assignment.value().variables(&mut names);
        }
    }
    names.into_iter().collect()
}

/// The variables one action writes.
fn writes(model: &Model, action: usize) -> BTreeSet<String> {
    let declared = &model.actions()[action];
    let mut names: BTreeSet<String> = BTreeSet::new();
    for outcome in declared.outcomes() {
        for assignment in outcome.assignments() {
            names.insert(assignment.variable().as_str().to_owned());
        }
    }
    names
}

/// The variables the invariant reads — the demand the slice starts from.
fn property_reads(model: &Model) -> BTreeSet<String> {
    let index = model
        .predicate_index(INVARIANT)
        .expect("the invariant is declared");
    let mut names: Vec<String> = Vec::new();
    model.predicates()[index].body().variables(&mut names);
    names.into_iter().collect()
}

/// **Harness stage 2** (declared): dynamic backward dependence slicing over the trace.
///
/// Walk the trace backwards with a demand set seeded from the property's reads; keep an
/// event iff it writes a demanded variable, and when keeping it, demand everything it
/// reads. Every affirmative *check* of what this returns — replay, closure, refutation —
/// runs through the production engine, and [`a01_dropping_any_causal_event_fails`] shows
/// the checks reject this function's own plausible mutants (a slicer that ignored guard
/// reads returns exactly the drop-`begin_txn` core, which fails replay).
fn causal_core(model: &Model, trace: &[TraceEvent]) -> (Vec<usize>, BTreeSet<String>) {
    let mut demand = property_reads(model);
    let mut kept: Vec<usize> = Vec::new();
    for event in trace.iter().rev() {
        if writes(model, event.action).intersection(&demand).count() > 0 {
            kept.push(event.index);
            demand.extend(reads(model, event.action));
        }
    }
    kept.reverse();
    (kept, demand)
}

/// What replaying a projected action sequence through the production engine produced.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Replay {
    /// Every step was enabled and the final state refutes the invariant.
    Refutes(State),
    /// Every step was enabled but the final state satisfies the invariant.
    CompletesWithoutRefuting(State),
    /// A step's guard refused — the projection is not an execution of the model.
    Disabled { position: usize, action: String },
}

/// Replay a sequence of actions from the initial state, strictly: a disabled step is a
/// typed failure, never skipped. Every step is `Model::action_successors` — production.
fn replay(model: &Model, actions: &[usize]) -> Replay {
    let invariant = model
        .predicate_index(INVARIANT)
        .expect("the invariant is declared");
    let mut state = model.initial_states()[0].clone();
    for (position, action) in actions.iter().enumerate() {
        let successors = model
            .action_successors(*action, &state)
            .expect("the model evaluates");
        match successors.as_slice() {
            [] => {
                return Replay::Disabled {
                    position,
                    action: model.actions()[*action].name().as_str().to_owned(),
                };
            }
            [next] => state = next.clone(),
            _ => panic!("every action in this model is deterministic"),
        }
    }
    if model
        .evaluate_predicate(invariant, &state)
        .expect("the invariant evaluates")
    {
        Replay::CompletesWithoutRefuting(state)
    } else {
        Replay::Refutes(state)
    }
}

/// The forgiving variant [`a02_textual_relevance_is_refused`] grants the textual
/// selector: a disabled step is skipped rather than fatal.
fn replay_skipping_disabled(model: &Model, actions: &[usize]) -> Replay {
    let invariant = model
        .predicate_index(INVARIANT)
        .expect("the invariant is declared");
    let mut state = model.initial_states()[0].clone();
    for action in actions {
        let successors = model
            .action_successors(*action, &state)
            .expect("the model evaluates");
        if let [next] = successors.as_slice() {
            state = next.clone();
        }
    }
    if model
        .evaluate_predicate(invariant, &state)
        .expect("the invariant evaluates")
    {
        Replay::CompletesWithoutRefuting(state)
    } else {
        Replay::Refutes(state)
    }
}

// =====================================================================================
// the compile: candidates, dispositions, and the assembled root pack
// =====================================================================================

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn delta_id(index: usize, variable: &str) -> Name {
    name(&format!("d_{index:04}_{variable}"))
}

/// Everything the compile produced, kept together so every test reads one construction.
struct Compile {
    model: Model,
    trace: Vec<TraceEvent>,
    core: Vec<usize>,
    /// The pack's `selected[]`, in canonical order.
    selected: Vec<SelectedItem>,
    /// The omitted noise transitions, exactly as the expansion must return them.
    omitted_items: Vec<SelectedItem>,
    /// The one manifest record the accounting derived.
    record: OmissionRecord,
    /// The query that retrieves the omitted group.
    query: ExpansionQuery,
    candidate_count: u32,
    /// The measured root pack document.
    root: Json,
    root_id: ArtifactHandle,
}

/// The expansion anchor: the violation's own event — the last causal event.
fn anchor_of(core: &[usize], trace: &[TraceEvent]) -> Name {
    trace[*core.last().expect("a non-empty core")].id()
}

fn compile() -> Compile {
    let model = durability_model();
    let trace = drive(&model);
    let (core, _) = causal_core(&model, &trace);
    let core_set: BTreeSet<usize> = core.iter().copied().collect();
    let property = property_reads(&model);
    let anchor = anchor_of(&core, &trace);
    let query = ExpansionQuery::new(PackRelation::SameOwner, anchor);

    // --- typed items, every one through a production constructor -----------------------
    let mut selected: Vec<SelectedItem> = Vec::new();
    let mut candidates: Vec<(Name, SelectionKind)> = Vec::new();
    let mut core_ids: Vec<Name> = Vec::new();
    let mut omitted: Vec<(Name, SelectedItem)> = Vec::new();

    for event in &trace {
        if core_set.contains(&event.index) {
            // A core event is an `event`-kind item: docs/38's split decides its role.
            let role = if writes(&model, event.action).intersection(&property).count() > 0 {
                EventRole::Observed
            } else {
                EventRole::CausalPredecessor
            };
            candidates.push((event.id(), SelectionKind::Event));
            core_ids.push(event.id());
            selected.push(EventRef::new(name(&event.name), role).into_selected_item(event.id()));
            // Its concrete state deltas are items too — the diagnosis substance.
            for (variable, before, after) in event.changed(&model) {
                let id = delta_id(event.index, &variable);
                candidates.push((id.clone(), SelectionKind::StateDelta));
                core_ids.push(id.clone());
                selected.push(
                    StateDeltaRef::new(
                        name(&variable),
                        StateDeltaClass::Concrete,
                        Some(Value::int(i128::from(before))),
                        Value::int(i128::from(after)),
                    )
                    .expect("a changed variable is a real delta")
                    .into_selected_item(id),
                );
            }
        } else {
            // A noise event has no truthful `event`-kind spelling — `EventRole` is the
            // causal-core split — so it is candidate as its concrete transitions, which
            // are statements the typed constructors can make truthfully (module doc).
            for (variable, before, after) in event.changed(&model) {
                let id = delta_id(event.index, &variable);
                candidates.push((id.clone(), SelectionKind::StateDelta));
                omitted.push((
                    id.clone(),
                    StateDeltaRef::new(
                        name(&variable),
                        StateDeltaClass::Concrete,
                        Some(Value::int(i128::from(before))),
                        Value::int(i128::from(after)),
                    )
                    .expect("a changed variable is a real delta")
                    .into_selected_item(id),
                ));
            }
        }
    }
    // The buggy action itself, as a `model`-kind item.
    let model_item_id = name("m_ack_before_flush");
    candidates.push((model_item_id.clone(), SelectionKind::Model));
    core_ids.push(model_item_id.clone());
    selected.push(
        ModelActionRef::action_only(name("ack_before_flush")).into_selected_item(model_item_id),
    );

    // --- the production accounting: one ledger, closed, reconciled ---------------------
    let candidate_set = CandidateSet::new(candidates).expect("no candidate repeats");
    let candidate_count = u32::try_from(candidate_set.len()).expect("a small set");
    let mut accounting = Accounting::over(candidate_set);
    for id in &core_ids {
        accounting.select(id).expect("a core item is a candidate");
    }
    for (id, _) in &omitted {
        accounting
            .omit(
                id,
                Omitted::Expandable {
                    reason: PackReason::SliceIrrelevant,
                    query: query.clone(),
                },
            )
            .expect("a noise transition is a candidate");
    }
    let closed = accounting.close().expect("every candidate dispositioned");
    closed.reconcile().expect("the counting equation holds");
    assert_eq!(closed.candidate_count(), candidate_count);
    let manifest = closed.manifest().clone();
    assert_eq!(manifest.records().len(), 1, "one omitted group");
    let record = manifest.records()[0].clone();

    // The selection, in the accounting's own canonical order.
    let order: BTreeMap<Name, usize> = closed
        .selected()
        .iter()
        .enumerate()
        .map(|(position, id)| (id.clone(), position))
        .collect();
    selected.sort_by_key(|item| order[item.id()]);
    let mut omitted_items: Vec<SelectedItem> = omitted.into_iter().map(|(_, item)| item).collect();
    omitted_items.sort();

    // --- the root pack: harness assembly of production fragments -----------------------
    let root_id = ArtifactHandle::new(
        ArtifactClass::ContextPack,
        &Blake3Hasher::hash(
            &Json::object([
                ("question".to_owned(), Json::String(QUESTION.to_owned())),
                ("snapshot".to_owned(), Json::String(SNAPSHOT.to_owned())),
            ])
            .expect("distinct keys")
            .to_canonical_bytes(),
        )
        .to_token(),
    )
    .expect("a digest token is a well-formed identity");

    let intent = ArtifactHandle::new(
        ArtifactClass::IntentContract,
        &Blake3Hasher::hash(format!("{INVARIANT}:ackwal-v1").as_bytes()).to_token(),
    )
    .expect("a digest token is a well-formed identity");
    let target = Target::new(
        intent,
        Question::compiled(QUESTION).expect("a plain question"),
    )
    .expect("an in_* handle");

    let crashpack = ArtifactHandle::new(
        ArtifactClass::Crashpack,
        &Blake3Hasher::hash(
            &Json::object([(
                "core".to_owned(),
                Json::Array(
                    core.iter()
                        .map(|index| Json::String(trace[*index].name.clone()))
                        .collect(),
                ),
            )])
            .expect("one key")
            .to_canonical_bytes(),
        )
        .to_token(),
    )
    .expect("a digest token is a well-formed identity");
    let replay_ref = ReplayRef::new(crashpack).expect("a crash_* handle");

    let evidence_handle = ArtifactHandle::new(
        ArtifactClass::Evidence,
        &Blake3Hasher::hash(&raw_named_json(&model, &trace)).to_token(),
    )
    .expect("a digest token is a well-formed identity");

    let unsupported = |token: &str| {
        DimensionEvidence::Unsupported(UnsupportedReason::new(token).expect("a plain token"))
    };
    let envelope = AssuranceEnvelope::all_unsupported(
        &UnsupportedReason::new("outside-this-campaign").expect("a plain token"),
    )
    .with(
        AssuranceDimension::Bounds,
        DimensionEvidence::produced(
            "continuum-engine-reference",
            "complete exploration of the declared finite model",
        )
        .expect("a producer"),
    )
    .with(
        AssuranceDimension::Schedules,
        DimensionEvidence::produced(
            "continuum-engine-reference",
            "all interleavings within the reachable set",
        )
        .expect("a producer"),
    )
    .with(
        AssuranceDimension::MemoryModel,
        unsupported(UnsupportedReason::SEQUENTIAL_CONSISTENCY_ONLY),
    );
    let assurance = Assurance::new(AssuranceLevel::Bounded, envelope);

    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    fields.insert(
        "schema_id".to_owned(),
        Json::String("https://continuum.dev/schema/context-pack.json".to_owned()),
    );
    fields.insert("schema_epoch".to_owned(), Json::Integer(1));
    fields.insert("context_id".to_owned(), Json::String(root_id.to_string()));
    fields.insert("snapshot".to_owned(), Json::String(SNAPSHOT.to_owned()));
    for (key, value) in target.to_json_fields() {
        fields.insert(key, value);
    }
    for (key, value) in PackVerdict::Refuted.to_json_fields() {
        fields.insert(key, value);
    }
    for (key, value) in assurance.to_json_fields() {
        fields.insert(key, value);
    }
    fields.insert(
        "selected".to_owned(),
        Json::Array(selected.iter().map(SelectedItem::to_json).collect()),
    );
    fields.insert("omissions".to_owned(), manifest.to_json());
    fields.insert("expansions".to_owned(), manifest.expansions_json());
    fields.insert(
        "evidence".to_owned(),
        Json::Array(vec![Json::String(evidence_handle.to_string())]),
    );
    fields.insert("replay".to_owned(), replay_ref.to_json());
    fields.insert(
        "guarantees".to_owned(),
        Json::Array(vec![
            Json::String("ReplayPreserving".to_owned()),
            Json::String("CausallyClosed".to_owned()),
        ]),
    );
    fields.insert(
        "semantic_epoch".to_owned(),
        Json::String(SEMANTIC_EPOCH.to_owned()),
    );
    fields.insert("parent".to_owned(), Json::Null);

    let root = assemble_root_pack(fields);

    Compile {
        model,
        trace,
        core,
        selected,
        omitted_items,
        record,
        query,
        candidate_count,
        root,
        root_id,
    }
}

/// **Harness assembly** (declared): write `content_budget.bytes` and `content_hash`
/// under the packer's own two readings — the *least* self-consistent size (RFC 0028
/// correction 17, `pack.rs`'s fixed-point section) and the identity-with-the-key-absent
/// preimage. Both readings are then *checked* against production: `pack::budget_bytes_of`
/// must read back exactly the published document's own canonical length.
fn assemble_root_pack(base: BTreeMap<String, Json>) -> Json {
    let mut measured: u64 = 0;
    for _ in 0..24 {
        let mut fields = base.clone();
        fields.insert(
            "content_budget".to_owned(),
            Json::object([(
                "bytes".to_owned(),
                Json::Integer(i64::try_from(measured).expect("a small document")),
            )])
            .expect("one key"),
        );
        let preimage = Json::Object(fields.clone()).to_canonical_bytes();
        fields.insert(
            "content_hash".to_owned(),
            Json::String(format!(
                "{}:{}",
                Blake3Hasher::ALGORITHM.token(),
                Blake3Hasher::hash(&preimage).to_token()
            )),
        );
        let document = Json::Object(fields);
        let length = document.to_canonical_bytes().len() as u64;
        if length == measured {
            return document;
        }
        measured = length;
    }
    panic!("the root pack's size did not settle");
}

// =====================================================================================
// raw-trace baselines (defined in the module documentation, computed here)
// =====================================================================================

fn state_record(model: &Model, state: &State) -> Json {
    Json::Object(
        model
            .variables()
            .iter()
            .zip(state.as_slice())
            .map(|(variable, value)| (variable.name().as_str().to_owned(), Json::Integer(*value)))
            .collect(),
    )
}

/// The graded baseline: the witness form — `(action, state)` pairs, states as named
/// records — as canonical JSON. The shape the Phase A spike's named baseline used.
fn raw_named_json(model: &Model, trace: &[TraceEvent]) -> Vec<u8> {
    let mut entries: Vec<Json> = Vec::with_capacity(trace.len() + 1);
    entries.push(
        Json::object([(
            "initial".to_owned(),
            state_record(model, &model.initial_states()[0]),
        )])
        .expect("one key"),
    );
    for event in trace {
        entries.push(
            Json::object([
                ("action".to_owned(), Json::String(event.name.clone())),
                ("state".to_owned(), state_record(model, &event.after)),
            ])
            .expect("distinct keys"),
        );
    }
    Json::Array(entries).to_canonical_bytes()
}

/// Sensitivity: the engine's own `Display` shape — `start --name--> (v, v, …)` per step.
fn raw_engine_display(model: &Model, trace: &[TraceEvent]) -> String {
    use std::fmt::Write as _;
    let mut rendered = format!("{}", model.initial_states()[0]);
    for event in trace {
        write!(rendered, " --{}--> {}", event.name, event.after).expect("string formatting");
    }
    rendered
}

/// Sensitivity: positional state vectors.
fn raw_positional_json(trace: &[TraceEvent]) -> Vec<u8> {
    Json::Array(
        trace
            .iter()
            .map(|event| {
                Json::object([
                    ("a".to_owned(), Json::String(event.name.clone())),
                    (
                        "s".to_owned(),
                        Json::Array(
                            event
                                .after
                                .as_slice()
                                .iter()
                                .copied()
                                .map(Json::Integer)
                                .collect(),
                        ),
                    ),
                ])
                .expect("distinct keys")
            })
            .collect(),
    )
    .to_canonical_bytes()
}

/// Sensitivity: changed-variables-only delta log.
fn raw_delta_log(model: &Model, trace: &[TraceEvent]) -> Vec<u8> {
    Json::Array(
        trace
            .iter()
            .map(|event| {
                Json::object([
                    ("a".to_owned(), Json::String(event.name.clone())),
                    (
                        "c".to_owned(),
                        Json::Object(
                            event
                                .changed(model)
                                .into_iter()
                                .map(|(variable, before, after)| {
                                    (
                                        variable,
                                        Json::Array(vec![
                                            Json::Integer(before),
                                            Json::Integer(after),
                                        ]),
                                    )
                                })
                                .collect(),
                        ),
                    ),
                ])
                .expect("distinct keys")
            })
            .collect(),
    )
    .to_canonical_bytes()
}

/// Sensitivity: a bare action-name log (replayable only by re-running the model).
fn raw_action_names(trace: &[TraceEvent]) -> Vec<u8> {
    Json::Array(
        trace
            .iter()
            .map(|event| Json::String(event.name.clone()))
            .collect(),
    )
    .to_canonical_bytes()
}

// =====================================================================================
// the daemon fixture (the same recipe as tests/daemon_context_budget.rs)
// =====================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor() -> ActorId {
    ActorId::new("agent:reader").expect("a well-formed actor identity")
}

fn capability() -> CapabilityHandle {
    CapabilityHandle::new("cap_reader").expect("a well-formed capability handle")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch(SEMANTIC_EPOCH)),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Null,
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-dx01-falsification".to_owned(),
        actor: actor(),
        capability: capability(),
        features: Optional::Absent,
    }
}

fn grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability(),
        actor: actor(),
        level: AuthorityLevel::Read,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
    }
}

/// A daemon holding the compiled root pack and the omitted group behind its one query.
fn daemon_with(compile: &Compile) -> Daemon {
    let payload = ExpansionPayload::new(compile.record.clone(), compile.omitted_items.clone())
        .expect("the record's count is the number of omitted items");
    let record = ContextPackRecord::new(
        compile.root.clone(),
        WorkspaceHandle::new(SNAPSHOT).expect("a workspace handle"),
        [payload],
    )
    .expect("the assembled root is a pack the daemon can navigate");

    let negotiated = negotiate(
        &[
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 1),
            version(),
        ],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello(),
    )
    .expect("3.2 is served");
    let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability())
        .epochs(epochs())
        .now(Timestamp::new(NOW).expect("a timestamp"))
        .capability(grant(), None)
        .family(ContextFamily)
        .build();
    daemon.state_mut().put_context_pack(
        ContextHandle::new(&compile.root_id.to_string()).expect("a context handle"),
        record,
    );
    daemon
}

fn budget(bytes: Optional<ByteCount>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(0),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes,
    }
}

fn envelope(request_id: &str, ceiling: Optional<ByteCount>) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a request identity"),
        actor: actor(),
        capability: capability(),
        operation: OperationName::new("context.expand").expect("a declared operation"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        budget: Optional::Present(budget(ceiling)),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        trace: Optional::Absent,
        arguments: continuumd::protocol::scalar::Opaque::from_bytes(b"{}".to_vec()),
        output_policy: Optional::Absent,
        page: Optional::Absent,
    }
}

fn expand(
    daemon: &mut Daemon,
    compile: &Compile,
    request_id: &str,
    anchor: &str,
    relation: ExpansionRelation,
    ceiling: Optional<ByteCount>,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope(request_id, ceiling),
        arguments: Arguments::ContextExpand(ContextExpandRequest {
            context: ContextHandle::new(&compile.root_id.to_string()).expect("a context handle"),
            anchor: anchor.to_owned(),
            relation,
            depth: Optional::Absent,
        }),
    })
}

/// Expand the one advertised group under an explicit generous ceiling.
///
/// The ceiling is explicit because the daemon's *default* is the parent's own measured
/// size ("an expansion may not cost more than the pack it expands unless the caller asks
/// for more") — and this parent is deliberately small, so the full answer needs asking.
fn expand_full(daemon: &mut Daemon, compile: &Compile, request_id: &str) -> OperationOutcome {
    let anchor = compile.query.anchor().to_string();
    expand(
        daemon,
        compile,
        request_id,
        &anchor,
        ExpansionRelation::SameOwner,
        Optional::Present(ByteCount::new(1 << 20)),
    )
}

fn child_pack(outcome: &OperationOutcome) -> Json {
    let Payload::ContextExpand(response) = &outcome.payload else {
        panic!(
            "expected a context.expand payload: {:?}",
            outcome.envelope.error
        );
    };
    Json::parse(response.pack.as_bytes()).expect("the pack is canonical JSON")
}

fn pack_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let Payload::ContextExpand(response) = &outcome.payload else {
        panic!("expected a context.expand payload");
    };
    response.pack.as_bytes().to_vec()
}

fn returned_context(outcome: &OperationOutcome) -> String {
    let Payload::ContextExpand(response) = &outcome.payload else {
        panic!("expected a context.expand payload");
    };
    response.context.as_str().to_owned()
}

fn array<'a>(document: &'a Json, key: &str) -> &'a [Json] {
    document.as_object().expect("object")[key]
        .as_array()
        .expect("array")
}

/// The promise, derived from the published record alone: parse the parent document's own
/// `omissions[0].expansion` and `context_id`, and derive the `ctx_*` that query resolves
/// to — nothing the daemon privately knows enters this function.
fn promised_handle(root: &Json) -> ExpansionHandle {
    let record = &array(root, "omissions")[0];
    let expansion = record.as_object().expect("object")["expansion"]
        .as_object()
        .expect("object");
    let anchor = name(expansion["anchor"].as_str().expect("a string"));
    let relation = PackRelation::from_wire_str(expansion["relation"].as_str().expect("a string"))
        .expect("a closed-vocabulary token");
    let parent = pack::identity_of(root).expect("a pack handle");
    ExpansionHandle::derive::<Blake3Hasher>(
        &parent,
        &ExpansionQuery::new(relation, anchor),
        Depth::DEFAULT,
    )
    .expect("the parent is a pack")
}

// =====================================================================================
// helpers for the drop-one / textual mutants
// =====================================================================================

fn core_actions(compile: &Compile) -> Vec<usize> {
    compile
        .core
        .iter()
        .map(|index| compile.trace[*index].action)
        .collect()
}

fn textual_selection(compile: &Compile) -> Vec<usize> {
    compile
        .trace
        .iter()
        .filter(|event| {
            ["write", "flush", "ack", "wal"]
                .iter()
                .any(|token| event.name.contains(token))
        })
        .map(|event| event.index)
        .collect()
}

// =====================================================================================
// the tests
// =====================================================================================

#[test]
fn c01_the_noisy_trace_is_a_real_durability_failure() {
    let model = durability_model();
    let trace = drive(&model);
    assert!(
        trace.len() >= 200,
        "the row demands a 200+ event trace; this one has {}",
        trace.len()
    );
    assert_eq!(trace.len(), 225, "the declared schedule's own length");

    // The trace ends in a state the production engine's own predicate refutes.
    let invariant = model.predicate_index(INVARIANT).expect("declared");
    let end = &trace.last().expect("a non-empty trace").after;
    assert_eq!(
        model.evaluate_predicate(invariant, end),
        Ok(false),
        "the full noisy trace is itself a witness to the durability failure"
    );

    // The noise is genuinely noisy: every red-herring actor fired, and the correct
    // protocol (`flush_wal`) exists in the model without firing here.
    let fired: BTreeSet<&str> = trace.iter().map(|event| event.name.as_str()).collect();
    for herring in [
        "submit_write_replica",
        "flush_wal_replica",
        "ack_after_flush_replica",
        "telemetry_flush",
        "scrub_write_verify",
        "recover",
    ] {
        assert!(
            fired.contains(herring),
            "{herring} must appear in the trace"
        );
    }
    assert!(
        !fired.contains("flush_wal"),
        "the bug is that the flush lost the race"
    );
}

#[test]
fn c02_the_production_check_refutes_and_its_witness_is_the_core() {
    let model = durability_model();
    let exploration =
        explore(&model, Bounds::new(60_000, 4_000, 2_000_000)).expect("the model explores");
    assert!(
        matches!(exploration, Exploration::Complete(_)),
        "the reachable set closes within the declared bounds"
    );
    let invariant = model.predicate_index(INVARIANT).expect("declared");
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(invariant),
    )
    .expect("the obligations name a declared predicate");
    assert_eq!(
        report.verdict(),
        EngineVerdict::Refuted,
        "the production checker refutes AckImpliesDurable over the complete exploration"
    );

    // Independent corroboration of the slice: the engine's own shortest counterexample
    // is exactly the four core actions, in order.
    let witness = witness::shortest(&model, &exploration, &WitnessTarget::Fails(invariant))
        .expect("a complete exploration with a refuted invariant has a witness");
    let steps: Vec<&str> = witness
        .steps()
        .iter()
        .map(|step| step.name().as_str())
        .collect();
    assert_eq!(steps, CORE_ACTIONS.to_vec());
}

#[test]
fn leg1_the_causal_core_is_replay_preserving_and_noise_free() {
    let compile = compile();

    // The slice found exactly the pre-registered four causal events.
    let actions: Vec<&str> = compile
        .core
        .iter()
        .map(|index| compile.trace[*index].name.as_str())
        .collect();
    assert_eq!(actions, CORE_ACTIONS.to_vec());

    // Noise exclusion: no noise event's index is in the core.
    let core_set: BTreeSet<usize> = compile.core.iter().copied().collect();
    for event in &compile.trace {
        if !core_set.contains(&event.index) {
            assert!(
                !CORE_ACTIONS.contains(&event.name.as_str()),
                "every core-action firing is in the core: {}",
                event.name
            );
        }
    }

    // Causal closure, witnessed: at every core step, the projected replay's demanded
    // sub-state equals the original trace's — every writer that matters was kept.
    let (_, demand) = causal_core(&compile.model, &compile.trace);
    let demand_indexes: Vec<usize> = compile
        .model
        .variables()
        .iter()
        .enumerate()
        .filter(|(_, variable)| demand.contains(variable.name().as_str()))
        .map(|(index, _)| index)
        .collect();
    let mut state = compile.model.initial_states()[0].clone();
    for index in &compile.core {
        let event = &compile.trace[*index];
        for slot in &demand_indexes {
            assert_eq!(
                state.as_slice()[*slot],
                event.before.as_slice()[*slot],
                "the demanded sub-state agrees with the original trace before `{}`",
                event.name
            );
        }
        let successors = compile
            .model
            .action_successors(event.action, &state)
            .expect("evaluates");
        assert_eq!(
            successors.len(),
            1,
            "`{}` is enabled in the projection",
            event.name
        );
        state = successors[0].clone();
    }

    // The replay check itself — production transition semantics, production predicate.
    match replay(&compile.model, &core_actions(&compile)) {
        Replay::Refutes(end) => {
            // The refuting sub-state is the diagnosis: acked, not durable, not buffered.
            let read = |variable: &str| {
                compile
                    .model
                    .binding(&end, variable)
                    .expect("a declared variable")
            };
            assert_eq!(read("client_acked"), 1);
            assert_eq!(read("wal_durable"), 0);
            assert_eq!(read("wal_buffered"), 0);
        }
        other => panic!("the causal core must replay and refute; got {other:?}"),
    }
}

#[test]
fn a01_dropping_any_causal_event_fails_the_pre_registered_way() {
    let compile = compile();
    let core = core_actions(&compile);
    assert_eq!(core.len(), 4);

    // Pre-registered per-row failure modes — the replay comparison can fail, and does,
    // distinctly, for every single-event deletion. (The drop-`begin_txn` row is also the
    // output of a mutant slicer that ignores guard reads, so it is that mutant's kill.)
    let expectations: [(usize, &str); 4] = [
        (0, "disabled at position 0: submit_write"),
        (1, "disabled at position 1: ack_before_flush"),
        (2, "completes without refuting"),
        (3, "completes without refuting"),
    ];
    for (drop, expected) in expectations {
        let mutant: Vec<usize> = core
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != drop)
            .map(|(_, action)| *action)
            .collect();
        let observed = match replay(&compile.model, &mutant) {
            Replay::Disabled { position, action } => {
                format!("disabled at position {position}: {action}")
            }
            Replay::CompletesWithoutRefuting(_) => "completes without refuting".to_owned(),
            Replay::Refutes(_) => "STILL REFUTES".to_owned(),
        };
        assert_eq!(
            observed, expected,
            "dropping core[{drop}] must fail the pre-registered way"
        );
    }
}

#[test]
fn a02_textual_relevance_is_refused_both_ways() {
    // RFC 0028 C2: "A set assembled from textual relevance is not accepted as
    // replay-preserving (docs/38)." The durability-vocabulary selector is exactly that
    // set, and it fails every reading.
    let compile = compile();
    let textual = textual_selection(&compile);
    let names: BTreeSet<&str> = textual
        .iter()
        .map(|index| compile.trace[*index].name.as_str())
        .collect();

    // It admits noise — the red herrings were named for this.
    for herring in [
        "submit_write_replica",
        "flush_wal_replica",
        "ack_after_flush_replica",
        "telemetry_flush",
        "scrub_write_verify",
    ] {
        assert!(names.contains(herring), "the selector admits {herring}");
    }
    let core_set: BTreeSet<usize> = compile.core.iter().copied().collect();
    let admitted_noise = textual
        .iter()
        .filter(|index| !core_set.contains(index))
        .count();
    assert!(
        admitted_noise > 40,
        "noise entered the textual core: {admitted_noise} events"
    );

    // It misses two of the four causal events — the ones without the vocabulary.
    assert!(!names.contains("begin_txn"));
    assert!(!names.contains("power_loss"));

    // Strict replay: not even an execution of the model.
    let actions: Vec<usize> = textual
        .iter()
        .map(|index| compile.trace[*index].action)
        .collect();
    match replay(&compile.model, &actions) {
        Replay::Disabled {
            position: 0,
            action,
        } => {
            assert_eq!(
                action, "telemetry_flush",
                "the first textual event is not enabled"
            );
        }
        other => panic!("the textual set must fail strict replay at its first step: {other:?}"),
    }

    // Forgiving replay (disabled steps skipped): completes, and does not refute —
    // without `power_loss` the acknowledged write is still buffered.
    match replay_skipping_disabled(&compile.model, &actions) {
        Replay::CompletesWithoutRefuting(_) => {}
        other => panic!("the forgiving textual replay must not refute: {other:?}"),
    }
}

#[test]
fn a03_the_core_out_of_order_does_not_replay() {
    let compile = compile();
    let core = core_actions(&compile);
    let mut swapped = core.clone();
    swapped.swap(0, 1);
    match replay(&compile.model, &swapped) {
        Replay::Disabled {
            position: 0,
            action,
        } => assert_eq!(action, "submit_write"),
        other => panic!("a reordered core must not replay: {other:?}"),
    }
}

#[test]
fn leg2_the_pack_is_ten_times_smaller_under_the_packers_own_counting_rule() {
    let compile = compile();

    // The counting rule is production: the recorded measurement is the published
    // canonical encoding's own length, read back through `pack::budget_bytes_of`.
    let pack_size = pack::budget_bytes_of(&compile.root).expect("a measured pack");
    assert_eq!(
        pack_size,
        compile.root.to_canonical_bytes().len() as u64,
        "the recorded size is the size of the document that records it"
    );
    pack::required_keys_present(&compile.root).expect("all seventeen required keys");
    assert_eq!(
        pack::identity_of(&compile.root).expect("a pack handle"),
        compile.root_id
    );

    // INV-007 before any ratio: the reduction is not bought by dropping accounting.
    assert_eq!(
        compile.selected.len() as u64 + u64::from(compile.record.count()),
        u64::from(compile.candidate_count),
        "candidate set = selection + Σ manifest counts"
    );

    // The graded baseline.
    let raw = raw_named_json(&compile.model, &compile.trace).len() as u64;
    let ratio_x100 = raw * 100 / pack_size;
    assert!(
        ratio_x100 >= 1000,
        "the pass condition demands >=10x: raw {raw} / pack {pack_size} = {ratio_x100}x100"
    );
}

#[test]
fn a04_the_production_accounting_refuses_every_undercount_on_this_dataset() {
    let compile = compile();

    // A manifest record that undercounts the omitted group has no payload spelling.
    let undercount = OmissionRecord::expandable(
        SelectionKind::StateDelta,
        compile.record.count() - 1,
        PackReason::SliceIrrelevant,
        compile.query.clone(),
    );
    assert!(
        ExpansionPayload::new(undercount, compile.omitted_items.clone()).is_err(),
        "counts are exact: an undercounting record cannot be paired with the items"
    );

    // An undispositioned candidate refuses to close (INV-007 as a refusal to publish).
    let mut candidates: Vec<(Name, SelectionKind)> = compile
        .selected
        .iter()
        .map(|item| (item.id().clone(), item.kind()))
        .collect();
    candidates.extend(
        compile
            .omitted_items
            .iter()
            .map(|item| (item.id().clone(), item.kind())),
    );
    let set = CandidateSet::new(candidates).expect("no repeats");
    let mut accounting = Accounting::over(set);
    for item in &compile.selected {
        accounting.select(item.id()).expect("a candidate");
    }
    // Every omitted item but one:
    for item in compile.omitted_items.iter().skip(1) {
        accounting
            .omit(
                item.id(),
                Omitted::Expandable {
                    reason: PackReason::SliceIrrelevant,
                    query: compile.query.clone(),
                },
            )
            .expect("a candidate");
    }
    assert!(
        accounting.clone().close().is_err(),
        "a candidate with no disposition cannot be published at all"
    );

    // A selection from outside the candidate set is refused.
    assert!(
        accounting.select(&name("e_9999")).is_err(),
        "an item from nowhere would fake the equation in the generous direction"
    );

    // A no-change delta has no spelling (power_loss's rep_buffered write is 0 -> 0).
    assert_eq!(
        StateDeltaRef::new(
            name("rep_buffered"),
            StateDeltaClass::Concrete,
            Some(Value::int(0)),
            Value::int(0),
        ),
        Err(StateDeltaRefError::NoChange)
    );

    // A replay reference refuses every class but Crashpack.
    let not_a_crashpack =
        ArtifactHandle::new(ArtifactClass::Evidence, "notacrash").expect("well formed");
    assert_eq!(
        ReplayRef::new(not_a_crashpack),
        Err(ReplayRefError::WrongArtifactClass(ArtifactClass::Evidence))
    );
}

#[test]
fn leg3_the_promised_handle_is_the_returned_handle_and_the_expansion_is_exact() {
    let compile = compile();
    let mut daemon = daemon_with(&compile);

    // The promise, derived from the published artifact alone.
    let promised = promised_handle(&compile.root);

    // The answer, through Daemon::dispatch.
    let outcome = expand_full(&mut daemon, &compile, "req_full");
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    assert_eq!(
        returned_context(&outcome),
        promised.to_string(),
        "the ctx_* the manifest promised is the ctx_* the daemon returned"
    );

    // The expansion returns exactly the omitted content: item for item, byte for byte.
    let child = child_pack(&outcome);
    let returned: Vec<Vec<u8>> = array(&child, "selected")
        .iter()
        .map(Json::to_canonical_bytes)
        .collect();
    let expected: Vec<Vec<u8>> = compile
        .omitted_items
        .iter()
        .map(SelectedItem::to_canonical_bytes)
        .collect();
    assert_eq!(returned.len(), compile.record.count() as usize);
    assert_eq!(
        returned, expected,
        "exactly the omitted items, in canonical order"
    );

    // The child's empty manifest is the assertion of completeness, and the envelope's
    // omission list agrees with it record for record — here, by both being empty.
    assert!(array(&child, "omissions").is_empty());
    assert!(outcome.envelope.omissions.is_empty());

    // The child is a pack: required keys, inherited answer header, no free guarantees.
    pack::required_keys_present(&child).expect("all seventeen required keys");
    let root_fields = compile.root.as_object().expect("object");
    let child_fields = child.as_object().expect("object");
    for key in [
        "verdict",
        "assurance",
        "replay",
        "snapshot",
        "intent",
        "semantic_epoch",
    ] {
        assert_eq!(
            child_fields[key], root_fields[key],
            "`{key}` is inherited verbatim"
        );
    }
    assert_eq!(child_fields["guarantees"], Json::Array(Vec::new()), "C5");
    assert_eq!(
        child_fields["parent"],
        Json::String(compile.root_id.to_string())
    );

    // Idempotency without the ledger: a different key, the same handle, the same bytes.
    let again = expand_full(&mut daemon, &compile, "req_full_again");
    assert_eq!(returned_context(&again), promised.to_string());
    assert_eq!(pack_bytes(&outcome), pack_bytes(&again));
}

#[test]
fn a05_the_promise_check_can_fail_and_hostile_queries_are_refused() {
    let compile = compile();
    let mut daemon = daemon_with(&compile);
    let promised = promised_handle(&compile.root);

    // A tampered record — the anchor moved one event — promises a different handle, so
    // the leg-3 comparison is not vacuous.
    let tampered = ExpansionHandle::derive::<Blake3Hasher>(
        &compile.root_id,
        &ExpansionQuery::new(PackRelation::SameOwner, name("e_9999")),
        Depth::DEFAULT,
    )
    .expect("derives");
    assert_ne!(tampered.to_string(), promised.to_string());

    // An anchor the pack does not carry: MalformedRequest.
    let hostile = expand(
        &mut daemon,
        &compile,
        "req_hostile_anchor",
        "e_9999",
        ExpansionRelation::SameOwner,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    assert_eq!(
        hostile.envelope.error.value().expect("a refusal").code,
        ErrorCode::MalformedRequest
    );

    // A resolvable anchor with a relation nothing registered: UnsupportedSemanticFeature.
    let anchor = compile.query.anchor().to_string();
    let undefined = expand(
        &mut daemon,
        &compile,
        "req_undefined_relation",
        &anchor,
        ExpansionRelation::CausalPredecessors,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    assert_eq!(
        undefined.envelope.error.value().expect("a refusal").code,
        ErrorCode::UnsupportedSemanticFeature
    );

    // A pack this daemon does not hold: CapabilityDenied, never a distinguishable
    // not-found (RFC 0027 X2).
    let unheld = daemon.dispatch(&OperationRequest {
        envelope: envelope("req_unheld", Optional::Present(ByteCount::new(1 << 20))),
        arguments: Arguments::ContextExpand(ContextExpandRequest {
            context: ContextHandle::new("ctx_nobodyhome").expect("a context handle"),
            anchor: anchor.clone(),
            relation: ExpansionRelation::SameOwner,
            depth: Optional::Absent,
        }),
    });
    assert_eq!(
        unheld.envelope.error.value().expect("a refusal").code,
        ErrorCode::CapabilityDenied
    );
}

#[test]
fn leg3b_the_budget_branch_keeps_the_promise() {
    let compile = compile();
    let mut daemon = daemon_with(&compile);
    let promised = promised_handle(&compile.root);

    let full = expand_full(&mut daemon, &compile, "req_whole");
    assert_eq!(full.envelope.status, ResultStatus::Ok);
    let whole = pack::budget_bytes_of(&child_pack(&full)).expect("measured");

    // Under a ceiling the answer exceeds, the production packer publishes a smaller
    // child whose `budget` shortfall is recoverable by the same promised handle.
    let anchor = compile.query.anchor().to_string();
    let packed = expand(
        &mut daemon,
        &compile,
        "req_packed",
        &anchor,
        ExpansionRelation::SameOwner,
        Optional::Present(ByteCount::new(whole - 400)),
    );
    assert_eq!(packed.envelope.status, ResultStatus::Ok);
    let child = child_pack(&packed);
    let kept = array(&child, "selected").len() as u64;
    assert!(kept > 0 && kept < u64::from(compile.record.count()));

    // Conservation under the ceiling: nothing lost, nothing invented.
    let omitted: i64 = array(&child, "omissions")
        .iter()
        .map(|record| {
            record.as_object().expect("object")["count"]
                .as_integer()
                .expect("an exact count")
        })
        .sum();
    assert_eq!(
        kept as i64 + omitted,
        i64::from(compile.record.count()),
        "packing moves items between the halves and creates or destroys nothing"
    );

    // The wire projection promises recovery by the very handle this question answers to.
    let wire = packed
        .envelope
        .omissions
        .iter()
        .find(|omission| omission.reason == OmissionReason::Budget)
        .expect("the shortfall reaches the envelope");
    assert_eq!(wire.subject, "selected.state_delta");
    assert_eq!(
        wire.recoverable_by.value().expect("recoverable").as_str(),
        promised.to_string(),
        "the budget shortfall is recovered by the same question under a larger ceiling"
    );

    // Below the minimal child: nothing published, the refusal typed (SD-13).
    let floor = expand(
        &mut daemon,
        &compile,
        "req_floor",
        &anchor,
        ExpansionRelation::SameOwner,
        Optional::Present(ByteCount::new(1)),
    );
    let error = floor.envelope.error.value().expect("a refusal");
    assert_eq!(error.code, ErrorCode::BudgetExhausted);
    assert!(!error.non_resumable_reason.is_absent(), "SD-13");
    assert!(matches!(floor.payload, Payload::None), "nothing published");
}

// =====================================================================================
// the evidence artifact
// =====================================================================================

/// Render the whole campaign to one deterministic artifact.
fn evidence() -> String {
    use std::fmt::Write as _;

    let compile = compile();
    let model = &compile.model;
    let trace = &compile.trace;

    let noise_events = trace.len() - compile.core.len();
    let pack_size = pack::budget_bytes_of(&compile.root).expect("measured");
    let raw_named = raw_named_json(model, trace).len() as u64;
    let raw_display = raw_engine_display(model, trace).len() as u64;
    let raw_positional = raw_positional_json(trace).len() as u64;
    let raw_delta = raw_delta_log(model, trace).len() as u64;
    let raw_actions = raw_action_names(trace).len() as u64;
    let ratio = |raw: u64| raw * 100 / pack_size;

    // Pack decomposition: where the bytes live (redesign-actionable, the dx10 pattern).
    let fragment = |key: &str| {
        compile.root.as_object().expect("object")[key]
            .to_canonical_bytes()
            .len() as u64
    };
    let selected_bytes = fragment("selected");
    let manifest_bytes = fragment("omissions") + fragment("expansions");
    let header_bytes = pack_size - selected_bytes - manifest_bytes;

    let exploration = explore(model, Bounds::new(60_000, 4_000, 2_000_000)).expect("explores");
    let reachable = exploration.reachable().len();
    let invariant = model.predicate_index(INVARIANT).expect("declared");
    let report = checking::check(
        model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(invariant),
    )
    .expect("checks");
    let witness = witness::shortest(model, &exploration, &WitnessTarget::Fails(invariant))
        .expect("a witness");

    let mut daemon = daemon_with(&compile);
    let promised = promised_handle(&compile.root);
    let full = expand_full(&mut daemon, &compile, "req_evidence_full");
    let returned = returned_context(&full);
    let child = child_pack(&full);
    let child_size = pack::budget_bytes_of(&child).expect("measured");
    let packed = {
        let anchor = compile.query.anchor().to_string();
        expand(
            &mut daemon,
            &compile,
            "req_evidence_packed",
            &anchor,
            ExpansionRelation::SameOwner,
            Optional::Present(ByteCount::new(child_size - 400)),
        )
    };
    let packed_child = child_pack(&packed);
    let packed_kept = array(&packed_child, "selected").len();

    let core_line = compile
        .core
        .iter()
        .map(|index| format!("{}:{}", trace[*index].id(), trace[*index].name))
        .collect::<Vec<_>>()
        .join(",");
    let witness_line = witness
        .steps()
        .iter()
        .map(|step| step.name().as_str().to_owned())
        .collect::<Vec<_>>()
        .join(",");

    let mut out = String::new();
    let _ = writeln!(out, "G0-DX-01 falsification evidence (bn-37gu)");
    let _ = writeln!(
        out,
        "row: 200+ event noisy durability failure -> Context Pack"
    );
    let _ = writeln!(
        out,
        "legs: causal core replay-preserving | >=10x reduction | exact expansion handles"
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "[trace]");
    let _ = writeln!(
        out,
        "events={} noise_events={noise_events} core_events={}",
        trace.len(),
        compile.core.len()
    );
    let _ = writeln!(out, "core={core_line}");
    let _ = writeln!(
        out,
        "engine_check_verdict={} invariant={INVARIANT} reachable_states={reachable}",
        report.verdict().as_str()
    );
    let _ = writeln!(
        out,
        "shortest_witness={witness_line} len={} equals_core={}",
        witness.len(),
        witness_line == CORE_ACTIONS.join(",")
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "[leg1 replay-preserving core]");
    let _ = writeln!(
        out,
        "core_replay={}",
        match replay(model, &core_actions(&compile)) {
            Replay::Refutes(_) => "refutes",
            _ => "FAILED",
        }
    );
    for (drop, expected) in [
        (0usize, "disabled_at_submit_write"),
        (1, "disabled_at_ack_before_flush"),
        (2, "completes_without_refuting"),
        (3, "completes_without_refuting"),
    ] {
        let mutant: Vec<usize> = core_actions(&compile)
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != drop)
            .map(|(_, action)| *action)
            .collect();
        let observed = match replay(model, &mutant) {
            Replay::Disabled { action, .. } => format!("disabled_at_{action}"),
            Replay::CompletesWithoutRefuting(_) => "completes_without_refuting".to_owned(),
            Replay::Refutes(_) => "STILL_REFUTES".to_owned(),
        };
        let _ = writeln!(
            out,
            "drop_{}={observed} pre_registered={expected} held={}",
            CORE_ACTIONS[drop],
            observed == expected
        );
    }
    let textual = textual_selection(&compile);
    let core_set: BTreeSet<usize> = compile.core.iter().copied().collect();
    let textual_noise = textual
        .iter()
        .filter(|index| !core_set.contains(index))
        .count();
    let textual_actions: Vec<usize> = textual.iter().map(|index| trace[*index].action).collect();
    let _ = writeln!(
        out,
        "textual_selection: admits_noise={textual_noise} misses=begin_txn,power_loss \
         strict={} forgiving={}",
        match replay(model, &textual_actions) {
            Replay::Disabled { action, .. } => format!("disabled_at_{action}"),
            _ => "REPLAYED".to_owned(),
        },
        match replay_skipping_disabled(model, &textual_actions) {
            Replay::CompletesWithoutRefuting(_) => "completes_without_refuting",
            _ => "REFUTES",
        }
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "[leg2 reduction, measured under the packer's counting rule]"
    );
    let _ = writeln!(
        out,
        "pack_bytes={pack_size} (== published canonical length) candidates={} selected={} \
         omitted={}",
        compile.candidate_count,
        compile.selected.len(),
        compile.record.count()
    );
    let _ = writeln!(
        out,
        "graded raw_named_json={raw_named} ratio_x100={} pass_10x={}",
        ratio(raw_named),
        ratio(raw_named) >= 1000
    );
    let _ = writeln!(
        out,
        "sensitivity raw_engine_display={raw_display} ratio_x100={}",
        ratio(raw_display)
    );
    let _ = writeln!(
        out,
        "sensitivity raw_positional_json={raw_positional} ratio_x100={}",
        ratio(raw_positional)
    );
    let _ = writeln!(
        out,
        "sensitivity raw_delta_log={raw_delta} ratio_x100={}",
        ratio(raw_delta)
    );
    let _ = writeln!(
        out,
        "sensitivity raw_action_names={raw_actions} ratio_x100={}",
        ratio(raw_actions)
    );
    let _ = writeln!(
        out,
        "pack_decomposition selected={selected_bytes} manifest+expansions={manifest_bytes} \
         header={header_bytes}"
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "[leg3 exact expansion handles, wire-live]");
    let _ = writeln!(out, "parent={}", compile.root_id);
    let _ = writeln!(
        out,
        "promised={promised} returned={returned} equal={}",
        promised.to_string() == returned
    );
    let _ = writeln!(
        out,
        "child_items={} exact_match={} child_manifest_empty={} child_bytes={child_size}",
        array(&child, "selected").len(),
        array(&child, "selected")
            .iter()
            .map(Json::to_canonical_bytes)
            .collect::<Vec<_>>()
            == compile
                .omitted_items
                .iter()
                .map(SelectedItem::to_canonical_bytes)
                .collect::<Vec<_>>(),
        array(&child, "omissions").is_empty()
    );
    let _ = writeln!(
        out,
        "packed(ceiling=child-400): kept={packed_kept} dropped={} recoverable_by_promised={}",
        compile.record.count() as usize - packed_kept,
        packed
            .envelope
            .omissions
            .iter()
            .find(|omission| omission.reason == OmissionReason::Budget)
            .and_then(|omission| omission.recoverable_by.value().cloned())
            .map(|handle| handle.as_str() == promised.to_string())
            .unwrap_or(false)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "[grain]");
    let _ = writeln!(
        out,
        "leg1: selection=harness(stage-2 dynamic dependence slice; no compiler) \
         check=production(engine successors+check+witness)"
    );
    let _ = writeln!(
        out,
        "leg2: counting=production(RFC 0028 correction 17) assembly=harness(no root assembler)"
    );
    let _ = writeln!(out, "leg3: production end-to-end (Daemon::dispatch)");
    let _ = writeln!(
        out,
        "missing_producers: ten-stage compiler (context.compile refused); root pack \
         assembly; crashpack producer; intent registry resolution"
    );
    out
}

#[test]
fn d_the_campaign_renders_one_byte_stable_evidence_artifact() {
    // Two independent builds — two compiles, two daemons, two expansions — must render
    // byte-identical evidence, and the evidence must match the pinned golden artifact.
    let first = evidence();
    let second = evidence();
    assert_eq!(first, second, "the campaign is deterministic (INV-005)");

    let golden = include_str!("golden/dx01_falsification_evidence.txt");
    if first != golden {
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("dx01_falsification_evidence_actual.txt");
        std::fs::write(&path, &first).expect("write the drifted artifact");
        panic!(
            "the evidence artifact drifted from tests/golden/dx01_falsification_evidence.txt; \
             the actual rendering was written to {}",
            path.display()
        );
    }
}
