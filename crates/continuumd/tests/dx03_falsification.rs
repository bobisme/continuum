//! G0-DX-03 falsification campaign: *is all state explicit and resumable?*
//!
//! # Why this file exists
//!
//! `notes/plan/notes/G0_SPIKE_MATRIX.md` states the G0-DX-03 required experiment as
//! **"create snapshot, start bounded task, resume continuation, mutate workspace, reuse old
//! handle"** and its pass condition as **"idempotence, stale snapshot rejection,
//! deterministic resume"**. Its recorded evidence was `spikes/R3_SPIKE_REPORT.md` §3 — an
//! in-memory Python workbench that validated an *artifact shape*, explicitly not
//! "concurrency, authorization, persistence, or crash recovery". The G0 promotion rule
//! refuses to turn a spike into architecture without adversarial mutations and a reference
//! implementation.
//!
//! This file is that campaign against the production implementation
//! (`crates/continuumd/src/daemon/{mod,task,verification,workspace,budget,state}.rs` and
//! `crates/continuum-workspace/src/{staleness,lineage}.rs`). It exists to make the pass
//! condition **false**, and it is deliberately hostile to the code it exercises. `src/` is
//! not touched: a falsification campaign that edits the thing it is falsifying proves
//! nothing (the bn-21dd precedent, `dx13_falsification.rs`'s own house rule).
//!
//! # Result of the campaign, and the fixes that closed it
//!
//! **Two of the three conjuncts were falsified.** Twenty-two attacks ran across four axes,
//! on top of the five baseline scenarios below — twenty-seven tests. Seventeen attacks held;
//! five landed, on three independent defects, all three of them in the *daemon* and none of
//! them in `continuum-workspace`'s snapshot/lineage layer, which refused every stale handle
//! it was shown.
//!
//! **All three defects have since been fixed, and all five reproductions have flipped**:
//! they are the same wire sequences, asserting the behaviour the rules require instead of
//! the behaviour the daemon had, and they run un-ignored in the default suite as regression
//! guards. DEFECT 1 by bn-n1xou, DEFECT 2 by bn-10093, DEFECT 3 by bn-h1zqz. Each defect is
//! described below as the campaign found it, followed by what the repair was; the tests
//! themselves carry the same two halves. The seventeen attacks that held are unchanged and
//! still hold.
//!
//! - **DEFECT 1 (fixed, bn-n1xou) — a replayed `workspace.create` rewinds a lineage.**
//!   `daemon::workspace::create` derives the lineage's [`ForkName`] from the *content
//!   identity* of the snapshot it creates and then calls `DaemonState::put_lineage`, which
//!   is an unconditional `BTreeMap::insert`. So a second `workspace.create` naming the same
//!   components — the same content, therefore the same `ws_*` handle, therefore the same
//!   fork name — replaces the live `Fork` with a *fresh* `Fork::diverge`, whose `parent` is
//!   `None` and whose head is the original snapshot. Every advance the lineage had made is
//!   forgotten. The consequence is exactly the DX-03 experiment's own sentence: **after the
//!   workspace has been mutated, the old handle works again.** `verification.start` over the
//!   superseded snapshot, refused with `StaleSnapshot` one request earlier, runs a campaign;
//!   and the snapshot that *was* the lineage head becomes unsealable, refused with
//!   `CapabilityDenied` — the `LineageError::Unknown` arm — so the mutation is stranded.
//!   Pinned by [`regression_a_replayed_create_converges_and_leaves_the_old_snapshot_stale`],
//!   [`regression_a_converged_create_does_not_strand_the_snapshot_that_is_current`], and
//!   [`regression_an_unsealed_re_create_does_not_revoke_a_valid_continuation`].
//!   The idempotency ledger was the *only* thing standing between the daemon and this
//!   rewind, which is what [`attack_staleness_the_same_create_under_the_same_key_changes_nothing`]
//!   shows: replay the create under the key it already used and the dispatch returns the
//!   recorded outcome at step 7, before the family handler at step 8 runs at all, so the
//!   lineage is untouched and the stale handle stays refused. A caller only had to arrive
//!   with a fresh key — which `rule idempotency.replay` explicitly contemplates, since a key
//!   is honoured for "at least the retention window the daemon declares" and not forever —
//!   or to be a second actor, since keys are scoped per actor.
//!   **Fixed (bn-n1xou):** an identical create *converges*. `DaemonState::open_lineage` is
//!   put-if-absent, so a lineage this daemon already holds keeps its advances; the held
//!   workspace record is not replaced; and `sealed` is monotone, so `seal: false` means "do
//!   not seal it" and never "un-seal it". The design is the G0-DX-13 disposition for
//!   identical creation — one semantic identity, nothing lost — read at the lineage.
//!
//! - **DEFECT 2 (fixed, bn-10093) — `task.resume` writes a terminal task's budget that
//!   `task.update_budget` refuses to write.** `daemon::task::resume` applied the request's
//!   optional budget to the task's [`BudgetLedger`] *before* it tested whether the task is
//!   terminal, and `daemon::task::update_budget` tested first. So the same ceiling, on the
//!   same cancelled task, in the same daemon, was refused through one operation and accepted
//!   through the other. `resume`'s own documentation calls the terminal path "a no-op", and
//!   it was not one: `TaskRecord.budget` afterwards reported a ceiling the task never ran
//!   under. Pinned by
//!   [`regression_resume_refuses_the_terminal_budget_write_update_budget_refuses`].
//!   **Fixed (bn-10093):** the terminal check moved above the ledger write, so the budget arm
//!   of a resume produces `update_budget`'s own observable for a terminal task — nothing
//!   written, the terminal status answered. bn-3p32's A1 is the same fix from the DX-14 side.
//!
//! - **DEFECT 3 (fixed, bn-h1zqz) — the idempotency ledger's replay key omits
//!   `RequestEnvelope.budget`.** `rule idempotency.replay` requires the same key with "a
//!   different request" to be rejected with `IdempotencyKeyReused`, and `budget` is a field
//!   of `RequestEnvelope`. It is also *semantically load-bearing*:
//!   `verification::start_handle` writes `budget_preimage` into the `task_*` content
//!   identity, so two budgets are two tasks. A `verification.start` replayed under one key
//!   with a larger budget was neither rejected nor honoured — it returned the first budget's
//!   task verbatim, so a caller that asked for a 64-state campaign was answered with a
//!   4-state one and could not tell.
//!   Pinned by [`regression_the_replay_key_covers_the_envelope_budget`].
//!   **Fixed (bn-h1zqz):** `ReplayKey` carries the whole canonical request — `budget`,
//!   `output_policy` and `page` beside the four fields it already had — and its
//!   documentation carries the field-by-field audit of what is deliberately left out
//!   (`request_id` and `trace` are per-attempt; `actor` is the ledger's other map key).
//!
//! All five reproductions are **green in both directions of the cycle**: they were green as
//! pins, asserting what the daemon actually did, and they are green as guards, asserting what
//! the rules require. The sequences are unchanged — the same requests, in the same order —
//! and only the assertions flipped, which is what makes them evidence that the repair fixed
//! *this* defect rather than evidence that some test now passes. That is the shape
//! `dx13_falsification.rs` settled on and bn-2siid completed: a defect's reproduction belongs
//! in the default suite, not behind an `#[ignore]`, and it becomes the regression guard.
//! They must not be deleted or weakened.
//!
//! # The baseline this campaign had to cover first
//!
//! `spikes/R3_SPIKE_REPORT.md` §3 lists five things the workbench demonstrated. Each is
//! re-run here against the production daemon, through `Daemon::dispatch`:
//!
//! | R3 spike §3 scenario | production test |
//! |---|---|
//! | canonical workspace snapshot identities | [`baseline_snapshot_and_task_identities_are_canonical_across_two_fresh_daemons`] |
//! | idempotent identical task creation | [`baseline_an_identical_keyed_start_is_byte_identical_and_mints_one_task`] |
//! | rejection of idempotency-key reuse with a different request | [`baseline_a_key_reused_for_a_different_request_is_refused`] |
//! | resumable continuation | [`baseline_a_parked_continuation_resumes_to_the_frozen_die_hard_verdict`] |
//! | rejection of continuation under a different workspace snapshot | [`baseline_a_resume_naming_a_different_snapshot_is_stale`] |
//!
//! # The campaign map
//!
//! | # | Axis | Attack | Test | Result |
//! |---|---|---|---|---|
//! | 1 | idempotence | keyed replay after unrelated interleaved operations | [`attack_idempotence_a_keyed_replay_after_interleaved_operations_is_byte_identical`] | held |
//! | 2 | idempotence | keyed replay on a reconnected daemon (INV-002's transplant device) | [`attack_idempotence_a_keyed_replay_on_a_reconnected_daemon_is_byte_identical`] | held |
//! | 3 | idempotence | keyed replay *across* a supersession — does the ledger re-run or re-admit? | [`attack_idempotence_a_keyed_replay_across_a_supersession_neither_reruns_nor_readmits`] | held |
//! | 4 | idempotence | a fresh key on an identical start (the IDL's cached-result lane) | [`attack_idempotence_two_fresh_keys_on_one_campaign_agree_byte_for_byte`] | held |
//! | 5 | idempotence | **the same key with a different envelope budget** | [`regression_the_replay_key_covers_the_envelope_budget`] | **FALSIFIED**, fixed by bn-h1zqz — now a guard |
//! | 6 | staleness | every snapshot-consuming operation against a superseded handle | [`attack_staleness_every_snapshot_consuming_operation_refuses_a_superseded_handle`] | held |
//! | 7 | staleness | a snapshot superseded twice over — the `LineageError::Unknown` arm | [`attack_staleness_a_snapshot_the_fork_can_no_longer_place_is_still_refused`] | held (with a mapping inconsistency, recorded) |
//! | 8 | staleness | **replay `workspace.create` under a fresh key, then reuse the old handle** | [`regression_a_replayed_create_converges_and_leaves_the_old_snapshot_stale`] | **FALSIFIED**, fixed by bn-n1xou — now a guard |
//! | 9 | staleness | the same rewind, seen from the snapshot that *was* current | [`regression_a_converged_create_does_not_strand_the_snapshot_that_is_current`] | **FALSIFIED**, fixed by bn-n1xou — now a guard |
//! | 10 | staleness | the same rewind with `seal: false` — a live continuation is revoked | [`regression_an_unsealed_re_create_does_not_revoke_a_valid_continuation`] | **FALSIFIED** (fails closed), fixed by bn-n1xou — now a guard |
//! | 11 | staleness | anti-vacuity: the same create under the *same* key changes nothing | [`attack_staleness_the_same_create_under_the_same_key_changes_nothing`] | held |
//! | 12 | resume | two fresh daemons, the whole DX-03 script, frame for frame | [`attack_determinism_two_fresh_daemons_produce_byte_identical_transcripts`] | held |
//! | 13 | resume | a hostile suite spliced at every position of the resume script (31 runs) | [`attack_determinism_a_resume_is_byte_identical_at_every_placement_of_a_hostile_suite`] | held |
//! | 14 | resume | resume under every arm of `rule task.update_budget`'s legality table | [`attack_determinism_no_budget_arm_lets_a_resume_un_explore_a_campaign`] | held |
//! | 15 | resume | double-resume one continuation — two campaigns, or one? | [`attack_reuse_double_resuming_one_continuation_is_one_task_and_one_campaign`] | held |
//! | 16 | resume | resume a continuation a *newer* continuation has superseded | [`attack_reuse_a_superseded_continuation_advances_the_same_task_not_a_second_one`] | held (with a recorded concern) |
//! | 17 | handle reuse | a completed task handle, on every observing operation, twice | [`attack_reuse_a_completed_task_answers_idempotently_on_every_observing_operation`] | held |
//! | 18 | handle reuse | **a continuation reused after cancellation, carrying a budget** | [`regression_resume_refuses_the_terminal_budget_write_update_budget_refuses`] | **FALSIFIED**, fixed by bn-10093 — now a guard |
//! | 19 | handle reuse | the same reuse *without* a budget — a genuine no-op? | [`attack_reuse_a_cancelled_continuation_without_a_budget_is_a_true_no_op`] | held |
//! | 20 | handle reuse | cancelling twice | [`attack_reuse_cancelling_twice_changes_nothing_the_second_time`] | held |
//! | 21 | handle reuse | an unheld handle vs. a lookalike daemon's denial (RFC 0027 X2) | [`attack_reuse_an_unheld_handle_is_byte_identical_with_a_lookalike_daemons_denial`] | held |
//! | 22 | resume | resume after the daemon's epochs advanced under it | [`attack_reuse_a_continuation_is_refused_when_the_daemons_epochs_moved`] | held |
//!
//! # Two concerns this campaign recorded, since dispositioned (bn-10wdo)
//!
//! Neither was a defect, and neither test below changed as a result — the two dispositions
//! are recorded in RFC 0026 and cited from here and from the daemon's own module docs, per
//! bn-10wdo's instruction to update a campaign test that pinned a concern to cite its
//! disposition rather than to weaken what it asserts.
//!
//! - **`LineageError::Unknown` maps to two different wire codes, and it stays that way.**
//!   `daemon::workspace` documents its choice at length — `Unknown` → `CapabilityDenied`,
//!   because "inventing a distinguishable not-found is precisely the existence oracle RFC
//!   0027 X2 forbids" — while `daemon::task::resume` and `daemon::verification::start` map
//!   *both* lineage arms to `StaleSnapshot`. RFC 0026's "An unplaceable lineage identity: one
//!   condition, two codes, and why that is a decision" ratifies both readings rather than
//!   aligning them: `daemon::workspace`'s two operations check a caller-declared identity
//!   about to be spent on a write (X2's compare-and-set shape); `task.resume` and
//!   `verification.start` check an identity their own `state.workspace(..)` lookup has
//!   already resolved, where no existence-oracle question remains open, only a currency one.
//!   Every path still refuses, so the pass condition is not touched. Recorded by attack 7.
//! - **`Continuation::bounds` and `Continuation::frontier` are pinned and read by nothing
//!   that decides whether a resume is admissible or that seeds the run it gates.** RFC 0026's
//!   "What a `cont_*` handle means, and what `bounds`/`frontier` are pinned for" dispositions
//!   this: a `cont_*` handle means "advance this task", not "resume from this point", and
//!   `bounds`/`frontier` are pinned as *provenance* (RFC 0030's budget rule) against which the
//!   monotonicity obligation — "the frontier after resume MUST include the frontier before
//!   it" — is checked, not instructions an engine consumes. `daemon::task::resume` re-derives
//!   the bound from `TaskEntry::bounds()` and the engine re-explores from the start, so the
//!   pinned frontier constrains nothing *by being read*; it is sound today only because
//!   breadth-first exploration makes the parked set a provable superset of the resumed one.
//!   That is *why* attack 16 finds a superseded continuation admitted rather than refused. The
//!   silent-break the concern named — a future non-prefix engine breaking this while
//!   `bounds`/`frontier` stay unread — now has a guard: `daemon::verification::run` carries a
//!   `debug_assert` that every state the resumed continuation's frontier named is in the new
//!   exploration's reachable set, fed `continuation.frontier` from `task::resume` itself. It
//!   fires on the same path attack 16 exercises, and on the resume in
//!   `tests/daemon_task_operations.rs`'s `budget_exhaustion_parks_a_continuation_…` test, which
//!   independently re-derives the same inclusion property from outside the daemon.
//!
//! # House rules
//!
//! - **`src/` is not touched**, and no pre-existing test is edited, weakened, or moved.
//! - **Every assertion is on a typed value or on canonical bytes, never on timing.** This
//!   file makes no concurrency claim: `Daemon::dispatch` takes `&mut self`, so one thread
//!   drives one request at a time, which is this daemon's documented grain. Concurrency is
//!   G0-DX-13's axis and is evidenced in `continuum-workspace`'s own campaign.
//! - **Bounded by construction.** The one sweep is 6 hostile operations × 5 placements + 1
//!   control = 31 whole-campaign runs over a 16-state model; nothing here allocates from a
//!   loop bound, doubles a buffer, or spawns a thread.
//!
//! [`ForkName`]: continuum_workspace::lineage::ForkName
//! [`BudgetLedger`]: continuum_task::budget::BudgetLedger

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::operations::encode_payload;
use continuumd::codec::to_bytes;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskSubscribeRequest,
    TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceDiffRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::task::TaskRecord;
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind, TaskStatus,
};

