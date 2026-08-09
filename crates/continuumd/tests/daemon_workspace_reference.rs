//! `workspace.create_by_reference` — port-by-reference on the wire (protocol 3.6, bn-3of5h).
//!
//! # What this file holds
//!
//! The operation makes one promise and spends one argument to keep it: **naming a component
//! set by its content identity must be the same act as enumerating it.** Every clause of
//! `rule snapshot.by_reference` is a test here rather than a sentence in a doc comment, and
//! the two that matter most are the ones a redesign is normally allowed to fudge — that the
//! inline form did not move, and that the reference is not a session-scoped alias.
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | resolving names the same act: same handle, same response | `rule snapshot.by_reference` | [`positive_the_two_spellings_publish_the_same_snapshot`] |
//! | the inline form is the registration; there is no verb | `rule snapshot.by_reference` | [`positive_an_inline_create_registers_what_the_reference_names`] |
//! | the commitment is a function of the components | `rule encoding.canonical_form` | [`positive_the_identity_a_client_derives_is_the_one_the_daemon_resolves`] |
//! | a set this daemon does not hold is a denial, not a not-found | RFC 0027 X2 | [`negative_an_unresolvable_reference_is_a_denial_byte_identical_with_any_other`] |
//! | the request's `intent` governs, and admission reads it | INV-015, RFC 0027 T2/T3 | [`negative_the_intent_is_read_from_the_request_and_admission_scopes_on_it`] |
//! | the stored value's `intent` does not govern | `rule snapshot.by_reference` | [`positive_the_request_intent_overrides_the_stored_one`] |
//! | the request's `epochs` govern and are checked | `rule envelope.unknown_fields` | [`negative_an_epoch_this_daemon_does_not_serve_is_refused_on_this_lane_too`] |
//! | an unaccepted intent is `AcceptanceChainInvalid` | INV-001 | [`negative_an_unaccepted_intent_is_refused_on_this_lane_too`] |
//! | `read` cannot reach a `propose` verb | RFC 0027 T1 | [`negative_a_read_capability_cannot_reach_this_verb`] |
//! | `seal` is monotone here too | `daemon::workspace` | [`boundary_seal_is_monotone_on_the_by_reference_lane`] |
//! | the whole thing works across the real byte boundary | this file | [`positive_the_operation_crosses_the_byte_boundary`] |
//! | the reference outlives the connection | `rule handshake.no_session_state` | [`positive_a_reference_survives_the_connection_that_established_it`] |
//! | every fault stays inside the declared union | `rule errors.common` | [`every_fault_this_lane_raises_is_declared`] |
//! | **the inline surface did not move** | `rule versioning.compatible_change` | [`the_inline_create_is_byte_identical_at_the_bump`] |
//! | a client below 3.6 gets its own version | RFC 0026 "Version window" | [`a_client_pinned_below_3_6_is_served_its_own_version`] |
//!
//! # Why the byte assertions are literals
//!
//! [`the_inline_create_is_byte_identical_at_the_bump`] pins `workspace.create`'s request and
//! result frames as byte strings, not as decoded shapes. A minor that adds an operation
//! claims that an existing client is unaffected, and "unaffected" is a statement about
//! bytes: `rule conformance.golden_traces` says as much ("a golden vector is a byte
//! sequence, not a shape"). A test that compared decoded structs would still pass if the
//! daemon had started emitting a new field, which is precisely the failure
//! `rule versioning.compatible_change`'s "MUST NOT emit fields the negotiated version does
//! not define" is about.

use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{self, Blake3Identity};
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateRequest,
};
use continuumd::protocol::registry::{ENCODINGS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, Encoding, ErrorCode, ResultStatus};
use continuumd::transport::{Server, decode_result, encode_request};

/// The TV-009 port's model, verbatim — the corpus port a real client would name.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

// --- fixtures ----------------------------------------------------------------------------

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

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    intents: Vec<IntentHandle>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents,
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Absent,
    }
}

