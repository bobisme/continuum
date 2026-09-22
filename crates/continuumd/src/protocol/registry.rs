//! The operation registry: the IDL's §10 declarations, as data.
//!
//! This is the table the conformance test compares against
//! `notes/plan/schemas/continuumd-native-protocol.idl`. Every field list in it is
//! reached through [`ProtocolStruct`], so it cannot describe a struct differently
//! from how that struct is declared — renaming a Rust field renames it here.
//!
//! [`ProtocolStruct`]: crate::protocol::spec::ProtocolStruct

use super::operations::{
    benchmark::*, context::*, correspondence::*, debug::*, evidence::*, failure::*, forge::*,
    intent::*, model::*, observe::*, program::*, proof::*, query::*, refinement::*, repair::*,
    task::*, verification::*, whiteboard::*, workspace::*,
};
use super::prelude::*;
use super::spec::{
    AliasSpec, Annotation, EnumSpec, HandleSpec, OperationSpec, StructSpec, UnionSpec,
};

/// The IDL document version this registry transcribes (`protocol.idl_version`).
///
/// `"1.13"` as of bn-3ncfp, which adds `rule artifact_class.spelling` (the
/// artifact-class token vocabulary, 44 -> 45 rules) and moves no declaration,
/// so it leaves [`PROTOCOL_VERSION`] at `"3.6"`. `"1.12"` (bn-12plt) was a
/// doc-comment-only revision on the same footing.
pub const IDL_VERSION: &str = "1.13";

