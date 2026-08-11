//! Acceptance re-derivation for release gate **G1-03** (bone `bn-3huh7`).
//!
//! > requests are idempotent under idempotency keys
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, G1 bullet 3
//!
//! The normative source is not that bullet. It is `rule idempotency.replay`, in
//! `notes/plan/schemas/continuumd-native-protocol.idl`, restated and extended by RFC 0026
//! ("Ordering, idempotency, and atomicity"):
//!
//! > A mutation replayed with the same `idempotency_key` and a byte-identical canonical
//! > request MUST return the same task or artifact identity. The same key with a different
//! > request MUST be rejected with `IdempotencyKeyReused`. Keys are scoped per actor and
//! > MUST be honored for at least the retention window the daemon declares in
//! > `ServerLimits.idempotency_retention_ms` (default 24h).
//!
//! # Why this file exists beside the evidence that already passes
//!
//! [`PHASE_A_EXIT_PACKAGE.md`] §7.2 records G1-03 as "evidenced and falsified-then-fixed —
//! `dx03_falsification.rs` idempotence axis; `ReplayKey` defect fixed by `bn-h1zqz`", and
//! §9.4 A17 states its own limit: the package "re-ran the *delivering* suites, not an
//! independent re-derivation". This file is the re-derivation, and it is deliberately built
//! out of different material.
//!
//! | Delivering evidence (`dx03_falsification.rs`) | This file |
//! |---|---|
//! | one operation — `verification.start` — carries the whole idempotence axis | **all 47 `@mutation` operations are enumerated** and each is classified; the 18 a typed dispatch can be handed are each driven twice ([`sweep_every_reachable_mutation_is_idempotent_under_one_key`]) |
//! | the enumeration is implicit: whatever the fixture happens to call | the enumeration is an **independent parse of the IDL text** ([`idl_operations`]), cross-checked against `registry::OPERATIONS`, so an operation cannot be silently skipped ([`instrument_the_idl_and_the_registry_declare_the_same_forty_seven_mutations`]) |
//! | idempotence is read off the *answer* — `wire_bytes(first) == wire_bytes(replay)` | idempotence is read off the **world**: a [`Fingerprint`] over the publication store's bytes, its authorization audit log, the evidence graph, the intent registry, and the answers of independently dispatched `@readonly` operations ([`Fingerprint::of`]) |
//! | the replay carries the **same** `request_id` as the first call | the replay carries a **fresh** `request_id`, which is what a real retry does and what `ReplayKey`'s own documentation says it must be free to do — and that is where this file's finding comes from ([`finding_a_replay_echoes_the_first_attempts_request_id`]) |
//! | "a different request" is a different envelope `budget` (the bn-h1zqz regression) | "a different request" is a different envelope `output_policy`, applied **uniformly to all 18** operations, so the discrimination is a property of the ledger rather than of one operation's preimage |
//! | the negative control is one campaign under two keys | the negative control is the **whole sweep** re-run under two fresh keys, per operation, reporting for each whether a genuine second execution is observably different from the recorded one ([`control_a_fresh_key_re_executes_and_the_sweep_says_where_that_is_observable`]) |
//! | single actor | cross-actor and cross-operation scoping are probed in both directions ([`scoping_another_actors_key_behaves_as_an_unused_key`], [`scoping_one_key_may_not_group_two_operations`]) |
//!
//! # The verdict this file reaches
//!
//! **SATISFIED-AT-NARROWER-SCOPE, with one finding.**
//!
//! - **Satisfied** for the 18 `@mutation` operations this daemon's typed dispatch surface can
//!   be handed. For each: the same key with a byte-equal canonical request returns the
//!   recorded outcome and moves no observable state; the same key with a different canonical
//!   request is refused `IdempotencyKeyReused` and moves no observable state; and the refusal
//!   is not vacuous, because that same request driven as the *first* call of a fresh daemon
//!   lands in exactly the lane the original body lands in
//!   ([`sweep_the_refusal_is_not_vacuous_because_the_variant_body_is_an_ordinary_request`]).
//! - **Narrower scope**: 29 of the 47 `@mutation` operations are **unreachable** — no
//!   `Arguments` variant names them, so a request for one cannot pass the dispatch's shape
//!   check and never reaches the ledger at all. Their idempotency is **unprobed, not passed**
//!   ([`scope_twenty_nine_mutations_are_unreachable_and_therefore_unprobed`]).
//! - **Finding (`request_id` echo).** RFC 0026's envelope table declares `request_id`
//!   "client-unique; **echoed in the result**". The ledger returns the recorded
//!   `OperationOutcome` verbatim, so a retry under a fresh `request_id` is answered with an
//!   envelope echoing the **first attempt's** `request_id` — and, for `@audit_recorded`
//!   mutations, the first attempt's `audit` correlation, which `rule audit.correlation`
//!   requires to be a function of *this* request's identity. The delivering evidence cannot
//!   see this because it replays with an unchanged `request_id`. Recorded as executable
//!   assertions in [`finding_a_replay_echoes_the_first_attempts_request_id`] and
//!   [`finding_a_replay_returns_the_first_attempts_audit_correlation`]; both are written to
//!   *pass* on today's daemon, describing what it does, so that a repair flips them and
//!   nothing here silently rots.
//!
//! # Absences, stated rather than papered over (INV-007, INV-008)
//!
//! 1. **The retention window is not probed.** `ServerLimits.idempotency_retention_ms` is a
//!    declared 24h default, and this daemon's ledger has no eviction path and no clock input
//!    to drive one — [`absence_the_retention_window_has_no_reachable_expiry`] asserts the
//!    absence rather than claiming the window holds.
//! 2. **Concurrency is not probed.** Every dispatch here is sequential and single-process.
//!    "Two racing retries of one key" is not a question this surface can be asked.
//! 3. **No transport, no encoding.** The probes go through [`Daemon::dispatch`], so
//!    "byte-identical canonical request" is compared through the typed [`ReplayKey`] the
//!    daemon itself uses. That is the same stand-in `daemon::state` documents; a codec-level
//!    re-derivation would be a different and stronger instrument, and this file does not
//!    claim it.
//! 4. **Crash and restart are not probed.** The ledger is volatile state (`recovery`), so a
//!    replay across a restart is G1-08's question, already answered with a declared decline.
//! 5. **The daemon is convergent, which weakens what a same-key replay proves on its own.**
//!    Many of these operations are content-addressed, so a genuine second execution would
//!    have produced the same artifact anyway.
//!    [`control_a_fresh_key_re_executes_and_the_sweep_says_where_that_is_observable`] measures
//!    it: **12 of the 18** have a detectable second execution — 5 answer differently and 7
//!    move the world without answering differently — and for all 12 the same-key replay
//!    produced the recorded outcome and moved nothing, so on those the ledger demonstrably
//!    short-circuited. For the other **6** this instrument cannot tell a ledger hit from a
//!    convergent re-run, and three of those six are lanes this deployment answers with a
//!    typed refusal at all. Twelve, not eighteen, is what the ledger is shown to carry.
//! 6. **Three of the eighteen probes drive a first call that is itself a typed refusal** —
//!    `context.compile`, `context.expand`, `intent.propose_revision`. They still exercise the
//!    ledger, because a refusal is recorded and replayed like any other outcome, but they
//!    cannot exercise exactly-once *effect*, because there is no effect
//!    ([`sweep_every_reachable_mutation_is_idempotent_under_one_key`] counts them).
//!
//! [`PHASE_A_EXIT_PACKAGE.md`]: ../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md
//! [`ReplayKey`]: continuumd::daemon::state::ReplayKey

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::whiteboard::WhiteboardFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, identity};
use continuumd::protocol::envelope::{Budget, EpochSet, OutputPolicy, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::{ContextCompileRequest, ContextExpandRequest};
use continuumd::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentGetRequest, IntentLockRequest, IntentProposeRevisionRequest,
    IntentRejectRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::whiteboard::WhiteboardCompileRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateRequest, WorkspaceForkRequest,
    WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle,
    EpochIdentity, EvidenceHandle, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId,
    TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    EvidenceQuery, FileOverlay, IntentChangeSet, SnapshotComponents, SnapshotEpochs, Target,
};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, ExpansionRelation, Portfolio, ResultStatus,
    TargetKind,
};

// =========================================================================================
// A — the enumeration instrument: the IDL, parsed here, from its own text
// =========================================================================================

/// The IDL document itself. Read as text, not through the registry that transcribes it.
const IDL: &str = include_str!("../../../notes/plan/schemas/continuumd-native-protocol.idl");

/// One `operation` declaration, as the IDL text spells it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IdlOperation {
    /// The `namespace.verb` name.
    name: String,
    /// The `@…` annotations attached to the declaration, in text order.
    annotations: Vec<String>,
}