/// The TV-009 port's model, verbatim — the bytes that go into the snapshot.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// The park point and the completion point, the two numbers
/// `daemon_task_operations.rs` and `inv002_no_hidden_state_evidence.rs` already proved
/// reliable for this model: a `states: 4` budget parks Die Hard, `states: 64` closes it.
const PARK_BUDGET: u64 = 4;
/// The bound that admits Die Hard's whole 16-state reachable set.
const CLOSING_BUDGET: u64 = 64;
/// Die Hard's frozen reachable-state count.
const FROZEN_STATES: u64 = 16;

// =========================================================================================
// fixture plumbing — the idioms `daemon_task_operations.rs`,
// `inv002_no_hidden_state_evidence.rs` and `pr8_exit_evidence.rs` already share
// =========================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
        instances: Optional::Absent,
    }
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-dx03-falsification".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.2 is served")
}

/// The epochs this daemon serves. Pinned rather than unpinned, so a continuation has
/// something to pin and the resume predicate has something to disagree with.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// A daemon with all four families and every capability this file's workflows need,
/// serving `served` as its epoch set.
fn shell_with(served: EpochSet) -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(served)
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

fn shell() -> Daemon {
    shell_with(epochs())
}

/// INV-002's transplant device, reused verbatim: move `DaemonState` — and only
/// `DaemonState` — out of `old` into a freshly, independently negotiated daemon. `old` is
/// left holding an empty state and plays the part of the closed socket.
fn reconnect(old: &mut Daemon) -> Daemon {
    let mut fresh = shell();
    *fresh.state_mut() = std::mem::replace(old.state_mut(), DaemonState::new());
    fresh
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(states) => Optional::Present(states),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(Some(states)));
    envelope
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

/// The wire bytes of one result: the envelope followed by the typed payload's own
/// encoding, exactly as `transport::Server::answer` splices them into one frame. Two calls
/// compare byte-identical here if and only if they would have produced the identical frame
/// on the real wire (`inv002_no_hidden_state_evidence.rs`'s own `wire_bytes`).
fn wire_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let mut bytes = to_bytes(&outcome.envelope).expect("a result envelope encodes");
    if let Some(payload) = encode_payload(&outcome.payload).expect("a payload encodes") {
        bytes.extend_from_slice(payload.as_bytes());
    }
    bytes
}

/// A bootstrapped daemon plus everything a caller needs to re-issue the exact
/// `workspace.create` that produced its snapshot — which is what attack 8 needs and what
/// no existing fixture in this crate exposes.
struct Fixture {
    daemon: Daemon,
    snapshot: WorkspaceHandle,
    components: SnapshotComponents,
}

fn bootstrap() -> Fixture {
    bootstrap_with(epochs())
}

fn bootstrap_with(served: EpochSet) -> Fixture {
    let mut daemon = shell_with(served);
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );

    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );

    let components = SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent,
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![configuration],
        file_components: Optional::Absent,
    };

    let created = create(&mut daemon, &components, true, "req_create", "idem-create");
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = snapshot_of(&created.payload);

    Fixture {
        daemon,
        snapshot,
        components,
    }
}

// --- the operations, each as one `dispatch` ----------------------------------------------

fn create(
    daemon: &mut Daemon,
    components: &SnapshotComponents,
    seal_it: bool,
    request: &str,
    key: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(seal_it),
        }),
    })
}

fn fork(
    daemon: &mut Daemon,
    base: &WorkspaceHandle,
    content: &[u8],
    request: &str,
    key: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.fork", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: content.to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    })
}

fn seal(
    daemon: &mut Daemon,
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.seal", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: snapshot.clone(),
        }),
    })
}

fn diff(
    daemon: &mut Daemon,
    before: &WorkspaceHandle,
    after: &WorkspaceHandle,
    request: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            envelope("workspace.diff", "agent:builder", "cap_builder", request),
            CLOSING_BUDGET,
        ),
        arguments: Arguments::WorkspaceDiff(WorkspaceDiffRequest {
            before: before.clone(),
            after: after.clone(),
            layers: Vec::new(),
        }),
    })
}