/// The protocol version this registry defines (`protocol.version`).
///
/// `"3.3"` as of IDL 1.4, and still `"3.3"` at IDL 1.5. The 3.0 -> 3.1 minor
/// bump covered IDL 1.1's compatible fixes and IDL 1.2's RFC 0027 F1-F8 sweep
/// together, ratified by the user on 2026-07-31 and taken once by bn-3ayom on
/// 2026-08-01 — the deferral recorded here at IDL 1.1
/// (`notes/plan/notes/PR0_EXIT_EVIDENCE.md` §6) is discharged, not carried. The
/// 3.1 -> 3.2 bump covers IDL 1.3 — the bn-i4aem defect reconciliation, the
/// `file_components` addition, and the three encoding rules — and is taken
/// once at the end of that revision on the same precedent.
///
/// The 3.2 -> 3.3 bump covers IDL 1.4 (bn-3sypm) and is the first one that
/// **adds an operation**: `evidence.link`, RFC 0038's F14 decision on the wire.
/// "Adding an operation" is the first compatible change
/// `rule versioning.compatible_change` names, so this is still a minor and
/// [`MAJORS_SERVED`] is untouched — an operation a 3.0, 3.1, or 3.2 client never
/// names cannot reach it. What makes the growth checkable rather than assumed is
/// the F13 discipline: the row exists in plan §10.2, in RFC 0027's authority
/// table, and in the IDL, or it exists nowhere
/// (`rule conformance.registry_agreement`, `tests/registry_agreement.rs`).
///
/// IDL 1.5 (`rule handshake.bootstrap_encoding`, bn-1h158) does **not** bump
/// this value. It declares that `ClientHello`, `ServerWelcome`, and
/// `ServerReject` are `canonical_json` unconditionally and that the negotiated
/// encoding governs from the first `RequestEnvelope` onward — a fact
/// [`crate::transport::encode_hello`], [`crate::transport::Server::open`], and
/// [`crate::transport::Server::answer`] already implement (bn-1mhcr's
/// canonical_cbor delivery left it as an unwritten, deliberately un-invented
/// rule). None of `rule versioning.compatible_change`'s five triggers fires —
/// no operation, field, enum member, or relaxed constraint — so no wire byte
/// moves and no flag is raised (the bn-23j7s no-F-flag precedent).
///
/// The 3.3 -> 3.4 bump covers IDL 1.6 (bn-3jrtz), the fourth
/// deferred-and-bundled protocol minor, and pays RFC 0026's F19 and F20
/// together. F19 declares `Error.data`'s first shape — `CertificateRejection`,
/// for `code = CertificateRejected` — an ordinary compatible addition: `data`
/// has been a defined `optional` field since 3.0, and a pre-3.4 reader handles
/// the newly-shaped bytes exactly as the pre-3.4 text already obliged it to
/// (an `Opaque` whose shape its version does not declare, carried verbatim,
/// never guessed at). F20 flips which of `verification.start`'s two *declared*
/// answer shapes an already-terminal, non-`Completed` identity lands on
/// (`task_started` -> `ok` on the observing lane, mirroring `task.resume`'s
/// terminal short-circuit): no declaration moves, a client is already required
/// to handle both shapes, and the lane is "a fact about what the operation
/// did" (correction 45) — the same nature of change as 3.2's item 9, which is
/// the precedent that made the lanes reachable at all. The registry is
/// untouched: 73 operations, every count table unchanged.
///
/// IDL 1.7 (`rule subscription.delivery`, bn-3080b) does **not** bump this
/// value either, and the reason is IDL 1.5's with one honest difference. 1.5
/// wrote down what the transport already did; nothing had ever delivered an
/// `events` frame, so 1.7 *decides* rather than reports — what an event frame
/// is (the declared event struct, told apart from a `ResultEnvelope` by its
/// required members), that one delta is one frame however many of a
/// connection's scopes select it, and which deltas a scope selects. Still none
/// of `rule versioning.compatible_change`'s five triggers fires: no operation,
/// field, enum member, error code, or relaxed constraint. And no flag is
/// raised, because `rule errors.common` as relaxed at 3.2 already makes
/// "whether a lane has shipped" a property of a deployment rather than of an
/// operation — a daemon that serves the channel answers `ok`, one that does not
/// keeps `UnsupportedSemanticFeature`, and both conform.
///
/// The 3.4 -> 3.5 bump covers IDL 1.9 (bn-1as8e) and is the **second** one that
/// adds an operation: `whiteboard.compile`, the 74th, in a 19th namespace, and
/// the wire paragraph RFC 0038 deferred when the whiteboard compiler landed as
/// a crate-level surface. It was checked for avoidability first — three of the
/// last four revisions left this value alone by *serving already-declared
/// vocabulary*, which is the cheapest payment there is — and there was nothing
/// to serve: no operation, no `EvidenceQuery` member, and no struct in the IDL
/// names a note. `forge.create`'s `sketch` is RFC 0033's synthesis input and
/// answers with a `ForgeHandle`; `context.compile` runs the other way, from an
/// evidence root to a pack. So the registry grows for the second time in this
/// protocol's life, and again not as a defect paid with a verb (RFC 0027
/// correction 30) but as a subsystem the dossier already specified reaching the
/// wire. Counts move where they must be visible: 74 rows in 19 namespaces,
/// `propose` 9, `@mutation` 46, `structural` verdicts 27, named structs 47,
/// rules 42. No flag rides the bump and one is raised (RFC 0026 F21).
///
/// The 3.5 -> 3.6 bump covers IDL 1.11 (bn-3of5h) and is the **third** one that
/// adds an operation: `workspace.create_by_reference`, the 75th, and the first
/// registry growth that enters an **existing** namespace. What it pays is a
/// measurement rather than a deferral. `SnapshotComponents` is the largest
/// argument this protocol carries -- 17,208 B over the DX-10 instrument's 24
/// `workspace.create` calls -- and the same act through a local CLI costs a
/// port name, because the CLI resolves the components from the filesystem;
/// PR-10 IMPL-02 recorded the asymmetry and left it open, and
/// `notes/plan/notes/DX10_BYTE_LEDGER.md` §8 then measured seven candidate
/// answers and left exactly one inside major 3. This is that one.
///
/// It was checked for avoidability first, which is what 1.5, 1.7, 1.8, and 1.10
/// each did instead of bumping: no declared member anywhere takes a components
/// identity, `workspace.fork` names a snapshot and preserves its intent binding
/// so it cannot open a lineage under a different contract, and
/// `SnapshotComponents.file_components` is `optional` beside nine `required`
/// peers, so the inline argument has no absent form. "Adding an operation" is
/// the first change `rule versioning.compatible_change` names, so the bump is a
/// minor and [`MAJORS_SERVED`] is untouched -- a 3.0 through 3.5 client never
/// sends the name, and an operation it never names cannot reach it.
/// `workspace.create` keeps `components`, `overlay`, and `seal`;
/// `SnapshotComponents` keeps all eleven members and their presence markers.
///
/// Counts move where they must be visible: 75 rows in 19 namespaces, `propose`
/// 10, `@mutation` 47, `structural` verdicts 28, named structs 47 (unchanged --
/// both bodies are anonymous over already-declared types), rules 43 -> 44
/// (`snapshot.by_reference`). No flag rides the bump and none is raised; RFC
/// 0026's flags preamble records the sweep, including why F6, touched at 1.10,
/// is not payable here either.
pub const PROTOCOL_VERSION: &str = "3.6";

