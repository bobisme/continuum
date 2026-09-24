//! The five-kind evidence map for `INV-008` (`notes/plan/plan.md:335-337`), engine/task/daemon
//! half, for bone `bn-n9a1`. The kernel half — "unsupported semantics", proved as a
//! cross-crate sweep over all four `continuum-kernel-*` crates — is
//! `crates/continuum-kernel-core/tests/inv008_unsupported_vs_rejected_evidence.rs`.
//!
//! > `INV-008` — Typed inconclusiveness
//! >
//! > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and
//! > incomplete proof search are distinct outcomes.
//! >
//! > — `notes/plan/plan.md:335-337`
//!
//! # The audit: where each of the five named kinds lands today
//!
//! The dossier's closed six-member vocabulary is
//! [`continuum_value::assurance::InconclusiveReason`] (`Unsupported`, `ResourceExhausted`,
//! `EngineError`, `InsufficientTelemetry`, `AbstractionAmbiguity`, `IncompleteProofSearch`) —
//! six, not five, because `EngineError` ("the engine failed rather than answered") is a real,
//! distinct outcome the contract sentence does not name but the schema and RFC 0026 both
//! require. Mapped to the five named kinds plus that sixth:
//!
//! | INV-008 kind | `InconclusiveReason` | Landed where | Landed test |
//! |---|---|---|---|
//! | timeout | `ResourceExhausted` | engine (`checking::Unresolved::ResourceExhausted`), daemon wire, end to end through `Daemon::dispatch` | `checking_diehard.rs::no_bounded_run_ever_reports_holds`; `daemon_task_operations.rs::budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete` |
//! | unsupported semantics | `Unsupported` | kernel `Verdict::Unsupported` (all four crates); `continuum-task`'s `VerificationTaskResult::unsupported_empty_task` (PR-1 exit); daemon's sibling wire refusal `ErrorCode::UnsupportedSemanticFeature` | kernel: see the sibling evidence file; task: `pr_1_exit_unsupported_empty_task.rs` |
//! | insufficient telemetry | `InsufficientTelemetry` | daemon `evidence.verify` over a redacted reference | `daemon_evidence.rs::verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction` |
//! | abstraction ambiguity | `AbstractionAmbiguity` | **not yet** — declared vocabulary only | [`boundary_abstraction_ambiguity_is_declared_vocabulary_with_no_producer_yet`] below |
//! | incomplete proof search | `IncompleteProofSearch` | **not yet** — declared vocabulary only | [`boundary_incomplete_proof_search_has_no_engine_to_produce_it_yet`] below |
//! | (unnamed sixth) engine failure | `EngineError` | engine (`checking::Unresolved::EngineError`), daemon wire, end to end | `checking_contract.rs::a_predicate_that_cannot_be_evaluated_is_an_engine_error`; `inv006_replay_stability_evidence.rs::positive_an_engine_evaluation_failure_reaches_the_wire_as_a_typed_engine_error` |
//!
//! Four of six are landed and mechanically tested today; the honest gap is
//! `AbstractionAmbiguity` and `IncompleteProofSearch`, both Phase-D-adjacent (the
//! correspondence/refinement family and the isolated Lean proof service, RFC 0035/PR 28,
//! neither open). Reporting that gap truthfully — rather than inventing a producer to close
//! the table — is itself the discipline INV-008 and `docs/12`'s `GOV-1-12` ("ambiguous
//! behavior is an error, not implementation freedom") both ask for: a kind with no landed
//! surface is *recorded as absent*, never guessed at.
//!
//! A seventh, adjacent-but-distinct concept: **budget exhaustion is never a verdict at all**
//! (plan §11.4). `ResourceExhausted` is what a *bounded exploration that still ran to its own
//! completion* reports for the property it could not decide; `ErrorCode::BudgetExhausted`
//! (`crates/continuum-task/src/budget.rs`, ratified by bn-1gc) is the *task*-level result when
//! the whole campaign parks before any check runs at all, carrying a continuation rather than
//! a reason. `crates/continuumd/src/daemon/verification.rs`'s own module doc states the two
//! are never substituted for each other ("Nothing here re-decides anything the engine
//! decided. In particular **budget exhaustion is not a verdict**"), and
//! `continuumd::protocol::vocabulary::ErrorCode::QuotaExhausted`'s own doc comment
//! distinguishes it a third way ("Distinct from `BudgetExhausted`, which is task-budget spend
//! and carries a continuation"). `continuum-task/src/budget/dimension.rs`'s
//! [`DimensionOmission`](continuum_task::budget::dimension::DimensionOmission) is the fourth,
//! narrower cousin of the same "never silently satisfy" discipline, one layer down: a budget
//! *dimension* a caller declares but this daemon cannot meter (`MeterSet::STATES_ONLY` meters
//! only `states`) is a typed omission (INV-007), not a silently-ignored ceiling and not a
//! charge invented against a meter that does not exist.
//!
//! # What this file adds, and why
//!
//! Everything cited above is landed, tested, and cited — not re-implemented. Four things were
//! genuinely missing, closed below:
//!
//! 1. **The never-a-pass asymmetry, re-asserted as THE INV-008 statement, for a property that
//!    actually holds.** `checking_diehard.rs::no_bounded_run_ever_reports_holds` proves the
//!    asymmetry at the engine layer, over both of Die Hard's invariants at once, generically
//!    across nine bounds. `daemon_task_operations.rs`'s budget-exhaustion test proves the
//!    daemon-wire half, but its target is `AllClaims`, which also reaches `NotSolved` — a
//!    property that is *refuted* at depth 6, not one that would look established from a
//!    truncated prefix. Nothing existing pins the sharper case: a *single* target known to
//!    hold over the complete model (`TypeOK`, a tautology — `diehard.rs`'s own doc:
//!    "`TypeOK` is transcribed even though it is a tautology over this model's declared
//!    types"), true at *every* state a truncated run sees, still answering `inconclusive` —
//!    never `established` — through a real `Daemon::dispatch`, in direct textual contrast
//!    with `daemon_task_operations.rs::a_single_property_target_establishes_the_invariant_that_holds`,
//!    which is the same target under a large-enough budget. [`positive_the_same_target_is_established_unbounded_and_inconclusive_bounded`]
//!    below is that contrast, in one test, and it goes one step further than either existing
//!    test: it also recomputes the engine's own `Unresolved::ResourceExhausted` directly
//!    (bypassing the daemon) over the identical model and bound, and asserts its `.as_str()`
//!    spelling equals the wire `InconclusiveReason`'s `.as_wire()` token for the *same* live
//!    value — a direct binding `registry_agreement.rs`'s indirect, source-vs-source token
//!    comparison does not reach, because it never touches
//!    `continuum_engine_reference::checking::Unresolved` at all (by design —
//!    `checking.rs`'s own doc: that crate holds no dependency edge to `continuum-value`,
//!    "the ambiguity `GOV-1-12` exists to forbid" per `ident.rs`; the mapping is asserted as
//!    strings in that crate's own tests instead). This file is not that crate, and already
//!    depends on both, so it closes the triangle live rather than by three separate
//!    citations.
//! 2. **A sweep proving no verdict-value surface on the wire is boolean-typed.** The four
//!    `*VerdictValue` structs (`SemanticVerdictValue`, `EvaluationVerdictValue`,
//!    `PolicyVerdictValue`, `StructuralVerdictValue`, `protocol/envelope.rs`) and the `Verdict`
//!    union wrapping them are read at compile time and swept for a `bool`-typed field — there
//!    is none, and [`negative_the_boolean_sweep_is_not_vacuous`] proves the sweep can find one
//!    if a mutant introduces it.
//! 3. **The closed-count anti-drift pin**, restated here rather than only in
//!    `registry_agreement.rs`: `SemanticVerdict::ALL` is 3, the wire `InconclusiveReason::ALL`
//!    and the value-layer `InconclusiveReason::ALL` both 6, and
//!    `continuum_engine_reference::checking::Verdict::ALL` — the engine's own
//!    "established/refuted/inconclusive", independently declared and deliberately *not* the
//!    same type (`checking.rs`'s own doc: "deliberately not `continuum-kernel-core`'s
//!    `Verdict`") — is also 3.
//! 4. **The two honest gaps, pinned as boundaries** (the `inv006_replay_stability_evidence.rs`
//!    precedent: "recorded here as the honest boundary rather than patched"), so a future PR
//!    landing either producer is what turns these tests red and forces a revisit, rather than
//!    leaving the gap to prose alone.
//!
//! # House rules
//!
//! `src/` is not touched, in this crate or any other, and no existing test is touched.
//! Everything cited above is cited, not re-implemented.