fn hello(low: ProtocolVersion, high: ProtocolVersion) -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange { low, high },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-workspace-reference-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    }
}

fn negotiated() -> Negotiated {
    negotiate(
        &[version()],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello(version(), version()),
    )
    .expect("3.1 is served")
}

struct Fixture {
    daemon: Daemon,
    intent: IntentHandle,
    /// A second accepted contract, so "which intent governs" is a real question.
    other_intent: IntentHandle,
    files: Vec<Commitment>,
}

fn contract(text: &str) -> IntentContract {
    IntentContract::decode(text.trim_end().as_bytes()).expect("the fixture decodes")
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

/// A second contract, differing only in a field that changes its identity.
fn other_contract() -> IntentContract {
    // The change has to be *inside* the identity preimage, which excludes `intent_id` and
    // `name`: a second record with the same identity would make every "which intent
    // governs" assertion below vacuous. `bounds.faults` is in it.
    let text = DIE_HARD_CONTRACT
        .trim_end()
        .replace("\"faults\":0", "\"faults\":1");
    contract(&text)
}

fn fixture() -> Fixture {
    fixture_with(None)
}

fn fixture_with(pinned: Option<EpochSet>) -> Fixture {
    let die_hard = contract(DIE_HARD_CONTRACT);
    let intent = intent_handle(&die_hard);
    let other = other_contract();
    let other_intent = intent_handle(&other);
    assert_ne!(intent, other_intent, "the two fixtures are two identities");

    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"));
    if let Some(epochs) = pinned {
        builder = builder.epochs(epochs);
    }
    let mut daemon = builder
        .capability(
            {
                let mut descriptor = grant(
                    "cap_root",
                    "service:continuumd",
                    AuthorityLevel::Promote,
                    Vec::new(),
                );
                // A delegation never exceeds its parent in depth (D6/D7), so the root is
                // one deeper than the three it mints.
                descriptor.delegation_depth = 4;
                descriptor
            },
            None,
        )
        // `propose`: the level this operation and `workspace.create` both declare.
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                Vec::new(),
            ),
            root.clone(),
        )
        // The same level, scoped to ONE intent. This is what makes
        // `negative_the_intent_is_read_from_the_request_and_admission_scopes_on_it` a test
        // of admission rather than of the handler.
        .capability(
            grant(
                "cap_scoped",
                "agent:scoped",
                AuthorityLevel::Propose,
                vec![intent.clone()],
            ),
            root.clone(),
        )
        // Below the operation's minimum: `read` cannot reach a `propose` verb (T1).
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                Vec::new(),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .build();

    for (handle, record) in [(intent.clone(), die_hard), (other_intent.clone(), other)] {
        daemon.state_mut().put_intent(
            handle,
            IntentRecord {
                contract: record,
                // Accepted directly: `intent.accept` is a precondition of a create, not
                // the subject of this file (`daemon_operations.rs` owns that lane).
                status: RegistryStatus::Accepted,
                supersedes: None,
                superseded_by: None,
                acceptance: None,
            },
        );
    }

    let mut files = Vec::new();
    for (path, content) in [("DieHard.ctm", DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
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

    Fixture {
        daemon,
        intent,
        other_intent,
        files,
    }
}

/// The same fixture on a deployment that *pins* a semantic epoch.
///
/// `check_epochs` is a comparison against what the deployment serves, and a daemon that
/// pins nothing accepts every declaration — so the epoch clause of
/// `rule snapshot.by_reference` is only observable on a daemon that pins one.
fn pinned_epoch_fixture() -> Fixture {
    fixture_with(Some(EpochSet {
        protocol: version(),
        semantic: Nullable::Value(EpochIdentity::new("semantic-1").expect("epoch")),
        intent: Nullable::Null,
        evidence: Nullable::Null,
        proof: Nullable::Value(EpochIdentity::new("proof-1").expect("epoch")),
        corpus: Nullable::Null,
        engine: Nullable::Null,
    }))
}

fn epochs() -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: EpochIdentity::new("semantic-1").expect("epoch"),
        proof: EpochIdentity::new("proof-1").expect("epoch"),
        toolchain: Optional::Absent,
    }
}