fn start_target(
    daemon: &mut Daemon,
    snapshot: &WorkspaceHandle,
    states: u64,
    target: Target,
    request: &str,
    key: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope("verification.start", "agent:runner", "cap_runner", request),
                    key,
                ),
                states,
            ),
            snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target,
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn all_claims() -> Target {
    Target {
        kind: TargetKind::AllClaims,
        id: "DieHard".to_owned(),
    }
}

fn start(
    daemon: &mut Daemon,
    snapshot: &WorkspaceHandle,
    states: u64,
    request: &str,
    key: &str,
) -> OperationOutcome {
    start_target(daemon, snapshot, states, all_claims(), request, key)
}

/// `task.resume`. `snapshot` is the *envelope*'s snapshot — the field `rule errors.common`
/// makes staleness-checkable and the one R3 spike §3's "rejection of continuation under a
/// different workspace snapshot" is about.
fn resume(
    daemon: &mut Daemon,
    continuation: &ContinuationHandle,
    states: Option<u64>,
    snapshot: Option<&WorkspaceHandle>,
    request: &str,
    key: &str,
) -> OperationOutcome {
    let mut request_envelope = keyed(
        envelope("task.resume", "agent:runner", "cap_runner", request),
        key,
    );
    request_envelope.budget = Optional::Present(budget(Some(CLOSING_BUDGET)));
    if let Some(snapshot) = snapshot {
        request_envelope = on(request_envelope, snapshot);
    }
    daemon.dispatch(&OperationRequest {
        envelope: request_envelope,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: match states {
                Some(states) => Optional::Present(budget(Some(states))),
                None => Optional::Absent,
            },
        }),
    })
}

fn cancel(daemon: &mut Daemon, task: &TaskHandle, request: &str, key: &str) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", request),
            key,
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}

fn update_budget(
    daemon: &mut Daemon,
    task: &TaskHandle,
    states: Option<u64>,
    request: &str,
    key: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.update_budget", "agent:runner", "cap_runner", request),
            key,
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: task.clone(),
            budget: budget(states),
        }),
    })
}

fn status(daemon: &mut Daemon, task: &TaskHandle, request: &str) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    })
}

fn subscribe(daemon: &mut Daemon, task: &TaskHandle, request: &str) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("task.subscribe", "agent:reader", "cap_reader", request),
        arguments: Arguments::TaskSubscribe(TaskSubscribeRequest { task: task.clone() }),
    })
}

fn verification_result(daemon: &mut Daemon, task: &TaskHandle, request: &str) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("verification.result", "agent:runner", "cap_runner", request),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    })
}

// --- readers -----------------------------------------------------------------------------

fn snapshot_of(payload: &Payload) -> WorkspaceHandle {
    match payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        Payload::WorkspaceFork(response) => response.snapshot.clone(),
        other => panic!("expected a workspace payload, got {other:?}"),
    }
}

/// `WorkspaceCreateResponse.sealed` — "whether the snapshot is sealed", the field a
/// converging create answers the `seal` divergence through.
fn sealed_of(payload: &Payload) -> bool {
    match payload {
        Payload::WorkspaceCreate(response) => response.sealed,
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

fn task_of(outcome: &OperationOutcome) -> TaskHandle {
    outcome
        .envelope
        .task
        .value()
        .cloned()
        .expect("a task-starting result names its task")
}

fn record_of(outcome: &OperationOutcome) -> TaskRecord {
    match &outcome.payload {
        Payload::TaskStatus(record) => record.clone(),
        other => panic!("expected a task.status payload, got {other:?}"),
    }
}

fn continuation_of(record: &TaskRecord) -> ContinuationHandle {
    record
        .continuation
        .value()
        .cloned()
        .expect("a suspended task is resumable by definition (plan §11.4)")
}

/// A daemon holding the Die Hard workspace sealed, with a campaign parked at
/// [`PARK_BUDGET`]: the shape every resume attack starts from.
struct Parked {
    fixture: Fixture,
    task: TaskHandle,
    continuation: ContinuationHandle,
}

fn drive_to_park() -> Parked {
    let mut fixture = bootstrap();
    let started = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        PARK_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskSuspended,
        "a `states: 4` budget parks Die Hard inside the dispatch that started it: {:?}",
        started.envelope.error
    );
    let task = task_of(&started);
    let continuation = continuation_of(&record_of(&status(&mut fixture.daemon, &task, "req_s0")));
    Parked {
        fixture,
        task,
        continuation,
    }
}

// =========================================================================================
// baseline — R3 spike §3's five scenarios, at the production grain
// =========================================================================================

/// **Baseline (spike §3: "canonical workspace snapshot identities").** Two independently
/// built daemons, given the identical script, mint the identical `ws_*` snapshot handle and
/// the identical `task_*` handle. Nothing about which process, which daemon object, or which
/// call ordering enters either identity: both are content identities of what the artifact
/// *is* (`daemon::task`'s "Identity, and why it is content-addressed").
#[test]
fn baseline_snapshot_and_task_identities_are_canonical_across_two_fresh_daemons() {
    let one = drive_to_park();
    let two = drive_to_park();
    assert_eq!(one.fixture.snapshot, two.fixture.snapshot);
    assert_eq!(one.task, two.task);
    assert_eq!(one.continuation, two.continuation);
    assert_ne!(
        one.fixture.snapshot.as_str(),
        "",
        "a vacuous identity proves nothing"
    );
}

/// **Baseline (spike §3: "idempotent identical task creation").** A `verification.start`
/// replayed under the same key returns byte-identical wire bytes, and exactly one task
/// exists. Stronger than the spike's claim, which was about identity alone.
#[test]
fn baseline_an_identical_keyed_start_is_byte_identical_and_mints_one_task() {
    let mut fixture = bootstrap();
    let first = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    let replay = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(first.envelope.status, ResultStatus::TaskStarted);
    assert_eq!(wire_bytes(&first), wire_bytes(&replay));
    assert_eq!(
        fixture.daemon.state().tasks().handles().len(),
        1,
        "one campaign, one task"
    );
}

/// **Baseline (spike §3: "rejection of idempotency-key reuse with different request").**
/// Two dimensions of "different request" are refused with `IdempotencyKeyReused`: the
/// operation's own arguments (a different `Target`) and the envelope's `snapshot`. The
/// third dimension — the envelope's `budget` — is *not*, and that is attack 5.
#[test]
fn baseline_a_key_reused_for_a_different_request_is_refused() {
    let mut fixture = bootstrap();
    start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );

    let different_arguments = start_target(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        Target {
            kind: TargetKind::Property,
            id: diehard::TYPE_OK.to_owned(),
        },
        "req_start_2",
        "idem-start",
    );
    assert_eq!(
        different_arguments.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "a different `Target` is a different request"
    );
    assert_eq!(
        different_arguments.payload,
        Payload::None,
        "no partial effect"
    );

    // A different envelope `snapshot`: `ReplayKey` carries it, so this half holds.
    let forked = fork(
        &mut fixture.daemon,
        &fixture.snapshot,
        b"# moved on\n",
        "req_fork",
        "idem-fork",
    );
    let derived = snapshot_of(&forked.payload);
    let different_snapshot = start(
        &mut fixture.daemon,
        &derived,
        CLOSING_BUDGET,
        "req_start_3",
        "idem-start",
    );
    assert_eq!(
        different_snapshot.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "a different envelope `snapshot` is a different request, and is caught *before* the \
         snapshot is even resolved"
    );
}

/// **Baseline (spike §3: "resumable continuation").** The parked campaign resumes to Die
/// Hard's frozen verdict: 16 states, the exploration closed, the task `Completed`, and the
/// continuation gone from the record because a closed campaign parks nothing.
#[test]
fn baseline_a_parked_continuation_resumes_to_the_frozen_die_hard_verdict() {
    let mut parked = drive_to_park();
    let resumed = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume",
    );
    assert_eq!(
        resumed.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        resumed.envelope.error
    );
    let record = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"));
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.cost.states, Optional::Present(FROZEN_STATES));
    assert!(
        record.continuation.is_absent(),
        "a closed campaign parks nothing"
    );
}

/// **Baseline (spike §3: "rejection of continuation under a different workspace
/// snapshot").** The exact scenario the spike named, at the production grain: a resume whose
/// *envelope* names a snapshot other than the one the continuation pinned is refused with
/// `StaleSnapshot`, before any run. The control is the same request with the field left
/// null, which the decision table admits ("naming no snapshot says nothing").
#[test]
fn baseline_a_resume_naming_a_different_snapshot_is_stale() {
    let mut parked = drive_to_park();
    let forked = fork(
        &mut parked.fixture.daemon,
        &parked.fixture.snapshot,
        b"# elsewhere\n",
        "req_fork",
        "idem-fork",
    );
    let elsewhere = snapshot_of(&forked.payload);

    let refused = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        Some(&elsewhere),
        "req_resume_bad",
        "idem-resume-bad",
    );
    assert_eq!(refused.error_code(), Some(ErrorCode::StaleSnapshot));
    assert_eq!(refused.payload, Payload::None, "no partial effect");
    let untouched = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"));
    assert_eq!(
        untouched.status,
        TaskStatus::Suspended,
        "the refused resume advanced nothing"
    );
    assert_eq!(untouched.cost.states, Optional::Present(3));
}

// =========================================================================================
// axis 1 — idempotence
// =========================================================================================

/// **Attack 1.** A keyed replay is byte-identical *after* a run of unrelated operations has
/// moved the daemon's state around it: reads by three different actors, a denial, a
/// malformed shape, a refused key reuse, and a second actor's own campaign. None of that
/// leaks into the recorded outcome the ledger returns.
#[test]
fn attack_idempotence_a_keyed_replay_after_interleaved_operations_is_byte_identical() {
    let mut fixture = bootstrap();
    let first = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    let task = task_of(&first);

    for hostile in HOSTILE {
        hostile(&mut fixture, &task);
    }

    let replay = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(
        wire_bytes(&first),
        wire_bytes(&replay),
        "the ledger returns the recorded outcome verbatim, whatever happened in between"
    );
}

