//! Dedicated evidence map for `INV-006` — replay stability (`notes/plan/plan.md:327-329`,
//! the invariant bone `bn-2eeg`).
//!
//! > A failure advertised as replayable must reproduce under its pinned semantic epoch or
//! > be downgraded to an engine defect.
//! >
//! > — `notes/plan/plan.md:329`
//!
//! # What already closes this sentence, and where
//!
//! The "must reproduce" half is evidenced everywhere in this workspace already — every
//! crate's own determinism suite, `crates/continuumd/tests/pr8_exit_evidence.rs`'s
//! `positive_the_whole_frame_sequence_replayed_from_a_fresh_daemon_is_byte_identical` (two
//! independently built daemons, one script, byte-identical transcripts) foremost among
//! them — and the "pinned semantic epoch" half by
//! `daemon_task_operations.rs`'s
//! `resume_admissibility_separates_an_epoch_disagreement_from_an_unheld_epoch` (a
//! continuation pinning a different epoch than the one this daemon currently serves is
//! `ContinuationEpochMismatch`, never silently resumed under the new one) and by
//! `budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete` (a
//! suspended campaign resumed under its own epoch completes to the frozen TV-009 facts, not
//! an approximation of them). This file does not re-litigate either half; it closes the one
//! clause neither suite reaches.
//!
//! # The clause this file closes: "or be downgraded to an engine defect"
//!
//! `crates/continuum-value/src/assurance.rs`'s `InconclusiveReason::EngineError` is the
//! closed-vocabulary type this downgrade lands in, and its own doc comment says so by name:
//! "Also where a failure advertised as replayable but not reproducible is downgraded to
//! (INV-006)". `crates/continuumd/src/daemon/verification.rs:775` lifts
//! `continuum_engine_reference::checking::Unresolved::EngineError` into that same wire
//! member, and `crates/continuum-engine-reference/tests/checking_contract.rs`'s
//! `a_predicate_that_cannot_be_evaluated_is_an_engine_error` proves the engine *produces*
//! that value. What no existing test proved is that the daemon's lift line is live: every
//! daemon-level test that reaches `InconclusiveReason` exercises `ResourceExhausted`
//! (`daemon_task_operations.rs`'s own `budget_exhaustion_parks_a_continuation_…` and
//! `codec_canonical_form.rs`), and none drives a real dispatch down the `EngineError` arm.
//! [`positive_an_engine_evaluation_failure_reaches_the_wire_as_a_typed_engine_error`] below
//! is that test: a model whose one declared predicate cannot be evaluated (the same
//! overflow recipe `checking_contract.rs`'s `unevaluable()` uses, built here from the same
//! public constructors because a `tests/*.rs` file is its own crate and cannot `use` a
//! sibling one — the idiom `pr8_exit_evidence.rs` already documents), run end to end
//! through [`Daemon::dispatch`], and read back off the wire as `Inconclusive` /
//! `EngineError` — never a semantic verdict, exactly as `docs/35`'s engine-defect lifecycle
//! and RFC 0026 both require.
//!
//! # The boundary this file also pins: the *wire-operation* half of the downgrade is
//! unbuilt scaffolding, not a silently broken gap
//!
//! `program.replay`'s registry entry (`crates/continuumd/src/protocol/registry.rs:284-296`)
//! declares `ErrorCode::ReplayDiverged` — the wire code plan §4.7 and RFC 0026 name for
//! "a replay diverged from the recorded execution; emits a `defect_*`" — but no
//! `daemon::family::Arguments` variant exists for `program.replay` (`family.rs`'s own
//! 25-variant enum, one per operation whose family has landed), so
//! `codec::operations::decode_arguments` cannot produce one: its own doc comment states
//! plainly that this is one of "the 45 of the 74 whose families have not landed". `git grep`
//! confirms `ErrorCode::ReplayDiverged` is constructed nowhere in `src/` — only declared, in
//! the vocabulary token table and this one registry row. This is PR 11 (Context Pack
//! `ReplayPreserving`, `crates/continuum-context` — a PR-1/IMPL-01 scaffold today, see its
//! own module doc) and PR 14/15 (asupersync semantic journal, `program.replay`'s actual
//! producer) territory, neither landed; recorded here as the honest boundary rather than
//! patched, per the evidence-bone precedent (bn-dw81's deferred-Lean-receipt finding).
//! [`boundary_program_replay_is_declared_vocabulary_with_no_decodable_producer`] pins it
//! mechanically so a future PR landing the producer — rather than merely adding an
//! `Arguments` variant — is what turns this test red and forces it to be revisited.
//!
//! A third boundary — full multi-operation daemon-session replay *after a process
//! restart* (state rebuilt from a durable store, as opposed to the in-process replay both
//! suites above prove) — has no artifact to pin at all: `Daemon::builder` only ever
//! constructs a fresh in-memory [`DaemonState`](continuumd::daemon::state::DaemonState),
//! there is no store-backed reconstruction path, and `docs/35`'s own "On restart the daemon
//! MUST resolve every task it left in `Running`…" contract is unimplemented normative text,
//! not a regression. The owning bone is `bn-3dr` ("Daemon crash recovery: no stale index
//! entries or orphan tasks"), open, blocked on `bn-19u` (PR 6, also open) —
//! `notes/plan/notes/G0_SPIKE_MATRIX.md`'s own words for exactly this gap: "daemon-scale
//! linearizability and crash-injection evidence remains PR 6+ per docs/35". Recorded in the
//! bone comment rather than pinned here, because there is nothing yet to pin against.