fn components(fixture: &Fixture, intent: &IntentHandle) -> SnapshotComponents {
    SnapshotComponents {
        files: fixture.files.clone(),
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: epochs(),
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Absent,
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

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn inline_create(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    components: SnapshotComponents,
    seal: Optional<bool>,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components,
            overlay: Optional::Absent,
            seal,
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn by_reference(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    capability: &str,
    actor: &str,
    reference: &Commitment,
    intent: &IntentHandle,
    seal: Optional<bool>,
) -> OperationOutcome {
    by_reference_at(
        fixture,
        request,
        key,
        capability,
        actor,
        reference,
        intent,
        epochs(),
        seal,
    )
}

#[allow(clippy::too_many_arguments)]
fn by_reference_at(
    fixture: &mut Fixture,
    request: &str,
    key: &str,
    capability: &str,
    actor: &str,
    reference: &Commitment,
    intent: &IntentHandle,
    epochs: SnapshotEpochs,
    seal: Optional<bool>,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.create_by_reference", actor, capability, request),
            key,
        ),
        arguments: Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
            components: reference.clone(),
            epochs,
            intent: intent.clone(),
            seal,
        }),
    })
}

fn snapshot_of(outcome: &OperationOutcome) -> WorkspaceHandle {
    match &outcome.payload {
        Payload::WorkspaceCreate(body) => body.snapshot.clone(),
        Payload::WorkspaceCreateByReference(body) => body.snapshot.clone(),
        other => panic!("expected a create payload, got {other:?}"),
    }
}

fn code(outcome: &OperationOutcome) -> Option<ErrorCode> {
    outcome.envelope.error.value().map(|error| error.code)
}

/// Register a component set out of band, the way a deployment stages the content it names.
fn register(fixture: &mut Fixture, components: SnapshotComponents) -> Commitment {
    fixture
        .daemon
        .state_mut()
        .register_components(&Blake3Identity, components)
        .expect("blake3 names every input")
}

// --- the promise ---------------------------------------------------------------------------

#[test]
fn positive_the_two_spellings_publish_the_same_snapshot() {
    // `rule snapshot.by_reference`: "two spellings of one argument MUST NOT become two
    // behaviours". The strongest available statement of that is not that the two responses
    // are similar — it is that they name the *same content identity*, because a `ws_`
    // handle is derived from the snapshot's Merkle root and nothing else.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set.clone());

    let inline = inline_create(
        &mut fixture,
        "req_inline",
        "idem-inline",
        set,
        Optional::Absent,
    );
    assert_eq!(
        inline.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        inline.envelope.error
    );

    let referenced = by_reference(
        &mut fixture,
        "req_reference",
        "idem-reference",
        "cap_builder",
        "agent:builder",
        &reference,
        &intent,
        Optional::Absent,
    );
    assert_eq!(
        referenced.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        referenced.envelope.error
    );

    assert_eq!(
        snapshot_of(&inline),
        snapshot_of(&referenced),
        "one component set, one snapshot, whichever spelling named it"
    );
    let Payload::WorkspaceCreate(a) = &inline.payload else {
        panic!("expected an inline payload");
    };
    let Payload::WorkspaceCreateByReference(b) = &referenced.payload else {
        panic!("expected a by-reference payload");
    };
    assert_eq!(a.sealed, b.sealed);
    assert_eq!(a.diagnostics, b.diagnostics);
    assert_eq!(
        inline.envelope.verdict, referenced.envelope.verdict,
        "the same structural outcome"
    );
    assert_eq!(
        inline.envelope.artifacts, referenced.envelope.artifacts,
        "the same artifact is named on both envelopes"
    );
}

