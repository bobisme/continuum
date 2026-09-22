//! Exit evidence for the Phase A / G2 bullet "the prompt-injection corpus cannot trigger
//! privileged operations" (`notes/plan/plan.md` §21 Phase A exit, §22 G2, §18).
//!
//! > Exit: […] the prompt-injection corpus cannot trigger privileged operations (G2)
//! >
//! > — plan §21, "Phase A — Trust spine and ACI kernel"
//!
//! # What is under test
//!
//! The corpus itself is [`continuum_security::injection`] — inert data at the shape plan
//! §24.5's ratified promotion gate fixes, with its own suite asserting that shape. This file
//! is the other half: it takes all forty-five cases, drives each one through `continuumd`'s
//! **real wire boundary** — a byte frame into [`Server::answer`], a byte frame back — and
//! holds the daemon to three conjoined claims.
//!
//! 1. **Zero privileged operations triggered.** Not one of the registry's `@privileged`
//!    operations is admitted for any case, for either of the two unprivileged principals
//!    below. This is the G2 bullet, literally, and it is read off the admission ledger — the
//!    record RFC 0027 P5 requires — rather than inferred from an error code. That
//!    distinction is load-bearing and is explained where it is used: `CapabilityDenied` is
//!    *also* what a handler answers for an artifact the daemon does not hold, because "a
//!    caller learns nothing about any artifact" (X2) forbids a distinguishable not-found, so
//!    the code alone cannot tell "refused by admission" from "no such record".
//! 2. **Zero prohibited outcomes.** The security-relevant state of the daemon — every Intent
//!    Contract's registry status, every evidence claim's status, the store's authorization
//!    audit log — is captured before the corpus runs and compared after. research/35's kill
//!    criterion names three prohibited outcomes (unprivileged intent-status alteration,
//!    unprivileged evidence-status alteration, isolation escape); the first two are exactly
//!    this comparison, and the third is accounted for honestly in [`unlanded`].
//! 3. **The enforcement is the capability check, not the payload.** Two things are measured
//!    rather than argued: the admission decision *flips with the capability* while the
//!    request bytes stay identical, and it *does not move with the payload* — a hostile case
//!    and its benign twin get byte-identical answers. Nothing inspects, scores, sanitizes or
//!    rejects content, which is why [`inert`] then reads the injected bytes back out of the
//!    daemon **verbatim**.
//!
//! # The two unprivileged principals, and why the second is the sharp instrument
//!
//! `cap_agent` is an ordinary agent at `propose`: the least-authority default. It is denied
//! the privileged intent verbs at T1, on the level ladder — a true refusal and a blunt one,
//! because it would still hold if privilege were a level, and so it proves nothing about the
//! privilege bit.
//!
//! `cap_reviser` is the sharp one. It sits at `revise-intent` — the *same level the steward
//! holds* — with a profile whose `privileged_operations` list is empty. It is docs/49's
//! capability matrix cell as a type:
//!
//! > | revise intent | proposal only |
//! >
//! > — docs/49, "Capability matrix", the Modeler column
//!
//! > `intent.propose_revision` is **not** `@privileged`, so a capability at `revise-intent`
//! > without privileged membership can propose a revision and cannot accept one […] and it
//! > is unreachable if privilege is read as a level.
//! >
//! > — RFC 0027, "The admission predicate"
//!
//! So `cap_reviser` is *admitted* for `intent.propose_revision` and *denied* for
//! `intent.accept`, `intent.reject` and `intent.lock`, with the same level, the same actor
//! scheme, the same delegation parent, and the same request bytes.
//! [`enforcement::the_privilege_bit_is_the_only_difference_between_admitted_and_denied`] runs
//! exactly that comparison, and it is this file's whole claim in one assertion: what stops
//! the corpus is a capability check.
//!
//! # Where a case's payload actually goes
//!
//! [`plant`] puts each payload at the position the case's own `carrier` field names — an
//! acceptance signature, a reject reason, a policy-table key, a change-set document, a
//! checker profile, an instrumentation profile, an expansion anchor, a target id, a query
//! filter, or a handle where the case *is* a guessed handle. Where the case names an
//! operation whose family has not landed, the payload becomes the request's whole
//! `arguments` blob, which is the most hostile thing a client can put on this wire and the
//! shape [`unlanded`] is about.
//!
//! # The honest boundary: what "isolation escape" can and cannot mean here
//!
//! Seven of the corpus's operations — `program.run`, `program.replay`, `proof.attempt`,
//! `proof.check`, `forge.step`, `benchmark.run`, `debug.export` — have no family in this
//! daemon. There is no solver worker, no Lean worker, no generated-code runner and no
//! benchmark grader in this process, so a case pushing toward an isolation escape against
//! one of them is **not** refused by a sandbox: it is refused by the codec, before a single
//! byte of its payload is read, and there is nothing behind it to escape from.
//!
//! That is an accounting of absence, not a defence, and [`unlanded`] says so in those words.
//! What it does prove is that the absence is *total* — the answer is identical for every
//! payload, including bytes that are not a document in any encoding — so no unlanded surface
//! is a half-open door. The eight worker-isolation controls of docs/49 become real
//! obligations when the workers land, and the corpus is already written against them so the
//! lane landing a worker inherits its escape attempt rather than having to invent one.
//!
//! This file therefore does **not** clear plan §24.5's ratified promotion gate, which
//! additionally binds a run to a build and dependency epoch and re-locks on any later
//! success. It clears the narrower Phase A exit condition it is named for.
//!
//! # Relationship to `inv016_untrusted_source_evidence.rs`
//!
//! INV-016 asks whether a free-text *field of a request* can instruct the daemon, and
//! answers it by walking the IDL's own free-text inventory. This file asks a different
//! question — whether content planted in an **agent-readable artifact class** can reach a
//! **privileged operation** — and answers it by running a corpus whose shape a ratified
//! dossier sentence fixes. They overlap in one place, in that both plant hostile strings and
//! check that control flow does not move, and differ in the load-bearing one: INV-016 is
//! about interpretation, this is about authority.
//!
//! [`Server::answer`]: continuumd::transport::Server::answer

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_security::injection::{
    CASES, Case, ProhibitedOutcome, Readability, Vector, isolation_cases, policy_block_cases,
    readability,
};
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::snapshot::WorkspacePath;