use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::checking::{self, DeadlockPolicy, Obligations, Unresolved};
use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::assurance::InconclusiveReason as ValueInconclusiveReason;
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
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, ErrorCode, InconclusiveReason, Portfolio, SemanticVerdict,
    TargetKind,
};

use continuum_workspace::snapshot::WorkspacePath;

// ---------------------------------------------------------------------------------------
// fixtures: the real Die Hard port and contract, `include_str!`'d — one Die Hard, per
// `daemon_task_operations.rs`'s own rationale for not copying either.
// ---------------------------------------------------------------------------------------

const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";

/// The frozen TV-009 fact this file's contrast test depends on: the complete Die Hard state
/// space closes at 16 states, so any bound strictly below it trips a bound before the
/// exploration reaches the same answer.
const FROZEN_STATES: u64 = 16;

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
        encodings: vec![continuumd::protocol::vocabulary::Encoding::CanonicalJson],
        client: "continuumd-inv008-evidence-test".to_owned(),
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

fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-inv008-probe-v1"),
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

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}

/// A daemon holding the Die Hard workspace sealed, its contract accepted, and the Die Hard
/// model registered in the catalog — the same recipe `daemon_task_operations.rs` and
/// `inv006_replay_stability_evidence.rs` each use, rebuilt here because a `tests/*.rs` file
/// cannot `use` a sibling one's private fixture helpers.
fn fixture() -> Fixture {
    let mut daemon = daemon();
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
        continuumd::protocol::vocabulary::ResultStatus::Ok,
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
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![configuration],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        continuumd::protocol::vocabulary::ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    Fixture { daemon, snapshot }
}