#[test]
fn positive_an_inline_create_registers_what_the_reference_names() {
    // The whole of "whatever registration verb RFC 0019's manifest needs": there is none.
    // Nothing here is registered out of band — the daemon learns the set from the inline
    // create it already accepted, and the client names it afterwards by deriving the same
    // identity from the same value.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let derived =
        DaemonState::components_identity(&Blake3Identity, &set).expect("blake3 names every input");

    let before = by_reference(
        &mut fixture,
        "req_before",
        "idem-before",
        "cap_builder",
        "agent:builder",
        &derived,
        &intent,
        Optional::Absent,
    );
    assert_eq!(
        code(&before),
        Some(ErrorCode::CapabilityDenied),
        "before any create, this daemon holds no such set"
    );

    let inline = inline_create(
        &mut fixture,
        "req_register",
        "idem-register",
        set,
        Optional::Absent,
    );
    assert_eq!(inline.envelope.status, ResultStatus::Ok);

    let after = by_reference(
        &mut fixture,
        "req_after",
        "idem-after",
        "cap_builder",
        "agent:builder",
        &derived,
        &intent,
        Optional::Absent,
    );
    assert_eq!(
        after.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        after.envelope.error
    );
    assert_eq!(snapshot_of(&inline), snapshot_of(&after));
}

#[test]
fn positive_the_identity_a_client_derives_is_the_one_the_daemon_resolves() {
    // A commitment is a function of the components and of nothing else — not of the
    // connection, not of the order the members were built in, and not of the daemon.
    let fixture = fixture();
    let one = components(&fixture, &fixture.intent);
    let two = components(&fixture, &fixture.intent);
    let identity = |set: &SnapshotComponents| {
        DaemonState::components_identity(&Blake3Identity, set).expect("blake3 names every input")
    };
    assert_eq!(
        identity(&one),
        identity(&two),
        "equal values have one identity"
    );

    let mut changed = components(&fixture, &fixture.intent);
    changed.files.reverse();
    assert_ne!(
        identity(&one),
        identity(&changed),
        "a different component set is a different identity — order is part of the value"
    );

    let mut other_epoch = components(&fixture, &fixture.intent);
    other_epoch.epochs.semantic = EpochIdentity::new("semantic-2").expect("epoch");
    assert_ne!(
        identity(&one),
        identity(&other_epoch),
        "the identity covers the whole value, including the members this lane then overrides"
    );
}

#[test]
fn negative_an_unresolvable_reference_is_a_denial_byte_identical_with_any_other() {
    // > a read of an artifact that does not exist and a read of one that exists but is out
    // > of scope return **byte-identical** envelopes
    // >
    // > — RFC 0027 X2
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let other = fixture.other_intent.clone();
    let set = components(&fixture, &intent);
    let held = register(&mut fixture, set);

    // One caller, one request identity, two refusals with two different causes: a
    // commitment this daemon does not hold, and a held one named under an intent the
    // caller's capability does not scope to. `audit` is `correlate(request_id, actor)`, so
    // holding both fixed is what makes the comparison a comparison.
    let unknown = Commitment::new("ws_this-daemon-holds-no-such-set");
    let absent = by_reference(
        &mut fixture,
        "req_x",
        "idem-x",
        "cap_scoped",
        "agent:scoped",
        &unknown,
        &intent,
        Optional::Absent,
    );
    let out_of_scope = by_reference(
        &mut fixture,
        "req_x",
        "idem-x",
        "cap_scoped",
        "agent:scoped",
        &held,
        &other,
        Optional::Absent,
    );

    assert_eq!(code(&absent), Some(ErrorCode::CapabilityDenied));
    assert_eq!(code(&out_of_scope), Some(ErrorCode::CapabilityDenied));
    assert_eq!(
        continuumd::codec::to_bytes(&absent.envelope).expect("the envelope encodes"),
        continuumd::codec::to_bytes(&out_of_scope.envelope).expect("the envelope encodes"),
        "an absent set and an out-of-scope one are byte-identical refusals"
    );
}