/// The protocol majors a conforming daemon serves concurrently: N and N-1
/// (`protocol.majors_served`).
pub const MAJORS_SERVED: &[u32] = &[3, 2];

/// The wire encodings the protocol fixes (`protocol.encodings`); exactly one is
/// negotiated per connection.
pub const ENCODINGS: &[Encoding] = &[Encoding::CanonicalJson, Encoding::CanonicalCbor];

/// The number of operations in the plan §10.2 registry.
///
/// 72 from protocol 3.0 through 3.2; 73 as of 3.3, when `evidence.link` became
/// the first operation ever added to this protocol (bn-3sypm); 74 as of 3.5,
/// when `whiteboard.compile` became the second (bn-1as8e); 75 as of 3.6, when
/// `workspace.create_by_reference` became the third and the first to enter an
/// existing namespace (bn-3of5h).
pub const OPERATION_COUNT: usize = 75;

/// Every operation the IDL declares, in its declaration order.
pub const OPERATIONS: &[OperationSpec] = &[
    OperationSpec {
        name: "workspace.create",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<WorkspaceCreateRequest>(),
        response: StructSpec::of::<WorkspaceCreateResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::AcceptanceChainInvalid,
        ],
    },
    OperationSpec {
        name: "workspace.create_by_reference",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<WorkspaceCreateByReferenceRequest>(),
        response: StructSpec::of::<WorkspaceCreateByReferenceResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::AcceptanceChainInvalid],
    },
    OperationSpec {
        name: "workspace.fork",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<WorkspaceForkRequest>(),
        response: StructSpec::of::<WorkspaceForkResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "workspace.diff",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Readonly, Annotation::TaskStarting],
        request: StructSpec::of::<WorkspaceDiffRequest>(),
        response: StructSpec::of::<WorkspaceDiffResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "workspace.seal",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<WorkspaceSealRequest>(),
        response: StructSpec::of::<WorkspaceSealResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[],
    },
    OperationSpec {
        name: "intent.get",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<IntentGetRequest>(),
        response: StructSpec::of::<IntentGetResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::AcceptanceChainInvalid],
    },
    OperationSpec {
        name: "intent.diff",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<IntentDiffRequest>(),
        response: StructSpec::of::<IntentDiffResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "intent.propose_revision",
        authority: AuthorityLevel::ReviseIntent,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<IntentProposeRevisionRequest>(),
        response: StructSpec::of::<IntentProposeRevisionResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::IntentMutationDenied,
            ErrorCode::UnsupportedSemanticFeature,
        ],
    },
    OperationSpec {
        name: "intent.accept",
        authority: AuthorityLevel::ReviseIntent,
        annotations: &[
            Annotation::Mutation,
            Annotation::Privileged,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<IntentAcceptRequest>(),
        response: StructSpec::of::<IntentAcceptResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::IntentMutationDenied,
            ErrorCode::AcceptanceChainInvalid,
            ErrorCode::PolicyGateFailed,
        ],
    },
    OperationSpec {
        name: "intent.reject",
        authority: AuthorityLevel::ReviseIntent,
        annotations: &[
            Annotation::Mutation,
            Annotation::Privileged,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<IntentRejectRequest>(),
        response: StructSpec::of::<IntentRejectResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::IntentMutationDenied],
    },
    OperationSpec {
        name: "intent.lock",
        authority: AuthorityLevel::ReviseIntent,
        annotations: &[
            Annotation::Mutation,
            Annotation::Privileged,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<IntentLockRequest>(),
        response: StructSpec::of::<IntentLockResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::IntentMutationDenied, ErrorCode::MalformedRequest],
    },
    OperationSpec {
        name: "verification.start",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<VerificationStartRequest>(),
        response: StructSpec::of::<VerificationStartResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
            ErrorCode::BudgetExhausted,
            ErrorCode::UntrustedDomainBoundary,
        ],
    },
    OperationSpec {
        name: "verification.result",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<VerificationResultRequest>(),
        response: StructSpec::of::<VerificationResult>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::BudgetExhausted,
            ErrorCode::ContinuationEpochMismatch,
        ],
    },
    OperationSpec {
        name: "verification.await",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::TaskStarting],
        request: StructSpec::of::<VerificationAwaitRequest>(),
        response: StructSpec::of::<VerificationResult>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::BudgetExhausted,
            ErrorCode::ContinuationEpochMismatch,
        ],
    },
    OperationSpec {
        name: "model.check",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ModelCheckRequest>(),
        response: StructSpec::of::<VerificationResult>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "model.explore",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ModelExploreRequest>(),
        response: StructSpec::of::<ModelExploreResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "model.compare",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::TaskStarting],
        request: StructSpec::of::<ModelCompareRequest>(),
        response: StructSpec::of::<ModelCompareResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "program.extract",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ProgramExtractRequest>(),
        response: StructSpec::of::<ProgramExtractResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::AmbiguousCorrespondence,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "program.run",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ProgramRunRequest>(),
        response: StructSpec::of::<ProgramRunResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
            ErrorCode::UntrustedDomainBoundary,
        ],
    },
    OperationSpec {
        name: "program.replay",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ProgramReplayRequest>(),
        response: StructSpec::of::<ProgramReplayResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::ReplayDiverged,
            ErrorCode::BudgetExhausted,
            ErrorCode::EpochUnsupported,
        ],
    },
    OperationSpec {
        name: "refinement.check",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<RefinementCheckRequest>(),
        response: StructSpec::of::<VerificationResult>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::AmbiguousCorrespondence,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "refinement.explain",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<RefinementExplainRequest>(),
        response: StructSpec::of::<RefinementExplainResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "proof.goal",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<ProofGoalRequest>(),
        response: StructSpec::of::<ProofGoalResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "proof.attempt",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ProofAttemptRequest>(),
        response: StructSpec::of::<ProofAttemptResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
            ErrorCode::CertificateRejected,
        ],
    },
    OperationSpec {
        name: "proof.check",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ProofCheckRequest>(),
        response: StructSpec::of::<ProofCheckResponse>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::CertificateRejected,
            ErrorCode::BudgetExhausted,
            ErrorCode::EpochUnsupported,
        ],
    },
    OperationSpec {
        name: "proof.slice",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<ProofSliceRequest>(),
        response: StructSpec::of::<ProofSliceResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "correspondence.bind",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<CorrespondenceBindRequest>(),
        response: StructSpec::of::<CorrespondenceBindResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::AmbiguousCorrespondence,
            ErrorCode::UnsupportedSemanticFeature,
        ],
    },
    OperationSpec {
        name: "correspondence.status",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<CorrespondenceStatusRequest>(),
        response: StructSpec::of::<CorrespondenceStatusResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "correspondence.drift",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<CorrespondenceDriftRequest>(),
        response: StructSpec::of::<CorrespondenceDriftResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::AmbiguousCorrespondence,
        ],
    },
    OperationSpec {
        name: "debug.open",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugOpenRequest>(),
        response: StructSpec::of::<DebugOpenResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::EpochUnsupported,
        ],
    },
    OperationSpec {
        name: "debug.state",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<DebugStateRequest>(),
        response: StructSpec::of::<DebugStateResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.enabled",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<DebugEnabledRequest>(),
        response: StructSpec::of::<DebugEnabledResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.step_event",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugStepEventRequest>(),
        response: StructSpec::of::<DebugStepEventResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.step_abstract",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugStepAbstractRequest>(),
        response: StructSpec::of::<DebugStepAbstractResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.reverse_causal",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugReverseCausalRequest>(),
        response: StructSpec::of::<DebugReverseCausalResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.branch",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugBranchRequest>(),
        response: StructSpec::of::<DebugBranchResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.compare",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<DebugCompareRequest>(),
        response: StructSpec::of::<DebugCompareResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.why_enabled",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<DebugWhyEnabledRequest>(),
        response: StructSpec::of::<DebugWhyEnabledResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.why_blocked",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<DebugWhyBlockedRequest>(),
        response: StructSpec::of::<DebugWhyBlockedResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "debug.export",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<DebugExportRequest>(),
        response: StructSpec::of::<DebugExportResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::PublicationAborted,
        ],
    },
    OperationSpec {
        name: "context.compile",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ContextCompileRequest>(),
        response: StructSpec::of::<ContextCompileResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "context.expand",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ContextExpandRequest>(),
        response: StructSpec::of::<ContextExpandResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "failure.explain",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::TaskStarting],
        request: StructSpec::of::<FailureExplainRequest>(),
        response: StructSpec::of::<FailureExplainResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "failure.minimize",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<FailureMinimizeRequest>(),
        response: StructSpec::of::<FailureMinimizeResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "failure.branch",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<FailureBranchRequest>(),
        response: StructSpec::of::<FailureBranchResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "repair.begin",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<RepairBeginRequest>(),
        response: StructSpec::of::<RepairBeginResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::PolicyGateFailed,
        ],
    },
    OperationSpec {
        name: "repair.apply",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<RepairApplyRequest>(),
        response: StructSpec::of::<RepairApplyResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::IntentMutationDenied,
            ErrorCode::UnsupportedSemanticFeature,
        ],
    },
    OperationSpec {
        name: "repair.attach",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<RepairAttachRequest>(),
        response: StructSpec::of::<RepairAttachResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::InsufficientEvidence, ErrorCode::StatusConflict],
    },
    OperationSpec {
        name: "repair.evaluate",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<RepairEvaluateRequest>(),
        response: StructSpec::of::<RepairEvaluateResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::BudgetExhausted,
            ErrorCode::InsufficientEvidence,
            ErrorCode::PolicyGateFailed,
        ],
    },
    OperationSpec {
        name: "repair.resume",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<RepairResumeRequest>(),
        response: StructSpec::of::<RepairResumeResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::StaleSnapshot,
            ErrorCode::ContinuationEpochMismatch,
            ErrorCode::BudgetExhausted,
            ErrorCode::EpochUnsupported,
        ],
    },
    OperationSpec {
        name: "repair.review",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<RepairReviewRequest>(),
        response: StructSpec::of::<RepairReviewResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[ErrorCode::InsufficientEvidence],
    },
    OperationSpec {
        name: "repair.promote",
        authority: AuthorityLevel::Promote,
        annotations: &[
            Annotation::Mutation,
            Annotation::Privileged,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<RepairPromoteRequest>(),
        response: StructSpec::of::<RepairPromoteResponse>(),
        verdict: Some("PolicyVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::PolicyGateFailed,
            ErrorCode::InsufficientEvidence,
            ErrorCode::StatusConflict,
            ErrorCode::PublicationAborted,
            ErrorCode::CertificateRejected,
        ],
    },
    OperationSpec {
        name: "repair.reject",
        authority: AuthorityLevel::Promote,
        annotations: &[
            Annotation::Mutation,
            Annotation::Privileged,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<RepairRejectRequest>(),
        response: StructSpec::of::<RepairRejectResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::StatusConflict],
    },
    OperationSpec {
        name: "observe.ingest",
        authority: AuthorityLevel::Execute,
        annotations: &[
            Annotation::Mutation,
            Annotation::TaskStarting,
            Annotation::AuditRecorded,
        ],
        request: StructSpec::of::<ObserveIngestRequest>(),
        response: StructSpec::of::<ObserveIngestResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::UntrustedDomainBoundary,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "observe.classify",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<ObserveClassifyRequest>(),
        response: StructSpec::of::<ObserveClassifyResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "observe.result",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<ObserveResultRequest>(),
        response: StructSpec::of::<VerificationResult>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "forge.create",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ForgeCreateRequest>(),
        response: StructSpec::of::<ForgeCreateResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "forge.step",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<ForgeStepRequest>(),
        response: StructSpec::of::<ForgeStepResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::BudgetExhausted,
            ErrorCode::UnsupportedSemanticFeature,
        ],
    },
    OperationSpec {
        name: "forge.archive",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<ForgeArchiveRequest>(),
        response: StructSpec::of::<ForgeArchiveResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "forge.materialize",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<ForgeMaterializeRequest>(),
        response: StructSpec::of::<ForgeMaterializeResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::InsufficientEvidence,
        ],
    },
    OperationSpec {
        name: "benchmark.run",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<BenchmarkRunRequest>(),
        response: StructSpec::of::<BenchmarkRunResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::BudgetExhausted,
            ErrorCode::PolicyGateFailed,
        ],
    },
    OperationSpec {
        name: "task.status",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<TaskStatusRequest>(),
        response: StructSpec::of::<TaskRecord>(),
        verdict: None,
        events: None,
        errors: &[],
    },
    OperationSpec {
        name: "task.cancel",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<TaskCancelRequest>(),
        response: StructSpec::of::<TaskCancelResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::PublicationAborted],
    },
    OperationSpec {
        name: "task.resume",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<TaskResumeRequest>(),
        response: StructSpec::of::<TaskResumeResponse>(),
        verdict: None,
        events: None,
        errors: &[
            ErrorCode::StaleSnapshot,
            ErrorCode::ContinuationEpochMismatch,
            ErrorCode::EpochUnsupported,
            ErrorCode::BudgetExhausted,
        ],
    },
    OperationSpec {
        name: "task.subscribe",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly, Annotation::Streaming],
        request: StructSpec::of::<TaskSubscribeRequest>(),
        response: StructSpec::of::<TaskSubscribeResponse>(),
        verdict: None,
        events: Some("TaskEvent"),
        errors: &[],
    },
    OperationSpec {
        name: "task.update_budget",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<TaskUpdateBudgetRequest>(),
        response: StructSpec::of::<TaskUpdateBudgetResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::BudgetExhausted, ErrorCode::QuotaExhausted],
    },
    OperationSpec {
        name: "evidence.get",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly],
        request: StructSpec::of::<EvidenceGetRequest>(),
        response: StructSpec::of::<EvidenceGetResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::EpochUnsupported],
    },
    OperationSpec {
        name: "evidence.query",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<EvidenceQueryRequest>(),
        response: StructSpec::of::<EvidenceQueryResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "evidence.verify",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<EvidenceVerifyRequest>(),
        response: StructSpec::of::<EvidenceVerifyResponse>(),
        verdict: Some("SemanticVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::StatusConflict,
            ErrorCode::CertificateRejected,
            ErrorCode::InsufficientEvidence,
            ErrorCode::EpochUnsupported,
        ],
    },
    OperationSpec {
        name: "evidence.subscribe",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly, Annotation::Streaming],
        request: StructSpec::of::<EvidenceSubscribeRequest>(),
        response: StructSpec::of::<EvidenceSubscribeResponse>(),
        verdict: None,
        events: Some("EvidenceEvent"),
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "evidence.link",
        authority: AuthorityLevel::Execute,
        annotations: &[Annotation::Mutation, Annotation::AuditRecorded],
        request: StructSpec::of::<EvidenceLinkRequest>(),
        response: StructSpec::of::<EvidenceLinkResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::InsufficientEvidence],
    },
    OperationSpec {
        name: "whiteboard.compile",
        authority: AuthorityLevel::Propose,
        annotations: &[Annotation::Mutation],
        request: StructSpec::of::<WhiteboardCompileRequest>(),
        response: StructSpec::of::<WhiteboardCompileResponse>(),
        verdict: Some("StructuralVerdictValue"),
        events: None,
        errors: &[ErrorCode::InsufficientEvidence],
    },
    OperationSpec {
        name: "query.explain_reuse",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<QueryExplainReuseRequest>(),
        response: StructSpec::of::<QueryExplainReuseResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "query.explain_invalidation",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Readonly, Annotation::Paginated],
        request: StructSpec::of::<QueryExplainInvalidationRequest>(),
        response: StructSpec::of::<QueryExplainInvalidationResponse>(),
        verdict: None,
        events: None,
        errors: &[ErrorCode::UnsupportedSemanticFeature],
    },
    OperationSpec {
        name: "query.clean_compare",
        authority: AuthorityLevel::Read,
        annotations: &[Annotation::Mutation, Annotation::TaskStarting],
        request: StructSpec::of::<QueryCleanCompareRequest>(),
        response: StructSpec::of::<QueryCleanCompareResponse>(),
        verdict: Some("EvaluationVerdictValue"),
        events: None,
        errors: &[
            ErrorCode::BudgetExhausted,
            ErrorCode::UnsupportedSemanticFeature,
        ],
    },
];