/// Every operation the IDL declares, parsed from the document.
///
/// The grammar this reads is the IDL's own: a contiguous run of `@annotation` lines
/// immediately above a line of the form `operation namespace.verb {`. Contiguity is what
/// keeps it honest — the token `@mutation` also appears inside the header's notation table
/// and inside several `rule` bodies, and any line that is neither an annotation line nor an
/// operation header clears the pending run, so no stray mention can attach itself to a
/// declaration.
///
/// This shares no code with [`registry::OPERATIONS`], which is the point:
/// [`instrument_the_idl_and_the_registry_declare_the_same_forty_seven_mutations`] compares
/// the two, and a registry that had quietly dropped a mutation would keep every existing
/// suite green and fail here.
fn idl_operations() -> Vec<IdlOperation> {
    let mut operations = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for line in IDL.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('@') {
            pending.extend(annotation_tokens(rest));
        } else if let Some(name) = operation_header(trimmed) {
            operations.push(IdlOperation {
                name,
                annotations: core::mem::take(&mut pending),
            });
        } else {
            pending.clear();
        }
    }
    operations
}

/// The `@`-prefixed tokens on one annotation line, `@mutation @privileged @audit_recorded`
/// included. The leading `@` is already consumed by the caller.
fn annotation_tokens(rest: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut reading = true;
    for character in rest.chars() {
        match character {
            'a'..='z' | '_' if reading => current.push(character),
            '@' => {
                if !current.is_empty() {
                    tokens.push(core::mem::take(&mut current));
                }
                reading = true;
            }
            _ => {
                if !current.is_empty() {
                    tokens.push(core::mem::take(&mut current));
                }
                reading = false;
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// The operation name on an `operation namespace.verb {` header, if this line is one.
///
/// The `namespace.verb` shape is required, not merely the keyword: the word "operation" also
/// starts prose lines inside the IDL's `rule` bodies, and one of them would otherwise be read
/// as a declaration.
fn operation_header(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("operation ")?.strip_suffix(" {")?;
    let (namespace, verb) = rest.split_once('.')?;
    let nameable = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '_')
    };
    (nameable(namespace) && nameable(verb)).then(|| rest.to_owned())
}

/// The mutating operations, as the IDL declares them.
fn idl_mutations() -> BTreeSet<String> {
    idl_operations()
        .into_iter()
        .filter(|operation| {
            operation
                .annotations
                .iter()
                .any(|annotation| annotation == "mutation")
        })
        .map(|operation| operation.name)
        .collect()
}

/// The mutating operations, as the Rust registry declares them.
fn registry_mutations() -> BTreeSet<String> {
    OPERATIONS
        .iter()
        .filter(|spec| spec.has(Annotation::Mutation))
        .map(|spec| spec.name.to_owned())
        .collect()
}

/// The count the IDL's own revision note fixes for protocol 3.6 / IDL 1.11.
const DECLARED_MUTATIONS: usize = 47;

/// The count the IDL's own revision note fixes for the whole registry.
const DECLARED_OPERATIONS: usize = 75;

#[test]
fn instrument_the_idl_and_the_registry_declare_the_same_forty_seven_mutations() {
    let parsed = idl_operations();
    assert_eq!(
        parsed.len(),
        DECLARED_OPERATIONS,
        "the IDL's own count table says {DECLARED_OPERATIONS} rows; the parse found {}",
        parsed.len()
    );
    let names: BTreeSet<&str> = parsed.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names.len(),
        parsed.len(),
        "no operation is declared twice, so a set is a faithful enumeration"
    );

    let from_idl = idl_mutations();
    assert_eq!(
        from_idl.len(),
        DECLARED_MUTATIONS,
        "the IDL's own count table says `@mutation` is {DECLARED_MUTATIONS}"
    );
    assert_eq!(
        from_idl,
        registry_mutations(),
        "the registry transcribes the IDL's mutation set exactly \
         (`rule conformance.registry_agreement`)"
    );
}

// =========================================================================================
// B — the reachability partition: which mutations a typed dispatch can be handed
// =========================================================================================

/// The `@mutation` operations this daemon's dispatch surface can be handed at all.
///
/// Reachability is not a policy: it is decided by `daemon::family::Arguments`, whose variants
/// are the operation bodies this build can construct. Step 3 of the dispatch compares
/// `arguments.operation()` against the registry name, so a mutation with no variant cannot
/// produce a request that survives the shape check — and the shape check runs *before* the
/// idempotency ledger, so such an operation never files a key.
///
/// The list is written out rather than derived so that a new family landing without a probe
/// here fails [`sweep_the_reachable_set_is_exactly_the_operations_with_a_request_body`]
/// instead of silently shrinking the sweep.
const REACHABLE: &[&str] = &[
    "context.compile",
    "context.expand",
    "evidence.link",
    "evidence.verify",
    "intent.accept",
    "intent.lock",
    "intent.propose_revision",
    "intent.reject",
    "observe.ingest",
    "task.cancel",
    "task.resume",
    "task.update_budget",
    "verification.start",
    "whiteboard.compile",
    "workspace.create",
    "workspace.create_by_reference",
    "workspace.fork",
    "workspace.seal",
];

/// Every operation name an `Arguments` variant can carry, discovered by construction.
///
/// Built by asking each probe's own body for its operation name and adding the `@readonly`
/// bodies this file constructs for its fingerprint. That is the honest way to enumerate a
/// Rust enum from outside the crate: a variant nothing constructs is a variant no client can
/// send either.
fn constructible_mutation_bodies() -> BTreeSet<String> {
    let rig = fixture();
    probes()
        .iter()
        .map(|probe| (probe.arguments)(&rig).operation().to_owned())
        .collect()
}

#[test]
fn sweep_the_reachable_set_is_exactly_the_operations_with_a_request_body() {
    let reachable: BTreeSet<String> = REACHABLE.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(
        reachable.len(),
        REACHABLE.len(),
        "the reachable list has no duplicate"
    );
    assert_eq!(
        constructible_mutation_bodies(),
        reachable,
        "every probe drives a declared reachable operation and every one is probed"
    );
    assert!(
        reachable.is_subset(&registry_mutations()),
        "every probed operation is a `@mutation` in the registry"
    );
}

#[test]
fn scope_twenty_nine_mutations_are_unreachable_and_therefore_unprobed() {
    let reachable: BTreeSet<String> = REACHABLE.iter().map(|name| (*name).to_owned()).collect();
    let unreachable: BTreeSet<String> = registry_mutations()
        .difference(&reachable)
        .cloned()
        .collect();
    assert_eq!(
        reachable.len() + unreachable.len(),
        DECLARED_MUTATIONS,
        "the partition is total"
    );
    assert_eq!(reachable.len(), 18, "eighteen mutations are reachable");
    assert_eq!(
        unreachable.len(),
        29,
        "twenty-nine are not, and their idempotency is unprobed rather than passed"
    );

    // The unreachable set is exactly the eleven namespaces this daemon serves no family for.
    let served: BTreeSet<&str> = [
        "context",
        "evidence",
        "intent",
        "observe",
        "task",
        "verification",
        "whiteboard",
        "workspace",
    ]
    .into_iter()
    .collect();
    for name in &unreachable {
        let namespace = name.split_once('.').expect("a `namespace.verb` name").0;
        assert!(
            !served.contains(namespace),
            "{name} is in a served namespace, so it should have been reachable"
        );
    }

    // And the unreachability is a *fact about the dispatch*, not an inference from the enum:
    // a request naming one of them is refused before the ledger, which is checked below.
    assert!(
        unreachable.contains("model.check"),
        "the witness the next test drives is in the unreachable set"
    );
}

#[test]
fn scope_an_unreachable_mutation_is_refused_before_the_ledger_and_files_no_key() {
    // `model.check` is `@mutation @task_starting` and has no `Arguments` variant. The
    // strongest available statement is not that it fails — it is *where* it fails: the shape
    // check is step 3 and the ledger is step 7, so a refusal here leaves the ledger empty,
    // and a key a client believed it had spent is in fact unspent.
    let mut rig = fixture();
    let mut envelope = envelope("model.check", "agent:runner", "cap_runner", "req_unreach");
    envelope.idempotency_key = Optional::Present("idem-unreachable".to_owned());
    envelope.budget = Optional::Present(budget(STATE_CEILING));

    let refused = rig.daemon.dispatch(&OperationRequest {
        envelope,
        // A well-formed body for a *different* operation: the only body this build can
        // construct, and the reason `model.check` is unreachable at all.
        arguments: Arguments::TaskStatus(TaskStatusRequest {
            task: rig.task.clone(),
        }),
    });
    assert_eq!(
        refused.error_code(),
        Some(ErrorCode::MalformedRequest),
        "the shape check refuses a body that is not the operation's own"
    );
    assert!(
        rig.daemon
            .state()
            .replay("agent:runner", "idem-unreachable")
            .is_none(),
        "step 3 precedes step 7: nothing was filed under the key"
    );
}

// =========================================================================================
// C — the fixture
// =========================================================================================

/// The Die Hard model source, the port the whole daemon suite verifies against.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The Die Hard Intent Contract.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// Where the model sits inside the snapshot, and in the model-source preimage.
const MODULE_PATH: &str = "DieHard.ctm";

/// A trace body, for `observe.ingest` to name.
const TRACE: &str = "{\"event\":\"send\",\"at\":0}\n{\"event\":\"recv\",\"at\":1}\n";

/// A second trace, so an ingest probe appends a node the rig does not already hold.
const OTHER_TRACE: &str = "{\"event\":\"send\",\"at\":2}\n{\"event\":\"recv\",\"at\":3}\n";

/// A receipt body, for `evidence.link` to name.
const RECEIPT: &str = "{\"receipt\":\"gate-g1-03\"}\n";

/// The instrumentation profile every ingest here declares.
const PROFILE: &str = "trace/v1";

/// The `states` bound that parks the Die Hard campaign with a continuation.
const PARK_BUDGET: u64 = 4;

/// The `states` bound that admits Die Hard's whole reachable set.
const STATE_CEILING: u64 = 64;

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

fn profile(privileged: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: true,
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

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g1-03-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this file's daemon pins, matching the exit package's own deployment (§8.2).
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

fn snapshot_epochs() -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: epoch("semantic-1"),
        proof: epoch("proof-1"),
        toolchain: Optional::Absent,
    }
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

fn contract(text: &str) -> IntentContract {
    IntentContract::decode(text.trim_end().as_bytes()).expect("the fixture decodes")
}

/// A contract variant whose identity differs from the base one's.
///
/// The change is inside the RFC 0037 ID1/ID2 identity preimage — `bounds.faults` is, and
/// `intent_id` and `name` are not — so two variants are two registry records rather than two
/// spellings of one.
fn contract_variant(faults: u32) -> IntentContract {
    contract(
        &DIE_HARD_CONTRACT
            .trim_end()
            .replace("\"faults\":0", &format!("\"faults\":{faults}")),
    )
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

/// The acceptance record `intent.accept` takes.
fn acceptance_bytes(signature: &str) -> Opaque {
    let mut fields: BTreeMap<String, continuum_intent::canonical_json::Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", signature),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(
            key.to_owned(),
            continuum_intent::canonical_json::Json::String(value.to_owned()),
        );
    }
    Opaque::from_bytes(continuum_intent::canonical_json::Json::Object(fields).to_canonical_bytes())
}