use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuumd::codec::CodecError;
use continuumd::codec::operations::decode_arguments;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, RequestEnvelope, Verdict};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, Encoding, ErrorCode, InconclusiveReason, Portfolio,
    ResultStatus, SemanticVerdict, TargetKind,
};

use continuum_workspace::snapshot::WorkspacePath;

/// The Die Hard Intent Contract, reused unchanged. Intent acceptance is administrative
/// (`rule` IDL §7) and no operation here consults the contract's own claim names against
/// the model's predicate names — `daemon/verification.rs`'s `obligations` resolves a
/// target against `Model::predicate_index` alone — so reusing this fixture for a
/// non-Die-Hard model is the same move `daemon_task_operations.rs` and
/// `pr8_exit_evidence.rs` already make of the Die Hard `.ctm` port: one accepted contract,
/// used to admit workspace creation, orthogonal to which model this test registers.
const PROBE_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Placeholder `.ctm` bytes. This daemon has no CML front end (`verification.rs`'s own
/// module doc: "There is no CML front end"); a snapshot's module content is hashed for
/// identity and never parsed, so any distinct bytes name a distinct, registerable model.
const PROBE_MODULE: &str = "// inv006 probe: a predicate this model cannot evaluate\n";
const PROBE_MODULE_PATH: &str = "Probe.ctm";

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

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-inv006-evidence-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

fn epochs() -> continuumd::protocol::envelope::EpochSet {
    continuumd::protocol::envelope::EpochSet {
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

/// A daemon with exactly the families this file's fixture needs.
fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
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

struct Fixture {
    daemon: Daemon,
    snapshot: WorkspaceHandle,
}

fn probe_contract() -> IntentContract {
    IntentContract::decode(PROBE_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
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

fn probe_source() -> Commitment {
    model_source(
        &Blake3Identity,
        [(PROBE_MODULE_PATH, PROBE_MODULE.as_bytes())],
    )
    .expect("blake3 names the module set")
}

/// A one-variable, one-action model that never terminates (so `DeadlockPolicy::Defect` —
/// the only policy `verification.rs::obligations` ever declares for a single-property
/// target — never turns a deadlock into a competing refutation, letting the predicate
/// failure be the campaign's only source of inconclusiveness) and whose one declared
/// predicate overflows `i64` at every evaluation, unconditionally. Same recipe as
/// `continuum-engine-reference/tests/checking_contract.rs`'s `unevaluable()`, rebuilt here
/// from the same public constructors (`ModelBuilder`, `ActionDecl`, `BoolExpr`, `IntExpr`)
/// because that file's helper is private to a sibling crate's test binary.
fn engine_error_model() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Stay",
            BoolExpr::Const(true),
            vec![],
        ))
        .predicate(
            "Overflows",
            BoolExpr::compare(
                CmpOp::Gt,
                IntExpr::plus(IntExpr::constant(i64::MAX), IntExpr::constant(1)),
                IntExpr::constant(0),
            ),
        )
        .build()
        .expect("a self-loop with an unconditionally overflowing predicate is a valid model")
}

fn acceptance_bytes() -> Opaque {
    use continuum_intent::canonical_json::Json;
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-inv006-probe-v1"),
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

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    });
    envelope
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

/// A daemon holding the probe workspace sealed, its (reused Die Hard) contract accepted,
/// and the probe model registered in the catalog.
fn fixture() -> Fixture {
    let mut daemon = daemon();
    let contract = probe_contract();
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

    let file = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(PROBE_MODULE_PATH).expect("a workspace path"),
            PROBE_MODULE.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    daemon
        .state_mut()
        .models_mut()
        .register(probe_source(), engine_error_model());

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

    let created = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: vec![file],
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: Vec::new(),
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

    Fixture { daemon, snapshot }
}

fn started(fixture: &mut Fixture) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        "req_start",
                    ),
                    "idem-start",
                ),
                64,
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::Property,
                id: "Overflows".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