fn start(fixture: &mut Fixture, states: u64, request_id: &str, id: &str) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        request_id,
                    ),
                    id,
                ),
                states,
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::Property, diehard::TYPE_OK),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn semantic_verdict(
    fixture: &mut Fixture,
    task: continuumd::protocol::scalar::TaskHandle,
) -> (
    SemanticVerdict,
    Optional<InconclusiveReason>,
    AssuranceClass,
) {
    let outcome = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task }),
    });
    match &outcome.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => (
            value.verdict,
            value.inconclusive_reason,
            value.assurance_class,
        ),
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------
// 1. the never-a-pass asymmetry, re-asserted as THE INV-008 statement, live and end to end
// ---------------------------------------------------------------------------------------

/// **Positive.** `TypeOK` is a tautology over Die Hard's declared state domain: true at every
/// one of the model's 16 reachable states, and therefore true at every state *any* prefix of
/// the exploration sees too. Under a large-enough budget the exploration closes and the wire
/// answers `established` — this half is
/// `daemon_task_operations.rs::a_single_property_target_establishes_the_invariant_that_holds`,
/// not re-tested here. Under a bound that stops before the 16th state, the same target over
/// the same model is `inconclusive`/`ResourceExhausted`, never `established` — the case no
/// existing test pins for a property that actually holds throughout what was seen, which is
/// the sharpest reading of INV-008's own words ("not finding a violation in a partial set is
/// inconclusive … never a pass") and of `checking_diehard.rs::no_bounded_run_ever_reports_holds`,
/// restated here through the wire rather than the engine API.
///
/// The test goes one step further: it also runs the identical model and bound directly
/// through `continuum_engine_reference::bfs`/`checking` — bypassing the daemon entirely — and
/// asserts that the engine's own `Unresolved::ResourceExhausted::as_str()` spells the same
/// token as the wire `InconclusiveReason`'s `as_wire()`, for the *same* live outcome. This is
/// the triangle `checking.rs`'s own doc says is closed only by citation (engine spelling
/// asserted as a string in `checking_contract.rs`; wire spelling checked against
/// `continuum-value` in `registry_agreement.rs`) — closed here directly, because this crate,
/// unlike `continuum-engine-reference`, already depends on both without violating the
/// dependency-freedom `GOV-1-12` rationale `ident.rs` records.
#[test]
fn positive_the_same_target_is_established_unbounded_and_inconclusive_bounded() {
    // The daemon path, bounded: three states is short of the sixteen `TypeOK` needs to close.
    // A bound this tight trips before `verification.start` can even run the campaign to a
    // synchronous completion, so the envelope reports `task_suspended` rather than
    // `task_started` — both name a real task (`ResultStatus`'s own doc: "started" carries
    // `task`, "suspended" carries `task` *and* `continuation`), and either way the point
    // holds: nothing here is `ok`/`established` at start time.
    let mut fixture = fixture();
    let started = start(&mut fixture, 3, "req_start", "idem-start");
    assert!(
        matches!(
            started.envelope.status,
            continuumd::protocol::vocabulary::ResultStatus::TaskStarted
                | continuumd::protocol::vocabulary::ResultStatus::TaskSuspended
        ),
        "a verification.start under a tripped bound must still name a task, not fail \
         outright: {:?} ({:?})",
        started.envelope.status,
        started.envelope.error
    );
    let task = match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a completed campaign still names its task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    let (verdict, reason, assurance) = semantic_verdict(&mut fixture, task);
    assert_eq!(
        verdict,
        SemanticVerdict::Inconclusive,
        "TypeOK holds at every one of the 3 explored states, but a truncated prefix is not \
         the model — never a pass"
    );
    assert_eq!(
        reason,
        Optional::Present(InconclusiveReason::ResourceExhausted)
    );
    assert_eq!(assurance, AssuranceClass::Bounded);

    // The engine path, direct: the same model, the same bound, no daemon in between.
    let model = diehard::model().expect("the port builds");
    let bounds = Bounds::CERTIFIABLE.with_states(3);
    let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
    assert!(
        exploration.closed().is_none(),
        "the bound must trip before the model closes"
    );
    let index = model
        .predicates()
        .iter()
        .position(|p| p.name().as_str() == diehard::TYPE_OK)
        .expect("TypeOK is declared");
    let obligations = Obligations::new(DeadlockPolicy::Allowed).invariant(index);
    let report =
        checking::check(&model, &exploration, &obligations).expect("the obligation is declared");
    let outcome = report
        .invariants()
        .iter()
        .find(|result| result.index() == index)
        .expect("TypeOK was upheld")
        .outcome();
    let checking::CheckOutcome::Inconclusive(unresolved @ Unresolved::ResourceExhausted { .. }) =
        outcome
    else {
        panic!("a 3-state bound must be a resource-exhausted inconclusive, got {outcome:?}");
    };

    // The live value, not a freshly constructed stand-in: this is the actual `Unresolved`
    // this exploration produced, and its spelling must equal the actual wire token the
    // daemon reported above for the identical model and bound.
    assert_eq!(
        unresolved.as_str(),
        InconclusiveReason::ResourceExhausted.as_wire(),
        "the engine's own spelling and the wire's must be the same token for the same reason \
         — the binding `checking.rs` cites three other tests to establish, closed directly \
         here instead"
    );

    // Sanity: this is really the same model the daemon dispatched, and it really closes at
    // the frozen 16 given room. Otherwise "bounded" above would prove nothing about the
    // asymmetry — it would just be a model with fewer than 3 states.
    let complete = bfs::explore(&model, Bounds::CERTIFIABLE).expect("Die Hard evaluates");
    assert_eq!(
        complete.closed().expect("unbounded closes").len() as u64,
        FROZEN_STATES
    );
}