/// **Attack 2.** The same replay, on a daemon that is not the one that recorded it: INV-002's
/// transplant device moves `DaemonState` alone into a freshly negotiated daemon, and the
/// replay is still byte-identical. The ledger is state, not session.
#[test]
fn attack_idempotence_a_keyed_replay_on_a_reconnected_daemon_is_byte_identical() {
    let mut fixture = bootstrap();
    let first = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    let mut reconnected = reconnect(&mut fixture.daemon);
    let replay = start(
        &mut reconnected,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(wire_bytes(&first), wire_bytes(&replay));
}

/// **Attack 3.** A replay whose snapshot has been superseded since the first call. The
/// ledger answers with the recorded success — it neither re-runs the campaign against the
/// new tree nor re-admits the stale snapshot — and the anti-vacuity control is the identical
/// request under a *fresh* key, which is refused `StaleSnapshot`. So the replay is a replay,
/// not a second admission of a handle the lineage has moved past.
#[test]
fn attack_idempotence_a_keyed_replay_across_a_supersession_neither_reruns_nor_readmits() {
    let mut fixture = bootstrap();
    let first = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    let task = task_of(&first);
    fork(
        &mut fixture.daemon,
        &fixture.snapshot,
        b"# moved on\n",
        "req_fork",
        "idem-fork",
    );

    let replay = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(wire_bytes(&first), wire_bytes(&replay));
    assert_eq!(
        fixture.daemon.state().tasks().handles().len(),
        1,
        "no second campaign ran"
    );
    assert_eq!(
        fixture
            .daemon
            .state()
            .tasks()
            .get(&task)
            .expect("held")
            .publications(),
        1,
        "and no second publication was committed"
    );

    let fresh_key = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start_2",
        "idem-start-2",
    );
    assert_eq!(
        fresh_key.error_code(),
        Some(ErrorCode::StaleSnapshot),
        "anti-vacuity: without the ledger, this request is refused — so the replay above \
         really was the ledger's answer and not a second admission"
    );
}

/// **Attack 4.** Two *different* keys on the identical campaign. The IDL declares this lane
/// — "a cached result when one exists for the same snapshot, intent, target, and epochs" —
/// so the answer is deliberately not the first call's (`result` present instead of `task`).
/// What must hold, and does, is that two fresh keys agree with *each other* byte for byte,
/// that both name one task identity, and that no second campaign runs.
#[test]
fn attack_idempotence_two_fresh_keys_on_one_campaign_agree_byte_for_byte() {
    let mut fixture = bootstrap();
    let first = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-a",
    );
    let task = task_of(&first);
    let cached_one = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-b",
    );
    let cached_two = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-c",
    );
    assert_eq!(cached_one.envelope.status, ResultStatus::Ok);
    assert_eq!(
        wire_bytes(&cached_one),
        wire_bytes(&cached_two),
        "the cached-result lane is a pure function of the task it reports"
    );
    assert_eq!(
        cached_one.envelope.task.value(),
        Some(&task),
        "the envelope still names the one task identity"
    );
    assert_eq!(fixture.daemon.state().tasks().handles().len(), 1);
    assert_eq!(
        fixture
            .daemon
            .state()
            .tasks()
            .get(&task)
            .expect("held")
            .publications(),
        1,
        "the cached lane published nothing new"
    );
}

/// **Attack 5 — was FALSIFICATION, now a regression guard (DEFECT 3, fixed by bn-h1zqz).**
///
/// *What must hold.* `rule idempotency.replay`:
///
/// > A mutation replayed with the same `idempotency_key` and a byte-identical canonical
/// > request MUST return the same task or artifact identity. The same key with a different
/// > request MUST be rejected with `IdempotencyKeyReused`.
///
/// `budget` is a field of `RequestEnvelope`, so two requests with different budgets are
/// different requests. They are also *different tasks*:
/// `daemon::verification::start_handle` writes `budget_preimage` into the `task_*` content
/// identity, which is why the two campaigns below have distinct handles when they are run
/// under distinct keys.
///
/// *What this attack found.* `daemon::state::ReplayKey` carried the operation name, the
/// envelope's `snapshot`, the envelope's `intent`, and the decoded arguments — and not the
/// envelope's `budget`. So a caller that asked for a 64-state campaign under a key it had
/// already used for a 4-state one was neither refused nor served: it was handed the 4-state
/// task's recorded answer, whose `status` is `task_suspended` and whose task handle is the
/// *other* campaign's. No error, no new task, and nothing in the answer said the budget was
/// ignored.
///
/// *What the fix did.* bn-h1zqz put the whole canonical request in the key — `budget`,
/// `output_policy`, and `page` beside the four already there — so the same key with a
/// different budget is the refusal the rule's own text requires. `ReplayKey`'s documentation
/// carries the field-by-field audit, including the fields deliberately left out (`request_id`
/// and `trace` are per-attempt; `actor` is the ledger's other map key).
///
/// The control below is unchanged and is what keeps this from being vacuous: under two
/// *different* keys these two requests really are two campaigns with two identities, so the
/// refusal above is a distinction the daemon makes rather than one it invents.
#[test]
fn regression_the_replay_key_covers_the_envelope_budget() {
    let mut fixture = bootstrap();
    let parked = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        PARK_BUDGET,
        "req_start",
        "idem-shared",
    );
    assert_eq!(parked.envelope.status, ResultStatus::TaskSuspended);
    let small = task_of(&parked);

    // The same key, a *larger* budget — a different canonical request by the IDL's own
    // definition of `RequestEnvelope`.
    let larger = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start_2",
        "idem-shared",
    );

    assert_eq!(
        larger.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "`rule idempotency.replay` requires this refusal: {:?}",
        larger.envelope.error
    );
    assert_eq!(larger.payload, Payload::None, "no partial effect");
    assert_eq!(
        fixture.daemon.state().tasks().handles().len(),
        1,
        "the refused request started nothing, and the recorded campaign is untouched"
    );

    // And the *true* replay — the identical canonical request under the identical key — is
    // still answered from the ledger, byte for byte. A key that refused this would have
    // repaired the rule's second sentence by breaking its first.
    let replay = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        PARK_BUDGET,
        "req_start",
        "idem-shared",
    );
    assert_eq!(
        wire_bytes(&replay),
        wire_bytes(&parked),
        "an identical canonical request under the same key returns the recorded outcome \
         verbatim"
    );

    // Control: under two *different* keys the same two requests are two campaigns with two
    // identities, so the refusal above separates two requests the daemon really does
    // distinguish.
    let mut control = bootstrap();
    let small_control = start(
        &mut control.daemon,
        &control.snapshot,
        PARK_BUDGET,
        "req_start",
        "idem-small",
    );
    let large_control = start(
        &mut control.daemon,
        &control.snapshot,
        CLOSING_BUDGET,
        "req_start_2",
        "idem-large",
    );
    assert_eq!(task_of(&small_control), small);
    assert_ne!(
        task_of(&large_control),
        small,
        "the budget is in the task's own preimage: two budgets are two tasks"
    );
    assert_eq!(control.daemon.state().tasks().handles().len(), 2);
}

// =========================================================================================
// axis 2 — stale snapshot rejection
// =========================================================================================

/// **Attack 6.** Every operation that consumes a snapshot, against a handle the lineage has
/// superseded: `verification.start`, `task.resume`, `workspace.fork`, and `workspace.seal`
/// all answer `StaleSnapshot`, and `workspace.diff` answers `StaleSnapshot` for the unsealed
/// derived snapshot (RFC 0031's sealed-input rule) rather than computing a partial answer. No path silently runs against
/// the new tree, and no path silently runs against the old one.
#[test]
fn attack_staleness_every_snapshot_consuming_operation_refuses_a_superseded_handle() {
    let mut parked = drive_to_park();
    let old = parked.fixture.snapshot.clone();
    let forked = fork(
        &mut parked.fixture.daemon,
        &old,
        b"# moved on\n",
        "req_fork",
        "idem-fork",
    );
    assert_eq!(forked.envelope.status, ResultStatus::Ok);
    let derived = snapshot_of(&forked.payload);

    let start_refusal = start(
        &mut parked.fixture.daemon,
        &old,
        CLOSING_BUDGET,
        "req_start_stale",
        "idem-start-stale",
    );
    assert_eq!(start_refusal.error_code(), Some(ErrorCode::StaleSnapshot));

    let resume_refusal = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume_stale",
        "idem-resume-stale",
    );
    assert_eq!(resume_refusal.error_code(), Some(ErrorCode::StaleSnapshot));

    let fork_refusal = fork(
        &mut parked.fixture.daemon,
        &old,
        b"# again\n",
        "req_fork_stale",
        "idem-fork-stale",
    );
    assert_eq!(fork_refusal.error_code(), Some(ErrorCode::StaleSnapshot));

    let seal_refusal = seal(
        &mut parked.fixture.daemon,
        &old,
        "req_seal_stale",
        "idem-seal-stale",
    );
    assert_eq!(seal_refusal.error_code(), Some(ErrorCode::StaleSnapshot));

    let diff_refusal = diff(&mut parked.fixture.daemon, &old, &derived, "req_diff");
    assert_eq!(
        diff_refusal.error_code(),
        Some(ErrorCode::StaleSnapshot),
        "the diff consults its carriers (bn-27mx7): the derived snapshot is unsealed, and \
         RFC 0031 requires both inputs sealed"
    );

    // Nothing advanced. Every refusal above was a refusal, not a partial effect.
    let untouched = record_of(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_status",
    ));
    assert_eq!(untouched.status, TaskStatus::Suspended);
    assert_eq!(untouched.cost.states, Optional::Present(3));
}

