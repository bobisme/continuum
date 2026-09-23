//! Acceptance re-derivation for release gate **G1-04** (bone `bn-16v3x`).
//!
//! > continuation resume validates epochs and inputs before any reuse
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, G1 bullet 4
//!
//! # Why this file exists beside evidence that already passes
//!
//! [`PHASE_A_EXIT_PACKAGE.md`] §9.4 A17 states its own limit: the package "re-ran the
//! *delivering* suites, not an independent re-derivation". Re-running
//! `daemon_task_operations.rs` and `dx03_falsification.rs` proves those suites still pass.
//! It does not re-derive the criterion, for two reasons this file attacks directly.
//!
//! **First, the delivering suites reach the epoch classes by forging the continuation.**
//! `resume_admissibility_separates_an_epoch_disagreement_from_an_unheld_epoch` clones a real
//! continuation, rewrites `pinned.semantic` / `pinned.corpus` / `pinned.engine`, renames it
//! `cont_forged`, and parks it back. That checks the predicate against a hand-written pin. It
//! does not check what the criterion actually says, which is about a *deployment whose epochs
//! moved under a continuation nobody touched*.
//!
//! **Second, "before any reuse" is checked by sampling two observables.** The delivering
//! predicate is `payload == Payload::None` plus "the task is still `Suspended`". Two fields.
//! A refusal that wrote a budget, opened a region, published an artifact, or advanced a
//! lineage would satisfy both.
//!
//! | Delivering evidence | This file |
//! |---|---|
//! | forges `PinnedEpochs` on a cloned continuation | builds a **second daemon at advanced epochs**, hands it the first daemon's world, and resumes the *unmodified* continuation ([`advanced`]) |
//! | three epoch axes (semantic, corpus, engine) | **twelve** — five compatibility epochs × {disagreement, unheld} and engine identity × {disagreement, unheld}, each its own `#[test]` |
//! | deployment leaves `evidence` and `corpus` `Null`, so neither can refuse ([`PHASE_A_EXIT_PACKAGE.md`] §9.2: "clause 4's guarantee covers five epochs, not six") | deployment pins **all five compatibility epochs plus engine**, so each is probed and the "five, not six" reading is shown to be a property of that deployment, not of the predicate |
//! | asserts `payload == None` and `status == Suspended` | asserts a **total structural fingerprint** of the daemon's whole effect surface is byte-identical ([`Trace`]) |
//! | no control that the no-trace check can fail | drives a refusal that **does** leave a trace — a publication abort after admission — and shows the weak predicate calls it clean while the strict one catches it ([`negative_control_the_weak_no_trace_predicate_misses_what_the_strict_one_catches`]) |
//! | asserts refusals | asserts refusals **and** that the predicate is not over-broad ([`control_a_protocol_minor_difference_is_not_a_resume_refusal`], [`control_an_unrelated_lineage_and_an_unrelated_task_do_not_refuse_a_resume`]) |
//!
//! # The verdict this file reaches
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** The narrowing had three parts, each an executable
//! assertion rather than prose. Parts 1 and 3 still narrow it; part 2 was repaired by
//! bn-3oocz and is now a regression guard:
//!
//! 1. **Within one daemon process lifetime — no longer for a continuation (bn-20142).**
//!    [`PHASE_A_EXIT_PACKAGE.md`] §6.4 declared ten of thirteen `VolatileFact`s lost on
//!    restart, task and continuation tables among them. bn-20142 moved both to `resolved`:
//!    a park commits a continuation record, and a restart restores the task and its
//!    continuation from it with every pin intact.
//!    [`regression_a_continuation_survives_a_restart_with_its_pins`] is the guard, and the
//!    resume decision table is then applied to the restored continuation exactly as to the
//!    live one (`tests/task_lifecycle_schedule_matrix.rs` resumes it to the cold record).
//!    The other declared-lost facts are unchanged, and this file relitigates nothing else
//!    about them.
//! 2. **"Validated before any reuse" is true of all thirteen refusal classes — as of
//!    bn-3oocz.** This file first found it false of one: the "the model the continuation
//!    names is no longer one this daemon can construct" row of the exit package's own §6.1
//!    decision table was raised inside the run, after `task.resume` wrote the request's
//!    budget onto the ledger and after `verification::advance` opened a region and stamped
//!    `TaskEntry::region`/`worker`. `daemon::task::resume` now checks the catalog before its
//!    first write, and [`class_model_availability_is_refused_before_any_reuse`] guards the
//!    repair with the same total fingerprint as the other twelve. This item no longer
//!    narrows the verdict; it is kept so the history of the finding stays readable.
//! 3. **The continuation's pinned `in_*` intent is not revalidated at resume.**
//!    [`scope_the_pinned_intent_is_not_revalidated_at_resume`] shows a resume succeeding
//!    after the governing contract left the registry. RFC 0026's resume decision table does
//!    not require the check, so this is a gap between the criterion's word "inputs" and the
//!    normative table, not a violation of the table.
//!
//! # Findings this file states rather than papers over (INV-007, INV-008)
//!
//! - **F1 — the guarantee covers every one of the five compatibility epochs, plus engine
//!   identity — not three of them.** `evidence` and `corpus` refuse a resume exactly like
//!   `semantic` does, once a deployment pins them, and each of the five refuses in both
//!   directions (disagreement and unheld). The exit package's "clause 4's guarantee covers
//!   five epochs, not six" counts `EpochKind::ALL`'s six *including* `protocol`, and its
//!   unreachable axis is a fact about its own configuration rather than about the predicate.
//!   This is **wider** than declared. See [`class_p1_evidence_epoch_disagreement`] and
//!   [`class_p1_corpus_epoch_disagreement`].
//! - **F2 — an unheld *engine* identity answers `ContinuationEpochMismatch`, not
//!   `EpochUnsupported`.** The five compatibility epochs answer `EpochUnsupported` when the
//!   daemon pins nothing for the kind; engine identity does not, because P2 is equality and
//!   engine is not an epoch. bn-3oocz read RFC 0026's resume decision table against it and
//!   kept it: the table's `EpochUnsupported` row is about "a pinned *epoch*", the P2 row maps
//!   any engine-identity disagreement to `ContinuationEpochMismatch`, and RFC 0026 forbids
//!   grouping `engine` under "epochs". Guarded at [`class_p2_engine_identity_unheld`].
//! - **F3 — a refused resume is not observationally silent, and must not be.** It appends an
//!   admission record, and — when the request carried an idempotency key — a replay record,
//!   so an identical retry returns the recorded refusal without re-evaluating the predicate.
//!   That agrees with the error taxonomy's "identical retry can succeed: no" column, and it
//!   means recovery after a staleness refusal needs a *new* request, which is what
//!   "re-derive against the new epoch" already meant. See
//!   [`stale_then_fresh_a_refusal_is_recoverable_but_the_refused_key_stays_refused`].
//! - **F4 — an admission-layer denial and a handler-layer denial leave different traces.** A
//!   revoked capability is refused before the idempotency ledger is written; an unheld
//!   continuation is refused inside the handler and *is* written to it. Both answer
//!   `CapabilityDenied`. Recorded at [`class_capability_revoked`]; it is not an existence
//!   oracle, because both continuation outcomes — held-and-refused and unheld — take the
//!   handler path.
//! - **F5 — two daemons differing only in negotiated protocol version derive the same
//!   snapshot, task, and continuation identities**, so SD-13's "the protocol epoch is not
//!   part of any artifact identity" is checked at the deployment level here rather than at
//!   the `admissible_epochs` unit level. See
//!   [`control_a_protocol_minor_difference_is_not_a_resume_refusal`].
//! - **O1 — a `ws_*` identity is blind to the epochs its snapshot was declared under**, so
//!   "resume validates the pinned snapshot" does not imply "resume validates the epochs".
//!   Two deployments a semantic epoch apart mint the *same* snapshot handle from the same
//!   files, which makes `admissible_epochs` load-bearing rather than a second reading of the
//!   snapshot guard. Consistent with the dossier — docs/35 spends the epochs at the task
//!   level and plan §4.4's `ws_*` is a content identity of content — and found here by an
//!   assertion written expecting the opposite. See
//!   [`observation_a_snapshot_identity_is_blind_to_the_epochs_it_was_declared_under`].
//!
//! # The thirteen input classes, enumerated
//!
//! From RFC 0026's "Resume decision table" and `daemon::task::resume`'s guard order, one
//! `#[test]` each (the epoch rows are twelve tests over two classes, one per kind and
//! direction, so no fused check can mask a missing kind):
//!
//! 1. continuation identity — [`class_continuation_is_not_held`];
//! 2. envelope/pin snapshot disagreement — [`class_envelope_names_a_different_snapshot`];
//! 3. pinned snapshot not held — [`class_pinned_snapshot_is_not_held`];
//! 4. pinned snapshot not sealed — [`class_pinned_snapshot_is_no_longer_sealed`];
//! 5. lineage currency — [`class_lineage_superseded_the_pinned_snapshot`];
//! 6. P1 compatibility-epoch disagreement — `class_p1_*`, five kinds;
//! 7. unheld epoch kind — `class_unheld_*`, five kinds;
//! 8. P2 engine-identity disagreement — [`class_p2_engine_identity_disagreement`];
//! 9. P2 engine identity unheld — [`class_p2_engine_identity_unheld`];
//! 10. capability revoked — [`class_capability_revoked`];
//! 11. capability authority below the operation —
//!     [`class_capability_authority_is_below_the_operation`];
//! 12. protocol version outside the connection —
//!     [`class_protocol_version_outside_the_connection`];
//! 13. model availability — [`class_model_availability_is_refused_before_any_reuse`], the
//!     one class that was refused but not before any reuse until bn-3oocz.
//!
//! [`class_idempotency_key_reused_for_a_different_request`] is deliberately outside that
//! list. It is request hygiene rather than an input the continuation depends on, and it is
//! driven only so the coverage accounting below can place `IdempotencyKeyReused` in the
//! probed column rather than the absent one.
//!
//! # Absences — what this file does not probe (INV-007)
//!
//! - **No transport and no CLI.** Every request goes through [`Daemon::dispatch`] in
//!   process. [`PHASE_A_EXIT_PACKAGE.md`] §6.4's third scope statement — the two epoch codes
//!   are not wire-exercised from the CLI — is untouched by this file.
//! - **No concurrency.** `Daemon::dispatch` takes `&mut self`; one request at a time.
//! - **No epoch *advance* operation.** No daemon here migrates from one epoch to another;
//!   two deployments exist at two pinnings, which is the observable consequence and not the
//!   migration. `EpochAdvanceNotice` ordering is out of scope.
//! - **`QuotaExhausted`, `BudgetExhausted` and `MalformedRequest`** are inside
//!   `task.resume`'s admissible error union and are driven by nothing here.
//!   `PublicationAborted` is driven only by the negative controls, through an injected store
//!   fault; INV-017's own evidence is `dx13`/`g1_crash_recovery_evidence`.
//!   [`the_probe_table_accounts_for_every_code_task_resume_may_answer_with`] asserts that
//!   accounting mechanically against the registry rather than describing it.
//! - **`Continuation::bounds` and `frontier`** are pinned provenance that no admissibility
//!   check reads (RFC 0026, "What a `cont_*` handle means"). This file does not re-derive the
//!   frontier-inclusion property; `daemon_task_operations.rs` already does.
//! - **The `Trace` fingerprint reads workspaces, lineages, continuations and staged content
//!   by handle**, because the daemon exposes total iteration only over tasks, intents and
//!   evidence. A continuation this file never learned the name of would go unread. Every
//!   handle any probe mints or grafts is registered ([`Known`]), so the gap is a handle no
//!   code path in this file produces.
//! - **Three of the thirteen classes are reached by a graft**, and each says so at its own
//!   test: an unsealed pinned snapshot, an unheld pinned snapshot, and an unconstructible
//!   model. The daemon has no operation that unseals a snapshot, drops a workspace record, or
//!   deregisters a model. The ten remaining classes — including every one of the twelve epoch
//!   probes — are driven entirely by real operations and real deployment configuration.
//!
//! [`PHASE_A_EXIT_PACKAGE.md`]: ../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::lineage::ForkName;
use continuum_workspace::publication::{AbortReason, PublicationPhase, StorageFaults};
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, errors, identity};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{TaskResumeRequest, TaskStatusRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
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

/// The `states` ceiling that parks a Die Hard campaign short of its sixteen states.
const PARKING_CEILING: u64 = 4;

/// The `states` ceiling a resume offers, large enough to close the campaign.
const CLOSING_CEILING: u64 = 64;

// =========================================================================================
// fixtures
// =========================================================================================

/// The protocol version every deployment here negotiates unless a probe says otherwise.
fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// The deployment this file's *parked* daemon serves.
///
/// **Every one of the six compatibility epochs is pinned, and so is engine identity.** That
/// is the one deliberate divergence from the exit package's deployment (§8.2), which leaves
/// `evidence` and `corpus` `Null` and therefore cannot refuse a resume on either. The
/// divergence is the point: it turns §9.2's "clause 4's guarantee covers five epochs, not
/// six" into a question this file can answer instead of inherit.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Value(epoch("evidence-1")),
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Value(epoch("corpus-1")),
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
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

/// A connection negotiated at `wanted`.
///
/// Parameterised in the version for exactly one probe — the SD-13 control, which needs two
/// deployments that differ in nothing but the negotiated protocol version.
fn negotiated_at(wanted: ProtocolVersion) -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: wanted,
            high: wanted,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g1-04-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[wanted], ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect("the requested version is served")
}