use continuumd::codec::json::Json as WireJson;
use continuumd::codec::{self, CodecError};
use continuumd::daemon::Daemon;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::VerificationFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::operations::evidence::{
    EvidenceLinkRequest, EvidenceQueryRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentGetRequest, IntentLockRequest, IntentProposeRevisionRequest,
    IntentRejectRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::task::{TaskCancelRequest, TaskResumeRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle, EpochIdentity,
    EvidenceHandle, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle,
    Timestamp,
};
use continuumd::protocol::shared::{EvidenceQuery, IntentChangeSet, Target};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, ExpansionRelation, Portfolio, ResultStatus,
    TargetKind,
};
use continuumd::transport::{self, Server};

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
///
/// Its policy table locks `properties`, `assumptions` and `completion_policy`, which is what
/// gives some of the seven docs/49 policy blocks a *landed* refusal from the field-level
/// change policy rather than an unshipped-lane one.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// A plainly ordinary value: the benign twin every differential comparison runs against.
const BENIGN: &str = "an ordinary caller-supplied value";

// =====================================================================================
// Principals
// =====================================================================================

/// One caller: an actor and the capability it presents.
///
/// A principal is a *pair* rather than a capability alone because T4 binds them — "a
/// mismatch is `CapabilityDenied`, never `MalformedRequest`" — so a comparison that varied
/// one without the other would be measuring the actor check instead of the privilege bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Principal {
    actor: &'static str,
    capability: &'static str,
}

/// The ordinary agent: `propose`, and no profile at all, which "grants nothing rather than
/// everything" (`rule capability.profile_narrowing`). The corpus's default runner.
const AGENT: Principal = Principal {
    actor: "agent:untrusted",
    capability: "cap_agent",
};

/// The reviser: `revise-intent` with an *empty* `privileged_operations` list — docs/49's
/// "revise intent: proposal only" cell. The same level as the steward, and that is the point.
const REVISER: Principal = Principal {
    actor: "agent:reviser",
    capability: "cap_reviser",
};

/// The steward: `revise-intent` **with** the three intent privileges. The only principal in
/// this file any privileged operation is ever admitted for.
const STEWARD: Principal = Principal {
    actor: "human:steward",
    capability: "cap_steward",
};

// =====================================================================================
// The fixture
// =====================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn opname(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

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