// ---------------------------------------------------------------------------------------
// positive: the downgrade is live, through a real dispatch
// ---------------------------------------------------------------------------------------

/// **Positive.** A model whose one declared predicate cannot be evaluated (`Overflows`,
/// unconditionally `i64::MAX + 1`) is targeted directly through `verification.start`. The
/// exploration itself succeeds — the model's only action assigns nothing and is always
/// enabled, so nothing about reachability is in question — and it is the *check* that
/// fails, exactly as `checking_contract.rs` names the distinction ("the model is well
/// formed and the exploration succeeds; it is the check that fails"). What crosses the
/// wire is `Inconclusive` with `InconclusiveReason::EngineError`, never a semantic verdict
/// and never silently established or refuted — the daemon-lift line
/// `assurance.rs`'s `EngineError` doc comment names for INV-006 ("Also where a failure
/// advertised as replayable but not reproducible is downgraded to"), proved live rather
/// than only declared.
#[test]
fn positive_an_engine_evaluation_failure_reaches_the_wire_as_a_typed_engine_error() {
    let mut fixture = fixture();
    let start = started(&mut fixture);
    assert_eq!(
        start.envelope.status,
        ResultStatus::TaskStarted,
        "a `@task_starting` operation answers `task_started` even when the campaign it \
         starts runs to completion synchronously: {:?}",
        start.envelope.error
    );
    let task = match &start.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a completed campaign still names its task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };

    let result = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task }),
    });
    assert_eq!(
        result.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        result.envelope.error
    );

    match &result.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(
                value.verdict,
                SemanticVerdict::Inconclusive,
                "a predicate the engine cannot evaluate is never a semantic answer"
            );
            assert_eq!(
                value.inconclusive_reason,
                Optional::Present(InconclusiveReason::EngineError),
                "the typed downgrade INV-006 names, not `ResourceExhausted` and not silence"
            );
            assert_eq!(
                value.assurance_class,
                AssuranceClass::Validated,
                "the exploration itself closed; only the check of it failed"
            );
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
    match &result.payload {
        Payload::VerificationResult(response) => {
            assert!(
                response.continuation.is_absent(),
                "an engine error is not a budget question; nothing is parked"
            );
        }
        other => panic!("expected a verification.result payload, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------
// boundary: the wire-operation half of the downgrade is scaffolding, not a silent gap
// ---------------------------------------------------------------------------------------

/// **Boundary.** `program.replay` declares `ErrorCode::ReplayDiverged` — the wire code for
/// exactly the sentence this bone maps ("or be downgraded to an engine defect") at the
/// *replayed-recording* grain RFC 0026 describes, distinct from the finite-model-service
/// grain the positive test above exercises. But no `Arguments` variant exists for it
/// (`family.rs`'s enum has 25 members, one per landed family), so the codec — the same
/// function the transport calls on every real request
/// (`crate::transport::mod.rs`'s own use of `decode_arguments`) — refuses to decode one at
/// all. This is PR 11 / PR 14-15 territory, not a regression in anything PR 8 shipped, and
/// the citation inside `codec::operations::decode_arguments`'s own doc comment ("the 45 of
/// the 74 whose families have not landed") is the mechanical fact this test pins so a
/// future family landing silently narrows what this test protects rather than leaving the
/// claim to prose alone.
#[test]
fn boundary_program_replay_is_declared_vocabulary_with_no_decodable_producer() {
    let spec = registry::operation("program.replay").expect(
        "PR 8's registry still declares `program.replay` (RFC 0026, plan §10.2's 74-row table)",
    );
    assert!(
        spec.errors.contains(&ErrorCode::ReplayDiverged),
        "the wire vocabulary for INV-006's downgrade at the replay-operation grain is \
         declared: {:?}",
        spec.errors
    );

    let decoded = decode_arguments("program.replay", &Opaque::from_bytes(b"{}".to_vec()));
    assert_eq!(
        decoded,
        Err(CodecError::UnknownOperation),
        "declared in the registry, refused by the codec: no family has landed to produce \
         `ReplayDiverged`, so nothing can silently mis-answer it either"
    );
}