/// A daemon serving `epochs` over a connection negotiated at `protocol`.
fn daemon_serving(
    epochs: &EpochSet,
    protocol: ProtocolVersion,
    faults: Option<StoreSwitch>,
) -> Daemon {
    let root = Some(cap("cap_root"));
    let builder = Daemon::builder(Blake3Identity, negotiated_at(protocol), cap("cap_root"));
    let builder = match faults {
        Some(switch) => builder.store_faults(switch),
        None => builder,
    };
    builder
        .epochs(epochs.clone())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
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
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

/// A storage seam that starts inert and refuses every publication phase once armed.
///
/// `Builder::store_faults` is a build-time option, so a fault installed at construction would
/// fire on the parking run too. Arming after the campaign parked makes the *resume's*
/// publication the first one refused. The negative controls use it, and only them: it is the
/// one reachable refusal of a resume whose inputs are all valid, so it runs and leaves a
/// trace by design.
#[derive(Debug, Clone, Default)]
struct StoreSwitch(Arc<AtomicBool>);

impl StoreSwitch {
    fn arm(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl StorageFaults for StoreSwitch {
    fn check(&self, _phase: PublicationPhase) -> Result<(), AbortReason> {
        if self.0.load(Ordering::SeqCst) {
            Err(AbortReason::StorageExhausted)
        } else {
            Ok(())
        }
    }
}

fn envelope_at(
    operation: &str,
    actor: &str,
    capability: &str,
    request: &str,
    protocol: ProtocolVersion,
) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: protocol,
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

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

fn budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
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
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
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

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}

/// The `SnapshotEpochs` a create request must declare against a deployment serving `epochs`.
///
/// `daemon::workspace::check_epochs` compares the declared `semantic` and `proof` against the
/// served ones and refuses a disagreement, so a deployment that serves `semantic-2` can only
/// mint snapshots declaring `semantic-2`. A deployment that serves *nothing* for a kind
/// accepts any declaration, and this function then declares the baseline — which is what
/// keeps the "unheld epoch" probes minting the *same* snapshot identity as the parked
/// deployment.
fn declared_epochs(epochs: &EpochSet) -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: epochs
            .semantic
            .value()
            .cloned()
            .unwrap_or_else(|| epoch("semantic-1")),
        proof: epochs
            .proof
            .value()
            .cloned()
            .unwrap_or_else(|| epoch("proof-1")),
        toolchain: Optional::Absent,
    }
}

// =========================================================================================
// the deployment under test
// =========================================================================================

/// One daemon, its world, and the parked campaign it holds.
struct Deployment {
    daemon: Daemon,
    epochs: EpochSet,
    protocol: ProtocolVersion,
    intent: IntentHandle,
    snapshot: WorkspaceHandle,
    lineage: ForkName,
    task: TaskHandle,
    continuation: ContinuationHandle,
    /// Every handle this file's fingerprint reads. See [`Known`].
    known: Known,
}

/// A deployment serving [`epochs`], with a Die Hard campaign parked at [`PARKING_CEILING`].
fn parked() -> Deployment {
    deployment(&epochs(), version())
}

/// A deployment serving `epochs`, with a Die Hard campaign parked at [`PARKING_CEILING`].
///
/// Everything is driven through [`Daemon::dispatch`] except the three out-of-band
/// registrations the protocol has no operation for — the intent registry, staged content,
/// and the model catalog — which is the same door `daemon_task_operations.rs` uses and the
/// one `DaemonState::state_mut`'s own documentation names.
fn deployment(epochs: &EpochSet, protocol: ProtocolVersion) -> Deployment {
    deployment_over(epochs, protocol, None)
}

/// [`parked`], over a store whose publications fail once the returned switch is armed.
fn parked_over_a_failing_store() -> (Deployment, StoreSwitch) {
    let switch = StoreSwitch::default();
    let deployment = deployment_over(&epochs(), version(), Some(switch.clone()));
    (deployment, switch)
}

/// [`deployment`], with an optional storage seam.
fn deployment_over(
    epochs: &EpochSet,
    protocol: ProtocolVersion,
    faults: Option<StoreSwitch>,
) -> Deployment {
    let mut daemon = daemon_serving(epochs, protocol, faults);
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
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );

    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope_at(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
                protocol,
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

    let created = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope_at(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
                protocol,
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: files.clone(),
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: declared_epochs(epochs),
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![configuration.clone()],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };
    let lineage = daemon
        .state()
        .workspace(&snapshot)
        .expect("the created snapshot is held")
        .lineage
        .clone();

    let started = daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope_at(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        "req_start",
                        protocol,
                    ),
                    "idem-start",
                ),
                PARKING_CEILING,
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskSuspended,
        "a bounded Die Hard campaign parks: {:?}",
        started.envelope.error
    );
    let task = match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    let entry = daemon.state().tasks().get(&task).expect("the task is held");
    assert_eq!(
        entry.status,
        TaskStatus::Suspended,
        "a Die Hard campaign bounded at {PARKING_CEILING} states parks",
    );
    let continuation = entry
        .continuation
        .clone()
        .expect("a suspended task has a continuation");

    let mut known = Known::default();
    known.workspaces.insert(snapshot.clone());
    known.continuations.insert(continuation.clone());
    known.lineages.insert(lineage.clone());
    known.commitments.extend(files.iter().cloned());
    known.commitments.insert(configuration.clone());

    Deployment {
        daemon,
        epochs: epochs.clone(),
        protocol,
        intent,
        snapshot,
        lineage,
        task,
        continuation,
        known,
    }
}

/// The content identity of a snapshot carrying only `DieHard.ctm` as a model source.
fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