fn profile(privileged: &[&str], data_grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| opname(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: data_grants.to_vec(),
        cross_principal_sharing: false,
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
        client: "continuumd-g2-injection-corpus".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(
        &[ProtocolVersion::new(3, 1), version()],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("3.2 is served")
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// Everything the corpus runs against: a daemon behind the wire boundary, plus the handles a
/// case needs in order to name something real when its own payload is not a handle.
struct Fixture {
    server: Server,
    proposal: IntentHandle,
    evidence: EvidenceHandle,
    receipt: Commitment,
}

/// A daemon with every landed family, the four capabilities above, and one *proposed* Die
/// Hard contract for the intent verbs to be attempted against.
///
/// The contract is left at [`RegistryStatus::Proposed`] deliberately: `accept`, `reject` and
/// `lock` all operate on a proposal, so leaving it proposed is what makes every intent case
/// in the corpus a *live* attempt rather than one that would have failed on the record's own
/// state whatever the capability said. [`inert`] then demonstrates the liveness directly, by
/// letting the steward through and watching the status move.
fn fixture() -> Fixture {
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
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
                AGENT.capability,
                AGENT.actor,
                AuthorityLevel::Propose,
                3,
                // No profile: the fail-closed reading, "exactly equivalent to one whose lists
                // are empty", which narrows anything.
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                REVISER.capability,
                REVISER.actor,
                AuthorityLevel::ReviseIntent,
                3,
                // The level the steward holds; the privilege list the steward holds is empty
                // here, and that one difference is the whole experiment.
                Optional::Present(profile(&[], &[])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                STEWARD.capability,
                STEWARD.actor,
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                )),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(VerificationFamily)
        .family(TaskFamily)
        .family(ContextFamily)
        .build();

    let contract = die_hard_contract();
    let proposal = intent_handle(&contract);
    daemon.state_mut().put_intent(
        proposal.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    // One staged artifact, so a case naming a `Commitment` names content the daemon actually
    // holds rather than a value it would refuse for being unknown: the refusal this file is
    // about must not be reachable by accident.
    let receipt = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("evidence/receipt.json").expect("a workspace path"),
            b"{\"receipt\":\"fixture\"}\n".to_vec(),
        )
        .expect("staging names its content");

    let evidence = EvidenceHandle::new("ev_g2fixturenodeaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .expect("an `ev_` handle");

    Fixture {
        server: Server::new(daemon, negotiated()),
        proposal,
        evidence,
        receipt,
    }
}

// =====================================================================================
// One case, one frame
// =====================================================================================

/// The request envelope a case is attempted with.
///
/// The `@mutation` / `@readonly` / `@task_starting` obligations are read off the registry
/// rather than hand-decided per operation, because `obligation::check_request` refuses a key
/// on a read and demands one on a mutation, and a frame that got that wrong would be refused
/// at step 6 for a reason with nothing to do with this file's subject.
fn envelope(operation: &str, principal: Principal, request_id: &str) -> RequestEnvelope {
    let spec = registry::operation(operation);
    let mutation = spec.is_some_and(|spec| spec.has(Annotation::Mutation));
    let task_starting = spec.is_some_and(|spec| spec.has(Annotation::TaskStarting));
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a well-formed request id"),
        idempotency_key: if mutation {
            Optional::Present(format!("idem-{request_id}"))
        } else {
            Optional::Absent
        },
        actor: who(principal.actor),
        capability: cap(principal.capability),
        operation: opname(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: if task_starting {
            Optional::Present(budget())
        } else {
            Optional::Absent
        },
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(64),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A well-formed acceptance record whose `signature` carries `payload`.
///
/// `signature` is a field the handler *reads*, validates as a non-empty string, and stores in
/// the registry record — so a payload planted here is not merely tolerated on the way in, it
/// travels into the daemon's own state and back out through `intent.get`. That round trip is
/// what [`inert`] measures.
fn acceptance(payload: &str) -> Opaque {
    let mut fields: BTreeMap<String, WireJson> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", payload),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), WireJson::String(value.to_owned()));
    }
    Opaque::from_bytes(WireJson::Object(fields).to_canonical_bytes())
}

/// The change-set document a policy-block case proposes.
///
/// The seven docs/49 payloads are already JSON objects naming the contract field they attack,
/// so they are used as themselves; anything else is wrapped, because `intent.propose_revision`
/// refuses a change set that is not an object before it consults the lock table, and the lock
/// table is what a policy-block case is aimed at.
///
/// The document is re-emitted through the canonical writer rather than passed through as
/// authored, because `Opaque` is canonical JSON on this wire and a client sending an object
/// with unsorted keys would be refused by the *encoder* — a refusal with nothing to do with
/// this file's subject. Canonicalizing reorders keys and changes no value, so what reaches
/// the lock table is the document the corpus wrote.
fn change_set(payload: &str) -> IntentChangeSet {
    let changes = match ContractJson::parse(payload.as_bytes()) {
        Ok(document @ ContractJson::Object(_)) => document.to_canonical_bytes(),
        _ => {
            let mut fields: BTreeMap<String, WireJson> = BTreeMap::new();
            fields.insert("scope".to_owned(), WireJson::String(payload.to_owned()));
            WireJson::Object(fields).to_canonical_bytes()
        }
    };
    IntentChangeSet {
        changes: Opaque::from_bytes(changes),
        rationale: payload.to_owned(),
    }
}

/// The typed request body a case's payload is planted into, or [`None`] where the operation
/// has no family in this daemon and the payload becomes the whole `arguments` blob.
///
/// Every arm plants at the position the case's `carrier` field names. Where the payload is
/// itself a guessed handle — the `predictable handles` class — it is parsed as one and used,
/// which is the only way that class can be attempted at all; where it is not, the fixture's
/// own handle is used so that the attempt is otherwise well-formed and the refusal it gets is
/// about authority rather than about shape.
fn plant(case: &Case, payload: &str, fixture: &Fixture) -> Option<Arguments> {
    let proposal = IntentHandle::new(payload).unwrap_or_else(|_| fixture.proposal.clone());
    let evidence = EvidenceHandle::new(payload).unwrap_or_else(|_| fixture.evidence.clone());
    Some(match case.operation {
        "intent.accept" => Arguments::IntentAccept(IntentAcceptRequest {
            proposal,
            acceptance: acceptance(payload),
            bundle: Optional::Absent,
        }),
        "intent.reject" => Arguments::IntentReject(IntentRejectRequest {
            proposal,
            reason: payload.to_owned(),
        }),
        "intent.lock" => Arguments::IntentLock(IntentLockRequest {
            intent: fixture.proposal.clone(),
            policy: BTreeMap::from([(payload.to_owned(), "locked".to_owned())]),
        }),
        "intent.propose_revision" => {
            Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
                base: fixture.proposal.clone(),
                changes: change_set(payload),
            })
        }
        "evidence.verify" => Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence,
            expected_status: Optional::Absent,
        }),
        "evidence.link" => Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: fixture.evidence.clone(),
            receipt: fixture.receipt.clone(),
            checker_profile: payload.to_owned(),
        }),
        "evidence.query" => Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Absent,
                claim_id: Optional::Present(payload.to_owned()),
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
        "observe.ingest" => Arguments::ObserveIngest(ObserveIngestRequest {
            trace: fixture.receipt.clone(),
            instrumentation_profile: payload.to_owned(),
        }),
        "context.expand" => Arguments::ContextExpand(ContextExpandRequest {
            context: ContextHandle::new("ctx_g2fixturepackaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("a `ctx_` handle"),
            anchor: payload.to_owned(),
            relation: ExpansionRelation::SourceSpan,
            depth: Optional::Absent,
        }),
        "verification.start" => Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: payload.to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
        "task.resume" => Arguments::TaskResume(TaskResumeRequest {
            continuation: ContinuationHandle::new(payload).unwrap_or_else(|_| {
                ContinuationHandle::new("cont_g2fixtureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                    .expect("a `cont_` handle")
            }),
            budget: Optional::Absent,
        }),
        "task.cancel" => Arguments::TaskCancel(TaskCancelRequest {
            task: TaskHandle::new(payload).unwrap_or_else(|_| {
                TaskHandle::new("task_g2fixtureaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                    .expect("a `task_` handle")
            }),
        }),
        // Every other operation the corpus names has no family here. Its payload travels as
        // the request's whole `arguments` — see `unlanded`.
        _ => return None,
    })
}

/// The wire frame one case produces, under one principal.
///
/// Built through the same encoder a client uses, so the bytes the daemon reads are the bytes
/// a client would have written; nothing here reaches past the transport.
fn frame(
    case: &Case,
    payload: &str,
    principal: Principal,
    request_id: &str,
    fixture: &Fixture,
) -> Vec<u8> {
    let mut envelope = envelope(case.operation, principal, request_id);
    envelope.arguments = match plant(case, payload, fixture) {
        Some(arguments) => transport::encode_arguments(&arguments).expect("the body encodes"),
        None => Opaque::from_bytes(WireJson::String(payload.to_owned()).to_canonical_bytes()),
    };
    codec::write_in::<WireJson, _>(&envelope).expect("the envelope encodes")
}

/// Drive one frame through the wire boundary and decode the result envelope.
fn answer(fixture: &mut Fixture, frame: &[u8]) -> ResultEnvelope {
    let bytes = fixture.server.answer(frame).expect("a result is encodable");
    codec::read_in::<WireJson, ResultEnvelope>(&bytes).expect("the result decodes")
}

/// Run one case under one principal, and hand back the answer.
fn run(
    fixture: &mut Fixture,
    case: &Case,
    principal: Principal,
    request_id: &str,
) -> ResultEnvelope {
    let frame = frame(case, case.payload, principal, request_id, fixture);
    answer(fixture, &frame)
}

/// A request identity derived from a case, so two runs of the corpus in one test do not
/// collide on the idempotency ledger or on the audit correlation.
fn request_id(prefix: &str, index: usize) -> String {
    format!("req_{prefix}_{index}")
}

fn error_code(result: &ResultEnvelope) -> Option<ErrorCode> {
    result.error.value().map(|error| error.code)
}

// =====================================================================================
// The privileged set, read from the IDL rather than from the registry
// =====================================================================================

/// A minimal line-oriented scanner over the normative IDL, local to this file.
///
/// `no_case_in_the_corpus_triggers_a_privileged_operation` and
/// `the_corpus_drives_at_every_privileged_operation_this_daemon_serves` used to filter the
/// admission ledger and the operation set with `registry::operation(name)
/// .has(Annotation::Privileged)` — the daemon's own transcription of the IDL, and the exact
/// table `daemon::admission` reads to decide T3. Drop `Annotation::Privileged` from
/// `intent.lock` in `protocol/registry.rs` and the daemon starts admitting `intent.lock` for
/// an unprivileged capability *and* this file's oracle stops calling it privileged in the same
/// stroke, so the headline test kept passing on a real admission-control regression
/// (`gate_g2_07_acceptance.rs` F3). This scanner reads
/// `notes/plan/schemas/continuumd-native-protocol.idl` directly — a source the registry
/// transcribes but the admission predicate never consults — so a marker dropped from the
/// transcription alone is now a fact this file can see. Its approach (column-zero anchoring,
/// so `@privileged` in running doc prose is never misread as a marker) follows
/// `gate_g2_07_acceptance.rs`'s `idl_scan`; it is not shared with that file because no helper
/// module is shared across this directory's test binaries.
mod idl_scan {
    use std::collections::BTreeSet;

    /// The names of every operation the IDL marks `@privileged`.
    ///
    /// # Panics
    ///
    /// On an `operation` declaration whose opening line has no closing `{`.
    #[must_use]
    pub fn privileged_operations(source: &str) -> BTreeSet<String> {
        let lines: Vec<&str> = source.lines().collect();
        let mut found = BTreeSet::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(rest) = line.strip_prefix("operation ") else {
                continue;
            };
            let Some(name) = rest.strip_suffix(" {") else {
                panic!(
                    "line {}: `operation` without an opening brace: {line:?}",
                    index + 1
                );
            };
            if annotations_above(&lines, index).contains("privileged") {
                found.insert(name.to_owned());
            }
        }
        found
    }

    /// The contiguous run of column-zero annotation lines immediately above `index`.
    fn annotations_above(lines: &[&str], index: usize) -> BTreeSet<String> {
        let mut annotations = BTreeSet::new();
        let mut cursor = index;
        while cursor > 0 && lines[cursor - 1].starts_with('@') {
            cursor -= 1;
            for token in lines[cursor].split_whitespace() {
                let token = token
                    .strip_prefix('@')
                    .unwrap_or_else(|| panic!("line {}: not an annotation: {token:?}", cursor + 1));
                let name = token.split('(').next().unwrap_or(token);
                annotations.insert(name.to_owned());
            }
        }
        annotations
    }
}