/// **Attack 7.** A snapshot superseded so far that the `Fork` value can no longer *place* it
/// — `check_current`'s honest `LineageError::Unknown` arm, reached after three further
/// advances. Every path still refuses; nothing is silently accepted because the lineage
/// forgot it.
///
/// It also exercises the concern the campaign recorded and bn-10wdo dispositioned: the two
/// lineage arms map to two different wire codes depending on which family asks, and RFC
/// 0026's "An unplaceable lineage identity" ratifies both rather than aligning them.
/// `daemon::workspace` argues its choice from RFC 0027 X2 ("inventing a distinguishable
/// not-found is precisely the existence oracle") for a caller-declared identity about to be
/// spent on a write, and answers `CapabilityDenied`; `daemon::task::resume` and
/// `daemon::verification::start` collapse both arms to `StaleSnapshot`, for an identity their
/// own lookup has already resolved, where no existence-oracle question remains open. Both
/// refuse, so DX-03's pass condition is untouched — the two spellings are the decision, not a
/// disagreement still to be resolved.
#[test]
fn attack_staleness_a_snapshot_the_fork_can_no_longer_place_is_still_refused() {
    let mut fixture = bootstrap();
    // Seal-and-campaign on the *second* point of the lineage, so the pinned snapshot can
    // fall out of the `Fork`'s three-identity memory while remaining a real sealed record.
    let second = snapshot_of(
        &fork(
            &mut fixture.daemon,
            &fixture.snapshot,
            b"# two\n",
            "req_fork_2",
            "idem-fork-2",
        )
        .payload,
    );
    assert_eq!(
        seal(&mut fixture.daemon, &second, "req_seal_2", "idem-seal-2")
            .envelope
            .status,
        ResultStatus::Ok
    );
    let started = start(
        &mut fixture.daemon,
        &second,
        PARK_BUDGET,
        "req_start",
        "idem-start",
    );
    assert_eq!(started.envelope.status, ResultStatus::TaskSuspended);
    let task = task_of(&started);
    let continuation = continuation_of(&record_of(&status(&mut fixture.daemon, &task, "req_s0")));

    // Three more advances: `second` is now neither the origin, nor the immediate parent,
    // nor the head, so `check_current` reports `Unknown` rather than claiming a staleness
    // it cannot prove.
    let mut head = second.clone();
    for (index, bytes) in [b"# three\n", b"# four4\n", b"# five5\n"]
        .into_iter()
        .enumerate()
    {
        let advanced = fork(
            &mut fixture.daemon,
            &head,
            bytes,
            &format!("req_advance_{index}"),
            &format!("idem-advance-{index}"),
        );
        assert_eq!(
            advanced.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            advanced.envelope.error
        );
        head = snapshot_of(&advanced.payload);
    }

    assert_eq!(
        resume(
            &mut fixture.daemon,
            &continuation,
            Some(CLOSING_BUDGET),
            None,
            "req_resume",
            "idem-resume",
        )
        .error_code(),
        Some(ErrorCode::StaleSnapshot),
        "`task.resume` maps both lineage arms to `StaleSnapshot`"
    );
    assert_eq!(
        start(
            &mut fixture.daemon,
            &second,
            CLOSING_BUDGET,
            "req_start_2",
            "idem-start-2",
        )
        .error_code(),
        Some(ErrorCode::StaleSnapshot),
        "`verification.start` does the same"
    );
    assert_eq!(
        seal(&mut fixture.daemon, &second, "req_seal_x", "idem-seal-x").error_code(),
        Some(ErrorCode::CapabilityDenied),
        "dispositioned (RFC 0026, \"An unplaceable lineage identity\"): `workspace.seal` \
         checks the same lineage verdict as a compare-and-set guard on a write, and answers \
         `CapabilityDenied` by RFC 0027 X2's reasoning — a refusal either way, two codes by \
         design, not a disagreement still open"
    );
}

/// **Attack 8 — was FALSIFICATION, now a regression guard (DEFECT 1, fixed by bn-n1xou).**
/// The DX-03 experiment's own sentence, run to its end: create a snapshot, start a bounded
/// task, mutate the workspace, reuse the old handle.
///
/// *What must hold.* RFC 0026: "the pinned snapshot is no longer the current sealed
/// snapshot" is `StaleSnapshot`, and `notes/plan/notes/G0_SPIKE_MATRIX.md` makes stale
/// snapshot rejection one third of DX-03's pass condition. `continuum-workspace`'s own
/// `staleness` module states the property it implements: "an old snapshot remains
/// reproducible after the working tree changes; **using a stale one is a typed error, never
/// a silent re-read**."
///
/// *What this attack found.* `daemon::workspace::create` derived the lineage's `ForkName`
/// from the created snapshot's content-addressed `ws_*` handle and then called
/// `DaemonState::put_lineage`, an unconditional insert. A second `workspace.create` naming
/// the same components therefore replaced the live `Fork` — whose head had advanced — with
/// a fresh `Fork::diverge` rooted at the original snapshot. The mutation was forgotten, and
/// the superseded handle was accepted again: the same `verification.start` that was refused
/// `StaleSnapshot` one request earlier ran a full campaign, against a tree the lineage had
/// already moved past, and `task.resume` of the continuation pinned to that snapshot was
/// un-refused by the same step.
///
/// A `@mutation` requires an idempotency key, so this needs a *fresh* key — which was never
/// a hurdle: `rule idempotency.replay` honours a key only "for at least the retention
/// window" the daemon declares, and keys are scoped per actor, so a retry after the window
/// or a second agent creating the same workspace both landed here.
///
/// *What the fix did.* bn-n1xou made an identical create **converge**: the lineage is opened
/// if absent and otherwise left exactly as it is (`DaemonState::open_lineage` is
/// put-if-absent), and the held workspace record is not replaced. The re-create still
/// answers with the snapshot it names — the identity is content-addressed and really does
/// already exist — and every refusal that stood one request before it stands one request
/// after it. That is the G0-DX-13 precedent read at the lineage: identical creation
/// converges on one semantic identity and nothing is lost.
#[test]
fn regression_a_replayed_create_converges_and_leaves_the_old_snapshot_stale() {
    let mut parked = drive_to_park();
    let old = parked.fixture.snapshot.clone();

    // Mutate the workspace.
    let forked = fork(
        &mut parked.fixture.daemon,
        &old,
        b"# moved on\n",
        "req_fork",
        "idem-fork",
    );
    assert_eq!(forked.envelope.status, ResultStatus::Ok);

    // The old handle is refused, exactly as the pass condition requires.
    assert_eq!(
        start(
            &mut parked.fixture.daemon,
            &old,
            CLOSING_BUDGET,
            "req_start_before",
            "idem-start-before",
        )
        .error_code(),
        Some(ErrorCode::StaleSnapshot),
        "the property under attack really does hold one request before the attack"
    );
    assert_eq!(
        resume(
            &mut parked.fixture.daemon,
            &parked.continuation,
            Some(CLOSING_BUDGET),
            None,
            "req_resume_before",
            "idem-resume-before",
        )
        .error_code(),
        Some(ErrorCode::StaleSnapshot)
    );

    // The attack: re-issue the identical `workspace.create` under a fresh key.
    let recreated = create(
        &mut parked.fixture.daemon,
        &parked.fixture.components,
        true,
        "req_recreate",
        "idem-recreate",
    );
    assert_eq!(
        recreated.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        recreated.envelope.error
    );
    assert_eq!(
        snapshot_of(&recreated.payload),
        old,
        "identical components name the identical snapshot — which is also what names the \
         lineage, and is the whole mechanism"
    );

    // The guard: the superseded handle is still superseded.
    let after = start(
        &mut parked.fixture.daemon,
        &old,
        CLOSING_BUDGET,
        "req_start_after",
        "idem-start-after",
    );
    assert_eq!(
        after.error_code(),
        Some(ErrorCode::StaleSnapshot),
        "`verification.start` over the superseded snapshot is refused after the re-create \
         exactly as it was before it — a stale handle is a typed error, never a silent \
         re-read: {:?}",
        after.envelope.error
    );

    // And so is the continuation pinned to it.
    let resumed = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume_after",
        "idem-resume-after",
    );
    assert_eq!(
        resumed.error_code(),
        Some(ErrorCode::StaleSnapshot),
        "the continuation pinned to the superseded snapshot stays refused: {:?}",
        resumed.envelope.error
    );
    let record = record_of(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_status",
    ));
    assert_eq!(
        record.status,
        TaskStatus::Suspended,
        "no campaign ran over a snapshot the lineage had superseded"
    );
    assert_eq!(
        record.cost.states,
        Optional::Present(3),
        "the parked campaign is where the park left it"
    );

    // Anti-vacuity, so this is a claim about the lineage rather than about the daemon
    // having forgotten how to run: the *current* head is still sealable and still serves a
    // campaign, after the re-create as before it.
    let head = snapshot_of(&forked.payload);
    assert_eq!(
        seal(
            &mut parked.fixture.daemon,
            &head,
            "req_seal_head",
            "idem-seal-head",
        )
        .error_code(),
        None,
        "the head the fork advanced to is still the head"
    );
    let served = start(
        &mut parked.fixture.daemon,
        &head,
        CLOSING_BUDGET,
        "req_start_head",
        "idem-start-head",
    );
    assert_eq!(
        served.error_code(),
        None,
        "and a campaign over it is admitted: {:?}",
        served.envelope.error
    );
}

/// **Attack 9 — was FALSIFICATION, now a regression guard (the same defect, seen from the
/// other side).** The snapshot that *was* the lineage head when the re-create landed used to
/// become an identity the rewound `Fork` could not place at all: the mutation a caller had
/// just made was unsealable and unforkable — answered `CapabilityDenied`, byte-identically
/// with a request for an artifact that does not exist (RFC 0027 X2) — while the snapshot it
/// replaced was served. The workspace's real head was stranded and the daemon offered no
/// recovery, because nothing in the refusal said a lineage had been rewound.
///
/// With the convergent create, the re-create is not an event in the lineage's life: the head
/// seals and forks afterwards exactly as it does in the control daemon that never saw one.
/// The control is the comparison that makes that a claim about the head rather than about
/// this fixture.
#[test]
fn regression_a_converged_create_does_not_strand_the_snapshot_that_is_current() {
    let mut fixture = bootstrap();
    let old = fixture.snapshot.clone();
    let derived = snapshot_of(
        &fork(
            &mut fixture.daemon,
            &old,
            b"# the real head\n",
            "req_fork",
            "idem-fork",
        )
        .payload,
    );

    // The control: the same daemon, the same fork, and no re-create at all.
    let mut control = bootstrap();
    let control_derived = snapshot_of(
        &fork(
            &mut control.daemon,
            &control.snapshot,
            b"# the real head\n",
            "req_fork",
            "idem-fork",
        )
        .payload,
    );
    let control_seal = seal(
        &mut control.daemon,
        &control_derived,
        "req_seal",
        "idem-seal",
    );
    assert_eq!(
        control_seal.envelope.status,
        ResultStatus::Ok,
        "the head seals"
    );

    let recreated = create(
        &mut fixture.daemon,
        &fixture.components,
        true,
        "req_recreate",
        "idem-recreate",
    );
    assert_eq!(
        recreated.envelope.status,
        ResultStatus::Ok,
        "the re-create is admitted — it converges rather than being refused: {:?}",
        recreated.envelope.error
    );
    assert_eq!(
        snapshot_of(&recreated.payload),
        old,
        "and names the snapshot that already exists"
    );

    let still_the_head = seal(&mut fixture.daemon, &derived, "req_seal", "idem-seal");
    assert_eq!(
        still_the_head.error_code(),
        None,
        "the snapshot that is the lineage head is still placeable and still seals: {:?}",
        still_the_head.envelope.error
    );
    assert_eq!(
        wire_bytes(&still_the_head),
        wire_bytes(&control_seal),
        "byte-identically with the daemon that never saw a re-create"
    );
    let forkable = fork(
        &mut fixture.daemon,
        &derived,
        b"# onward\n",
        "req_fork_2",
        "idem-fork-2",
    );
    assert_eq!(
        forkable.error_code(),
        None,
        "and can still be advanced: {:?}",
        forkable.envelope.error
    );
}