impl Deployment {
    /// `task.resume`, with everything a probe varies made explicit.
    fn resume(&mut self, plan: &Resume<'_>) -> OperationOutcome {
        let mut request_envelope = envelope_at(
            "task.resume",
            plan.actor,
            plan.capability,
            plan.request,
            plan.protocol.unwrap_or(self.protocol),
        );
        // `rule errors.common`'s companion obligation: `task.resume` is `@mutation`, so a
        // request without a non-empty `idempotency_key` is `MalformedRequest` and never
        // reaches the predicate. A probe that does not care which key it used gets one
        // derived from its request id, so no two probes collide in the ledger.
        let derived = format!("idem-{}", plan.request);
        request_envelope = keyed(request_envelope, plan.key.unwrap_or(&derived));
        if let Some(states) = plan.envelope_states {
            request_envelope = budgeted(request_envelope, states);
        }
        if let Some(snapshot) = plan.snapshot {
            request_envelope = on(request_envelope, snapshot);
        }
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::TaskResume(TaskResumeRequest {
                continuation: plan.continuation.clone(),
                budget: match plan.request_states {
                    Some(states) => Optional::Present(budget(states)),
                    None => Optional::Absent,
                },
            }),
        })
    }

    fn trace(&self) -> Trace {
        effect_trace(&self.daemon, &self.known)
    }

    fn status(&mut self, request: &str) -> TaskStatus {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: envelope_at(
                "task.status",
                "agent:reader",
                "cap_reader",
                request,
                self.protocol,
            ),
            arguments: Arguments::TaskStatus(TaskStatusRequest {
                task: self.task.clone(),
            }),
        });
        match &outcome.payload {
            Payload::TaskStatus(record) => record.status,
            other => panic!("expected a task.status payload, got {other:?}"),
        }
    }

    /// The status as the *table* holds it, which is a stronger reading than the wire's.
    fn table_status(&self) -> TaskStatus {
        self.daemon
            .state()
            .tasks()
            .get(&self.task)
            .expect("the task is held")
            .status
    }

    /// `workspace.fork` over `base`, which **advances the lineage** — the authentic way to
    /// supersede the snapshot a continuation pinned.
    fn fork(&mut self, base: &WorkspaceHandle, body: &str, request: &str) -> WorkspaceHandle {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope_at(
                    "workspace.fork",
                    "agent:builder",
                    "cap_builder",
                    request,
                    self.protocol,
                ),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
                base: base.clone(),
                overlay: Optional::Present(vec![FileOverlay {
                    path: "README.md".to_owned(),
                    content: body.as_bytes().to_vec(),
                }]),
                patches: Optional::Absent,
            }),
        });
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            outcome.envelope.error
        );
        let forked = match &outcome.payload {
            Payload::WorkspaceFork(response) => response.snapshot.clone(),
            other => panic!("expected a workspace.fork payload, got {other:?}"),
        };
        self.known.workspaces.insert(forked.clone());
        forked
    }

    fn seal(&mut self, snapshot: &WorkspaceHandle, request: &str) {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope_at(
                    "workspace.seal",
                    "agent:builder",
                    "cap_builder",
                    request,
                    self.protocol,
                ),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
                snapshot: snapshot.clone(),
            }),
        });
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            outcome.envelope.error
        );
    }

    /// `verification.start` over an arbitrary sealed snapshot, bounded so it parks.
    fn start_on(&mut self, snapshot: &WorkspaceHandle, request: &str) -> ContinuationHandle {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: on(
                budgeted(
                    keyed(
                        envelope_at(
                            "verification.start",
                            "agent:runner",
                            "cap_runner",
                            request,
                            self.protocol,
                        ),
                        &format!("idem-{request}"),
                    ),
                    PARKING_CEILING,
                ),
                snapshot,
            ),
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: target(TargetKind::AllClaims, "DieHard"),
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        });
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::TaskSuspended,
            "a bounded Die Hard campaign parks: {:?}",
            outcome.envelope.error
        );
        let task = match &outcome.payload {
            Payload::VerificationStart(response) => response
                .task
                .value()
                .cloned()
                .expect("a fresh start names a task"),
            other => panic!("expected a verification.start payload, got {other:?}"),
        };
        let continuation = self
            .daemon
            .state()
            .tasks()
            .get(&task)
            .expect("held")
            .continuation
            .clone()
            .expect("the bounded run parked");
        self.known.continuations.insert(continuation.clone());
        continuation
    }
}

/// The default probe: resume `continuation` under a raised ceiling.
///
/// The raised ceiling is deliberate. A resume that wrote its budget before validating would
/// move [`Trace`]'s task row, so every zero-trace assertion in this file is made against a
/// request that *has* something to leak.
fn probe<'probe>(continuation: &'probe ContinuationHandle, request: &'probe str) -> Resume<'probe> {
    Resume {
        continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request,
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    }
}

/// Everything a `task.resume` probe varies, named so a probe reads as a sentence.
struct Resume<'plan> {
    continuation: &'plan ContinuationHandle,
    snapshot: Option<&'plan WorkspaceHandle>,
    actor: &'plan str,
    capability: &'plan str,
    request: &'plan str,
    key: Option<&'plan str>,
    protocol: Option<ProtocolVersion>,
    envelope_states: Option<u64>,
    request_states: Option<u64>,
}

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome
        .error_code()
        .unwrap_or_else(|| panic!("expected a refusal, got {:?}", outcome.envelope.status))
}

// =========================================================================================
// the instrument: a total fingerprint of the daemon's effect surface
// =========================================================================================

/// The handles a [`Trace`] reads by name.
///
/// The daemon exposes total iteration over tasks, intents and evidence, and *keyed* access to
/// workspaces, lineages, continuations and staged content. So the fingerprint enumerates the
/// first group and looks the second group up by handle. Every handle this file ever learns
/// about goes in here, and each lookup records `Option<&T>` rather than `&T`, so a handle
/// that *vanished* is as visible as one that changed. The residual limit, stated because it
/// is one: a continuation this file never learned the name of would not be read.
#[derive(Debug, Default, Clone)]
struct Known {
    workspaces: BTreeSet<WorkspaceHandle>,
    continuations: BTreeSet<ContinuationHandle>,
    lineages: BTreeSet<ForkName>,
    commitments: BTreeSet<Commitment>,
}

/// Everything a resume could *do*, as one comparable value.
///
/// # What is inside, and why each row is here
///
/// | Row | What a violation would look like |
/// |---|---|
/// | `store` | a campaign record published by a refused resume |
/// | `tasks` | a budget written, a milestone appended, a status advanced, a region or worker stamped, an evidence commitment staged — every field of every `TaskEntry`, through its own `Debug` |
/// | `continuations` | a continuation minted, mutated, or dropped by a refusal |
/// | `workspaces`, `lineages` | a snapshot sealed or a lineage advanced under a resume |
/// | `regions`, `finalizations`, `defects` | a scope opened for work that was refused |
/// | `evidence_*` | a claim, edge or event written |
/// | `intents` | a contract touched |
/// | `staged` | staged content rewritten |
///
/// # What is deliberately outside it
///
/// **The audit surface.** `DaemonState::admissions`, the store's `AuditLog`, and the
/// idempotency ledger all move on a refusal, and all three *must*: RFC 0027 P5 requires an
/// admission record "whatever the answer was", and the store audits the reads this instrument
/// itself performs. A fingerprint that included them could never be equal across a refusal,
/// and the criterion is about reuse, not about silence.
/// [`a_refused_resume_is_audited_even_though_it_leaves_no_effect`] asserts the opposite
/// direction for the admission log, so "outside the fingerprint" does not become "unchecked".
#[derive(Debug, Clone, PartialEq, Eq)]
struct Trace {
    store: Vec<(String, Vec<u8>)>,
    tasks: Vec<String>,
    continuations: Vec<String>,
    workspaces: Vec<String>,
    lineages: Vec<String>,
    regions_opened: u32,
    finalizations: Vec<String>,
    region_defects: Vec<String>,
    evidence_nodes: Vec<String>,
    evidence_edges: Vec<String>,
    evidence_events: Vec<String>,
    intents: Vec<String>,
    staged: Vec<String>,
}

/// Read the whole effect surface of `daemon`.
fn effect_trace(daemon: &Daemon, known: &Known) -> Trace {
    let token =
        identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a store token");
    let mut store: Vec<(String, Vec<u8>)> = daemon
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit")
        .identities()
        .into_iter()
        .map(|handle| {
            let bytes = daemon
                .store()
                .read(&handle, &token)
                .expect("`cap_root` confers read on a published artifact");
            (handle.to_string(), bytes)
        })
        .collect();
    store.sort();

    let state = daemon.state();
    let tasks = state
        .tasks()
        .handles()
        .into_iter()
        .map(|handle| format!("{handle:?} => {:?}", state.tasks().get(handle)))
        .collect();
    let continuations = known
        .continuations
        .iter()
        .map(|handle| format!("{handle:?} => {:?}", state.tasks().continuation(handle)))
        .collect();
    let workspaces = known
        .workspaces
        .iter()
        .map(|handle| format!("{handle:?} => {:?}", state.workspace(handle)))
        .collect();

    let mut lineage_names = known.lineages.clone();
    for handle in &known.workspaces {
        if let Some(record) = state.workspace(handle) {
            lineage_names.insert(record.lineage.clone());
        }
    }
    let lineages = lineage_names
        .iter()
        .map(|name| format!("{name:?} => {:?}", state.lineage(name)))
        .collect();

    Trace {
        store,
        tasks,
        continuations,
        workspaces,
        lineages,
        regions_opened: state.regions().opened(),
        finalizations: state
            .regions()
            .finalizations()
            .iter()
            .map(|entry| format!("{entry:?}"))
            .collect(),
        region_defects: state
            .regions()
            .defects()
            .iter()
            .map(|entry| format!("{entry:?}"))
            .collect(),
        evidence_nodes: state
            .evidence_nodes()
            .map(|(handle, node)| format!("{handle:?} => {node:?}"))
            .collect(),
        evidence_edges: state
            .evidence_edges()
            .map(|(handle, edge)| format!("{handle:?} => {edge:?}"))
            .collect(),
        evidence_events: state
            .evidence_events()
            .iter()
            .map(|event| format!("{event:?}"))
            .collect(),
        intents: state
            .intents()
            .map(|(handle, record)| format!("{handle:?} => {record:?}"))
            .collect(),
        staged: known
            .commitments
            .iter()
            .map(|commitment| format!("{commitment:?} => {:?}", state.staged(commitment)))
            .collect(),
    }
}