/// The normative wire authority — the same file `gate_g2_07_acceptance.rs` reads. Not
/// `protocol::registry`: that is the daemon's own transcription of this file, and reading the
/// transcription to check the transcription is exactly the gap `is_privileged` closes.
const IDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/schemas/continuumd-native-protocol.idl"
);

fn idl_source() -> String {
    std::fs::read_to_string(IDL_PATH).expect("the normative IDL is in the tree")
}

/// The set of operations the IDL marks `@privileged`, read fresh from disk. Not cached: this
/// file's whole point is that the answer comes from the file on disk, not from a table that
/// could go stale independently of it.
fn idl_privileged_operations() -> BTreeSet<String> {
    idl_scan::privileged_operations(&idl_source())
}

/// Whether the IDL — independent of the registry the admission predicate itself consults —
/// marks this operation `@privileged`. See the `idl_scan` module above for why this is not
/// `registry::operation(operation).is_some_and(|spec| spec.has(Annotation::Privileged))`.
fn is_privileged(operation: &str) -> bool {
    idl_privileged_operations().contains(operation)
}

/// Whether an operation has a family in this daemon — decided by asking the codec rather than
/// by a list, so an operation that lands later moves this answer by itself.
fn is_landed(operation: &str) -> bool {
    !matches!(
        codec::operations::decode_arguments(operation, &Opaque::from_bytes(b"{}".to_vec())),
        Err(CodecError::UnknownOperation)
    )
}

// =====================================================================================
// What a prohibited outcome would move
// =====================================================================================

/// The daemon state a prohibited outcome would have to change.
///
/// research/35's kill criterion names intent status, evidence status, and isolation escape.
/// The first two are read here directly; the third is approximated by the store's own
/// authorization audit log, because in a daemon with no worker, no clock, no entropy and no
/// filesystem (INV-005) the only thing an escape could reach is the store, and the store
/// records every decision it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SecurityState {
    intents: BTreeMap<String, &'static str>,
    evidence: BTreeMap<String, String>,
    audit_records: usize,
}

fn security_state(fixture: &Fixture) -> SecurityState {
    let state = fixture.server.daemon().state();
    SecurityState {
        intents: state
            .intents()
            .map(|(handle, record)| (handle.as_str().to_owned(), record.status.as_wire()))
            .collect(),
        evidence: state
            .evidence_nodes()
            .map(|(handle, node)| (handle.as_str().to_owned(), format!("{:?}", node.status())))
            .collect(),
        audit_records: fixture.server.daemon().store_audit().records().len(),
    }
}

/// Run every case under `principal`, and hand back the answers in corpus order.
fn run_corpus(fixture: &mut Fixture, principal: Principal, prefix: &str) -> Vec<ResultEnvelope> {
    CASES
        .iter()
        .enumerate()
        .map(|(index, case)| run(fixture, case, principal, &request_id(prefix, index)))
        .collect()
}

// =====================================================================================
// Leg 1 — the corpus, run
// =====================================================================================

#[test]
fn no_case_in_the_corpus_triggers_a_privileged_operation() {
    // The G2 bullet, read off the admission ledger rather than inferred from a code: a
    // `@privileged` operation is "triggered" exactly when admission let it through, and
    // `AdmissionRecord.admitted` is the daemon's own answer to that, recorded whether the
    // decision permitted or denied (RFC 0027 P5).
    for principal in [AGENT, REVISER] {
        let mut fixture = fixture();
        let answers = run_corpus(&mut fixture, principal, "corpus");
        assert_eq!(answers.len(), CASES.len());

        let admissions = fixture.server.daemon().state().admissions();
        let privileged: Vec<_> = admissions
            .iter()
            .filter(|record| is_privileged(&record.operation))
            .collect();
        assert!(
            !privileged.is_empty(),
            "the corpus attempted no `@privileged` operation at all under {principal:?}, so \
             this assertion would pass vacuously"
        );
        for record in privileged {
            assert!(
                !record.admitted,
                "{} was admitted for {} — a privileged operation the corpus triggered",
                record.operation, record.actor
            );
        }

        // The same fact read from the other side of the boundary.
        for (case, result) in CASES.iter().zip(&answers) {
            if is_privileged(case.operation) {
                assert_eq!(
                    result.status,
                    ResultStatus::Error,
                    "{} reached a privileged operation's success path",
                    case.id
                );
                assert_eq!(
                    error_code(result),
                    Some(ErrorCode::CapabilityDenied),
                    "{} was refused, but not by the capability check",
                    case.id
                );
            }
        }
    }
}