/// Every named struct the IDL declares, in its declaration order.
pub const NAMED_STRUCTS: &[StructSpec] = &[
    StructSpec::of::<Budget>(),
    StructSpec::of::<Cost>(),
    StructSpec::of::<OutputPolicy>(),
    StructSpec::of::<TraceContext>(),
    StructSpec::of::<Page>(),
    StructSpec::of::<EpochSet>(),
    StructSpec::of::<Redacted>(),
    StructSpec::of::<ProducedDimension>(),
    StructSpec::of::<UnsupportedDimension>(),
    StructSpec::of::<AssuranceEnvelope>(),
    StructSpec::of::<ArtifactRef>(),
    StructSpec::of::<Omission>(),
    StructSpec::of::<Warning>(),
    StructSpec::of::<Diagnostic>(),
    StructSpec::of::<SourceSpan>(),
    StructSpec::of::<NextOperation>(),
    StructSpec::of::<Error>(),
    StructSpec::of::<CertificateRejection>(),
    StructSpec::of::<RequestEnvelope>(),
    StructSpec::of::<ResultEnvelope>(),
    StructSpec::of::<SemanticVerdictValue>(),
    StructSpec::of::<EvaluationVerdictValue>(),
    StructSpec::of::<PolicyVerdictValue>(),
    StructSpec::of::<StructuralVerdictValue>(),
    StructSpec::of::<GateOutcome>(),
    StructSpec::of::<VersionRange>(),
    StructSpec::of::<ClientHello>(),
    StructSpec::of::<ServerWelcome>(),
    StructSpec::of::<ServerReject>(),
    StructSpec::of::<EpochAdvanceNotice>(),
    StructSpec::of::<CapabilityDescriptor>(),
    StructSpec::of::<CapabilityProfile>(),
    StructSpec::of::<ServerLimits>(),
    StructSpec::of::<TaskRecord>(),
    StructSpec::of::<Milestone>(),
    StructSpec::of::<TaskEvent>(),
    StructSpec::of::<EvidenceEvent>(),
    StructSpec::of::<Target>(),
    StructSpec::of::<SnapshotComponents>(),
    StructSpec::of::<FileComponent>(),
    StructSpec::of::<SnapshotEpochs>(),
    StructSpec::of::<FileOverlay>(),
    StructSpec::of::<ContextPolicy>(),
    StructSpec::of::<IntentChangeSet>(),
    StructSpec::of::<WhiteboardTaskProposal>(),
    StructSpec::of::<EvidenceQuery>(),
    StructSpec::of::<VerificationResult>(),
];