/// The strict predicate: the effect surface did not move at all.
fn strict_no_trace(before: &Trace, after: &Trace) -> bool {
    before == after
}

/// The delivering suites' predicate, written here so the two can be compared.
///
/// `daemon_task_operations.rs` checks exactly this — `assert_eq!(refused.payload,
/// Payload::None, "no partial effect")` and "a refused resume never advances the task" — and
/// this restatement is *stronger* than the original on the second half, because it reads the
/// task table rather than the wire record.
fn weak_no_trace(outcome: &OperationOutcome, before: TaskStatus, after: TaskStatus) -> bool {
    outcome.payload == Payload::None && before == after
}

/// Assert a refusal is typed `expected` and moved nothing.
///
/// Every class probe ends here, so "before any reuse" is asserted the same way for every
/// class and cannot drift between them.
fn refused_without_trace(
    deployment: &mut Deployment,
    plan: &Resume<'_>,
    expected: ErrorCode,
    what: &str,
) -> OperationOutcome {
    let before = deployment.trace();
    let status_before = deployment.table_status();
    let outcome = deployment.resume(plan);
    let after = deployment.trace();
    let status_after = deployment.table_status();

    assert_eq!(code(&outcome), expected, "the typed refusal for {what}");
    assert_eq!(
        outcome.payload,
        Payload::None,
        "a refusal carries no payload for {what}"
    );
    assert!(
        strict_no_trace(&before, &after),
        "{what}: the refused resume moved the effect surface\n  before: {before:#?}\n  after: \
         {after:#?}",
    );
    assert!(
        weak_no_trace(&outcome, status_before, status_after),
        "{what}: the weak predicate must also hold wherever the strict one does",
    );
    outcome
}

// =========================================================================================
// the advanced deployment: an epoch that moved under a continuation nobody touched
// =========================================================================================

/// One epoch axis of the resume predicate.
struct Axis {
    /// How the advanced deployment's served epochs differ from the parked one's.
    advance: fn(&mut EpochSet),
    /// What `task.resume` must answer.
    expected: ErrorCode,
    /// Whether the advanced deployment can still mint the parked deployment's snapshot
    /// identity.
    ///
    /// **True on every axis**, which is [`observation_a_snapshot_identity_is_blind_to_the_epochs_it_was_declared_under`]'s
    /// subject: `check_epochs` forces a create request against a `semantic-2` deployment to
    /// declare `semantic-2`, and the resulting `ws_*` handle is nonetheless the one a
    /// `semantic-1` deployment mints, because `WorkspaceDescriptor`'s record is over the
    /// file tree and the dependency/toolchain/configuration components and nothing else. The
    /// field is kept — rather than deleted as a constant — because it is the assertion that
    /// would catch that changing.
    same_snapshot: bool,
    /// The probe's name, for assertion messages.
    what: &'static str,
}

/// Park a campaign on one deployment, then resume the **unmodified** continuation on a second
/// deployment whose epochs moved.
///
/// # Why this is the probe and not a forged pin
///
/// The criterion is about a continuation surviving into a world that changed. The delivering
/// suite reaches the same error codes by editing `PinnedEpochs` on a clone of a real
/// continuation, which tests the comparison but not the situation. Here the continuation is
/// bit-for-bit what `verification.start` minted, and the *daemon* is the thing that differs.
///
/// # What is transplanted, and what that costs
///
/// The advanced deployment builds its own world from the same fixtures, so its intent record,
/// staged content and model catalog are its own. Two things are carried across:
///
/// - the parked deployment's `WorkspaceRecord` and `Fork`, because the resume predicate
///   resolves the pinned snapshot *before* it reaches the epochs and would otherwise answer
///   `CapabilityDenied` for the two axes where the advanced daemon cannot mint the same
///   snapshot identity. Where it *can* (`same_snapshot`), this function asserts the two
///   deployments independently derived an identical record and lineage before carrying
///   anything, so the transplant is a checked no-op rather than an assumption;
/// - the continuation itself, parked into the advanced daemon's table.
///
/// The advanced deployment keeps its own live suspended task, its own continuation and its
/// own published campaign record, and all of them are inside the [`Trace`] the zero-trace
/// assertion covers. So the refusal is checked against a world with something to damage.
fn advanced(axis: &Axis) {
    let source = parked();
    let mut advanced_epochs = epochs();
    (axis.advance)(&mut advanced_epochs);
    assert_ne!(
        advanced_epochs, source.epochs,
        "{}: the axis must actually move an epoch",
        axis.what
    );

    let mut target_deployment = deployment(&advanced_epochs, version());
    assert_eq!(
        target_deployment.snapshot == source.snapshot,
        axis.same_snapshot,
        "{}: the axis table's `same_snapshot` must match what the deployments minted",
        axis.what
    );

    let record = source
        .daemon
        .state()
        .workspace(&source.snapshot)
        .expect("the parked deployment holds its own snapshot")
        .clone();
    let fork = source
        .daemon
        .state()
        .lineage(&source.lineage)
        .expect("the parked deployment holds its own lineage")
        .clone();
    if axis.same_snapshot {
        assert_eq!(
            target_deployment.daemon.state().workspace(&source.snapshot),
            Some(&record),
            "{}: two deployments that mint one snapshot identity must hold one record",
            axis.what
        );
        assert_eq!(
            target_deployment.daemon.state().lineage(&source.lineage),
            Some(&fork),
            "{}: and one lineage",
            axis.what
        );
    }
    target_deployment
        .daemon
        .state_mut()
        .put_workspace(source.snapshot.clone(), record);
    target_deployment.daemon.state_mut().put_lineage(fork);

    let continuation = source
        .daemon
        .state()
        .tasks()
        .continuation(&source.continuation)
        .expect("the parked deployment holds its own continuation")
        .clone();
    assert_eq!(
        continuation.pinned,
        continuumd::daemon::task::PinnedEpochs::of(&source.epochs),
        "{}: the continuation pins exactly what the parked deployment served",
        axis.what
    );
    target_deployment
        .daemon
        .state_mut()
        .tasks_mut()
        .park(continuation);
    target_deployment
        .known
        .workspaces
        .insert(source.snapshot.clone());
    target_deployment
        .known
        .continuations
        .insert(source.continuation.clone());
    target_deployment
        .known
        .lineages
        .insert(source.lineage.clone());

    let plan = Resume {
        continuation: &source.continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    };
    refused_without_trace(&mut target_deployment, &plan, axis.expected, axis.what);
}

// --- P1: the five compatibility epochs, one probe each -----------------------------------

/// `semantic` moved: ADR-0018's forbidden case, and the one the delivering suite covers.
#[test]
fn class_p1_semantic_epoch_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.semantic = Nullable::Value(epoch("semantic-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "a semantic epoch disagreement",
    });
}

/// `intent` moved: the Intent Contract vocabulary advanced under the continuation.
#[test]
fn class_p1_intent_epoch_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.intent = Nullable::Value(epoch("intent-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "an intent epoch disagreement",
    });
}

/// `evidence` moved — **F1**. The exit package records this axis as unprobeable because its
/// deployment left `evidence` `Null`; it is probeable, and it refuses.
#[test]
fn class_p1_evidence_epoch_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.evidence = Nullable::Value(epoch("evidence-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "an evidence epoch disagreement",
    });
}

/// `proof` moved: the Lean toolchain and theorem-package closure advanced.
#[test]
fn class_p1_proof_epoch_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.proof = Nullable::Value(epoch("proof-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "a proof epoch disagreement",
    });
}

/// `corpus` moved — the second half of **F1**.
#[test]
fn class_p1_corpus_epoch_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.corpus = Nullable::Value(epoch("corpus-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "a corpus epoch disagreement",
    });
}

// --- the unheld half: the daemon pins nothing for a kind the continuation pinned ----------

/// `semantic` unheld: a deployment that pins no semantic epoch cannot honour one.
#[test]
fn class_unheld_semantic_epoch() {
    advanced(&Axis {
        advance: |epochs| epochs.semantic = Nullable::Null,
        expected: ErrorCode::EpochUnsupported,
        same_snapshot: true,
        what: "an unheld semantic epoch",
    });
}

/// `intent` unheld.
#[test]
fn class_unheld_intent_epoch() {
    advanced(&Axis {
        advance: |epochs| epochs.intent = Nullable::Null,
        expected: ErrorCode::EpochUnsupported,
        same_snapshot: true,
        what: "an unheld intent epoch",
    });
}

/// `evidence` unheld.
#[test]
fn class_unheld_evidence_epoch() {
    advanced(&Axis {
        advance: |epochs| epochs.evidence = Nullable::Null,
        expected: ErrorCode::EpochUnsupported,
        same_snapshot: true,
        what: "an unheld evidence epoch",
    });
}

/// `proof` unheld.
#[test]
fn class_unheld_proof_epoch() {
    advanced(&Axis {
        advance: |epochs| epochs.proof = Nullable::Null,
        expected: ErrorCode::EpochUnsupported,
        same_snapshot: true,
        what: "an unheld proof epoch",
    });
}

/// `corpus` unheld — the axis the delivering suite reaches by forging `pinned.corpus`.
#[test]
fn class_unheld_corpus_epoch() {
    advanced(&Axis {
        advance: |epochs| epochs.corpus = Nullable::Null,
        expected: ErrorCode::EpochUnsupported,
        same_snapshot: true,
        what: "an unheld corpus epoch",
    });
}

// --- P2: engine identity ------------------------------------------------------------------

/// A different engine build. P1 holds throughout — every compatibility epoch agrees — so this
/// probe fails only if the second predicate is really being evaluated.
#[test]
fn class_p2_engine_identity_disagreement() {
    advanced(&Axis {
        advance: |epochs| epochs.engine = Nullable::Value(epoch("engine-reference-2")),
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "an engine identity disagreement",
    });
}