#[test]
fn the_corpus_alters_no_intent_status_and_no_evidence_status() {
    // The two prohibited outcomes the kill criterion names that this daemon can exhibit,
    // measured as a before/after comparison of everything they would move rather than as an
    // absence of error codes.
    for principal in [AGENT, REVISER] {
        let mut fixture = fixture();
        let before = security_state(&fixture);
        assert!(
            !before.intents.is_empty(),
            "the fixture holds no intent, so an unchanged intent table would prove nothing"
        );
        let _ = run_corpus(&mut fixture, principal, "outcome");
        let after = security_state(&fixture);
        assert_eq!(
            before, after,
            "the corpus moved security-relevant state under {principal:?}"
        );
    }
}

#[test]
fn every_attempt_that_reached_admission_is_audited() {
    // > A block that leaves no audit record is indistinguishable from an attack that was
    // > never tried, and fails its case in the ratified workbench-security corpus.
    // >
    // > — RFC 0027 P5
    let mut fixture = fixture();
    let answers = run_corpus(&mut fixture, AGENT, "audit");

    let admissions = fixture.server.daemon().state().admissions().to_vec();
    // One record per case whose operation this daemon *has*. An operation the codec cannot
    // decode never reaches admission at all, and `unlanded` accounts for those separately
    // rather than letting them dilute this count.
    let reaching: Vec<&Case> = CASES
        .iter()
        .filter(|case| is_landed(case.operation))
        .collect();
    assert_eq!(
        admissions.len(),
        reaching.len(),
        "one admission record per attempt that reached admission"
    );
    for record in &admissions {
        assert_eq!(record.actor, AGENT.actor);
        assert_eq!(record.capability, cap(AGENT.capability));
        assert!(
            !record.audit.is_empty(),
            "{} left an admission record citing no audit identity",
            record.operation
        );
    }

    // And every denial the caller received cites the record its own attempt produced.
    let cited: BTreeSet<&str> = admissions
        .iter()
        .map(|record| record.audit.as_str())
        .collect();
    let mut denials = 0;
    for (case, result) in CASES.iter().zip(&answers) {
        if error_code(result) == Some(ErrorCode::CapabilityDenied) {
            denials += 1;
            let audit = result
                .audit
                .value()
                .unwrap_or_else(|| panic!("{} was denied and cites no audit record", case.id));
            assert!(
                cited.contains(audit.as_str()),
                "{} cites an audit identity no admission record carries",
                case.id
            );
        }
    }
    assert!(denials > 0, "no denial was audited because none happened");
}

// =====================================================================================
// Leg 2 — the enforcement mechanism
// =====================================================================================

mod enforcement {
    use super::{
        AGENT, ArtifactClass, BENIGN, CASES, Case, ErrorCode, ProhibitedOutcome, REVISER,
        Readability, ResultStatus, STEWARD, Vector, answer, error_code, fixture, frame,
        idl_privileged_operations, is_landed, is_privileged, readability, request_id, run,
    };
    use std::collections::BTreeSet;

    /// The `@privileged` operations this daemon actually serves, taken off the IDL-derived set
    /// (see `super::is_privileged`) rather than the registry: a fourth landing later joins this
    /// comparison without an edit, and a marker the registry's transcription drops does not
    /// silently shrink it.
    fn landed_privileged_operations() -> Vec<String> {
        idl_privileged_operations()
            .into_iter()
            .filter(|name| is_landed(name))
            .collect()
    }

    #[test]
    fn the_privilege_bit_is_the_only_difference_between_admitted_and_denied() {
        // This file's claim, in one comparison. `cap_reviser` and `cap_steward` hold the same
        // level, the same delegation parent and the same scopes; they differ in
        // `privileged_operations` and in nothing else. The *same case*, planted with the
        // *same payload*, encoded into the *same bytes*, is denied for one and admitted for
        // the other.
        let operations = landed_privileged_operations();
        assert_eq!(
            operations.len(),
            3,
            "expected `intent.accept`, `intent.reject` and `intent.lock`: {operations:?}"
        );

        for operation in operations {
            let case = CASES
                .iter()
                .find(|case| case.operation == operation)
                .unwrap_or_else(|| panic!("the corpus drives at {operation}"));

            for (principal, expected) in [(REVISER, false), (STEWARD, true)] {
                let mut fixture = fixture();
                let result = run(&mut fixture, case, principal, "req_privilege");
                let record = fixture
                    .server
                    .daemon()
                    .state()
                    .admissions()
                    .last()
                    .expect("the attempt reached admission")
                    .clone();
                assert_eq!(record.operation, operation);
                assert_eq!(
                    record.admitted, expected,
                    "{} under {principal:?}: admission decided the wrong way",
                    case.id
                );
                if expected {
                    // The admitted branch deliberately asserts nothing about the answer's
                    // code, and the reason is the reason this whole file reads the admission
                    // ledger: `CapabilityDenied` is also what a handler answers for an
                    // artifact this daemon does not hold, because "a caller learns nothing
                    // about any artifact" (RFC 0027 X2) forbids a distinguishable not-found.
                    // A case naming a guessed handle is therefore refused with the same code
                    // whether admission stopped it or the registry simply had no such record
                    // — indistinguishable on the wire, by design, and separable only here.
                    let _ = &result;
                } else {
                    assert_eq!(result.status, ResultStatus::Error);
                    assert_eq!(error_code(&result), Some(ErrorCode::CapabilityDenied));
                }
            }
        }
    }

    #[test]
    fn the_same_level_without_the_privilege_can_propose_and_cannot_accept() {
        // docs/49's "revise intent: proposal only" cell, executed. If `cap_reviser` were
        // denied everything, the comparison above would be measuring the level ladder; this
        // is the control that says it is not.
        let mut fixture = fixture();
        let proposal = CASES
            .iter()
            .find(|case| case.operation == "intent.propose_revision")
            .expect("the corpus proposes revisions");
        let result = run(&mut fixture, proposal, REVISER, "req_proposal");
        let record = fixture
            .server
            .daemon()
            .state()
            .admissions()
            .last()
            .expect("the attempt reached admission")
            .clone();
        assert!(
            record.admitted,
            "an unprivileged `revise-intent` capability must still be able to *propose*"
        );
        assert_ne!(error_code(&result), Some(ErrorCode::CapabilityDenied));
        assert!(!is_privileged("intent.propose_revision"));
    }