/// One whiteboard-note entry, in the schema's key order.
fn note_entry(claim: &str, subject: &str, text: &str) -> String {
    format!(
        "{{\"claim\":\"{claim}\",\"references\":[],\"subject\":\"{subject}\",\"text\":\"{text}\"}}"
    )
}

/// A minimal whiteboard note whose author is `agent:builder`, in the schema's key order.
fn note_bytes(goal: &str) -> Vec<u8> {
    format!(
        "{{\"author\":\"agent:builder\",\"candidate_invariants\":[],\"counterexamples\":[],\
         \"created_at\":\"2026-08-01T00:00:00.000Z\",\"decisions\":[],\"experiments\":[],\
         \"goal\":{},\"known_facts\":[],\"note_id\":\"note_g1_03\",\"schema_epoch\":1,\
         \"schema_id\":\"https://continuum.dev/schema/whiteboard-note.json\",\
         \"unresolved_obligations\":[]}}",
        note_entry("claim-goal", "prop_g1_03", goal)
    )
    .into_bytes()
}

/// Everything a probe needs to name something real.
struct Rig {
    daemon: Daemon,
    /// The accepted contract that governs every snapshot here.
    governing: IntentHandle,
    /// A proposed contract, for the `intent.accept` probe.
    for_accept: IntentHandle,
    /// A second proposed contract, for the `intent.reject` probe.
    for_reject: IntentHandle,
    /// A third proposed contract, so the interleaving block can move the world without
    /// colliding with any probe's own subject.
    for_interleave: IntentHandle,
    /// The staged Die Hard files, in `workspace.create` order.
    files: Vec<Commitment>,
    /// A second file, so a create can name different components.
    extra: Commitment,
    configuration: Commitment,
    /// A registered `SnapshotComponents` value, for the by-reference lane.
    reference: Commitment,
    /// The sealed snapshot the campaigns run against.
    snapshot: WorkspaceHandle,
    /// An *unsealed* snapshot, so `workspace.seal` has something to seal.
    unsealed: WorkspaceHandle,
    trace: Commitment,
    other_trace: Commitment,
    receipt: Commitment,
    /// An evidence node the rig ingested, for `evidence.verify` and `evidence.link`.
    node: EvidenceHandle,
    /// A parked campaign and its continuation, for the three `task` verbs.
    task: TaskHandle,
    continuation: ContinuationHandle,
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
                5,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[DataGrant::ProductionTrace],
                )),
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
        // A second `propose` principal at the same level, for the cross-actor scoping probes.
        .capability(
            grant(
                "cap_rival",
                "agent:rival",
                AuthorityLevel::Propose,
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
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                )),
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
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
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
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(ContextFamily)
        .family(WhiteboardFamily)
        .build()
}

/// Stage one file's content out of band.
fn stage(daemon: &mut Daemon, path: &str, content: &str) -> Commitment {
    daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            content.as_bytes().to_vec(),
        )
        .expect("staging names its content")
}

fn components(
    intent: &IntentHandle,
    files: Vec<Commitment>,
    configuration: &Commitment,
) -> SnapshotComponents {
    SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: snapshot_epochs(),
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![configuration.clone()],
        file_components: Optional::Absent,
    }
}

/// Assert a dispatch did not fail, naming what it was.
fn ok(outcome: &OperationOutcome, what: &str) {
    assert!(
        !matches!(outcome.envelope.status, ResultStatus::Error),
        "the rig's {what} must succeed: {:?} {:?}",
        outcome.envelope.status,
        outcome.envelope.error
    );
}