/// **F2.** A deployment pinning *no* engine identity answers `ContinuationEpochMismatch`,
/// where the same shape on any compatibility epoch answers `EpochUnsupported`. **A
/// regression guard: RFC 0026 requires this code** (bn-3oocz read the table and kept it).
///
/// RFC 0026, "Resume decision table": "a pinned epoch names an identity the daemon no longer
/// holds at all → `EpochUnsupported`" and "the pinned engine identity disagrees with the
/// daemon's engine identity (P2 fails) → `ContinuationEpochMismatch`". The first row is about
/// epochs, and the RFC's "Epochs" section says "`engine` is engine identity, not an epoch"
/// and that prose "that groups `engine` under the word \"epochs\" is corrected by this
/// rule". P2 "is equality on `EpochIdentity`, never an ordering", so a pinned engine against
/// a daemon with none is a P2 disagreement, and "a disagreement in either yields
/// `ContinuationEpochMismatch`" ("The two-predicate obligation"). The error taxonomy agrees:
/// `EpochUnsupported` is "an artifact, page token, or continuation declares an *epoch* this
/// daemon does not implement". Changing this answer to `EpochUnsupported` would need an RFC
/// revision first. The asymmetry is kept visible because a reader of the decision table
/// would otherwise expect the epoch answer, and because INV-008 makes which code arrives a
/// fact, not a detail.
#[test]
fn class_p2_engine_identity_unheld() {
    advanced(&Axis {
        advance: |epochs| epochs.engine = Nullable::Null,
        expected: ErrorCode::ContinuationEpochMismatch,
        same_snapshot: true,
        what: "an unheld engine identity",
    });
}

// =========================================================================================
// the input classes that are not epochs
// =========================================================================================

/// The envelope names a snapshot other than the one the continuation pinned.
///
/// The named snapshot is a *real, current, sealed* snapshot of a second lineage — not the
/// `ws_elsewhere` placeholder the delivering suite uses — so the refusal is about
/// disagreement with the pin and cannot be a disguised not-found.
#[test]
fn class_envelope_names_a_different_snapshot() {
    let mut deployment = parked();
    let elsewhere = {
        let base = deployment.snapshot.clone();
        let forked = deployment.fork(&base, "a second line\n", "req_fork");
        deployment.seal(&forked, "req_seal");
        forked
    };
    assert_ne!(elsewhere, deployment.snapshot);
    assert!(
        deployment
            .daemon
            .state()
            .workspace(&elsewhere)
            .is_some_and(|record| record.sealed()),
        "the named snapshot is real, held and sealed",
    );

    let continuation = deployment.continuation.clone();
    let plan = Resume {
        snapshot: Some(&elsewhere),
        ..probe(&continuation, "req_resume")
    };
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "an envelope naming a snapshot the continuation did not pin",
    );
}

/// The lineage advanced under the pinned snapshot: `workspace.fork` moved the head, so the
/// snapshot the continuation names is no longer current.
///
/// Authentic end to end — a real operation supersedes a real snapshot.
#[test]
fn class_lineage_superseded_the_pinned_snapshot() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    deployment.fork(&base, "edited\n", "req_fork");

    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "a superseded pinned snapshot",
    );
}

/// The pinned snapshot is held but no longer sealed.
///
/// **Grafted**, and the graft is stated: this daemon has no operation that unseals a
/// snapshot, so the record's `seal` is cleared through `DaemonState::workspace_mut`.
/// Everything else — the continuation, the lineage, the epochs — is authentic.
#[test]
fn class_pinned_snapshot_is_no_longer_sealed() {
    let mut deployment = parked();
    let snapshot = deployment.snapshot.clone();
    deployment
        .daemon
        .state_mut()
        .workspace_mut(&snapshot)
        .expect("held")
        .seal = None;

    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "an unsealed pinned snapshot",
    );
}

/// The continuation is not one this daemon holds.
///
/// `CapabilityDenied`, not a not-found: RFC 0027 X2 forbids distinguishing "no such
/// continuation" from "not yours".
#[test]
fn class_continuation_is_not_held() {
    let mut deployment = parked();
    let unheld = ContinuationHandle::new("cont_nothing").expect("a well-formed handle");
    deployment.known.continuations.insert(unheld.clone());
    let plan = probe(&unheld, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::CapabilityDenied,
        "an unheld continuation",
    );
}

/// The pinned snapshot is not held at all.
///
/// **Grafted**: `DaemonState` has no way to drop a workspace record, so the probe parks a
/// continuation whose `snapshot` field names a handle this daemon never minted. The graft is
/// on the continuation, and it is the *only* class where this file forges one.
#[test]
fn class_pinned_snapshot_is_not_held() {
    let mut deployment = parked();
    let mut forged = deployment
        .daemon
        .state()
        .tasks()
        .continuation(&deployment.continuation)
        .expect("held")
        .clone();
    forged.snapshot = WorkspaceHandle::new("ws_elsewhere").expect("a well-formed handle");
    forged.handle = ContinuationHandle::new("cont_pinning_nothing").expect("a handle");
    let handle = forged.handle.clone();
    deployment.daemon.state_mut().tasks_mut().park(forged);
    deployment.known.continuations.insert(handle.clone());
    deployment
        .known
        .workspaces
        .insert(WorkspaceHandle::new("ws_elsewhere").expect("a handle"));

    let plan = probe(&handle, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::CapabilityDenied,
        "a pinned snapshot this daemon does not hold",
    );
}

/// The caller's capability was revoked between the park and the resume.
///
/// Authentic: `DaemonState::revoke_capability` is the deployment's own revocation surface,
/// and admission consults it. **F4** lives here: this refusal is decided at step 5 of
/// `dispatch`, *before* the idempotency ledger, so unlike every other class in this file it
/// leaves no replay record either.
#[test]
fn class_capability_revoked() {
    let mut deployment = parked();
    deployment
        .daemon
        .state_mut()
        .revoke_capability(&cap("cap_runner"));

    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let plan = Resume {
        continuation: &continuation,
        key: Some("idem-revoked"),
        ..plan
    };
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::CapabilityDenied,
        "a revoked capability",
    );
    assert!(
        deployment
            .daemon
            .state()
            .replay("agent:runner", "idem-revoked")
            .is_none(),
        "an admission denial is refused before the idempotency ledger is written (F4)",
    );
}

/// The caller presents a capability whose authority is below `task.resume`'s.
///
/// `cap_reader` confers `read`; the registry declares `task.resume` at `execute`. Authentic,
/// and a different door into `CapabilityDenied` than revocation.
#[test]
fn class_capability_authority_is_below_the_operation() {
    let mut deployment = parked();
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let plan = Resume {
        continuation: &continuation,
        actor: "agent:reader",
        capability: "cap_reader",
        ..plan
    };
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::CapabilityDenied,
        "a capability below the operation's authority",
    );
}

/// The request names a protocol version this connection did not negotiate.
///
/// RFC 0026's resume decision table row "the continuation's protocol major falls outside the
/// served window"; the daemon decides it at step 1, before any state is read.
#[test]
fn class_protocol_version_outside_the_connection() {
    let mut deployment = parked();
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let plan = Resume {
        continuation: &continuation,
        protocol: Some(ProtocolVersion::new(2, 0)),
        ..plan
    };
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::ProtocolVersionUnsupported,
        "a protocol version outside the negotiated connection",
    );
}

/// Reusing one idempotency key for two different resumes.
///
/// Inside `task.resume`'s admissible union and driven here so the coverage table can claim
/// it. The refusal is zero-trace like the rest.
#[test]
fn class_idempotency_key_reused_for_a_different_request() {
    let mut deployment = parked();
    let continuation = deployment.continuation.clone();

    let first = Resume {
        continuation: &continuation,
        key: Some("idem-shared"),
        request_states: Some(CLOSING_CEILING),
        ..probe(&continuation, "req_first")
    };
    let outcome = deployment.resume(&first);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );

    let second = Resume {
        continuation: &continuation,
        key: Some("idem-shared"),
        request_states: Some(CLOSING_CEILING / 2),
        ..probe(&continuation, "req_second")
    };
    refused_without_trace(
        &mut deployment,
        &second,
        ErrorCode::IdempotencyKeyReused,
        "one key answering two different resumes",
    );
}

/// Model availability: the model the continuation's task names is no longer one this daemon
/// can construct. **A regression guard for bn-3oocz.**
///
/// [`PHASE_A_EXIT_PACKAGE.md`] §6.1 lists "the model the continuation names is no longer one
/// this daemon can construct → `UnsupportedSemanticFeature`" as the last row of the decision
/// table. Before bn-3oocz it was not a guard of the same kind as the others: it was raised by
/// `daemon::verification::run_in`, *after* `task.resume` wrote the request's budget onto the
/// task ledger and after `verification::advance` opened a region and stamped
/// `TaskEntry::region` and `worker`. The code was right and the task did not advance, so the
/// delivering suites' two-observable predicate held — but the effect surface moved.
///
/// `daemon::task::resume` now checks the catalog before its first write, like the other
/// twelve classes, so this probe goes through [`refused_without_trace`] and the total
/// fingerprint must be byte-identical. The two extra assertions below name the rows the old
/// trace moved, so a regression reports which write came back.
///
/// The graft — retargeting `TaskEntry::model` at a commitment the catalog has no entry for —
/// is the only reachable spelling of "the daemon can no longer construct this model",
/// because `ModelCatalog` exposes registration and no removal.
///
/// [`PHASE_A_EXIT_PACKAGE.md`]: ../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md
#[test]
fn class_model_availability_is_refused_before_any_reuse() {
    let mut deployment = parked();
    let absent = model_source(
        &Blake3Identity,
        [("Nothing.ctm", b"not registered".as_slice())],
    )
    .expect("blake3 names the module set");
    assert!(
        deployment.daemon.state().models().get(&absent).is_none(),
        "the retargeted model source must genuinely be unregistered",
    );
    let task = deployment.task.clone();
    deployment
        .daemon
        .state_mut()
        .tasks_mut()
        .get_mut(&task)
        .expect("held")
        .model = absent;

    let before = deployment.trace();
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    refused_without_trace(
        &mut deployment,
        &Resume {
            continuation: &continuation,
            ..plan
        },
        ErrorCode::UnsupportedSemanticFeature,
        "a model this daemon can no longer construct",
    );
    let after = deployment.trace();
    assert_eq!(
        after.regions_opened, before.regions_opened,
        "the refusal must not open a region before it decides",
    );
    assert_eq!(
        after.tasks, before.tasks,
        "the refusal must not write the task record — the budget ledger, the region or the \
         worker",
    );
}