#[test]
fn negative_the_intent_is_read_from_the_request_and_admission_scopes_on_it() {
    // This is the operation's reason for declaring `intent` at all. `OperationFamily::scope`
    // is computed from the arguments alone, before the handler runs and without the state,
    // so a governing contract hidden behind a commitment would be a scope claim admission
    // could not see. Here it can: a capability scoped to one intent is denied when it names
    // the other, and the refusal happens whether or not the reference resolves.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let other = fixture.other_intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);

    let permitted = by_reference(
        &mut fixture,
        "req_ok",
        "idem-ok",
        "cap_scoped",
        "agent:scoped",
        &reference,
        &intent,
        Optional::Absent,
    );
    assert_eq!(
        permitted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        permitted.envelope.error
    );

    let refused = by_reference(
        &mut fixture,
        "req_no",
        "idem-no",
        "cap_scoped",
        "agent:scoped",
        &reference,
        &other,
        Optional::Absent,
    );
    assert_eq!(
        code(&refused),
        Some(ErrorCode::CapabilityDenied),
        "T2 is decided on the intent the request names"
    );
}

#[test]
fn positive_the_request_intent_overrides_the_stored_one() {
    // `rule snapshot.by_reference`: the resolved value's own `intent` is NOT read on this
    // lane. The set here was registered under one contract and is named under the other;
    // what governs the resulting snapshot is the request's.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let other = fixture.other_intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);

    let outcome = by_reference(
        &mut fixture,
        "req_regovern",
        "idem-regovern",
        "cap_builder",
        "agent:builder",
        &reference,
        &other,
        Optional::Absent,
    );
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    let handle = snapshot_of(&outcome);
    let record = fixture
        .daemon
        .state()
        .workspace(&handle)
        .expect("the create recorded a workspace");
    assert_eq!(
        record.intent, fixture.other_intent,
        "the request's intent governs, not the one the set was stored under"
    );
}

#[test]
fn negative_an_epoch_this_daemon_does_not_serve_is_refused_on_this_lane_too() {
    // The second member that travels, and the second reason for it. Both directions are
    // asserted, because only the pair proves which value reaches `check_epochs`:
    //
    //   - a request declaring an epoch this deployment does not serve is refused, even
    //     though the *stored* set declares one it does — so the request is checked;
    //   - a request declaring the served epoch is accepted, even though the stored set
    //     declares one this deployment would refuse — so the store is not checked.
    //
    // A daemon that read the epochs out of what it stored would be checking itself, and
    // neither assertion below could be written.
    let mut fixture = pinned_epoch_fixture();
    let intent = fixture.intent.clone();

    let set = components(&fixture, &intent);
    let served = register(&mut fixture, set);
    let refused = by_reference_at(
        &mut fixture,
        "req_epoch",
        "idem-epoch",
        "cap_builder",
        "agent:builder",
        &served,
        &intent,
        SnapshotEpochs {
            semantic: EpochIdentity::new("semantic-99").expect("epoch"),
            proof: EpochIdentity::new("proof-1").expect("epoch"),
            toolchain: Optional::Absent,
        },
        Optional::Absent,
    );
    assert_eq!(
        code(&refused),
        Some(ErrorCode::EpochUnsupported),
        "the request's declaration is what is checked"
    );

    let mut unserved_set = components(&fixture, &intent);
    unserved_set.epochs.semantic = EpochIdentity::new("semantic-99").expect("epoch");
    let unserved = register(&mut fixture, unserved_set);
    let accepted = by_reference(
        &mut fixture,
        "req_epoch_2",
        "idem-epoch-2",
        "cap_builder",
        "agent:builder",
        &unserved,
        &intent,
        Optional::Absent,
    );
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "the stored value's epochs are not read on this lane: {:?}",
        accepted.envelope.error
    );
}