/// Every enum the IDL declares, in its declaration order.
pub const ENUMS: &[EnumSpec] = &[
    EnumSpec::of::<AuthorityLevel>(),
    EnumSpec::of::<Encoding>(),
    EnumSpec::of::<ResultStatus>(),
    EnumSpec::of::<TaskStatus>(),
    EnumSpec::of::<SemanticVerdict>(),
    EnumSpec::of::<EvaluationVerdict>(),
    EnumSpec::of::<InconclusiveReason>(),
    EnumSpec::of::<EvidenceStatus>(),
    EnumSpec::of::<AssuranceClass>(),
    EnumSpec::of::<EvidenceKind>(),
    EnumSpec::of::<EvidenceNodeKind>(),
    EnumSpec::of::<EvidenceEdgeKind>(),
    EnumSpec::of::<Fragment>(),
    EnumSpec::of::<PolicyDecision>(),
    EnumSpec::of::<OmissionReason>(),
    EnumSpec::of::<RedactionReason>(),
    EnumSpec::of::<Compatibility>(),
    EnumSpec::of::<PriorityClass>(),
    EnumSpec::of::<Audience>(),
    EnumSpec::of::<Portfolio>(),
    EnumSpec::of::<GateName>(),
    EnumSpec::of::<GateStatus>(),
    EnumSpec::of::<ExpansionRelation>(),
    EnumSpec::of::<TargetKind>(),
    EnumSpec::of::<DiagnosticSeverity>(),
    EnumSpec::of::<ErrorCode>(),
    EnumSpec::of::<StructuralOutcome>(),
    EnumSpec::of::<DataGrant>(),
    EnumSpec::of::<TaskEventKind>(),
    EnumSpec::of::<EvidenceEventKind>(),
    EnumSpec::of::<DiffLayer>(),
    EnumSpec::of::<ExplorationStrategy>(),
    EnumSpec::of::<ExplanationLevel>(),
    EnumSpec::of::<GateProfile>(),
];