// =========================================================================================
// ordering: which guard decides when two inputs are invalid at once
// =========================================================================================

/// A continuation that is both unheld *and* would fail every later guard answers the first
/// guard's code.
#[test]
fn order_continuation_identity_is_decided_before_anything_else() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    deployment.fork(&base, "edited\n", "req_fork");
    let unheld = ContinuationHandle::new("cont_nothing").expect("a handle");
    deployment.known.continuations.insert(unheld.clone());

    let plan = probe(&unheld, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::CapabilityDenied,
        "an unheld continuation whose lineage also advanced",
    );
}

/// The envelope's snapshot disagreement is decided before the lineage's currency.
#[test]
fn order_the_envelope_snapshot_is_decided_before_the_lineage() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    let forked = deployment.fork(&base, "edited\n", "req_fork");
    deployment.seal(&forked, "req_seal");

    // Both are wrong at once: the envelope names the *new* head, and the pinned snapshot is
    // superseded. Both answer `StaleSnapshot`, so the code cannot separate them — what
    // separates them is that the envelope guard runs first, which the next test shows by
    // pairing the envelope with a guard that answers differently.
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let plan = Resume {
        continuation: &continuation,
        snapshot: Some(&forked),
        ..plan
    };
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "an envelope disagreement over a superseded lineage",
    );
}

/// The snapshot guards are decided before the epoch predicates.
///
/// The advanced deployment's epochs disagree *and* the pinned snapshot's lineage moved. The
/// answer is `StaleSnapshot`, so the snapshot half of "epochs and inputs" is evaluated first.
#[test]
fn order_the_snapshot_guards_are_decided_before_the_epoch_predicates() {
    let source = parked();
    let mut advanced_epochs = epochs();
    advanced_epochs.semantic = Nullable::Value(epoch("semantic-2"));
    let mut target_deployment = deployment(&advanced_epochs, version());

    let record = source
        .daemon
        .state()
        .workspace(&source.snapshot)
        .expect("held")
        .clone();
    let fork = source
        .daemon
        .state()
        .lineage(&source.lineage)
        .expect("held")
        .clone();
    target_deployment
        .daemon
        .state_mut()
        .put_workspace(source.snapshot.clone(), record);
    target_deployment.daemon.state_mut().put_lineage(fork);
    let continuation = source
        .daemon
        .state()
        .tasks()
        .continuation(&source.continuation)
        .expect("held")
        .clone();
    target_deployment
        .daemon
        .state_mut()
        .tasks_mut()
        .park(continuation);
    target_deployment
        .known
        .workspaces
        .insert(source.snapshot.clone());
    target_deployment
        .known
        .continuations
        .insert(source.continuation.clone());
    target_deployment
        .known
        .lineages
        .insert(source.lineage.clone());

    // Now supersede the transplanted snapshot in the advanced deployment too.
    let base = source.snapshot.clone();
    target_deployment.fork(&base, "edited\n", "req_fork");

    let plan = Resume {
        continuation: &source.continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    };
    refused_without_trace(
        &mut target_deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "a superseded snapshot under disagreeing epochs",
    );
}

/// The epoch predicates are decided before the task is resolved.
///
/// The advanced deployment holds the transplanted continuation and the snapshot it pins, but
/// **not the task it names** — the task handle's preimage carries the daemon's epochs, so an
/// advanced deployment mints a different one. A resume that resolved the task first would
/// answer `CapabilityDenied`. It answers the epoch code, so the epoch predicates run first.
#[test]
fn order_the_epoch_predicates_are_decided_before_the_task_is_resolved() {
    let source = parked();
    let mut advanced_epochs = epochs();
    advanced_epochs.engine = Nullable::Value(epoch("engine-reference-2"));
    let mut target_deployment = deployment(&advanced_epochs, version());

    assert!(
        target_deployment
            .daemon
            .state()
            .tasks()
            .get(&source.task)
            .is_none(),
        "the advanced deployment must not hold the parked deployment's task",
    );
    let continuation = source
        .daemon
        .state()
        .tasks()
        .continuation(&source.continuation)
        .expect("held")
        .clone();
    assert_eq!(
        continuation.task, source.task,
        "the continuation names the task the advanced deployment does not hold",
    );
    target_deployment
        .daemon
        .state_mut()
        .tasks_mut()
        .park(continuation);
    target_deployment
        .known
        .workspaces
        .insert(source.snapshot.clone());
    target_deployment
        .known
        .continuations
        .insert(source.continuation.clone());

    let plan = Resume {
        continuation: &source.continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    };
    refused_without_trace(
        &mut target_deployment,
        &plan,
        ErrorCode::ContinuationEpochMismatch,
        "an epoch disagreement over an unresolvable task",
    );
}

// =========================================================================================
// the positive controls
// =========================================================================================

/// Untouched inputs resume, and the continuation's declared effect happens **exactly once**.
///
/// The second half is what makes this a control on the first: a resume that ran twice, or a
/// terminal no-op that quietly re-ran, would move the effect surface a second time.
#[test]
fn control_untouched_inputs_resume_and_the_effect_happens_exactly_once() {
    let mut deployment = parked();
    let before = deployment.trace();
    let continuation = deployment.continuation.clone();

    let plan = probe(&continuation, "req_resume");
    let first = deployment.resume(&Resume {
        continuation: &continuation,
        ..plan
    });
    assert_eq!(
        first.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        first.envelope.error
    );
    match &first.payload {
        Payload::TaskResume(response) => {
            assert_eq!(response.status, TaskStatus::Completed);
            assert_eq!(response.task, deployment.task);
        }
        other => panic!("expected a task.resume payload, got {other:?}"),
    }
    let after_first = deployment.trace();
    assert_eq!(deployment.status("req_status"), TaskStatus::Completed);
    assert_eq!(
        after_first.store.len(),
        before.store.len() + 2,
        "the resumed run published exactly one further campaign record and, because it \
         closed, the task's one terminal record (bn-2g3ei)",
    );
    assert_eq!(
        after_first.regions_opened,
        before.regions_opened + 1,
        "the resumed run happened in exactly one further region",
    );

    // The same continuation, again. `rule task.status_monotonic` makes this a no-op.
    let plan = probe(&continuation, "req_resume_2");
    let second = deployment.resume(&Resume {
        continuation: &continuation,
        ..plan
    });
    assert_eq!(second.envelope.status, ResultStatus::Ok);
    match &second.payload {
        Payload::TaskResume(response) => assert_eq!(response.status, TaskStatus::Completed),
        other => panic!("expected a task.resume payload, got {other:?}"),
    }
    let after_second = deployment.trace();
    assert_eq!(
        after_first, after_second,
        "a second resume of a completed task is a no-op on the whole effect surface",
    );
}

/// **F5.** Two deployments differing only in negotiated protocol version derive the *same*
/// snapshot, task and continuation identities, and a resume across them is admitted.
///
/// RFC 0026: "The protocol epoch does not participate. A daemon MUST NOT reject a resume with
/// `ContinuationEpochMismatch` for a protocol-minor difference." The delivering suite checks
/// this against `admissible_epochs` directly, by handing it two `EpochSet`s. This checks the
/// deployment-level consequence, which is the stronger reading: SD-13 says the protocol epoch
/// is in no artifact identity, so the two daemons must agree on every handle as well.
#[test]
fn control_a_protocol_minor_difference_is_not_a_resume_refusal() {
    let source = deployment(&epochs(), ProtocolVersion::new(3, 1));
    let mut other = deployment(&epochs(), ProtocolVersion::new(3, 0));

    assert_eq!(
        other.snapshot, source.snapshot,
        "SD-13: the protocol epoch is not part of snapshot identity",
    );
    assert_eq!(
        other.task, source.task,
        "the task handle's preimage carries the six epochs and engine, never the protocol",
    );
    assert_eq!(
        other.continuation, source.continuation,
        "and neither does the continuation's",
    );

    let plan = Resume {
        continuation: &source.continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    };
    let outcome = other.resume(&plan);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "a protocol-minor difference is not a resume refusal: {:?}",
        outcome.envelope.error
    );
    assert_eq!(other.status("req_status"), TaskStatus::Completed);
}

/// The predicate is not over-broad: state a resume has no business consulting does not
/// refuse it.
///
/// A second lineage advances, a second campaign is started and left parked, and the intent
/// registry is read — and the resume still closes its own task. Without this, every refusal
/// above would be consistent with a daemon that refuses whenever anything changed.
#[test]
fn control_an_unrelated_lineage_and_an_unrelated_task_do_not_refuse_a_resume() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    let forked = deployment.fork(&base, "a second line\n", "req_fork");
    deployment.seal(&forked, "req_seal");
    let other_continuation = deployment.start_on(&forked, "req_start_2");
    assert_ne!(other_continuation, deployment.continuation);

    // The fork advanced *this* lineage, so the original continuation is now stale — which is
    // `class_lineage_superseded_the_pinned_snapshot`. The unrelated-state control is
    // therefore run the other way round: resume the *new* continuation, whose own snapshot is
    // current, while the old task sits parked and the old snapshot sits superseded.
    let plan = Resume {
        continuation: &other_continuation,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: None,
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    };
    let outcome = deployment.resume(&plan);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "a resume is not refused by state it does not depend on: {:?}",
        outcome.envelope.error
    );
    assert_eq!(
        deployment.table_status(),
        TaskStatus::Suspended,
        "and the unrelated parked task is untouched by it",
    );
}