#[test]
fn negative_an_unaccepted_intent_is_refused_on_this_lane_too() {
    // INV-001: only an accepted contract governs a snapshot. The refusal is
    // `AcceptanceChainInvalid`, which is the one code this operation declares beyond the
    // common union — the same code the inline lane answers with, from the same line.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);
    let proposed = fixture.other_intent.clone();
    let contract = fixture
        .daemon
        .state()
        .intent(&proposed)
        .expect("the fixture registered it")
        .contract
        .clone();
    fixture.daemon.state_mut().put_intent(
        proposed.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let outcome = by_reference(
        &mut fixture,
        "req_unaccepted",
        "idem-unaccepted",
        "cap_builder",
        "agent:builder",
        &reference,
        &proposed,
        Optional::Absent,
    );
    assert_eq!(code(&outcome), Some(ErrorCode::AcceptanceChainInvalid));
}

#[test]
fn negative_a_read_capability_cannot_reach_this_verb() {
    // T1, on the row this operation declares. If the level had fallen to `read` to match
    // "it only names content the daemon already holds", this assertion would be the one
    // that had to be deleted — which is the argument RFC 0027 records for keeping it at
    // `propose`.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);

    let outcome = by_reference(
        &mut fixture,
        "req_reader",
        "idem-reader",
        "cap_reader",
        "agent:reader",
        &reference,
        &intent,
        Optional::Absent,
    );
    assert_eq!(code(&outcome), Some(ErrorCode::CapabilityDenied));

    let declared = OPERATIONS
        .iter()
        .find(|spec| spec.name == "workspace.create_by_reference")
        .expect("the registry declares it");
    assert_eq!(declared.authority, AuthorityLevel::Propose);
    assert_eq!(
        declared.authority,
        OPERATIONS
            .iter()
            .find(|spec| spec.name == "workspace.create")
            .expect("the registry declares it")
            .authority,
        "one act, one minimum level (RFC 0027 A2)"
    );
}

#[test]
fn boundary_seal_is_monotone_on_the_by_reference_lane() {
    // The `seal` table in `daemon::workspace`'s documentation is a property of `create`,
    // and this lane inherits it by *calling* `create` — so the third row (`sealed`,
    // `seal: false` -> stays sealed) is true here without a second implementation of it.
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);

    let sealed = by_reference(
        &mut fixture,
        "req_seal",
        "idem-seal",
        "cap_builder",
        "agent:builder",
        &reference,
        &intent,
        Optional::Present(true),
    );
    assert_eq!(
        sealed.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        sealed.envelope.error
    );
    let Payload::WorkspaceCreateByReference(body) = &sealed.payload else {
        panic!("expected a by-reference payload");
    };
    assert!(body.sealed);

    let again = by_reference(
        &mut fixture,
        "req_unseal",
        "idem-unseal",
        "cap_builder",
        "agent:builder",
        &reference,
        &intent,
        Optional::Present(false),
    );
    let Payload::WorkspaceCreateByReference(body) = &again.payload else {
        panic!("expected a by-reference payload");
    };
    assert!(body.sealed, "`seal: false` is not an un-seal");
}

// --- the wire ------------------------------------------------------------------------------

#[test]
fn positive_the_operation_crosses_the_byte_boundary() {
    let mut fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let reference = register(&mut fixture, set);
    let intent = fixture.intent.clone();

    let mut server = Server::new(fixture.daemon, negotiated());
    let request = encode_request(
        &keyed(
            envelope(
                "workspace.create_by_reference",
                "agent:builder",
                "cap_builder",
                "req_wire",
            ),
            "idem-wire",
        ),
        &Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
            components: reference,
            epochs: epochs(),
            intent,
            seal: Optional::Absent,
        }),
    )
    .expect("the request encodes");
    let answer = server.answer(&request).expect("the daemon answers");
    let (result, payload) = decode_result("workspace.create_by_reference", &answer)
        .expect("the client decodes the frame");

    assert_eq!(result.status, ResultStatus::Ok, "{:?}", result.error);
    let Payload::WorkspaceCreateByReference(body) = payload else {
        panic!("expected a by-reference payload");
    };
    assert!(body.snapshot.as_str().starts_with("ws_"));
    assert_eq!(result.artifacts.len(), 1);
}