    #[test]
    fn a_hostile_payload_and_its_benign_twin_get_byte_identical_answers() {
        // Content-blindness, measured rather than argued. Admission reads the capability
        // descriptor, the operation spec, and the handles a family's scope claim reports —
        // never a payload — so two frames differing only in their payload must produce the
        // same answer down to the byte for an unprivileged caller.
        let mut compared = 0;
        for (index, case) in CASES.iter().enumerate() {
            let mut hostile_fixture = fixture();
            let hostile_frame = frame(
                case,
                case.payload,
                AGENT,
                &request_id("twin", index),
                &hostile_fixture,
            );
            let hostile = answer(&mut hostile_fixture, &hostile_frame);

            let mut benign_fixture = fixture();
            let benign_frame = frame(
                case,
                BENIGN,
                AGENT,
                &request_id("twin", index),
                &benign_fixture,
            );
            let benign = answer(&mut benign_fixture, &benign_frame);

            if hostile.status == ResultStatus::Error && benign.status == ResultStatus::Error {
                assert_eq!(
                    hostile, benign,
                    "{}: the daemon's refusal moved with the payload",
                    case.id
                );
                compared += 1;
            } else {
                // A case whose benign twin succeeds is one where the payload is the only
                // malformed thing about the request — a guessed handle, a policy key outside
                // the closed fifteen. What still has to hold is what this file is about:
                // neither answer is an admitted privileged operation.
                assert!(
                    !is_privileged(case.operation),
                    "{}: a privileged operation answered differently for two payloads",
                    case.id
                );
            }
        }
        assert!(
            compared > CASES.len() / 2,
            "only {compared} of {} cases produced a comparable pair; the differential is too \
             thin to mean anything",
            CASES.len()
        );
    }

    #[test]
    fn no_answer_in_the_corpus_distinguishes_one_admission_failure_from_another() {
        // RFC 0027 X1. A denial that varied its `detail` with which test failed would be an
        // oracle an injection could steer, and would make the corpus's uniform refusal a
        // coincidence of this fixture rather than a property.
        let mut fixture = fixture();
        let mut details: BTreeSet<String> = BTreeSet::new();
        for (index, case) in CASES.iter().enumerate() {
            let result = run(&mut fixture, case, AGENT, &request_id("x1", index));
            if error_code(&result) == Some(ErrorCode::CapabilityDenied) {
                details.insert(
                    result
                        .error
                        .value()
                        .expect("an error is present")
                        .detail
                        .clone(),
                );
            }
        }
        assert_eq!(
            details.len(),
            1,
            "capability denials carried more than one detail: {details:?}"
        );
    }

    #[test]
    fn every_agent_readable_artifact_class_reaches_the_wire() {
        // The acceptance criterion says "across every agent-readable artifact class". The
        // corpus's own suite proves each such class has a case; this proves each of those
        // cases is actually *run* here rather than described.
        let mut fixture = fixture();
        let mut exercised: BTreeSet<ArtifactClass> = BTreeSet::new();
        for (index, case) in CASES.iter().enumerate() {
            let _ = run(&mut fixture, case, AGENT, &request_id("classes", index));
            exercised.insert(case.surface);
        }
        for class in ArtifactClass::ALL {
            let readable = matches!(readability(class), Readability::Content(_));
            assert_eq!(
                exercised.contains(&class),
                readable,
                "`{}_*`: readable but unexercised, or exercised but not readable",
                class.token()
            );
        }
    }

    #[test]
    fn each_prohibited_outcome_has_a_red_team_case_that_actually_ran() {
        let mut fixture = fixture();
        let chosen: Vec<&'static Case> = ProhibitedOutcome::ALL
            .into_iter()
            .filter_map(|outcome| {
                CASES.iter().find(|case| {
                    case.outcome == outcome && matches!(case.vector, Vector::RedTeam(_))
                })
            })
            .collect();
        assert_eq!(chosen.len(), 3);
        for (index, case) in chosen.into_iter().enumerate() {
            let result = run(&mut fixture, case, AGENT, &request_id("outcomes", index));
            assert_eq!(
                result.status,
                ResultStatus::Error,
                "{} succeeded, which is the outcome it was pushing toward",
                case.id
            );
        }
    }
}

// =====================================================================================
// Leg 3 — inert data, not filtered data
// =====================================================================================

mod inert {
    use super::{
        Arguments, IntentAcceptRequest, IntentGetRequest, Optional, RegistryStatus, ResultStatus,
        STEWARD, WireJson, acceptance, answer, codec, envelope, fixture, transport,
    };