/// A refused resume leaves no *effect*, and is still audited.
///
/// The complement of every zero-trace assertion above: the admission log is outside [`Trace`]
/// precisely because it must move, and RFC 0027 P5 requires "the audit record that must exist
/// whatever the answer was".
#[test]
fn a_refused_resume_is_audited_even_though_it_leaves_no_effect() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    deployment.fork(&base, "edited\n", "req_fork");
    let before = deployment.daemon.state().admissions().len();

    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    refused_without_trace(
        &mut deployment,
        &plan,
        ErrorCode::StaleSnapshot,
        "a superseded pinned snapshot",
    );

    let admissions = deployment.daemon.state().admissions();
    assert!(
        admissions.len() > before,
        "the refusal is recorded in the admission log",
    );
    let last = admissions.last().expect("at least one admission");
    assert_eq!(last.operation, "task.resume");
    assert!(
        last.admitted,
        "the refusal was decided by the handler, not by admission",
    );
}

// =========================================================================================
// recoverability
// =========================================================================================

/// **F3.** A staleness refusal is recoverable — and the refused *key* is not.
///
/// Three facts, in one script because they are one story:
///
/// 1. a continuation whose lineage advanced is refused;
/// 2. a campaign re-derived against the new head parks and resumes to completion, so the
///    refusal is a currency answer and not a terminal one;
/// 3. retrying the refused request under its own idempotency key returns the *recorded*
///    refusal verbatim, without re-evaluating the predicate. That agrees with the taxonomy's
///    "identical retry can succeed: no" column for `StaleSnapshot`, and it means recovery is
///    always a new request — which is what "re-derive against the new epoch" already meant.
#[test]
fn stale_then_fresh_a_refusal_is_recoverable_but_the_refused_key_stays_refused() {
    let mut deployment = parked();
    let base = deployment.snapshot.clone();
    let forked = deployment.fork(&base, "edited\n", "req_fork");
    deployment.seal(&forked, "req_seal");

    let stale = deployment.continuation.clone();
    let refused = refused_without_trace(
        &mut deployment,
        &Resume {
            key: Some("idem-stale"),
            ..probe(&stale, "req_resume")
        },
        ErrorCode::StaleSnapshot,
        "a superseded pinned snapshot",
    );

    // (2) the re-derivation against the new head.
    let fresh = deployment.start_on(&forked, "req_start_2");
    let outcome = deployment.resume(&Resume {
        continuation: &fresh,
        snapshot: Some(&forked),
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume_fresh",
        key: Some("idem-fresh"),
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "the re-derived continuation resumes: {:?}",
        outcome.envelope.error
    );
    match &outcome.payload {
        Payload::TaskResume(response) => assert_eq!(response.status, TaskStatus::Completed),
        other => panic!("expected a task.resume payload, got {other:?}"),
    }

    // (3) the refused key stays refused, verbatim, without re-deciding.
    let replayed = deployment.resume(&Resume {
        continuation: &stale,
        snapshot: None,
        actor: "agent:runner",
        capability: "cap_runner",
        request: "req_resume",
        key: Some("idem-stale"),
        protocol: None,
        envelope_states: Some(CLOSING_CEILING),
        request_states: Some(CLOSING_CEILING),
    });
    assert_eq!(code(&replayed), ErrorCode::StaleSnapshot);
    assert_eq!(
        replayed.envelope, refused.envelope,
        "the recorded refusal replays byte for byte",
    );

    // …and a *new* request over the same stale continuation is refused on the merits, not
    // from the ledger, which is what keeps (3) from being an artefact of the ledger alone.
    refused_without_trace(
        &mut deployment,
        &Resume {
            key: Some("idem-stale-2"),
            ..probe(&stale, "req_resume_again")
        },
        ErrorCode::StaleSnapshot,
        "a superseded pinned snapshot, under a fresh key",
    );
}

// =========================================================================================
// negative controls on the instrument itself
// =========================================================================================

/// The fingerprint moves when something happens.
///
/// Anti-vacuity for every `strict_no_trace` assertion above: an instrument that always
/// reported "unchanged" would pass all of them.
#[test]
fn negative_control_the_fingerprint_moves_on_a_successful_resume() {
    let mut deployment = parked();
    let before = deployment.trace();
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let outcome = deployment.resume(&Resume {
        continuation: &continuation,
        ..plan
    });
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let after = deployment.trace();

    assert!(!strict_no_trace(&before, &after), "the instrument is live");
    assert_ne!(before.store, after.store, "the store row is live");
    assert_ne!(before.tasks, after.tasks, "the task row is live");
    assert_ne!(
        before.regions_opened, after.regions_opened,
        "the region row is live"
    );
    assert_ne!(
        before.finalizations, after.finalizations,
        "the finalization row is live"
    );
    // …and the row that does *not* move, which is a fact and not an instrument failure:
    // `TaskTable`'s continuation map is append-only, so a completed task's continuation is
    // still resolvable. That is what makes the terminal no-op of
    // `control_untouched_inputs_resume_and_the_effect_happens_exactly_once` reachable at
    // all, and it is why the continuation row's power in this file is to catch a *mutated*
    // or *minted* continuation rather than a consumed one.
    assert_eq!(
        before.continuations, after.continuations,
        "a resume does not consume the continuation it resumed",
    );
}

/// A resume whose every input is valid, refused by the run itself: the durable publication
/// of the campaign record aborts (INV-017), so the answer is `PublicationAborted`.
///
/// This is the one refusal in reach of this file that *legitimately* leaves a trace. The
/// predicate admitted the resume, so the budget reached the ledger and a region opened and
/// failed; only then did the store refuse. It is not one of the thirteen input classes, and
/// "before any reuse" does not cover it, because the reuse was admitted. The negative
/// controls below use it for exactly that reason: it is a real daemon path, not a hand-made
/// mutant, and it leaves a trace the weak predicate cannot see.
fn resume_into_a_failing_store() -> (OperationOutcome, Trace, Trace, TaskStatus, TaskStatus) {
    let (mut deployment, switch) = parked_over_a_failing_store();
    switch.arm();

    let before = deployment.trace();
    let status_before = deployment.table_status();
    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let outcome = deployment.resume(&Resume {
        continuation: &continuation,
        ..plan
    });
    let after = deployment.trace();
    let status_after = deployment.table_status();

    assert_eq!(code(&outcome), ErrorCode::PublicationAborted);
    assert_eq!(
        after.regions_opened,
        before.regions_opened + 1,
        "the admitted resume opened a region before the store refused",
    );
    (outcome, before, after, status_before, status_after)
}

/// The mutant: the weak predicate calls a trace-leaving refusal clean, and the strict one
/// does not.
///
/// This is the assertion that decides whether replacing the delivering suites' two-observable
/// check with a full fingerprint was worth doing. Until bn-3oocz the situation was
/// [`class_model_availability_is_refused_before_any_reuse`] itself; that refusal is
/// zero-trace now, so the control drives [`resume_into_a_failing_store`] instead. The two
/// predicates are the same two functions every other test in this file uses.
#[test]
fn negative_control_the_weak_no_trace_predicate_misses_what_the_strict_one_catches() {
    let (outcome, before, after, status_before, status_after) = resume_into_a_failing_store();

    assert!(
        weak_no_trace(&outcome, status_before, status_after),
        "the weak predicate reports no trace",
    );
    assert!(
        !strict_no_trace(&before, &after),
        "the strict predicate reports one",
    );
}

/// A weakened *instrument* misses what the full one catches.
///
/// The second half of the mutant argument, aimed at the fingerprint rather than at the
/// predicate: a fingerprint that read only the publication store — the row most people would
/// reach for — is clean across the same refusal, because nothing was published. Dropping any
/// single row of [`Trace`] is a real loss of power, and this shows it for the row that looks
/// most sufficient.
#[test]
fn negative_control_a_store_only_fingerprint_misses_the_same_trace() {
    let (_, before, after, _, _) = resume_into_a_failing_store();

    assert_eq!(
        before.store, after.store,
        "a store-only fingerprint reports no trace",
    );
    assert!(
        !strict_no_trace(&before, &after),
        "the full fingerprint reports one",
    );
}

// =========================================================================================
// scope, stated as assertions
// =========================================================================================

/// A continuation survives a restart with its pins (bn-20142). This was the declared
/// boundary `scope_a_continuation_does_not_survive_a_restart` confirmed; it is now a guard.
///
/// Crash the daemon, rebuild over the durable substrate, and check that the continuation
/// and its task come back from the committed continuation record: the same handle, the same
/// snapshot, intent, compatibility epochs, engine identity and bounds — every input the
/// resume decision table reads. The frontier is read back through the model by the first
/// resume, so it is held as vectors until then.
#[test]
fn regression_a_continuation_survives_a_restart_with_its_pins() {
    let deployment = parked();
    let continuation = deployment.continuation.clone();
    let task = deployment.task.clone();
    let before = deployment
        .daemon
        .state()
        .tasks()
        .continuation(&continuation)
        .expect("held")
        .clone();
    let record = deployment
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("held")
        .record();
    let substrate = deployment.daemon.crash();
    let restarted = Daemon::builder(Blake3Identity, negotiated_at(version()), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .over(substrate)
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Absent,
            ),
            None,
        )
        .family(TaskFamily)
        .build();

    let after = restarted
        .state()
        .tasks()
        .continuation(&continuation)
        .expect("the continuation table is resolved from the continuation record");
    assert_eq!(after.handle, before.handle);
    assert_eq!(after.task, before.task);
    assert_eq!(after.snapshot, before.snapshot);
    assert_eq!(after.intent, before.intent);
    assert_eq!(after.pinned, before.pinned);
    assert_eq!(after.bounds, before.bounds);
    assert!(
        restarted.state().tasks().is_pending(&continuation),
        "the frontier waits for the model"
    );
    assert_eq!(
        restarted.state().tasks().frontier_of(&continuation),
        Some(
            before
                .frontier
                .iter()
                .map(|state| state.as_slice().to_vec())
                .collect()
        ),
        "and it is the parked frontier, component for component"
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .get(&task)
            .expect("and so is the task table")
            .record(),
        record,
        "the restored task record is the pre-crash one"
    );
}