#[test]
fn positive_a_reference_survives_the_connection_that_established_it() {
    // The distinction from a session-scoped alias, which `rule handshake.no_session_state`
    // forbids and which the DX-10 pre-check found no conforming reader could refuse. A
    // commitment is a content identity: the connection that registered it is closed, and
    // the name still resolves on a new one, which is `rule versioning.handle_stability`'s
    // property held one level below a handle.
    let fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);
    let intent = fixture.intent.clone();

    let mut first = Server::new(fixture.daemon, negotiated());
    let request = encode_request(
        &keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_establish",
            ),
            "idem-establish",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set.clone(),
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    )
    .expect("the request encodes");
    let answer = first.answer(&request).expect("the daemon answers");
    let (established, _) =
        decode_result("workspace.create", &answer).expect("the client decodes the frame");
    assert_eq!(established.status, ResultStatus::Ok);

    // The connection ends here. Everything it held is gone; the daemon is what survives.
    let daemon = first.close();
    let derived =
        DaemonState::components_identity(&Blake3Identity, &set).expect("blake3 names every input");

    let mut second = Server::new(daemon, negotiated());
    let request = encode_request(
        &keyed(
            envelope(
                "workspace.create_by_reference",
                "agent:builder",
                "cap_builder",
                "req_reconnect",
            ),
            "idem-reconnect",
        ),
        &Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
            components: derived,
            epochs: epochs(),
            intent,
            seal: Optional::Absent,
        }),
    )
    .expect("the request encodes");
    let answer = second.answer(&request).expect("the daemon answers");
    let (result, payload) = decode_result("workspace.create_by_reference", &answer)
        .expect("the client decodes the frame");
    assert_eq!(result.status, ResultStatus::Ok, "{:?}", result.error);
    let Payload::WorkspaceCreateByReference(body) = payload else {
        panic!("expected a by-reference payload");
    };
    assert!(body.snapshot.as_str().starts_with("ws_"));
}

#[test]
fn the_inline_create_is_byte_identical_at_the_bump() {
    // The compatibility claim of this minor, as bytes rather than as a sentence. Both
    // frames below are what a 3.1 client sent and received before `workspace.create_by_
    // reference` existed; the operation added nothing to either, and `workspace.create`
    // now additionally *registers* its components, which is daemon-side state and reaches
    // no field.
    let fixture = fixture();
    let intent = fixture.intent.clone();
    let set = components(&fixture, &intent);

    let request = encode_request(
        &keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_pinned",
            ),
            "idem-pinned",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    )
    .expect("the request encodes");

    let mut server = Server::new(fixture.daemon, negotiated());
    let answer = server.answer(&request).expect("the daemon answers");

    assert_eq!(
        String::from_utf8(request).expect("canonical JSON is UTF-8"),
        PINNED_CREATE_REQUEST,
        "the inline request frame moved"
    );
    assert_eq!(
        String::from_utf8(answer).expect("canonical JSON is UTF-8"),
        PINNED_CREATE_RESULT,
        "the inline result frame moved"
    );

    // And the shapes those bytes are a spelling of, stated as the lists protocol 3.5
    // declared. The bytes above could in principle agree by accident on one value; two
    // field lists cannot.
    let spec = OPERATIONS
        .iter()
        .find(|spec| spec.name == "workspace.create")
        .expect("the registry declares it");
    let names = |fields: &[continuumd::protocol::spec::FieldSpec]| -> Vec<&'static str> {
        fields.iter().map(|field| field.name).collect()
    };
    assert_eq!(
        names(spec.request.fields),
        ["components", "overlay", "seal"]
    );
    assert_eq!(
        names(spec.response.fields),
        ["snapshot", "sealed", "diagnostics"]
    );
}