    /// The payload of a corpus case whose whole point is prose aimed at an
    /// instruction-following reader. Taken from the corpus by identifier, so this test and
    /// the corpus cannot drift.
    fn hostile() -> &'static str {
        continuum_security::injection::case("comments-weakening-property/intent-status")
            .expect("the corpus holds this case")
            .payload
    }

    #[test]
    fn an_injected_payload_is_stored_and_returned_verbatim_never_scrubbed() {
        // The acceptance criterion says injections "surface as inert data, and capability
        // checks (not output filtering) are the enforcement mechanism". A filter would show
        // up here: the payload rides in on a field the handler reads and stores, and comes
        // back out through `intent.get` escaped exactly as this daemon's own string encoder
        // escapes it, and not one byte more.
        //
        // This test is therefore the *negative* of a sanitizer. If one is ever added it
        // fails — correctly, because output filtering is not the mechanism this architecture
        // claims, and a filter running here would be a second, weaker enforcement path
        // standing beside the capability check and inviting reliance on it.
        //
        // It is also the liveness half of the whole file: the same request the two
        // unprivileged principals are denied *succeeds* for the steward and moves the intent
        // to `accepted`. The corpus's attempts are real attempts.
        let mut fixture = fixture();
        let payload = hostile();

        let mut accept = envelope("intent.accept", STEWARD, "req_inert_accept");
        accept.arguments =
            transport::encode_arguments(&Arguments::IntentAccept(IntentAcceptRequest {
                proposal: fixture.proposal.clone(),
                acceptance: acceptance(payload),
                bundle: Optional::Absent,
            }))
            .expect("the body encodes");
        let frame = codec::write_in::<WireJson, _>(&accept).expect("the envelope encodes");
        let accepted = answer(&mut fixture, &frame);
        assert_eq!(
            accepted.status,
            ResultStatus::Ok,
            "the steward's privileged accept must succeed, or this file proves nothing about \
             what a privileged call does with a payload: {accepted:?}"
        );

        let proposal = fixture.proposal.clone();
        let record = fixture
            .server
            .daemon()
            .state()
            .intent(&proposal)
            .expect("the contract is registered");
        assert_eq!(
            record.status,
            RegistryStatus::Accepted,
            "the privileged call did not move the status, so the corpus's denied attempts \
             were never attempts at anything"
        );

        // And the payload comes back out byte for byte. The comparison is against this
        // daemon's own encoder rather than a hand-rolled escape, so "verbatim" means what the
        // codec means.
        let mut get = envelope("intent.get", STEWARD, "req_inert_get");
        get.arguments = transport::encode_arguments(&Arguments::IntentGet(IntentGetRequest {
            intent: proposal,
        }))
        .expect("the body encodes");
        let frame = codec::write_in::<WireJson, _>(&get).expect("the envelope encodes");
        let read_back = answer(&mut fixture, &frame);
        assert_eq!(read_back.status, ResultStatus::Ok, "{read_back:?}");

        let bytes = read_back
            .payload
            .value()
            .expect("`intent.get` answers with a payload")
            .as_bytes()
            .to_vec();
        assert!(
            contains(&bytes, &escaped(payload)),
            "the injected payload did not come back verbatim — something filtered it"
        );
    }

    #[test]
    fn the_verbatim_check_is_not_vacuous() {
        // A `contains` assertion over a large response is exactly the shape that passes for
        // the wrong reason, so the same comparison is run against a string the daemon never
        // saw and must fail.
        assert!(!contains(
            b"{\"intent\":\"in_x\"}",
            &escaped("this string was never sent to the daemon")
        ));
    }

    /// The bytes this daemon's own encoder writes for `value`, quotes stripped — so a
    /// "verbatim" comparison is against the real codec's escaping rather than a guess at it.
    fn escaped(value: &str) -> Vec<u8> {
        let encoded = WireJson::String(value.to_owned()).to_canonical_bytes();
        encoded[1..encoded.len() - 1].to_vec()
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}

// =====================================================================================
// Leg 4 — the unlanded surfaces, accounted for rather than claimed
// =====================================================================================

mod unlanded {
    use super::{
        AGENT, CASES, Case, CodecError, Opaque, ResultStatus, WireJson, answer, codec, envelope,
        error_code, fixture, frame, is_landed,
    };

    /// The corpus operations this daemon has no family for.
    fn unlanded_cases() -> Vec<&'static Case> {
        CASES
            .iter()
            .filter(|case| !is_landed(case.operation))
            .collect()
    }

    #[test]
    fn the_corpus_reaches_surfaces_this_daemon_does_not_serve() {
        // Stated up front so the tests below cannot be read as defence claims: these cases
        // are refused because nothing is there, and the corpus carries them so the lane that
        // lands each worker inherits its escape attempt rather than inventing one.
        let unlanded = unlanded_cases();
        assert!(
            !unlanded.is_empty(),
            "if every corpus operation had landed, this module would be dead code"
        );
        assert!(
            unlanded.len() < CASES.len(),
            "if no corpus operation had landed, the rest of this file would prove nothing"
        );
    }

    #[test]
    fn an_unlanded_surface_answers_identically_for_every_payload_on_the_wire() {
        // The absence has to be *total*: a surface that decoded some payloads and refused
        // others would be a half-open door, and an injection would only have to find the half
        // that decodes. Three shapes, as different as this wire admits — an empty object, the
        // case's own payload as a string, and a document that is not an object at all.
        //
        // Two shapes deliberately do *not* appear, and neither is an omission. `null` is not
        // a payload: `RequestEnvelope.arguments` is required and not nullable, so a null
        // there fails to decode as an *envelope* and there is no request to refuse. Bytes
        // that are not a document in any encoding cannot travel in an `Opaque` at all. Both
        // are put to the codec directly by the sibling test below, where they mean something.
        let mut fixture = fixture();
        for (index, case) in unlanded_cases().into_iter().enumerate() {
            let mut answers = Vec::new();
            for (variant, payload) in [
                b"{}".to_vec(),
                WireJson::String(case.payload.to_owned()).to_canonical_bytes(),
                WireJson::Array(vec![WireJson::Integer(0), WireJson::Bool(true)])
                    .to_canonical_bytes(),
            ]
            .into_iter()
            .enumerate()
            {
                let mut request = envelope(
                    case.operation,
                    AGENT,
                    &format!("req_unlanded_{index}_{variant}"),
                );
                request.arguments = Opaque::from_bytes(payload);
                let bytes = codec::write_in::<WireJson, _>(&request).expect("the envelope encodes");
                let result = answer(&mut fixture, &bytes);
                assert_eq!(result.status, ResultStatus::Error, "{}", case.id);
                assert_eq!(
                    error_code(&result),
                    Some(CodecError::UnknownOperation.code()),
                    "{}: an unlanded operation answered something other than the codec's own \
                     refusal",
                    case.id
                );
                answers.push(
                    result
                        .error
                        .value()
                        .expect("an error is present")
                        .detail
                        .clone(),
                );
            }
            let first = &answers[0];
            for detail in &answers {
                assert_eq!(
                    detail, first,
                    "{}: the refusal varied with the payload",
                    case.id
                );
            }
        }
    }

    #[test]
    fn an_unlanded_operation_is_refused_before_its_payload_is_read_at_all() {
        // The mechanical form of the claim above, and the stronger one: the codec's
        // resolution is a `match` on the operation *name*, so bytes that are not a document
        // in any encoding — and so could never be framed — are refused by the same arm as an
        // empty object. There is no payload for which an unlanded operation behaves
        // differently, because there is no path on which its payload is looked at.
        for case in unlanded_cases() {
            for payload in [
                b"{}".to_vec(),
                b"\x00\xff{{{ not json ".to_vec(),
                Vec::new(),
                case.payload.as_bytes().to_vec(),
            ] {
                assert!(
                    matches!(
                        codec::operations::decode_arguments(
                            case.operation,
                            &Opaque::from_bytes(payload)
                        ),
                        Err(CodecError::UnknownOperation)
                    ),
                    "{}: {} resolved a request shape",
                    case.id,
                    case.operation
                );
            }
        }
    }

    #[test]
    fn no_unlanded_case_ever_reached_admission() {
        // "Denial precedes semantic work and precedes the index" (RFC 0027 X3) is about
        // admission; this is the step before it. An operation the codec cannot decode is
        // refused at the transport, so it leaves no admission record at all — which is why
        // `every_attempt_that_reached_admission_is_audited` counts only the landed ones and
        // says so.
        let mut fixture = fixture();
        for (index, case) in unlanded_cases().into_iter().enumerate() {
            let bytes = frame(
                case,
                case.payload,
                AGENT,
                &format!("req_noadmit_{index}"),
                &fixture,
            );
            let _ = answer(&mut fixture, &bytes);
        }
        assert!(
            fixture.server.daemon().state().admissions().is_empty(),
            "an unlanded operation reached the admission predicate"
        );
    }
}