/// Every union the IDL declares, in its declaration order.
pub const UNIONS: &[UnionSpec] = &[
    UnionSpec::of::<EnvelopeDimension>(),
    UnionSpec::of::<Verdict>(),
];

/// Every handle class the IDL declares, in its declaration order.
pub const HANDLES: &[HandleSpec] = &[
    HandleSpec::of::<WorkspaceHandle>(),
    HandleSpec::of::<IntentHandle>(),
    HandleSpec::of::<IntentBundleHandle>(),
    HandleSpec::of::<ModelHandle>(),
    HandleSpec::of::<CausalGraphHandle>(),
    HandleSpec::of::<CrashpackHandle>(),
    HandleSpec::of::<ContextHandle>(),
    HandleSpec::of::<ProofArtifactHandle>(),
    HandleSpec::of::<ProofStateHandle>(),
    HandleSpec::of::<TaskHandle>(),
    HandleSpec::of::<EvidenceHandle>(),
    HandleSpec::of::<DebugHandle>(),
    HandleSpec::of::<ReceiptHandle>(),
    HandleSpec::of::<RepairHandle>(),
    HandleSpec::of::<ForgeHandle>(),
    HandleSpec::of::<ContinuationHandle>(),
    HandleSpec::of::<CapabilityHandle>(),
    HandleSpec::of::<DiffHandle>(),
    HandleSpec::of::<DefectHandle>(),
];