#[test]
fn a_client_pinned_below_3_6_is_served_its_own_version() {
    // A minor is not a window. A client whose range tops out at 3.5 negotiates 3.5, is not
    // silently upgraded, and a request naming 3.6 is not admitted on that connection — so
    // the version this operation is `@since` cannot reach it. The complementary half of
    // the claim, that its inline sibling's bytes did not move, is the test above.
    let negotiated = negotiate(
        &[ProtocolVersion::new(3, 5), ProtocolVersion::new(3, 6)],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello(ProtocolVersion::new(3, 0), ProtocolVersion::new(3, 5)),
    )
    .expect("3.5 is served");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(3, 5));
    assert!(negotiated.admits_request(ProtocolVersion::new(3, 5)));
    assert!(
        !negotiated.admits_request(ProtocolVersion::new(3, 6)),
        "a connection negotiated at 3.5 does not admit a 3.6 request"
    );
}

#[test]
fn every_fault_this_lane_raises_is_declared() {
    // `rule errors.common` plus the operation's own `errors` clause, over the family's
    // declared fault table. A code outside the union is a defect whichever lane raised it.
    let spec = OPERATIONS
        .iter()
        .find(|spec| spec.name == "workspace.create_by_reference")
        .expect("the registry declares it");
    let common = [
        ErrorCode::MalformedRequest,
        ErrorCode::ProtocolVersionUnsupported,
        ErrorCode::CapabilityDenied,
        ErrorCode::QuotaExhausted,
        ErrorCode::EpochUnsupported,
        ErrorCode::UnsupportedSemanticFeature,
        // `@mutation` adds these two.
        ErrorCode::IdempotencyKeyReused,
        ErrorCode::PublicationAborted,
    ];
    for (operation, code) in continuumd::daemon::workspace::FAULTS {
        if *operation != "workspace.create_by_reference" {
            continue;
        }
        assert!(
            common.contains(code) || spec.errors.contains(code),
            "{code:?} is outside `workspace.create_by_reference`'s declared union"
        );
    }
    assert_eq!(
        spec.errors,
        &[ErrorCode::AcceptanceChainInvalid],
        "one code beyond the common union, and it is the one INV-001 needs"
    );
}

/// `workspace.create`'s request frame, pinned. See
/// [`the_inline_create_is_byte_identical_at_the_bump`].
const PINNED_CREATE_REQUEST: &str = "{\"actor\":\"agent:builder\",\"arguments\":{\"components\":{\"cml_modules\":[],\"configuration\":[],\"correspondence\":[],\"dependencies\":[],\"domain_packs\":[],\"epochs\":{\"proof\":\"proof-1\",\"semantic\":\"semantic-1\"},\"files\":[\"ws_63d1faf71a9ffa78df3299cf4599db39b2589ca27c3de7690bb01c2367576109\",\"ws_7ece0b2d7af16b1e4dbfd73ea6db474d0ed690807fc90dde32c7cc1893e1ee23\"],\"intent\":\"in_7c579d67c260f5b9f3739359cb76941aed0fa533568b317d5a173844c1267b42\",\"proof_environment\":[],\"rust_extraction\":[]},\"seal\":true},\"capability\":\"cap_builder\",\"idempotency_key\":\"idem-pinned\",\"intent\":null,\"operation\":\"workspace.create\",\"protocol_version\":\"3.1\",\"request_id\":\"req_pinned\",\"snapshot\":null}";

/// `workspace.create`'s result frame, pinned.
const PINNED_CREATE_RESULT: &str = "{\"artifacts\":[{\"handle\":\"ws_38218f2b84526de605c98995aaddd23695f257e399deb506122180a5e3295a32\",\"kind\":\"ws\"}],\"cost\":{},\"epochs\":{\"corpus\":null,\"engine\":null,\"evidence\":null,\"intent\":null,\"proof\":null,\"protocol\":\"3.1\",\"semantic\":null},\"next_operations\":[],\"omissions\":[],\"payload\":{\"diagnostics\":[],\"sealed\":true,\"snapshot\":\"ws_38218f2b84526de605c98995aaddd23695f257e399deb506122180a5e3295a32\"},\"request_id\":\"req_pinned\",\"status\":\"ok\",\"verdict\":{\"structural\":{\"outcome\":\"created\"}},\"warnings\":[]}";