// ---------------------------------------------------------------------------------------
// 2. no verdict-value wire surface is boolean-typed
// ---------------------------------------------------------------------------------------

const ENVELOPE_SRC: &str = include_str!("../src/protocol/envelope.rs");

/// The span of `protocol/envelope.rs` from the first `*VerdictValue` struct through the
/// closing brace of the `Verdict` union that wraps all four — every wire surface INV-008's
/// closed vocabulary reaches.
fn verdict_surface_span() -> &'static str {
    let start = ENVELOPE_SRC
        .find("struct SemanticVerdictValue")
        .expect("SemanticVerdictValue is declared");
    let rest = &ENVELOPE_SRC[start..];
    let union_at = rest
        .find("union Verdict {")
        .expect("the Verdict union follows");
    let after_union = &rest[union_at..];
    let end = after_union.find("\n}").expect("the union closes");
    &rest[..union_at + end]
}

/// Whether `source` declares a `bool`-typed field anywhere (`: bool` or `bool;`, the two
/// shapes this protocol DSL and plain Rust structs would each use).
fn declares_a_boolean_field(source: &str) -> bool {
    source.contains(": bool") || source.contains("bool;") || source.contains("bool required")
}

#[test]
fn positive_no_verdict_value_surface_declares_a_boolean_field() {
    let span = verdict_surface_span();
    assert!(
        span.contains("SemanticVerdictValue")
            && span.contains("EvaluationVerdictValue")
            && span.contains("PolicyVerdictValue")
            && span.contains("StructuralVerdictValue"),
        "the extracted span must cover all four verdict-value structs"
    );
    assert!(
        !declares_a_boolean_field(span),
        "a `bool`-typed field on any verdict-value struct would be exactly the bare success \
         flag INV-008 (and plan §3 B11's \"'Verified' alone is prohibited\") forbids"
    );
    // Each struct's outcome-carrying field must name one of the typed enums, never a flag.
    // The field is not always spelled `verdict` — `PolicyVerdictValue`'s is `decision` and
    // `StructuralVerdictValue`'s is `outcome` — but every one of the four is a closed enum,
    // never a `bool`, which the substring sweep above already covers; this loop additionally
    // pins each field's specific type so a future edit cannot widen one to `bool` by renaming
    // the field out from under the substring check.
    for (struct_name, field) in [
        ("SemanticVerdictValue", "verdict: SemanticVerdict"),
        ("EvaluationVerdictValue", "verdict: EvaluationVerdict"),
        ("PolicyVerdictValue", "decision: PolicyDecision"),
        ("StructuralVerdictValue", "outcome: StructuralOutcome"),
    ] {
        let at = span
            .find(struct_name)
            .unwrap_or_else(|| panic!("{struct_name} is declared"));
        let body = &span[at..];
        assert!(
            body.contains(field),
            "{struct_name}'s outcome field must be `{field}`, found near: {:?}",
            &body[..body.len().min(200)]
        );
    }
}