/// **Attack 10 — was FALSIFICATION, now a regression guard (the same defect, failing
/// closed).** `workspace.create` replaced the *workspace record* as well as the lineage, so
/// re-creating identical components with `seal: false` flipped an already-sealed snapshot
/// back to unsealed. A continuation that resumed one request earlier was then refused
/// `StaleSnapshot` — "the snapshot the continuation pinned is not the current sealed
/// snapshot" — for a snapshot nobody changed, by an operation that created nothing new.
///
/// That direction erred closed rather than open, which is why it was the least severe of the
/// three faces of DEFECT 1. It was pinned anyway because it is the same line of code, and
/// because "a durable continuation can be revoked by an unrelated caller's idempotent-looking
/// create" is not a property `rule task.resume` grants anyone.
///
/// The fix makes the seal **monotone**: `seal: false` means "do not seal it", never "un-seal
/// it", so the held record keeps the seal it has and the create reports the state the daemon
/// is actually in. The continuation survives, which is what this test now asserts — together
/// with the response field that says so, because the divergence between the request's `seal`
/// and the record's is the one thing an identical create can legitimately disagree about and
/// the caller is told the answer rather than left to infer it.
#[test]
fn regression_an_unsealed_re_create_does_not_revoke_a_valid_continuation() {
    let mut parked = drive_to_park();

    let allowed = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(8),
        None,
        "req_resume_before",
        "idem-resume-before",
    );
    assert_eq!(
        allowed.error_code(),
        None,
        "the continuation is live one request before the attack: {:?}",
        allowed.envelope.error
    );
    let live = continuation_of(&record_of(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_s1",
    )));

    let recreated = create(
        &mut parked.fixture.daemon,
        &parked.fixture.components,
        false,
        "req_recreate",
        "idem-recreate",
    );
    assert_eq!(
        recreated.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        recreated.envelope.error
    );
    assert!(
        sealed_of(&recreated.payload),
        "the response reports the seal the snapshot has, not the one the request asked for: \
         a create never un-seals"
    );

    let survived = resume(
        &mut parked.fixture.daemon,
        &live,
        Some(CLOSING_BUDGET),
        None,
        "req_resume_after",
        "idem-resume-after",
    );
    assert_eq!(
        survived.error_code(),
        None,
        "a create with `seal: false` does not un-seal the record the continuation pinned, \
         and does not revoke it: {:?}",
        survived.envelope.error
    );
    let record = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s2"));
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.cost.states, Optional::Present(FROZEN_STATES));
}

/// **Attack 11, anti-vacuity for attacks 8–10.** The *same* create replayed under the key it
/// already used is short-circuited by the idempotency ledger at step 7 of the dispatch, so
/// the family handler never runs, the lineage is untouched, and the superseded handle stays
/// refused. That is what made attacks 8–10 findings about `workspace.create` rather than
/// about the fixture: before the bn-n1xou fix, swapping only the idempotency key — nothing
/// else about the request — flipped the outcome from "refused, as required" to "admitted".
/// Post-fix an identical create converges either way; this test still isolates the ledger's
/// short-circuit as a distinct guard from `open_lineage`'s put-if-absent.
#[test]
fn attack_staleness_the_same_create_under_the_same_key_changes_nothing() {
    let mut fixture = bootstrap();
    let old = fixture.snapshot.clone();
    fork(
        &mut fixture.daemon,
        &old,
        b"# moved on\n",
        "req_fork",
        "idem-fork",
    );

    // `idem-create` is the key `bootstrap` already used for this exact request.
    let replayed = create(
        &mut fixture.daemon,
        &fixture.components,
        true,
        "req_create",
        "idem-create",
    );
    assert_eq!(replayed.envelope.status, ResultStatus::Ok);

    assert_eq!(
        start(
            &mut fixture.daemon,
            &old,
            CLOSING_BUDGET,
            "req_start",
            "idem-start",
        )
        .error_code(),
        Some(ErrorCode::StaleSnapshot),
        "the ledger is the only thing between this daemon and the rewind"
    );
}

// =========================================================================================
// axis 3 — deterministic resume
// =========================================================================================

/// One whole DX-03 script — create, seal-by-creation, start bounded, read, resume, read the
/// verdict — returning the canonical wire bytes of every answer, in order.
fn run_script() -> Vec<Vec<u8>> {
    let mut parked = drive_to_park();
    // Evaluated in order, left to right: each element is a dispatch against the daemon the
    // previous one advanced, so the sequence is the script and the `vec![]` is only how its
    // answers are collected.
    vec![
        wire_bytes(&status(&mut parked.fixture.daemon, &parked.task, "req_s1")),
        wire_bytes(&update_budget(
            &mut parked.fixture.daemon,
            &parked.task,
            Some(CLOSING_BUDGET),
            "req_budget",
            "idem-budget",
        )),
        wire_bytes(&resume(
            &mut parked.fixture.daemon,
            &parked.continuation,
            None,
            None,
            "req_resume",
            "idem-resume",
        )),
        wire_bytes(&verification_result(
            &mut parked.fixture.daemon,
            &parked.task,
            "req_result",
        )),
    ]
}

/// **Attack 12.** Two independently built daemons driven through the identical DX-03 script
/// produce byte-identical answers, frame for frame. Nothing is shared between the runs but
/// the script, so no daemon-instance state — a map layout, an allocation address, an
/// iteration order — can be reaching the wire.
#[test]
fn attack_determinism_two_fresh_daemons_produce_byte_identical_transcripts() {
    let first = run_script();
    let second = run_script();
    assert_eq!(first.len(), 4, "the transcript is not vacuous");
    assert_eq!(first.len(), second.len());
    for (index, (one, two)) in first.iter().zip(&second).enumerate() {
        assert_eq!(one, two, "answer {index} of two independent runs differed");
    }
}

/// The hostile operations spliced into the resume script by attack 13, and run in a block by
/// attack 1. Each is chosen to touch a *different* seam of the dispatch — the admission
/// predicate, the annotation obligations, the idempotency ledger, an unshipped lane, and two
/// pure reads — and none of them is allowed to be the thing that decides the resume's answer.
type Hostile = fn(&mut Fixture, &TaskHandle);

const HOSTILE: &[Hostile] = &[
    // A pure read by a third actor.
    |fixture, task| {
        let read = status(&mut fixture.daemon, task, "req_hostile_status");
        assert_eq!(read.envelope.status, ResultStatus::Ok);
    },
    // An admission denial: `cap_reader` is `read` authority and cannot start a campaign.
    |fixture, _task| {
        let denied = fixture.daemon.dispatch(&OperationRequest {
            envelope: on(
                budgeted(
                    keyed(
                        envelope(
                            "verification.start",
                            "agent:reader",
                            "cap_reader",
                            "req_hostile_denied",
                        ),
                        "idem-hostile-denied",
                    ),
                    CLOSING_BUDGET,
                ),
                &fixture.snapshot.clone(),
            ),
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: all_claims(),
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        });
        assert_eq!(denied.error_code(), Some(ErrorCode::CapabilityDenied));
    },
    // A shape disagreement: the envelope says one operation, the body is another's.
    |fixture, task| {
        let malformed = fixture.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_hostile_shape",
                ),
                "idem-hostile-shape",
            ),
            arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
        });
        assert_eq!(malformed.error_code(), Some(ErrorCode::MalformedRequest));
    },
    // A key reused for a different request. `bootstrap` recorded `idem-create` against a
    // `workspace.create` with `seal: true`; the same key with `seal: false` is a different
    // canonical request and is refused at step 7 of the dispatch, so the family handler
    // never runs and this hostile operation leaves nothing behind at all.
    |fixture, _task| {
        let components = fixture.components.clone();
        let clash = create(
            &mut fixture.daemon,
            &components,
            false,
            "req_hostile_key",
            "idem-create",
        );
        assert_eq!(clash.error_code(), Some(ErrorCode::IdempotencyKeyReused));
        assert_eq!(clash.payload, Payload::None, "no partial effect");
    },
    // An unshipped lane.
    |fixture, _task| {
        let snapshot = fixture.snapshot.clone();
        let refused = diff(
            &mut fixture.daemon,
            &snapshot,
            &snapshot,
            "req_hostile_diff",
        );
        assert_eq!(
            refused.error_code(),
            Some(ErrorCode::UnsupportedSemanticFeature)
        );
    },
    // A subscription read — the hints-only lane.
    |fixture, task| {
        let subscribed = subscribe(&mut fixture.daemon, task, "req_hostile_subscribe");
        assert_eq!(subscribed.envelope.status, ResultStatus::Ok);
    },
];