/// The pinned `in_*` intent is **not** revalidated at resume.
///
/// `Continuation::intent` is pinned at creation, and RFC 0026 requires it to be pinned. The
/// resume decision table does not list an intent condition, and `daemon::task::resume` reads
/// the field only to hand it to the publication record. So: drop the governing contract from
/// the registry and the resume still closes the task.
///
/// Stated as scope rather than as a failure, because the normative table governs — but it is
/// the one place where the criterion's word "inputs" reaches further than the table does, and
/// a reader deciding what G1-04 bought should know that.
#[test]
fn scope_the_pinned_intent_is_not_revalidated_at_resume() {
    let mut deployment = parked();
    let intent = deployment.intent.clone();
    let continuation = deployment
        .daemon
        .state()
        .tasks()
        .continuation(&deployment.continuation)
        .expect("held")
        .clone();
    assert_eq!(
        continuation.intent,
        Nullable::Value(intent.clone()),
        "the continuation pins the governing contract",
    );

    let dropped = deployment.daemon.state_mut().drop_intent(&intent);
    assert!(dropped.is_some(), "the contract was in the registry");
    assert!(
        deployment.daemon.state().intent(&intent).is_none(),
        "and is not any more",
    );

    let handle = deployment.continuation.clone();
    let plan = probe(&handle, "req_resume");
    let outcome = deployment.resume(&plan);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "the resume does not consult the pinned intent: {:?}",
        outcome.envelope.error
    );
    assert_eq!(deployment.table_status(), TaskStatus::Completed);
}

/// The model catalog is a trusted out-of-band registration, not a validated input.
///
/// Registering a *different* model under the source commitment the task names changes what a
/// resume runs, and nothing refuses it — the commitment is the content identity of the
/// module bytes, so the registration is a claim about bytes that the catalog does not
/// recheck. This is the deployment's own administration surface (`DaemonState::models_mut`,
/// beside capability provisioning and content staging), not a caller-reachable one, so it is
/// a declared trust boundary rather than a defect. It is written down because "validates …
/// inputs" would otherwise be read as covering it.
#[test]
fn scope_the_model_catalog_is_trusted_rather_than_revalidated() {
    let mut deployment = parked();
    let source = deployment
        .daemon
        .state()
        .tasks()
        .get(&deployment.task)
        .expect("held")
        .model
        .clone();
    assert_eq!(
        source,
        die_hard_source(),
        "the task names the Die Hard model"
    );

    // A second, genuinely different model filed under the first one's identity.
    let replacement = diehard::model().expect("the port builds");
    deployment
        .daemon
        .state_mut()
        .models_mut()
        .register(source, replacement);

    let continuation = deployment.continuation.clone();
    let plan = probe(&continuation, "req_resume");
    let outcome = deployment.resume(&Resume {
        continuation: &continuation,
        ..plan
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "a re-registered catalog entry is not a resume refusal: {:?}",
        outcome.envelope.error
    );
}

/// **O1.** A `ws_*` identity is blind to the epochs the snapshot was declared under; a
/// `task_*` and a `cont_*` identity are not.
///
/// Found while building this file, by an assertion that was written expecting the opposite.
/// `daemon::workspace::check_epochs` forces a create request to declare the deployment's own
/// `semantic` and `proof`, so a `semantic-2` deployment can only mint `semantic-2` snapshots
/// — and the handle it mints is bit-for-bit the one a `semantic-1` deployment mints from the
/// same files, because `WorkspaceDescriptor`'s canonical record is over the file tree plus
/// the dependency, toolchain and configuration components and carries no epoch.
///
/// This is consistent with the dossier rather than against it: docs/35's task-identity list
/// is "operation, snapshot, intent, semantic/proof/engine epochs, normalized parameters,
/// strategy class", which spends the epochs at the *task* level, and plan §4.4's `ws_*` is
/// the content identity of a workspace's content. Nothing normative puts an epoch in a
/// snapshot identity.
///
/// It matters to G1-04 because it settles what carries the weight. "Resume validates the
/// pinned snapshot" does **not** imply "resume validates the epochs": two deployments an
/// epoch apart agree on the `ws_*` handle, so the snapshot guard cannot separate them and
/// `admissible_epochs` is doing the whole of that job. The two halves of the criterion's
/// "epochs and inputs" are genuinely two halves.
#[test]
fn observation_a_snapshot_identity_is_blind_to_the_epochs_it_was_declared_under() {
    let one = parked();
    let mut advanced_epochs = epochs();
    advanced_epochs.semantic = Nullable::Value(epoch("semantic-2"));
    let two = deployment(&advanced_epochs, version());

    assert_eq!(
        one.snapshot, two.snapshot,
        "the declared semantic epoch is not part of snapshot identity",
    );
    assert_eq!(
        one.daemon.state().workspace(&one.snapshot),
        two.daemon.state().workspace(&two.snapshot),
        "and neither deployment's record of it differs in any other field",
    );
    assert_ne!(
        one.task, two.task,
        "the task identity does carry the epochs (docs/35's identity list)",
    );
    assert_ne!(
        one.continuation, two.continuation,
        "and so does the continuation identity",
    );
}

// =========================================================================================
// the coverage accounting (INV-007)
// =========================================================================================

/// Every code this file drives, and the class that drives it.
const PROBED: &[(ErrorCode, &str)] = &[
    (
        ErrorCode::CapabilityDenied,
        "class_continuation_is_not_held, class_pinned_snapshot_is_not_held, \
         class_capability_revoked, class_capability_authority_is_below_the_operation",
    ),
    (
        ErrorCode::StaleSnapshot,
        "class_envelope_names_a_different_snapshot, \
         class_lineage_superseded_the_pinned_snapshot, \
         class_pinned_snapshot_is_no_longer_sealed",
    ),
    (
        ErrorCode::ContinuationEpochMismatch,
        "class_p1_* (five epochs), class_p2_engine_identity_disagreement, \
         class_p2_engine_identity_unheld",
    ),
    (ErrorCode::EpochUnsupported, "class_unheld_* (five epochs)"),
    (
        ErrorCode::UnsupportedSemanticFeature,
        "class_model_availability_is_refused_before_any_reuse",
    ),
    (
        ErrorCode::PublicationAborted,
        "negative_control_* through resume_into_a_failing_store — a run refusal after \
         admission, not an input class",
    ),
    (
        ErrorCode::ProtocolVersionUnsupported,
        "class_protocol_version_outside_the_connection",
    ),
    (
        ErrorCode::IdempotencyKeyReused,
        "class_idempotency_key_reused_for_a_different_request",
    ),
];

/// Every code this file does **not** drive, and why (INV-007: an absence, not a pass).
const UNPROBED: &[(ErrorCode, &str)] = &[
    (
        ErrorCode::MalformedRequest,
        "a decode or shape failure, decided two layers above the resume predicate and \
         carrying no continuation; `codec_canonical_form.rs` owns it",
    ),
    (
        ErrorCode::QuotaExhausted,
        "this daemon enforces no quota, so no request can reach the code",
    ),
    (
        ErrorCode::BudgetExhausted,
        "a resumed run that parks again answers `task_suspended` with a continuation rather \
         than raising, and `budget::bounds_of` floors a lowered ceiling at recorded spend, so \
         no resume in this file's shape can raise it",
    ),
];

/// The accounting is mechanical: the registry decides which codes `task.resume` may answer
/// with, and this file must place each one.
///
/// `rule errors.common` is read through `daemon::errors::admits`, the same function the
/// dispatcher checks every outgoing fault against — so the enumeration this file claims to
/// cover is checked against the daemon's own union rather than against a list written here.
#[test]
fn the_probe_table_accounts_for_every_code_task_resume_may_answer_with() {
    let spec = registry::operation("task.resume").expect("the registry declares `task.resume`");
    let admissible: Vec<ErrorCode> = ErrorCode::ALL
        .iter()
        .copied()
        .filter(|code| errors::admits(spec, *code))
        .collect();
    assert!(
        !admissible.is_empty(),
        "the union must not be empty, or this test is vacuous",
    );

    for code in &admissible {
        let probed = PROBED.iter().any(|(entry, _)| entry == code);
        let unprobed = UNPROBED.iter().any(|(entry, _)| entry == code);
        assert!(
            probed ^ unprobed,
            "{code:?} is admissible for `task.resume` and is placed {} times in this file's \
             accounting; it must be placed exactly once",
            usize::from(probed) + usize::from(unprobed),
        );
    }
    for (code, _) in PROBED.iter().chain(UNPROBED) {
        assert!(
            admissible.contains(code),
            "{code:?} is accounted for but `task.resume` may not answer with it",
        );
    }
    assert_eq!(
        PROBED.len() + UNPROBED.len(),
        admissible.len(),
        "the accounting has no duplicates",
    );
}

/// The predicate consults engine identity as well as the five compatibility epochs, and the
/// count is read off the type rather than asserted.
///
/// `PinnedEpochs` has no `protocol` field — SD-13 as a property of the type — so the
/// enumeration this file probes is "five compatibility epochs plus engine", and both halves
/// have their own tests above. This test pins the *shape*, so a seventh field arriving with a
/// new epoch kind fails here rather than silently going unprobed.
#[test]
fn the_pinned_set_is_five_compatibility_epochs_plus_engine_identity() {
    let pinned = continuumd::daemon::task::PinnedEpochs::of(&epochs());
    let rendered = format!("{pinned:?}");
    for field in [
        "semantic", "intent", "evidence", "proof", "corpus", "engine",
    ] {
        assert!(
            rendered.contains(field),
            "`PinnedEpochs` must carry {field}, and this file probes each one separately",
        );
    }
    assert!(
        !rendered.contains("protocol"),
        "SD-13: the protocol epoch is not part of a continuation's pinned set",
    );
}