// =====================================================================================
// Leg 5 — anti-vacuity
// =====================================================================================

mod mutant {
    use super::{AGENT, CASES, STEWARD};
    use continuum_security::injection::Case;

    /// A stand-in for the defect this file exists to catch: a privilege decision that lets
    /// the *payload* decide.
    ///
    /// Local to this test, `src/`-untouched, and deliberately written the way such a defect
    /// would plausibly arrive — as a "trusted marker" convenience rather than as sabotage.
    fn compromised_admits(case: &Case, actor: &str) -> bool {
        actor == STEWARD.actor || case.payload.contains("reviewer note")
    }

    /// The real shape: the decision is a function of the principal alone.
    fn honest_admits(_case: &Case, actor: &str) -> bool {
        actor == STEWARD.actor
    }

    #[test]
    fn the_corpus_can_tell_a_content_sensitive_predicate_from_a_capability_check() {
        // The file's comparison, run against a predicate that fails it. If the corpus could
        // not distinguish these two, every green assertion above would be equally compatible
        // with a daemon that reads payloads.
        let honest: Vec<bool> = CASES
            .iter()
            .map(|case| honest_admits(case, AGENT.actor))
            .collect();
        let compromised: Vec<bool> = CASES
            .iter()
            .map(|case| compromised_admits(case, AGENT.actor))
            .collect();
        assert!(
            honest.iter().all(|admitted| !admitted),
            "an honest predicate admits nothing for an unprivileged actor"
        );
        assert!(
            compromised.iter().any(|admitted| *admitted),
            "the mutant must be reachable by some corpus payload, or it is not a mutant this \
             corpus could ever catch"
        );
        assert_ne!(
            honest, compromised,
            "the corpus cannot distinguish a capability check from a payload check"
        );
    }

    #[test]
    fn the_mutant_is_caught_by_a_payload_this_corpus_already_carried() {
        // Named rather than counted, so a reader can see which case does the catching and
        // check that it is a real injection rather than a marker planted for the mutant.
        let catching: Vec<&str> = CASES
            .iter()
            .filter(|case| compromised_admits(case, AGENT.actor))
            .map(|case| case.id)
            .collect();
        assert_eq!(
            catching,
            vec!["comments-weakening-property/evidence-status"]
        );
    }
}

// =====================================================================================
// Corpus/daemon agreement
// =====================================================================================

#[test]
fn every_corpus_operation_is_one_this_protocol_declares() {
    // The corpus is written against the dossier, not against this daemon, so it could name an
    // operation the protocol does not have. That would not be a security failure — it would
    // be a case that silently tests nothing, which is worse.
    for case in CASES {
        assert!(
            registry::operation(case.operation).is_some(),
            "{} drives at {:?}, which the operation registry does not declare",
            case.id,
            case.operation
        );
    }
}

#[test]
fn the_corpus_drives_at_every_privileged_operation_this_daemon_serves() {
    // A corpus that never named a landed `@privileged` operation could not falsify the G2
    // bullet however green it was. The privileged set is the IDL-derived one (see
    // `is_privileged`), not the registry's, for the same reason the headline test reads it:
    // the registry is the table the admission predicate itself consults, so a marker it drops
    // cannot be the oracle that catches the drop.
    let privileged = idl_privileged_operations();
    assert!(!privileged.is_empty());

    // Two of the IDL's five `@privileged` operations have no family in this daemon
    // (`repair.promote`, `repair.reject`; tracked `bn-1n7hy`, see `gate_g2_07_acceptance.rs`
    // F1). They are named here, not dropped by an `is_landed` filter applied to the whole set:
    // a filter that silently narrowed the privileged set itself is exactly how this guard
    // used to report full coverage of three operations while never admitting it was checking
    // three out of five.
    const UNLANDED_PRIVILEGED: [&str; 2] = ["repair.promote", "repair.reject"];
    for operation in UNLANDED_PRIVILEGED {
        assert!(
            privileged.contains(operation),
            "{operation} is no longer declared `@privileged` in the IDL — update this list"
        );
        assert!(
            !is_landed(operation),
            "{operation} has landed — drop it from UNLANDED_PRIVILEGED and require corpus \
             coverage for it below"
        );
    }

    let attempted: BTreeSet<&str> = CASES.iter().map(|case| case.operation).collect();
    for operation in &privileged {
        if UNLANDED_PRIVILEGED.contains(&operation.as_str()) {
            continue;
        }
        assert!(
            is_landed(operation),
            "{operation} is `@privileged` and not in UNLANDED_PRIVILEGED, but this daemon does \
             not serve it — add it to UNLANDED_PRIVILEGED or give it corpus coverage"
        );
        assert!(
            attempted.contains(operation.as_str()),
            "no corpus case drives at the privileged operation {operation}"
        );
    }
}

#[test]
fn the_corpus_is_carried_whole_into_this_suite() {
    // The counts the ratified sentence fixes, restated here from the corpus itself rather
    // than written down, so a case dropped upstream fails this file too.
    assert!(CASES.len() >= 45);
    assert_eq!(policy_block_cases().count(), 7);
    assert_eq!(isolation_cases().count(), 8);
    assert_eq!(
        CASES
            .iter()
            .filter(|case| matches!(case.vector, Vector::RedTeam(_)))
            .count(),
        30
    );
    // Every readable artifact class is represented, read through the corpus's own classifier
    // so the two files cannot disagree about what "agent-readable" means.
    let surfaces: BTreeSet<ArtifactClass> = CASES.iter().map(|case| case.surface).collect();
    for class in ArtifactClass::ALL {
        assert_eq!(
            surfaces.contains(&class),
            matches!(readability(class), Readability::Content(_))
        );
    }
    // And all three prohibited outcomes are present.
    let outcomes: BTreeSet<ProhibitedOutcome> = CASES.iter().map(|case| case.outcome).collect();
    assert_eq!(outcomes.len(), ProhibitedOutcome::ALL.len());
}