/// Every string alias the IDL declares, in its declaration order.
pub const ALIASES: &[AliasSpec] = &[
    AliasSpec {
        name: "ArtifactHandle",
        base: "String",
        pattern: Some(r"^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$"),
    },
    AliasSpec {
        name: "RequestId",
        base: "String",
        pattern: Some(r"^req_[A-Za-z0-9_-]+$"),
    },
    AliasSpec {
        name: "ActorId",
        base: "String",
        pattern: Some(r"^(agent|human|service|ci):[A-Za-z0-9._:-]+$"),
    },
    AliasSpec {
        name: "ProtocolVersion",
        base: "String",
        pattern: Some(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$"),
    },
    AliasSpec {
        name: "EpochIdentity",
        base: "String",
        pattern: Some(r"^[!-~]+$"),
    },
    AliasSpec {
        name: "Commitment",
        base: "String",
        pattern: None,
    },
    AliasSpec {
        name: "PageToken",
        base: "String",
        pattern: None,
    },
    AliasSpec {
        name: "OperationName",
        base: "String",
        pattern: Some(r"^[a-z]+\.[a-z_]+$"),
    },
    AliasSpec {
        name: "AuditCorrelationId",
        base: "String",
        pattern: Some("^[A-Za-z0-9_-]+$"),
    },
];

/// Look an operation up by its wire name.
#[must_use]
pub fn operation(name: &str) -> Option<&'static OperationSpec> {
    OPERATIONS.iter().find(|spec| spec.name == name)
}