/// The resume script's own steps, as one closure per step, so attack 13 can splice a hostile
/// operation into every gap between them.
fn resume_script(
    parked: &mut Parked,
    hostile: Option<Hostile>,
    at: usize,
) -> (Vec<u8>, Vec<u8>, usize) {
    let mut step = 0usize;
    let mut fired = 0usize;
    let fire = |parked: &mut Parked, step: &mut usize, fired: &mut usize| {
        if let Some(hostile) = hostile {
            if *step == at {
                hostile(&mut parked.fixture, &parked.task);
                *fired += 1;
            }
        }
        *step += 1;
    };

    fire(parked, &mut step, &mut fired);
    status(&mut parked.fixture.daemon, &parked.task, "req_s1");
    fire(parked, &mut step, &mut fired);
    update_budget(
        &mut parked.fixture.daemon,
        &parked.task,
        Some(CLOSING_BUDGET),
        "req_budget",
        "idem-budget",
    );
    fire(parked, &mut step, &mut fired);
    let resumed = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        None,
        None,
        "req_resume",
        "idem-resume",
    );
    fire(parked, &mut step, &mut fired);
    let result = verification_result(&mut parked.fixture.daemon, &parked.task, "req_result");
    fire(parked, &mut step, &mut fired);
    assert_eq!(step, 5, "five placements, counted rather than assumed");
    (wire_bytes(&resumed), wire_bytes(&result), fired)
}

/// **Attack 13.** The resume answer and the verdict that follows it are byte-identical at
/// every placement of every hostile operation: 6 operations × 5 placements = 30 runs, plus
/// the un-interleaved control. A resume that could be steered by an unrelated request landing
/// beside it would not be a deterministic resume, and the placements cover both sides of the
/// budget raise and both sides of the resume itself.
///
/// Bounded by construction: 31 whole-campaign runs over a 16-state model, no threads, no
/// allocation from a loop bound.
#[test]
fn attack_determinism_a_resume_is_byte_identical_at_every_placement_of_a_hostile_suite() {
    let mut control = drive_to_park();
    let (resumed, result, never) = resume_script(&mut control, None, usize::MAX);
    assert!(!resumed.is_empty() && !result.is_empty());
    assert_eq!(never, 0, "the control interleaves nothing");

    let mut runs = 0usize;
    for (which, hostile) in HOSTILE.iter().enumerate() {
        for at in 0..5 {
            let mut parked = drive_to_park();
            let (one, two, fired) = resume_script(&mut parked, Some(*hostile), at);
            assert_eq!(
                fired, 1,
                "anti-vacuity: hostile operation {which} really ran at placement {at}"
            );
            assert_eq!(
                one, resumed,
                "hostile operation {which} at placement {at} changed the resume answer"
            );
            assert_eq!(
                two, result,
                "hostile operation {which} at placement {at} changed the verdict"
            );
            runs += 1;
        }
    }
    assert_eq!(
        runs,
        HOSTILE.len() * 5,
        "6 hostile operations × 5 placements"
    );
}

/// **Attack 14.** Every arm of `rule task.update_budget`'s legality table, applied to a
/// parked campaign and then resumed: raised, unchanged, tightened-but-above-spend, lowered
/// *below* committed spend (B18), and withdrawn entirely. In no arm does a resume come back
/// with a smaller campaign — INV-009's "silent truncation of a campaign is prohibited" — and
/// the recorded cost is monotone across the whole sequence.
#[test]
fn attack_determinism_no_budget_arm_lets_a_resume_un_explore_a_campaign() {
    let mut parked = drive_to_park();
    let parked_cost = record_of(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_s_base",
    ))
    .cost
    .states;
    assert_eq!(parked_cost, Optional::Present(3));

    let mut last = 3u64;
    for (index, ceiling) in [Some(8), Some(8), Some(6), Some(1), None]
        .into_iter()
        .enumerate()
    {
        let updated = update_budget(
            &mut parked.fixture.daemon,
            &parked.task,
            ceiling,
            &format!("req_budget_{index}"),
            &format!("idem-budget-{index}"),
        );
        assert_eq!(
            updated.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            updated.envelope.error
        );
        let resumed = resume(
            &mut parked.fixture.daemon,
            &parked.continuation,
            None,
            None,
            &format!("req_resume_{index}"),
            &format!("idem-resume-{index}"),
        );
        assert_eq!(
            resumed.error_code(),
            None,
            "arm {index} refused a resume it should have served: {:?}",
            resumed.envelope.error
        );
        let record = record_of(&status(
            &mut parked.fixture.daemon,
            &parked.task,
            &format!("req_s_{index}"),
        ));
        let states = match record.cost.states {
            Optional::Present(states) => states,
            Optional::Absent => panic!("a campaign that ran reports its one meter"),
        };
        assert!(
            states >= last,
            "arm {index} (ceiling {ceiling:?}) un-explored a campaign: {last} → {states}"
        );
        last = states;
    }
    assert_eq!(
        last, FROZEN_STATES,
        "the withdrawn ceiling runs at `Bounds::CERTIFIABLE` and closes Die Hard"
    );
}

// =========================================================================================
// axis 4 — handle reuse
// =========================================================================================

/// **Attack 15.** The same continuation resumed twice, with no budget change between. The
/// question the bone asks is whether that is two divergent campaigns; it is not. One task,
/// one campaign value, a monotone cost, and a task table that did not grow. What *does*
/// grow is the committed-publication list, by one per resume — which is INV-009's "resuming
/// a task may add evidence … it may not silently replace prior artifacts" and RFC 0026's
/// "re-derived artifacts receive new identities", so the second resume adds a name rather
/// than overwriting one. It is recorded here rather than asserted away: a no-progress resume
/// still publishes.
#[test]
fn attack_reuse_double_resuming_one_continuation_is_one_task_and_one_campaign() {
    let mut parked = drive_to_park();
    let first = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        None,
        None,
        "req_resume_1",
        "idem-resume-1",
    );
    let after_first = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"));
    let second = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        None,
        None,
        "req_resume_2",
        "idem-resume-2",
    );
    let after_second = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s2"));

    assert_eq!(first.error_code(), None);
    assert_eq!(second.error_code(), None);
    assert_eq!(
        parked.fixture.daemon.state().tasks().handles().len(),
        1,
        "no second task"
    );
    assert_eq!(
        after_first.cost.states, after_second.cost.states,
        "no second, divergent campaign: the same bound explores the same states"
    );
    assert_eq!(after_first.status, after_second.status);
    assert_eq!(
        after_first.continuation, after_second.continuation,
        "the same frontier names the same `cont_*`"
    );

    let entry = parked
        .fixture
        .daemon
        .state()
        .tasks()
        .get(&parked.task)
        .expect("held");
    assert_eq!(
        entry.publications(),
        3,
        "one per run — the start and the two resumes — appended, never replaced"
    );
    let commitments: Vec<_> = entry
        .evidence
        .committed()
        .iter()
        .map(|publication| publication.commitment().clone())
        .collect();
    let mut unique = commitments.clone();
    unique.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    unique.dedup();
    assert_eq!(
        unique.len(),
        commitments.len(),
        "each re-derivation received a new identity rather than overwriting a prior one"
    );
}

/// **Attack 16.** A continuation that a *newer* continuation has superseded, resumed anyway.
/// It is neither refused nor divergent: it advances the same task, under the task's current
/// ledger, to the same place a resume of the current continuation would reach. One task, a
/// monotone cost, no second campaign.
///
/// This is where the campaign's second recorded concern is visible, since dispositioned in
/// RFC 0026 ("What a `cont_*` handle means, and what `bounds`/`frontier` are pinned for",
/// bn-10wdo). `Continuation` pins `bounds` and `frontier` at creation — RFC 0026 requires it,
/// as *provenance* — and `daemon::task::resume` reads neither to decide admissibility or to
/// seed the run: it re-derives the bound from `TaskEntry::bounds()` and the engine
/// re-explores from the start. So a `cont_*` handle means "advance this task" rather than
/// "resume from this point", which is why an older one is not a second meaning and cannot be
/// refused as one. It is sound today only because breadth-first exploration makes the parked
/// explored set a provable superset of the resumed one — and this test now runs that
/// exploration under `daemon::verification::run`'s `debug_assert` guard, fed the *old*
/// continuation's frontier through `resume`'s own `verification::advance` call, so a future
/// engine that broke the superset property would fail loudly here rather than silently admit
/// a stale continuation.
#[test]
fn attack_reuse_a_superseded_continuation_advances_the_same_task_not_a_second_one() {
    let mut parked = drive_to_park();
    let first = parked.continuation.clone();

    let stepped = resume(
        &mut parked.fixture.daemon,
        &first,
        Some(8),
        None,
        "req_resume_1",
        "idem-resume-1",
    );
    assert_eq!(stepped.envelope.status, ResultStatus::TaskSuspended);
    let second = continuation_of(&record_of(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_s1",
    )));
    assert_ne!(first, second, "the second park minted a new continuation");

    let via_old = resume(
        &mut parked.fixture.daemon,
        &first,
        Some(CLOSING_BUDGET),
        None,
        "req_resume_old",
        "idem-resume-old",
    );
    assert_eq!(
        via_old.error_code(),
        None,
        "the superseded continuation is admitted: {:?}",
        via_old.envelope.error
    );
    let record = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s2"));
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.cost.states, Optional::Present(FROZEN_STATES));
    assert_eq!(
        parked.fixture.daemon.state().tasks().handles().len(),
        1,
        "one task, however many of its continuations are held"
    );
}

/// **Attack 17.** A completed task's handle, reused on every operation that observes one —
/// `task.status`, `task.subscribe`, `verification.result` — twice each. Every answer is
/// byte-identical to its own repeat, and the task's committed publications do not grow: a
/// read is a read.
#[test]
fn attack_reuse_a_completed_task_answers_idempotently_on_every_observing_operation() {
    let mut fixture = bootstrap();
    let started = start(
        &mut fixture.daemon,
        &fixture.snapshot,
        CLOSING_BUDGET,
        "req_start",
        "idem-start",
    );
    let task = task_of(&started);
    let publications = fixture
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("held")
        .publications();

    for (which, request) in ["req_a", "req_b", "req_c"].into_iter().enumerate() {
        let one = match which {
            0 => status(&mut fixture.daemon, &task, request),
            1 => subscribe(&mut fixture.daemon, &task, request),
            _ => verification_result(&mut fixture.daemon, &task, request),
        };
        let two = match which {
            0 => status(&mut fixture.daemon, &task, request),
            1 => subscribe(&mut fixture.daemon, &task, request),
            _ => verification_result(&mut fixture.daemon, &task, request),
        };
        assert_eq!(
            one.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            one.envelope.error
        );
        assert_eq!(
            wire_bytes(&one),
            wire_bytes(&two),
            "observing operation {which} is not a pure function of the task it reports"
        );
    }
    assert_eq!(
        fixture
            .daemon
            .state()
            .tasks()
            .get(&task)
            .expect("held")
            .publications(),
        publications,
        "reading a task publishes nothing"
    );
}