/// The rig every probe runs against, rebuilt from scratch for each one.
///
/// Rebuilding rather than sharing is deliberate: a probe that observed another probe's
/// leftovers would be measuring the sweep's order rather than the ledger.
#[allow(clippy::too_many_lines)]
fn fixture() -> Rig {
    let mut daemon = daemon();

    // Three contracts: one accepted to govern the snapshots, two left proposed so the
    // `intent.accept` and `intent.reject` probes are live attempts rather than refusals on a
    // record's own state.
    let governing_contract = contract(DIE_HARD_CONTRACT);
    let governing = intent_handle(&governing_contract);
    let accept_contract = contract_variant(1);
    let for_accept = intent_handle(&accept_contract);
    let reject_contract = contract_variant(2);
    let for_reject = intent_handle(&reject_contract);
    let interleave_contract = contract_variant(3);
    let for_interleave = intent_handle(&interleave_contract);
    assert_eq!(
        BTreeSet::from([&governing, &for_accept, &for_reject, &for_interleave]).len(),
        4,
        "the four fixtures are four identities"
    );
    for (handle, contract) in [
        (&governing, governing_contract),
        (&for_accept, accept_contract),
        (&for_reject, reject_contract),
        (&for_interleave, interleave_contract),
    ] {
        daemon.state_mut().put_intent(
            handle.clone(),
            IntentRecord {
                contract,
                status: RegistryStatus::Proposed,
                supersedes: None,
                superseded_by: None,
                acceptance: None,
            },
        );
    }

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(stage(&mut daemon, path, content));
    }
    let extra = stage(&mut daemon, "SECOND.md", "# a second file\n");
    let configuration = stage(&mut daemon, "default.model.toml", DIE_HARD_CONFIG);
    let trace = stage(&mut daemon, "traces/die-hard.jsonl", TRACE);
    let other_trace = stage(&mut daemon, "traces/other.jsonl", OTHER_TRACE);
    let receipt = stage(&mut daemon, "evidence/receipt.json", RECEIPT);

    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("the module set is nameable"),
        continuum_engine_reference::diehard::model().expect("the TV-009 port builds"),
    );

    let mut rig = Rig {
        daemon,
        governing,
        for_accept,
        for_reject,
        for_interleave,
        files,
        extra,
        configuration,
        reference: Commitment::new("placeholder"),
        snapshot: WorkspaceHandle::new("ws_placeholder").expect("a placeholder handle"),
        unsealed: WorkspaceHandle::new("ws_placeholder").expect("a placeholder handle"),
        trace,
        other_trace,
        receipt,
        node: EvidenceHandle::new("ev_placeholder").expect("a placeholder handle"),
        task: TaskHandle::new("task_placeholder").expect("a placeholder handle"),
        continuation: ContinuationHandle::new("cont_placeholder").expect("a placeholder handle"),
    };

    // The governing contract is accepted on the wire, because `workspace.create` refuses an
    // unaccepted one.
    let mut accept_envelope = envelope(
        "intent.accept",
        "human:steward",
        "cap_steward",
        "req_rig_accept",
    );
    accept_envelope.idempotency_key = Optional::Present("rig-accept".to_owned());
    let proposal = rig.governing.clone();
    let accepted = rig.daemon.dispatch(&OperationRequest {
        envelope: accept_envelope,
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal,
            acceptance: acceptance_bytes("sig-die-hard-v1"),
            bundle: Optional::Absent,
        }),
    });
    ok(&accepted, "intent.accept");

    // A sealed snapshot for the campaigns, and an unsealed one for `workspace.seal`.
    rig.snapshot = created(&mut rig, "req_rig_sealed", "rig-sealed", true, false);
    rig.unsealed = created(&mut rig, "req_rig_open", "rig-open", false, true);

    // The by-reference lane's argument: the same component set, registered out of band.
    let set = components(&rig.governing, rig.files.clone(), &rig.configuration);
    rig.reference = rig
        .daemon
        .state_mut()
        .register_components(&Blake3Identity, set)
        .expect("blake3 names every input");

    // One evidence node, so `evidence.verify` and `evidence.link` name something real.
    let mut ingest_envelope = envelope(
        "observe.ingest",
        "agent:observer",
        "cap_observer",
        "req_rig_ingest",
    );
    ingest_envelope.idempotency_key = Optional::Present("rig-ingest".to_owned());
    ingest_envelope.budget = Optional::Present(budget(STATE_CEILING));
    let trace = rig.trace.clone();
    let ingested = rig.daemon.dispatch(&OperationRequest {
        envelope: ingest_envelope,
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    ok(&ingested, "observe.ingest");
    rig.node = match &ingested.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names the node it appended"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    };

    // A parked campaign: `states: 4` suspends Die Hard with a continuation, which is what the
    // three `task` verbs need to be live.
    let mut start_envelope = envelope(
        "verification.start",
        "agent:runner",
        "cap_runner",
        "req_rig_start",
    );
    start_envelope.idempotency_key = Optional::Present("rig-start".to_owned());
    start_envelope.budget = Optional::Present(budget(PARK_BUDGET));
    start_envelope.snapshot = Nullable::Value(rig.snapshot.clone());
    let parked = rig.daemon.dispatch(&OperationRequest {
        envelope: start_envelope,
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::Property,
                id: "NotSolved".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    ok(&parked, "verification.start");
    rig.task = parked
        .envelope
        .task
        .value()
        .cloned()
        .expect("a parked campaign names its task");
    rig.continuation = parked
        .envelope
        .continuation
        .value()
        .cloned()
        .expect("a parked campaign names its continuation");
    rig
}

/// Attempt a snapshot creation through the wire, whatever the answer is.
fn try_create(
    rig: &mut Rig,
    request: &str,
    key: &str,
    seal: bool,
    second_file: bool,
) -> OperationOutcome {
    let mut files = rig.files.clone();
    if second_file {
        files.push(rig.extra.clone());
    }
    let set = components(&rig.governing, files, &rig.configuration);
    let mut request_envelope =
        envelope("workspace.create", "agent:builder", "cap_builder", request);
    request_envelope.idempotency_key = Optional::Present(key.to_owned());
    rig.daemon.dispatch(&OperationRequest {
        envelope: request_envelope,
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set,
            overlay: Optional::Absent,
            seal: Optional::Present(seal),
        }),
    })
}

/// Create a snapshot through the wire, and hand back its handle.
fn created(
    rig: &mut Rig,
    request: &str,
    key: &str,
    seal: bool,
    second_file: bool,
) -> WorkspaceHandle {
    let outcome = try_create(rig, request, key, seal, second_file);
    ok(&outcome, "workspace.create");
    match &outcome.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

// =========================================================================================
// D — the probe table: one entry per reachable mutation
// =========================================================================================

/// Which of a probe's two canonical requests to send.
///
/// `B` differs from `A` in `RequestEnvelope.output_policy` and in nothing else. That field is
/// one of the seven `daemon::state::ReplayKey` carries, and it was one of the three bn-h1zqz
/// added, so a daemon that ignored it would answer two different questions with one recorded
/// reply. Applying the same difference to all eighteen operations makes the discrimination a
/// property of the ledger rather than of any one operation's preimage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    A,
    B,
}

/// One reachable mutation, with everything needed to drive it.
struct Probe {
    /// The wire operation name.
    operation: &'static str,
    /// The principal it is driven as.
    actor: &'static str,
    /// That principal's capability.
    capability: &'static str,
    /// The operation's request body, built from the rig's handles.
    arguments: fn(&Rig) -> Arguments,
    /// The envelope fields beyond the common ones — a pinned snapshot, mostly.
    pin: fn(&Rig, RequestEnvelope) -> RequestEnvelope,
}

/// The default envelope shaping: nothing beyond the common fields.
fn unpinned(_: &Rig, envelope: RequestEnvelope) -> RequestEnvelope {
    envelope
}

/// Pin the envelope's `snapshot` to the rig's sealed one.
fn on_snapshot(rig: &Rig, mut envelope: RequestEnvelope) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(rig.snapshot.clone());
    envelope
}