#[test]
fn negative_the_boolean_sweep_is_not_vacuous() {
    let mutant = "struct SemanticVerdictValue { established: bool required; }";
    assert!(
        declares_a_boolean_field(mutant),
        "a mutant introducing a boolean success flag must be detected, not passed vacuously"
    );
}

// ---------------------------------------------------------------------------------------
// 3. closed-count anti-drift, restated here
// ---------------------------------------------------------------------------------------

#[test]
fn positive_the_verdict_and_reason_vocabularies_are_closed_at_the_counts_inv008_requires() {
    assert_eq!(
        SemanticVerdict::ALL.len(),
        3,
        "established | refuted | inconclusive, per RFC 0026 and GOV-1-12's non-ambiguity"
    );
    assert_eq!(
        InconclusiveReason::ALL.len(),
        6,
        "the wire InconclusiveReason"
    );
    assert_eq!(
        ValueInconclusiveReason::ALL.len(),
        6,
        "continuum-value's canonical InconclusiveReason"
    );
    assert_eq!(
        checking::Verdict::ALL.len(),
        3,
        "the engine's own established/refuted/inconclusive — a distinct type from the wire's \
         SemanticVerdict and from continuum-kernel-core's Verdict, by design (checking.rs's \
         own doc), but the same count, because the schema fixes it at three everywhere a \
         verdict is a verdict"
    );
}