/// **Attack 18 — was FALSIFICATION, now a regression guard (DEFECT 2, fixed by bn-10093).**
/// A continuation handle reused after the task it names was cancelled, carrying the optional
/// budget `task.resume` accepts.
///
/// *What must hold.* `daemon::task::update_budget` states the rule and enforces it:
///
/// > A terminal task keeps the budget it ran under and answers `Unchanged`: a terminal
/// > task's budget is a historical fact, and rewriting it would make its recorded cost
/// > unreadable.
///
/// and `daemon::task::resume` claims the terminal path is inert:
///
/// > A terminal task is not resumed. […] a no-op does neither, and returning the terminal
/// > status is the honest answer to "resume this".
///
/// *What this attack found.* `resume` applied the request's budget to the task's
/// `BudgetLedger` **before** it tested `is_terminal`, and `update_budget` tested first. So
/// the identical ceiling, on the identical cancelled task, in the identical daemon, was
/// refused through one operation and written through the other — and `TaskRecord.budget`
/// afterwards reported a ceiling the task never ran under, while `TaskRecord.cost` still
/// reported the spend of the campaign that did run. The "no-op" was not one.
///
/// *What the fix did.* bn-10093 moved the terminal check above the ledger write. The
/// assertion below is now the one this test named when it was a pin: **the two operations
/// produce one observable**, compared here on the same record, through the two doors, in one
/// dispatch sequence. bn-3p32's A1 pins the same fix from the DX-14 side, byte-for-byte on
/// the whole record; this one is the DX-03 reading — a *handle reuse* that writes nothing.
#[test]
fn regression_resume_refuses_the_terminal_budget_write_update_budget_refuses() {
    let mut parked = drive_to_park();
    let before = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s0"));
    assert_eq!(before.budget.states, Optional::Present(PARK_BUDGET));

    let cancelled = cancel(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_cancel",
        "idem-cancel",
    );
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);

    // The guard, through the operation that always had it.
    let refused = update_budget(
        &mut parked.fixture.daemon,
        &parked.task,
        Some(4096),
        "req_budget",
        "idem-budget",
    );
    assert_eq!(refused.envelope.status, ResultStatus::Ok);
    let after_update = wire_bytes(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"));
    assert_eq!(
        record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"))
            .budget
            .states,
        Optional::Present(PARK_BUDGET),
        "`task.update_budget` keeps a terminal task's budget, as it documents"
    );

    // The same write, through the operation that used not to have it.
    let resumed = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        Some(4096),
        None,
        "req_resume",
        "idem-resume",
    );
    assert_eq!(
        resumed.error_code(),
        None,
        "the resume reports the terminal status rather than refusing: {:?}",
        resumed.envelope.error
    );
    let after_resume = record_of(&status(&mut parked.fixture.daemon, &parked.task, "req_s1"));
    assert_eq!(
        after_resume.status,
        TaskStatus::Cancelled,
        "the status is monotone, as `rule task.status_monotonic` requires"
    );
    assert_eq!(
        after_resume.budget.states,
        Optional::Present(PARK_BUDGET),
        "`task.resume` refuses the terminal task's budget write that `task.update_budget` \
         had just refused: the documented no-op is one"
    );
    assert_eq!(
        wire_bytes(&status(&mut parked.fixture.daemon, &parked.task, "req_s1")),
        after_update,
        "the two operations produce one observable for one ceiling on one terminal task — \
         the record is byte-identical across the resume"
    );
    assert_eq!(
        after_resume.cost.states, before.cost.states,
        "and the recorded spend is still priced against the ceiling the campaign ran under"
    );
}

/// **Attack 19.** The same reuse *without* a budget: a genuine no-op. The record is
/// byte-identical before and after, twice over, and the task's committed publications do not
/// grow — so the defect attack 18 pins is the budget write alone, not the terminal resume
/// path as a whole.
#[test]
fn attack_reuse_a_cancelled_continuation_without_a_budget_is_a_true_no_op() {
    let mut parked = drive_to_park();
    cancel(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_cancel",
        "idem-cancel",
    );
    let before = wire_bytes(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_status",
    ));
    let publications = parked
        .fixture
        .daemon
        .state()
        .tasks()
        .get(&parked.task)
        .expect("held")
        .publications();

    let one = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        None,
        None,
        "req_resume",
        "idem-resume-1",
    );
    let two = resume(
        &mut parked.fixture.daemon,
        &parked.continuation,
        None,
        None,
        "req_resume",
        "idem-resume-2",
    );
    assert_eq!(
        wire_bytes(&one),
        wire_bytes(&two),
        "two resumes of a terminal task are one answer"
    );
    assert_eq!(
        wire_bytes(&status(
            &mut parked.fixture.daemon,
            &parked.task,
            "req_status"
        )),
        before,
        "and the record they report is unchanged"
    );
    assert_eq!(
        parked
            .fixture
            .daemon
            .state()
            .tasks()
            .get(&parked.task)
            .expect("held")
            .publications(),
        publications,
        "a terminal resume publishes nothing"
    );
}

/// **Attack 20.** Cancelling twice. The second cancel reports `Unchanged` rather than
/// `Cancelled` — `rule task.status_monotonic`'s "a terminal status never changes" — and
/// leaves the record byte-identical, with no second continuation, no second milestone, and
/// no publication added or removed.
#[test]
fn attack_reuse_cancelling_twice_changes_nothing_the_second_time() {
    let mut parked = drive_to_park();
    let first = cancel(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_cancel",
        "idem-cancel-1",
    );
    let after_first = wire_bytes(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_status",
    ));
    let second = cancel(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_cancel",
        "idem-cancel-2",
    );
    let after_second = wire_bytes(&status(
        &mut parked.fixture.daemon,
        &parked.task,
        "req_status",
    ));

    assert_eq!(first.envelope.status, ResultStatus::Ok);
    assert_eq!(second.envelope.status, ResultStatus::Ok);
    assert_ne!(
        wire_bytes(&first),
        wire_bytes(&second),
        "the two answers differ, and honestly: `Cancelled` then `Unchanged`"
    );
    assert_eq!(
        after_first, after_second,
        "but the task they are about is unchanged by the second"
    );
}

/// **Attack 21.** RFC 0027 X2, at the handles DX-03 is about: a `cont_*` this daemon does
/// not hold is refused byte-identically by a daemon that is running the campaign and by a
/// look-alike that never ran anything. Neither answer distinguishes "no such continuation"
/// from "not yours", so reusing a handle after supersession leaks nothing about what the
/// daemon holds.
#[test]
fn attack_reuse_an_unheld_handle_is_byte_identical_with_a_lookalike_daemons_denial() {
    let mut parked = drive_to_park();
    let mut lookalike = bootstrap();

    // A well-formed `cont_*` this daemon never minted: the real one with its last hex digit
    // rotated, so it is the same shape and a different identity.
    let real = parked.continuation.as_str().to_owned();
    let mut rotated: Vec<char> = real.chars().collect();
    let last = rotated.len() - 1;
    rotated[last] = if rotated[last] == '0' { '1' } else { '0' };
    let phantom = ContinuationHandle::new(&rotated.into_iter().collect::<String>())
        .expect("a rotated digit is still a well-formed handle");
    assert_ne!(phantom, parked.continuation);

    let busy = resume(
        &mut parked.fixture.daemon,
        &phantom,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume",
    );
    let idle = resume(
        &mut lookalike.daemon,
        &phantom,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume",
    );
    assert_eq!(busy.error_code(), Some(ErrorCode::CapabilityDenied));
    assert_eq!(
        wire_bytes(&busy),
        wire_bytes(&idle),
        "a daemon holding a parked campaign and one holding none answer identically"
    );

    // And the real handle, against the daemon that never ran it, is the same denial.
    let transplanted = resume(
        &mut lookalike.daemon,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume-2",
    );
    assert_eq!(
        transplanted.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "the look-alike denies the continuation the honest daemon holds"
    );
}

/// **Attack 22.** A continuation resumed against a daemon whose compatibility epochs moved
/// under it. Both halves of RFC 0026's two-predicate obligation refuse: a *changed* semantic
/// epoch is `ContinuationEpochMismatch`, an *absent* one is `EpochUnsupported`, and neither
/// is confused with staleness. The daemon that never moved serves the identical request, so
/// the refusals are decisions about the epochs rather than about the transplant.
#[test]
fn attack_reuse_a_continuation_is_refused_when_the_daemons_epochs_moved() {
    let mut parked = drive_to_park();

    // A daemon serving a *different* semantic epoch, given the parked state.
    let mut moved_on = EpochSet {
        semantic: Nullable::Value(epoch("semantic-2")),
        ..epochs()
    };
    let mut shifted = shell_with(moved_on.clone());
    *shifted.state_mut() = std::mem::replace(parked.fixture.daemon.state_mut(), DaemonState::new());
    let mismatched = resume(
        &mut shifted,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume",
    );
    assert_eq!(
        mismatched.error_code(),
        Some(ErrorCode::ContinuationEpochMismatch),
        "P1: a pinned compatibility epoch disagrees with the daemon's current one"
    );

    // A daemon pinning *no* semantic epoch at all.
    moved_on.semantic = Nullable::Null;
    let mut unpinned = shell_with(moved_on);
    *unpinned.state_mut() = std::mem::replace(shifted.state_mut(), DaemonState::new());
    let unsupported = resume(
        &mut unpinned,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume-2",
    );
    assert_eq!(
        unsupported.error_code(),
        Some(ErrorCode::EpochUnsupported),
        "the continuation pins an epoch this daemon holds no identity for"
    );

    // Anti-vacuity: the same request, on a daemon whose epochs did not move, is served.
    let mut honest = shell();
    *honest.state_mut() = std::mem::replace(unpinned.state_mut(), DaemonState::new());
    let served = resume(
        &mut honest,
        &parked.continuation,
        Some(CLOSING_BUDGET),
        None,
        "req_resume",
        "idem-resume-3",
    );
    assert_eq!(served.error_code(), None, "{:?}", served.envelope.error);
}