/// The eighteen probes, in the order [`REACHABLE`] lists them.
fn probes() -> Vec<Probe> {
    vec![
        Probe {
            operation: "context.compile",
            actor: "agent:reader",
            capability: "cap_reader",
            arguments: |_| {
                Arguments::ContextCompile(ContextCompileRequest {
                    evidence_root: ArtifactHandle::new(
                        "ev_g103rootaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("a well-formed artifact handle"),
                    question: "does the campaign close?".to_owned(),
                    audience: Optional::Absent,
                    guarantees: Optional::Absent,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "context.expand",
            actor: "agent:reader",
            capability: "cap_reader",
            arguments: |_| {
                Arguments::ContextExpand(ContextExpandRequest {
                    context: ContextHandle::new("ctx_g103packaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                        .expect("a well-formed context handle"),
                    anchor: "anchor-0".to_owned(),
                    relation: ExpansionRelation::CausalPredecessors,
                    depth: Optional::Absent,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "evidence.link",
            actor: "service:kernel-core",
            capability: "cap_checker",
            arguments: |rig| {
                Arguments::EvidenceLink(EvidenceLinkRequest {
                    subject: rig.node.clone(),
                    receipt: rig.receipt.clone(),
                    checker_profile: "kernel-core/1".to_owned(),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "evidence.verify",
            actor: "agent:reader",
            capability: "cap_reader",
            arguments: |rig| {
                Arguments::EvidenceVerify(EvidenceVerifyRequest {
                    evidence: rig.node.clone(),
                    expected_status: Optional::Absent,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "intent.accept",
            actor: "human:steward",
            capability: "cap_steward",
            arguments: |rig| {
                Arguments::IntentAccept(IntentAcceptRequest {
                    proposal: rig.for_accept.clone(),
                    acceptance: acceptance_bytes("sig-probe-accept"),
                    bundle: Optional::Absent,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "intent.lock",
            actor: "human:steward",
            capability: "cap_steward",
            arguments: |rig| {
                let mut policy = BTreeMap::new();
                policy.insert("bounds".to_owned(), "locked".to_owned());
                Arguments::IntentLock(IntentLockRequest {
                    intent: rig.governing.clone(),
                    policy,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "intent.propose_revision",
            actor: "human:steward",
            capability: "cap_steward",
            arguments: |rig| {
                Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
                    base: rig.governing.clone(),
                    changes: IntentChangeSet {
                        changes: Opaque::from_bytes(b"{\"bounds\":{\"faults\":3}}".to_vec()),
                        rationale: "a revision the daemon cannot classify".to_owned(),
                    },
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "intent.reject",
            actor: "human:steward",
            capability: "cap_steward",
            arguments: |rig| {
                Arguments::IntentReject(IntentRejectRequest {
                    proposal: rig.for_reject.clone(),
                    reason: "superseded by the governing contract".to_owned(),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "observe.ingest",
            actor: "agent:observer",
            capability: "cap_observer",
            arguments: |rig| {
                Arguments::ObserveIngest(ObserveIngestRequest {
                    trace: rig.other_trace.clone(),
                    instrumentation_profile: PROFILE.to_owned(),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "task.cancel",
            actor: "agent:runner",
            capability: "cap_runner",
            arguments: |rig| {
                Arguments::TaskCancel(TaskCancelRequest {
                    task: rig.task.clone(),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "task.resume",
            actor: "agent:runner",
            capability: "cap_runner",
            arguments: |rig| {
                Arguments::TaskResume(TaskResumeRequest {
                    continuation: rig.continuation.clone(),
                    budget: Optional::Present(budget(STATE_CEILING)),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "task.update_budget",
            actor: "agent:runner",
            capability: "cap_runner",
            arguments: |rig| {
                Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
                    task: rig.task.clone(),
                    budget: budget(STATE_CEILING),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "verification.start",
            actor: "agent:runner",
            capability: "cap_runner",
            arguments: |_| {
                Arguments::VerificationStart(VerificationStartRequest {
                    target: Target {
                        kind: TargetKind::Property,
                        id: "NotSolved".to_owned(),
                    },
                    portfolio: Portfolio::Interactive,
                    context_policy: Optional::Absent,
                    priority_class: Optional::Absent,
                })
            },
            pin: on_snapshot,
        },
        Probe {
            operation: "whiteboard.compile",
            actor: "agent:builder",
            capability: "cap_builder",
            arguments: |_| {
                Arguments::WhiteboardCompile(WhiteboardCompileRequest {
                    note: Opaque::from_bytes(note_bytes("the jugs measure 4")),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "workspace.create",
            actor: "agent:builder",
            capability: "cap_builder",
            arguments: |rig| {
                let mut files = rig.files.clone();
                files.push(rig.extra.clone());
                files.push(rig.trace.clone());
                Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                    components: components(&rig.governing, files, &rig.configuration),
                    overlay: Optional::Absent,
                    seal: Optional::Present(true),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "workspace.create_by_reference",
            actor: "agent:builder",
            capability: "cap_builder",
            arguments: |rig| {
                Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
                    components: rig.reference.clone(),
                    epochs: snapshot_epochs(),
                    intent: rig.governing.clone(),
                    seal: Optional::Present(true),
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "workspace.fork",
            actor: "agent:builder",
            capability: "cap_builder",
            arguments: |rig| {
                Arguments::WorkspaceFork(WorkspaceForkRequest {
                    // The rig's *latest* create, because a fork advances a lineage and the
                    // guarded advance requires the base to be that lineage's head.
                    base: rig.unsealed.clone(),
                    overlay: Optional::Present(vec![FileOverlay {
                        path: "README.md".to_owned(),
                        content: b"# TV-009, forked\n".to_vec(),
                    }]),
                    // `patches` is `UnsupportedSemanticFeature` on this daemon, and an
                    // unsupported lane would make this probe a ledger-only one.
                    patches: Optional::Absent,
                })
            },
            pin: unpinned,
        },
        Probe {
            operation: "workspace.seal",
            actor: "agent:builder",
            capability: "cap_builder",
            arguments: |rig| {
                Arguments::WorkspaceSeal(WorkspaceSealRequest {
                    snapshot: rig.unsealed.clone(),
                })
            },
            pin: unpinned,
        },
    ]
}

/// The `output_policy` that makes [`Body::B`] a different canonical request.
///
/// `max_tokens` is the IDL's *advisory* ceiling — "bytes remain the enforced contract" — so
/// this difference is as close to semantically inert as a declared field gets. It is still a
/// declared member of `RequestEnvelope` and still a member of `ReplayKey`, which is exactly
/// the point: the ledger must discriminate on it even where the handler would not.
fn variant_policy() -> OutputPolicy {
    OutputPolicy {
        max_bytes: Optional::Absent,
        max_tokens: Optional::Present(4_096),
        max_nodes: Optional::Absent,
        audience: Optional::Absent,
    }
}

/// Build one probe's request.
fn request(rig: &Rig, probe: &Probe, body: Body, request_id: &str, key: &str) -> OperationRequest {
    request_as(
        rig,
        probe,
        body,
        request_id,
        key,
        probe.actor,
        probe.capability,
    )
}

/// Build one probe's request under a chosen principal.
fn request_as(
    rig: &Rig,
    probe: &Probe,
    body: Body,
    request_id: &str,
    key: &str,
    actor: &str,
    capability: &str,
) -> OperationRequest {
    let spec = registry::operation(probe.operation).expect("the registry declares the operation");
    let mut envelope = envelope(probe.operation, actor, capability, request_id);
    envelope.idempotency_key = Optional::Present(key.to_owned());
    if spec.has(Annotation::TaskStarting) {
        envelope.budget = Optional::Present(budget(STATE_CEILING));
    }
    if body == Body::B {
        envelope.output_policy = Optional::Present(variant_policy());
    }
    OperationRequest {
        envelope: (probe.pin)(rig, envelope),
        arguments: (probe.arguments)(rig),
    }
}

// =========================================================================================
// E — the instrument: a fingerprint of the observable world
// =========================================================================================

/// A rendering of everything a caller could observe about this daemon.
///
/// Deliberately not a digest: two fingerprints are compared as ordered lists so that a
/// failure names the line that moved rather than a hash that differs. The five sections are
/// read through five different surfaces, and three of them are *independent queries* — real
/// `@readonly` dispatches, so the answer is what a second client would see rather than what
/// this file can reach into the daemon for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint(Vec<String>);

impl Fingerprint {
    /// Read the world.
    ///
    /// Takes `&mut Daemon` because three of the five sections are dispatches. Those are
    /// `@readonly` operations, which carry no idempotency key and file nothing — the one
    /// thing they *do* move is the admission ledger, which is deliberately not a section
    /// here: RFC 0027 P5 requires a record of every admission decision including a replay's,
    /// so a fingerprint that included it could never be stable and would be measuring the
    /// audit obligation rather than the effect.
    fn of(rig: &mut Rig) -> Self {
        let mut lines = Vec::new();

        // 1. The publication store: every artifact it holds, with its bytes.
        let token = identity::capability_to_store(&cap("cap_root")).expect("a store token");
        let identities = {
            let view = rig
                .daemon
                .store()
                .audit_view(&token)
                .expect("`cap_root` confers audit");
            let mut identities = view.identities();
            identities.sort_by_key(std::string::ToString::to_string);
            identities
        };
        for handle in identities {
            let content = rig
                .daemon
                .store()
                .read(&handle, &token)
                .expect("`cap_root` confers read on a published artifact");
            lines.push(format!("store {handle} {}", hex(&content)));
        }

        // 2. The store's authorization audit log (plan §18.5), restricted to the decisions
        //    that are *effects*. `Read` and `Audit` are excluded because this instrument
        //    itself reads through the capability-checked surface — section 1 above is a
        //    `Read` per artifact and an `Audit` per pass — so including them would make the
        //    fingerprint a record of its own footprint. `Publish` and `Administer` are the
        //    decisions a mutation produces, and every one of them is kept.
        for (index, record) in rig
            .daemon
            .store_audit()
            .records()
            .iter()
            .filter(|record| {
                matches!(
                    record.action(),
                    continuum_workspace::publication::Action::Publish
                        | continuum_workspace::publication::Action::Administer
                )
            })
            .enumerate()
        {
            lines.push(format!("store-audit {index} {record:?}"));
        }

        // 3. The evidence graph, through the daemon's own records...
        for (handle, node) in rig.daemon.state().evidence_nodes() {
            lines.push(format!("node {} {node:?}", handle.as_str()));
        }
        for (handle, edge) in rig.daemon.state().evidence_edges() {
            lines.push(format!("edge {} {edge:?}", handle.as_str()));
        }
        for (index, event) in rig.daemon.state().evidence_events().iter().enumerate() {
            lines.push(format!("evidence-event {index} {event:?}"));
        }

        // 4. ...and the intent registry and the task table.
        let intents: Vec<IntentHandle> = rig
            .daemon
            .state()
            .intents()
            .map(|(handle, _)| handle.clone())
            .collect();
        for (handle, record) in rig.daemon.state().intents() {
            lines.push(format!(
                "intent {} {:?} supersedes={:?} superseded_by={:?} accepted={}",
                handle.as_str(),
                record.status,
                record.supersedes,
                record.superseded_by,
                record.acceptance.is_some()
            ));
        }
        let mut tasks: Vec<TaskHandle> = rig
            .daemon
            .state()
            .tasks()
            .handles()
            .into_iter()
            .cloned()
            .collect();
        tasks.sort_by(|left, right| left.as_str().cmp(right.as_str()));

        // 5. The independent queries: the same facts, asked for on the wire.
        for (index, handle) in intents.iter().enumerate() {
            let answer = rig.daemon.dispatch(&OperationRequest {
                envelope: envelope(
                    "intent.get",
                    "agent:reader",
                    "cap_reader",
                    &format!("req_fp_intent_{index}"),
                ),
                arguments: Arguments::IntentGet(IntentGetRequest {
                    intent: handle.clone(),
                }),
            });
            lines.push(format!(
                "wire intent.get {} {:?} {:?}",
                handle.as_str(),
                answer.envelope.status,
                answer.payload
            ));
        }
        for (index, handle) in tasks.iter().enumerate() {
            let answer = rig.daemon.dispatch(&OperationRequest {
                envelope: envelope(
                    "task.status",
                    "agent:runner",
                    "cap_runner",
                    &format!("req_fp_task_{index}"),
                ),
                arguments: Arguments::TaskStatus(TaskStatusRequest {
                    task: handle.clone(),
                }),
            });
            lines.push(format!(
                "wire task.status {} {:?} {:?}",
                handle.as_str(),
                answer.envelope.status,
                answer.payload
            ));
        }
        let queried = rig.daemon.dispatch(&OperationRequest {
            envelope: envelope(
                "evidence.query",
                "agent:reader",
                "cap_reader",
                "req_fp_query",
            ),
            arguments: Arguments::EvidenceQuery(EvidenceQueryRequest {
                query: EvidenceQuery {
                    node_kinds: Optional::Absent,
                    edge_kinds: Optional::Absent,
                    statuses: Optional::Absent,
                    claim_id: Optional::Absent,
                    roots: Optional::Absent,
                    max_depth: Optional::Absent,
                },
            }),
        });
        lines.push(format!(
            "wire evidence.query {:?} {:?}",
            queried.envelope.status, queried.payload
        ));
        for (index, handle) in rig
            .daemon
            .state()
            .evidence_nodes()
            .map(|(handle, _)| handle.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .enumerate()
        {
            let answer = rig.daemon.dispatch(&OperationRequest {
                envelope: envelope(
                    "evidence.get",
                    "agent:reader",
                    "cap_reader",
                    &format!("req_fp_node_{index}"),
                ),
                arguments: Arguments::EvidenceGet(EvidenceGetRequest {
                    evidence: handle.clone(),
                    inline: Optional::Absent,
                }),
            });
            lines.push(format!(
                "wire evidence.get {} {:?} {:?}",
                handle.as_str(),
                answer.envelope.status,
                answer.payload
            ));
        }

        Self(lines)
    }

    /// The lines this fingerprint has that `other` does not, and the other way round.
    fn delta(&self, other: &Self) -> Vec<String> {
        let mine: BTreeSet<&String> = self.0.iter().collect();
        let theirs: BTreeSet<&String> = other.0.iter().collect();
        mine.symmetric_difference(&theirs)
            .map(|line| (*line).clone())
            .collect()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// =========================================================================================
// F — the fingerprint's own controls, before it is trusted
// =========================================================================================

#[test]
fn control_the_fingerprint_is_stable_under_pure_reads() {
    let mut rig = fixture();
    let first = Fingerprint::of(&mut rig);
    let second = Fingerprint::of(&mut rig);
    assert_eq!(
        first.delta(&second),
        Vec::<String>::new(),
        "reading the world must not change it, or every comparison below is noise"
    );
    assert_eq!(first, second, "and the ordering is stable too");
    assert!(
        first.0.len() > 20,
        "the instrument reads something: {} lines",
        first.0.len()
    );
}

#[test]
fn control_the_fingerprint_detects_a_genuinely_new_artifact() {
    // The anti-vacuity control for every "the fingerprint did not move" assertion in this
    // file: a mutation that really does publish something new must move it.
    let mut rig = fixture();
    let before = Fingerprint::of(&mut rig);
    let created = created(&mut rig, "req_control_new", "control-new", true, true);
    assert_ne!(
        created, rig.snapshot,
        "the control publishes a snapshot the rig did not already hold"
    );
    let after = Fingerprint::of(&mut rig);
    assert_ne!(
        before, after,
        "the instrument must report a genuine second effect"
    );
    assert!(
        !before.delta(&after).is_empty(),
        "and must name the lines that moved"
    );
}

// =========================================================================================
// G — the sweep: every reachable mutation, twice, under one key
// =========================================================================================

/// What one probe's same-key replay did.
#[derive(Debug, Clone)]
struct Replayed {
    /// The operation.
    operation: &'static str,
    /// The first call's status.
    first_status: ResultStatus,
    /// Whether the first call succeeded, i.e. whether the probe is non-vacuous.
    effective: bool,
    /// Whether the replay's outcome equals the first call's, field for field.
    outcome_equal: bool,
    /// Whether the world moved between the two calls.
    world_moved: bool,
}

/// Drive one probe twice under one key, with a *fresh* `request_id` on the retry.
fn replay_once(probe: &Probe) -> Replayed {
    let mut rig = fixture();
    let key = format!("idem-{}", probe.operation.replace('.', "-"));

    let first_request = request(&rig, probe, Body::A, "req_first", &key);
    let first = rig.daemon.dispatch(&first_request);
    let after_first = Fingerprint::of(&mut rig);

    // The retry a real client sends: same key, same canonical request, *new* `request_id`.
    // `ReplayKey`'s own documentation is what licenses the change — "a retry carries a fresh
    // one by construction, so including it would make every genuine replay a conflict".
    let replay_request = request(&rig, probe, Body::A, "req_retry", &key);
    assert_ne!(
        first_request.envelope.request_id, replay_request.envelope.request_id,
        "the retry is a retry, not a resend"
    );
    let replay = rig.daemon.dispatch(&replay_request);
    let after_replay = Fingerprint::of(&mut rig);

    Replayed {
        operation: probe.operation,
        first_status: first.envelope.status,
        effective: first.envelope.status != ResultStatus::Error,
        outcome_equal: first == replay,
        world_moved: after_first != after_replay,
    }
}

#[test]
fn sweep_every_reachable_mutation_is_idempotent_under_one_key() {
    let probes = probes();
    assert_eq!(probes.len(), REACHABLE.len(), "the sweep is complete");

    let results: Vec<Replayed> = probes.iter().map(replay_once).collect();

    let unequal: Vec<&str> = results
        .iter()
        .filter(|result| !result.outcome_equal)
        .map(|result| result.operation)
        .collect();
    assert!(
        unequal.is_empty(),
        "`rule idempotency.replay`: a replay returns the recorded outcome; these did not: \
         {unequal:?}"
    );

    let moved: Vec<&str> = results
        .iter()
        .filter(|result| result.world_moved)
        .map(|result| result.operation)
        .collect();
    assert!(
        moved.is_empty(),
        "exactly-once: a replay moves no observable state; these moved it: {moved:?}"
    );

    // INV-007: how much of the sweep is non-vacuous. A probe whose first call was refused
    // still tests the ledger — the outcome is recorded and returned — but it cannot test
    // exactly-once effect, because there was no effect.
    let effective: Vec<&str> = results
        .iter()
        .filter(|result| result.effective)
        .map(|result| result.operation)
        .collect();
    let refused: Vec<(&str, ResultStatus)> = results
        .iter()
        .filter(|result| !result.effective)
        .map(|result| (result.operation, result.first_status))
        .collect();
    assert_eq!(
        effective.len() + refused.len(),
        REACHABLE.len(),
        "the classification is total"
    );
    assert_eq!(
        effective.len(),
        15,
        "fifteen of the eighteen probes drive a first call that actually does something; \
         the rest are ledger-only probes: {refused:?}"
    );
    assert_eq!(
        refused
            .iter()
            .map(|(operation, _)| *operation)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "context.compile",
            "context.expand",
            "intent.propose_revision"
        ]),
        "the three lanes this daemon answers with a typed refusal rather than a body"
    );
}

#[test]
fn sweep_one_key_with_a_different_request_is_refused_and_changes_nothing() {
    let mut refusals = Vec::new();
    for probe in &probes() {
        let mut rig = fixture();
        let key = format!("idem-{}", probe.operation.replace('.', "-"));

        let first = rig
            .daemon
            .dispatch(&request(&rig, probe, Body::A, "req_first", &key));
        let after_first = Fingerprint::of(&mut rig);

        let clash = rig
            .daemon
            .dispatch(&request(&rig, probe, Body::B, "req_clash", &key));
        let after_clash = Fingerprint::of(&mut rig);

        assert_eq!(
            clash.error_code(),
            Some(ErrorCode::IdempotencyKeyReused),
            "{}: the same key with a different `output_policy` is a different canonical \
             request and MUST be refused; got {:?}",
            probe.operation,
            clash.envelope.error
        );
        assert_eq!(
            clash.payload,
            Payload::None,
            "{}: a refused request has no partial effect",
            probe.operation
        );
        assert_eq!(
            after_first,
            after_clash,
            "{}: the refusal moved the world: {:?}",
            probe.operation,
            after_first.delta(&after_clash)
        );
        assert_ne!(
            first, clash,
            "{}: the refusal is not the recorded answer either",
            probe.operation
        );
        refusals.push(probe.operation);
    }
    assert_eq!(refusals.len(), REACHABLE.len(), "every probe was clashed");
}

#[test]
fn sweep_the_refusal_is_not_vacuous_because_the_variant_body_is_an_ordinary_request() {
    // Without this, `sweep_one_key_with_a_different_request_is_refused_and_changes_nothing`
    // would also pass on a daemon that refused the `B` body for some *other* reason. So the
    // `B` body is driven as the **first** call of a fresh rig, under its own key, and is
    // required to land in the same lane the `A` body lands in when *it* is the first call.
    // Two rigs rather than two calls on one, because most of these operations are stateful
    // and a second call would be measuring the state rather than the body.
    let mut ran = 0_usize;
    for probe in &probes() {
        let key = format!("idem-{}-solo", probe.operation.replace('.', "-"));

        let mut only_a = fixture();
        let alone_a = only_a
            .daemon
            .dispatch(&request(&only_a, probe, Body::A, "req_solo", &key));
        let mut only_b = fixture();
        let alone_b = only_b
            .daemon
            .dispatch(&request(&only_b, probe, Body::B, "req_solo", &key));

        assert_ne!(
            alone_b.error_code(),
            Some(ErrorCode::IdempotencyKeyReused),
            "{}: under its own key the `B` body is an ordinary request",
            probe.operation
        );
        assert_eq!(
            alone_a.envelope.status, alone_b.envelope.status,
            "{}: the two bodies differ only in an advisory output ceiling, so they reach the \
             same lane: {:?} vs {:?}",
            probe.operation, alone_a.envelope.error, alone_b.envelope.error
        );
        assert_eq!(
            alone_a.error_code(),
            alone_b.error_code(),
            "{}: and answer with the same code",
            probe.operation
        );
        ran += 1;
    }
    assert_eq!(ran, REACHABLE.len());
}

// =========================================================================================
// H — the negative control: what a genuine second execution looks like
// =========================================================================================

/// What one probe's *fresh-key* second drive did, beside its same-key replay.
#[derive(Debug, Clone)]
struct Control {
    /// The operation.
    operation: &'static str,
    /// Whether a second execution under a fresh key answered differently.
    answer_differs: bool,
    /// Whether a second execution under a fresh key moved the world.
    world_differs: bool,
}

fn control_once(probe: &Probe) -> Control {
    let mut rig = fixture();
    let first_key = format!("idem-{}-1", probe.operation.replace('.', "-"));
    let second_key = format!("idem-{}-2", probe.operation.replace('.', "-"));

    let first = rig
        .daemon
        .dispatch(&request(&rig, probe, Body::A, "req_one", &first_key));
    let after_first = Fingerprint::of(&mut rig);
    let second = rig
        .daemon
        .dispatch(&request(&rig, probe, Body::A, "req_two", &second_key));
    let after_second = Fingerprint::of(&mut rig);

    Control {
        operation: probe.operation,
        // `request_id` is the one field a fresh-key re-execution is *expected* to differ in,
        // so it is normalized away here: what is being asked is whether the daemon *did*
        // something different, not whether it labelled the answer differently.
        answer_differs: !same_but_for_request_id(&first, &second),
        world_differs: after_first != after_second,
    }
}

/// Whether two outcomes agree once the per-attempt `request_id` is set aside.
fn same_but_for_request_id(left: &OperationOutcome, right: &OperationOutcome) -> bool {
    let mut normalized = right.clone();
    normalized.envelope.request_id = left.envelope.request_id.clone();
    // `audit` is `f(request_id, actor)` by `rule audit.correlation`, so it moves with the
    // identity that was just normalized away and must be normalized with it.
    normalized.envelope.audit = left.envelope.audit.clone();
    *left == normalized
}

/// The five operations whose *answer* changes when the second call really executes.
///
/// Each is stateful in a way the answer reports: an accepted proposal cannot be accepted
/// again, a cancelled task cannot be cancelled again, a lineage that has advanced answers a
/// second fork from the old head differently, and `verification.start` reports a different
/// lane once the first campaign has left `Created`.
const DISTINGUISHED_BY_ANSWER: &[&str] = &[
    "intent.accept",
    "intent.reject",
    "task.cancel",
    "verification.start",
    "workspace.fork",
];

/// The seven whose answer converges but whose *world* moves on a second execution.
///
/// These are the ones only the [`Fingerprint`] can see, and they are the reason this file
/// reads the world instead of trusting the answer: a caller comparing responses alone would
/// conclude that a second `workspace.create` of identical components did nothing, when the
/// store's authorization log records a second `Publish` decision and the evidence graph
/// records a second status write.
const DISTINGUISHED_BY_WORLD: &[&str] = &[
    "evidence.link",
    "evidence.verify",
    "observe.ingest",
    "task.update_budget",
    "workspace.create",
    "workspace.create_by_reference",
    "workspace.seal",
];

#[test]
fn control_a_fresh_key_re_executes_and_the_sweep_says_where_that_is_observable() {
    let results: Vec<Control> = probes().iter().map(control_once).collect();

    let by_answer: BTreeSet<&str> = results
        .iter()
        .filter(|result| result.answer_differs)
        .map(|result| result.operation)
        .collect();
    let by_world: BTreeSet<&str> = results
        .iter()
        .filter(|result| result.world_differs)
        .map(|result| result.operation)
        .collect();

    assert_eq!(
        by_answer,
        DISTINGUISHED_BY_ANSWER.iter().copied().collect(),
        "these operations answer a genuine second execution differently"
    );
    assert_eq!(
        by_world,
        DISTINGUISHED_BY_WORLD.iter().copied().collect(),
        "and these move observable state a second time, which only the fingerprint sees"
    );
    assert!(
        by_answer.is_disjoint(&by_world),
        "the two discriminators happen to partition rather than overlap; if that ever \
         changes the counts below stop being a partition"
    );

    let distinguishable: BTreeSet<&str> = by_answer.union(&by_world).copied().collect();
    assert_eq!(
        distinguishable.len(),
        12,
        "twelve of the eighteen have a detectable second execution"
    );

    // The claim this whole file is for: for every one of those twelve, the *same-key* replay
    // produced the recorded outcome and moved nothing. So on twelve operations the ledger
    // demonstrably short-circuited rather than re-ran, and the equality is evidence rather
    // than a coincidence of convergence.
    for probe in probes()
        .iter()
        .filter(|probe| distinguishable.contains(probe.operation))
    {
        let replayed = replay_once(probe);
        assert!(
            replayed.outcome_equal && !replayed.world_moved,
            "{}: a second execution is detectable here, so the same-key replay's equality is \
             a fact about the ledger",
            probe.operation
        );
    }

    // INV-007, the other half. For the remaining six a second execution is *not* detectable
    // by this instrument, so their same-key equality is consistent with a ledger hit and
    // equally consistent with a convergent re-run. That is an absence in this evidence, not
    // a property of the daemon, and it is named rather than counted as a pass.
    let indistinguishable: BTreeSet<&str> = REACHABLE
        .iter()
        .copied()
        .filter(|operation| !distinguishable.contains(operation))
        .collect();
    assert_eq!(
        indistinguishable,
        BTreeSet::from([
            "context.compile",
            "context.expand",
            "intent.lock",
            "intent.propose_revision",
            "task.resume",
            "whiteboard.compile",
        ]),
        "six operations converge — three of them because this deployment answers them with a \
         typed refusal at all — so the ledger's contribution is unmeasured there"
    );
}

// =========================================================================================
// I — key scoping, in both directions
// =========================================================================================

#[test]
fn scoping_another_actors_key_behaves_as_an_unused_key() {
    // > replaying *another* actor's key MUST behave as an unused key, never as
    // > `IdempotencyKeyReused`. A key that could collide across principals is an existence
    // > oracle.
    // >
    // > — RFC 0026, "Idempotency"
    let probe = named_probe("workspace.create");
    let mut rig = fixture();
    let key = "idem-shared-across-principals";

    let mine = rig.daemon.dispatch(&request_as(
        &rig,
        &probe,
        Body::A,
        "req_mine",
        key,
        "agent:builder",
        "cap_builder",
    ));
    assert_eq!(mine.envelope.status, ResultStatus::Ok);

    // The other actor, same key, a *different* request. If keys were global this would be
    // `IdempotencyKeyReused`, which is the oracle: it would tell `agent:rival` that
    // `agent:builder` had used that key.
    let theirs = rig.daemon.dispatch(&request_as(
        &rig,
        &probe,
        Body::B,
        "req_theirs",
        key,
        "agent:rival",
        "cap_rival",
    ));
    assert_ne!(
        theirs.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "the key is another actor's, so it is unused here"
    );
    assert_eq!(theirs.envelope.status, ResultStatus::Ok);

    // And the same key with the *same* request from the other actor is a fresh execution
    // rather than a cached answer: the two ledgers do not share a record.
    let mut fresh = fixture();
    let first = fresh.daemon.dispatch(&request_as(
        &fresh,
        &probe,
        Body::A,
        "req_a",
        key,
        "agent:builder",
        "cap_builder",
    ));
    let crossed = fresh.daemon.dispatch(&request_as(
        &fresh,
        &probe,
        Body::A,
        "req_b",
        key,
        "agent:rival",
        "cap_rival",
    ));
    assert_eq!(crossed.envelope.status, ResultStatus::Ok);
    assert_eq!(
        crossed.envelope.request_id,
        RequestId::new("req_b").expect("a well-formed request id"),
        "a cross-actor call is answered as its own call, not as the other actor's record \
         (which would echo `req_a`)"
    );
    assert_ne!(
        first.envelope.request_id, crossed.envelope.request_id,
        "the two answers are two answers"
    );
}

#[test]
fn scoping_one_key_may_not_group_two_operations() {
    // > An idempotency key is not a transaction identifier and MUST NOT be reused to group
    // > operations. Each mutation carries its own.
    // >
    // > — RFC 0026, "Idempotency"
    //
    // The observable form of that rule is that the *operation name* is part of the compared
    // canonical request, so one key across two verbs is a conflict rather than a grouping.
    let mut rig = fixture();
    let key = "idem-one-key-two-verbs";
    let create = named_probe("workspace.create");
    let fork = named_probe("workspace.fork");

    let first = rig
        .daemon
        .dispatch(&request(&rig, &create, Body::A, "req_create", key));
    assert_eq!(first.envelope.status, ResultStatus::Ok);
    let before = Fingerprint::of(&mut rig);

    let second = rig
        .daemon
        .dispatch(&request(&rig, &fork, Body::A, "req_fork", key));
    assert_eq!(
        second.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "a second verb under one key is a different canonical request"
    );
    let after = Fingerprint::of(&mut rig);
    assert_eq!(before, after, "and it grouped nothing and started nothing");
}

/// The probe for one operation, by name.
fn named_probe(operation: &str) -> Probe {
    probes()
        .into_iter()
        .find(|probe| probe.operation == operation)
        .expect("the sweep declares a probe for this operation")
}

// =========================================================================================
// J — interleaving: a replay after the world has moved
// =========================================================================================

#[test]
fn interleaving_a_replay_after_other_operations_still_neither_reruns_nor_corrupts() {
    // The delivering evidence holds this for `verification.start`. This holds it for every
    // reachable mutation, and adds the half `dx03` does not measure: that the *interleaved*
    // world is itself unmoved by the replay.
    for probe in &probes() {
        let mut rig = fixture();
        let key = format!("idem-{}-inter", probe.operation.replace('.', "-"));

        let first = rig
            .daemon
            .dispatch(&request(&rig, probe, Body::A, "req_first", &key));

        // Move the world with three unrelated mutations by three different principals, none
        // of which is this probe's own subject. Their individual outcomes are deliberately
        // not asserted — an `intent.lock` probe supersedes the governing contract, so a
        // create after it is legitimately refused — and what *is* asserted is the thing the
        // interleaving is for: that the world moved before the replay ran.
        let quiet = Fingerprint::of(&mut rig);
        let _ = try_create(&mut rig, "req_move_create", "move-create", true, true);
        let mut ingest_envelope = envelope(
            "observe.ingest",
            "agent:observer",
            "cap_observer",
            "req_move_ingest",
        );
        ingest_envelope.idempotency_key = Optional::Present("move-ingest".to_owned());
        ingest_envelope.budget = Optional::Present(budget(STATE_CEILING));
        let other_trace = rig.other_trace.clone();
        let moved_ingest = rig.daemon.dispatch(&OperationRequest {
            envelope: ingest_envelope,
            arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                trace: other_trace,
                instrumentation_profile: PROFILE.to_owned(),
            }),
        });
        ok(&moved_ingest, "interleaved observe.ingest");
        let mut reject_envelope = envelope(
            "intent.reject",
            "human:steward",
            "cap_steward",
            "req_move_reject",
        );
        reject_envelope.idempotency_key = Optional::Present("move-reject".to_owned());
        let for_interleave = rig.for_interleave.clone();
        let moved_reject = rig.daemon.dispatch(&OperationRequest {
            envelope: reject_envelope,
            arguments: Arguments::IntentReject(IntentRejectRequest {
                proposal: for_interleave,
                reason: "moved out of the way".to_owned(),
            }),
        });
        ok(&moved_reject, "interleaved intent.reject");

        let before = Fingerprint::of(&mut rig);
        assert_ne!(
            quiet, before,
            "{}: the interleaving must actually move the world, or the probe is vacuous",
            probe.operation
        );
        let replay = rig
            .daemon
            .dispatch(&request(&rig, probe, Body::A, "req_replay", &key));
        let after = Fingerprint::of(&mut rig);

        assert_eq!(
            first, replay,
            "{}: the ledger returns the recorded outcome whatever happened in between",
            probe.operation
        );
        assert_eq!(
            before,
            after,
            "{}: and the moved world is untouched by the replay: {:?}",
            probe.operation,
            before.delta(&after)
        );
    }
}

// =========================================================================================
// K — the finding: what the replay's own envelope says about which call it answers
// =========================================================================================

#[test]
fn finding_a_replay_echoes_the_first_attempts_request_id() {
    // RFC 0026's `RequestEnvelope` table:
    //
    // > `request_id` | `RequestId` (`^req_…`) | required | client-unique; **echoed in the
    // > result**; a tracing identifier, never an artifact identity
    //
    // and the `ResultEnvelope` row for the same field reads, in full, "echo".
    //
    // `daemon::state::ReplayKey` deliberately excludes `request_id` — "a retry carries a
    // fresh one by construction, so including it would make every genuine replay a conflict"
    // — and step 7 of the dispatch then returns `previous.outcome.clone()`, envelope and
    // all. So the two statements meet at a retry: the client sends `req_retry`, and the
    // answer echoes `req_first`.
    //
    // This assertion describes what the daemon does today, so it passes; a repair flips it,
    // which is the intent. The observable cost is correlation: a client with several
    // requests in flight matches answers to requests by this field, and an answer bearing an
    // identifier it never sent cannot be matched at all.
    let probe = named_probe("workspace.create");
    let mut rig = fixture();
    let key = "idem-echo";

    let first = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_first", key));
    assert_eq!(first.envelope.status, ResultStatus::Ok);
    let replay = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_retry", key));

    assert_eq!(
        replay.envelope.request_id,
        RequestId::new("req_first").expect("a well-formed request id"),
        "today: the replay echoes the first attempt's identity"
    );
    assert_ne!(
        replay.envelope.request_id,
        RequestId::new("req_retry").expect("a well-formed request id"),
        "which is not the identity the retry sent — the echo rule is not kept on this lane"
    );
}

#[test]
fn finding_a_replay_returns_the_first_attempts_audit_correlation() {
    // The same cause, on a field with its own rule:
    //
    // > The identity MUST be a function of the request identity alone — `request_id` and
    // > `actor` — and MUST NOT vary with the outcome, the decision, the presented
    // > capability, or whether a named artifact exists.
    // >
    // > — `rule audit.correlation`
    //
    // A replay returns the recorded envelope, whose `audit` is `f(req_first, actor)`. It is
    // therefore not a function of *this* call's request identity, and a caller that cites it
    // is citing the first attempt's record.
    let probe = named_probe("intent.accept");
    let mut rig = fixture();
    let key = "idem-audit";

    let first = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_first", key));
    assert_eq!(first.envelope.status, ResultStatus::Ok);
    let recorded = first
        .envelope
        .audit
        .value()
        .cloned()
        .expect("`intent.accept` is `@audit_recorded`, so its result names a record");

    let replay = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_retry", key));
    let replayed = replay
        .envelope
        .audit
        .value()
        .cloned()
        .expect("the recorded outcome carries the recorded correlation");
    assert_eq!(
        replayed, recorded,
        "today: the retry is answered with the first attempt's audit correlation"
    );

    // The control that makes the above a defect rather than a coincidence: the same actor
    // sending a *different* `request_id` on a non-replayed call gets a different correlation,
    // so the field really is per-attempt everywhere else.
    let mut fresh = fixture();
    let one = fresh
        .daemon
        .dispatch(&request(&fresh, &probe, Body::A, "req_one", "idem-one"));
    let two = fresh
        .daemon
        .dispatch(&request(&fresh, &probe, Body::A, "req_two", "idem-two"));
    assert_ne!(
        one.envelope.audit.value(),
        two.envelope.audit.value(),
        "two attempts by one actor carry two correlations when neither is a replay"
    );
}

// =========================================================================================
// L — absences, asserted rather than described
// =========================================================================================

#[test]
fn absence_the_retention_window_has_no_reachable_expiry() {
    // > Keys are honored for at least `ServerLimits.idempotency_retention_ms` (default 24h).
    // > Outside the window the daemon MAY treat the key as unused; it MUST NOT return a
    // > different identity for a replay it still remembers.
    // >
    // > — RFC 0026, "Idempotency"
    //
    // This daemon's ledger is a `BTreeMap` with an insert and a lookup and no eviction, and
    // `Services::now` is a single fixed reading supplied at build time rather than a clock.
    // So "outside the window" is not a state this surface can be driven into, and the window
    // is **unprobed**. The assertion below is the honest form of that: the key is still
    // honoured after the longest sequence this file can drive, which is a lower bound on the
    // retention and not a test of the declared one.
    let probe = named_probe("workspace.create");
    let mut rig = fixture();
    let key = "idem-retention";

    let first = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_first", key));
    for index in 0..64 {
        let _ = created(
            &mut rig,
            &format!("req_churn_{index}"),
            &format!("churn-{index}"),
            true,
            index % 2 == 0,
        );
    }
    let replay = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_retry", key));
    assert_eq!(
        first, replay,
        "the key survives every intervening call this file can make; the declared 24h window \
         itself has no reachable expiry and is not tested"
    );
}

#[test]
fn absence_the_ledger_records_a_key_for_a_refused_mutation_too() {
    // Not a defect, and worth stating because it is easy to assume otherwise: the ledger is
    // step 7 and the handler is step 8, so a mutation the *handler* refuses still files its
    // key and its refusal. A retry is answered with the same refusal rather than re-run.
    // That is what `rule idempotency.replay` says — "the same task or artifact identity", not
    // "the same success" — but it means a client cannot clear a bad outcome by retrying under
    // the key it already spent.
    let probe = named_probe("context.compile");
    let mut rig = fixture();
    let key = "idem-refused";

    let first = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_first", key));
    assert_eq!(
        first.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature),
        "this lane has no producer in this deployment"
    );
    assert!(
        rig.daemon.state().replay("agent:reader", key).is_some(),
        "the refusal is filed under the key"
    );
    let replay = rig
        .daemon
        .dispatch(&request(&rig, &probe, Body::A, "req_retry", key));
    assert_eq!(first, replay, "and is what the retry is answered with");
}