// ---------------------------------------------------------------------------------------
// 4. the two honest gaps
// ---------------------------------------------------------------------------------------

/// **Boundary.** `AbstractionAmbiguity` is declared in the closed `InconclusiveReason`
/// vocabulary and its wire-error sibling `ErrorCode::AmbiguousCorrespondence` is declared on
/// four registry rows (`program.extract`, `refinement.check`, `correspondence.bind`,
/// `correspondence.drift`) — but no `daemon::family::Arguments` variant exists for any of the
/// `program.*`/`refinement.*`/`correspondence.*` operations (`family.rs`'s enum has no
/// `Program`, `Refinement`, or `Correspondence` variant at all), so the codec that every real
/// request goes through refuses to decode one. This is the same shape
/// `inv006_replay_stability_evidence.rs` pins for `program.replay`'s `ErrorCode::ReplayDiverged`
/// — declared vocabulary, scaffolding rather than a producer — extended here to the sibling
/// reason this invariant names by name. Correspondence/refinement machinery is not this
/// bone's to build; recording the honest boundary is.
#[test]
fn boundary_abstraction_ambiguity_is_declared_vocabulary_with_no_producer_yet() {
    assert!(InconclusiveReason::ALL.contains(&InconclusiveReason::AbstractionAmbiguity));
    assert!(ValueInconclusiveReason::ALL.contains(&ValueInconclusiveReason::AbstractionAmbiguity));

    for operation in [
        "program.extract",
        "refinement.check",
        "correspondence.bind",
        "correspondence.drift",
    ] {
        let spec = registry::operation(operation)
            .unwrap_or_else(|| panic!("the registry still declares {operation:?}"));
        assert!(
            spec.errors.contains(&ErrorCode::AmbiguousCorrespondence),
            "{operation}'s declared errors must include AmbiguousCorrespondence: {:?}",
            spec.errors
        );
        let decoded = decode_arguments(operation, &Opaque::from_bytes(b"{}".to_vec()));
        assert_eq!(
            decoded,
            Err(CodecError::UnknownOperation),
            "{operation} is declared vocabulary with no decodable Arguments variant — no \
             family has landed to construct AbstractionAmbiguity or AmbiguousCorrespondence \
             from a real request"
        );
    }
}

/// **Boundary.** `IncompleteProofSearch` is declared in the closed `InconclusiveReason`
/// vocabulary with no wire-error sibling and no producer at all: `continuum-engine-symbolic`
/// (docs/01 §7.2, the solver-backed engine) and `continuum-proof-client` (plan §15, PR 28,
/// the Lean proof-service client) are each still the bare PR-1/IMPL-01 scaffold their own
/// module docs say they are — no `pub fn`, only the responsibility and dependency-boundary
/// prose. Neither crate's PR has opened; this pins the current scaffold shape mechanically
/// so that the day either crate grows a public API, this test goes red and the gap must be
/// revisited rather than silently narrowed.
#[test]
fn boundary_incomplete_proof_search_has_no_engine_to_produce_it_yet() {
    assert!(InconclusiveReason::ALL.contains(&InconclusiveReason::IncompleteProofSearch));
    assert!(ValueInconclusiveReason::ALL.contains(&ValueInconclusiveReason::IncompleteProofSearch));

    const SYMBOLIC_LIB: &str = include_str!("../../continuum-engine-symbolic/src/lib.rs");
    const PROOF_CLIENT_LIB: &str = include_str!("../../continuum-proof-client/src/lib.rs");
    for (name, source) in [
        ("continuum-engine-symbolic", SYMBOLIC_LIB),
        ("continuum-proof-client", PROOF_CLIENT_LIB),
    ] {
        assert!(
            source.contains("PR-1 / IMPL-01 scaffold"),
            "{name} must still name itself a scaffold"
        );
        assert!(
            !source.contains("pub fn")
                && !source.contains("pub struct")
                && !source.contains("pub enum"),
            "{name} must not yet declare a public API — the moment it does, something may be \
             producing IncompleteProofSearch and this boundary needs re-examination"
        );
    }
}

// ---------------------------------------------------------------------------------------
// an undefined read is refused, never a verdict (bn-24a5c)
// ---------------------------------------------------------------------------------------

/// **Negative.** A model whose action reads a partial map at an absent key at a state it
/// reaches carries `Read#defined`, false at the initial state (RFC 0003, "Definedness").
/// The reference engine reports that as `CheckOutcome::Undefined`, and no member of the
/// wire's `InconclusiveReason` names an invalid model, so the daemon refuses the campaign
/// as `UnsupportedSemanticFeature` — the refusal it already gives a transition relation
/// that is undefined at a reachable state — rather than answer `established` for
/// `TypeOK`, which is what the lowered model alone would have given.
#[test]
fn negative_an_undefined_read_at_a_reachable_state_is_refused_not_established() {
    use continuum_engine_reference::{ActionDecl, BoolExpr, CmpOp, IntExpr, ModelBuilder};
    let eq = |name: &str, value: i64| {
        BoolExpr::compare(CmpOp::Eq, IntExpr::var(name), IntExpr::constant(value))
    };
    let partial = ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("m?0", 0, 1)
        .variable("m!0", 0, 1)
        // `Read`: `require x == 0; x' = m[0]`, lowered with `D(U)` conjoined to the guard.
        .action(ActionDecl::deterministic(
            "Read",
            BoolExpr::and(eq("x", 0), eq("m?0", 1)),
            vec![("x", IntExpr::var("m!0"))],
        ))
        .predicate("Read#defined", BoolExpr::implies(eq("x", 0), eq("m?0", 1)))
        .predicate(diehard::TYPE_OK, BoolExpr::constant(true))
        .initial_state(&[("x", 0), ("m?0", 0), ("m!0", 0)])
        .build()
        .expect("the partial-map model builds");

    // The engine half: the lowered model alone would establish `TypeOK`.
    let exploration = bfs::explore(&partial, Bounds::CERTIFIABLE).expect("explores");
    let type_ok = partial.predicate_index(diehard::TYPE_OK).expect("declared");
    let report = checking::check(
        &partial,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect).invariant(type_ok),
    )
    .expect("declared");
    let undefined = report.undefined().expect("an undefined read is reported");
    assert_eq!(undefined.subject(), "Read");
    assert_eq!(report.verdict(), checking::Verdict::Inconclusive);

    // The daemon half: the same model under the snapshot's source is refused, typed.
    let mut fixture = fixture();
    fixture
        .daemon
        .state_mut()
        .models_mut()
        .register(die_hard_source(), partial);
    let started = start(&mut fixture, 1_000, "req_undefined", "idem-undefined");
    assert_eq!(
        started.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature),
        "{:?}",
        started.envelope
    );
}
