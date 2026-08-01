//! The request and result envelopes and the values they carry (IDL §6).
//!
//! Every operation is invoked through [`RequestEnvelope`] and answered through
//! [`ResultEnvelope`]; the operation's own request and response structs travel in
//! `arguments` and `payload` (RFC 0026, "Request envelope" and "Result envelope").

use super::prelude::*;
use crate::{protocol_struct, protocol_union};

protocol_struct! {
    /// The budget dimensions of `schemas/verification-task.schema.json`.
    /// The same nine dimensions are used for requested budget, reported
    /// cost, and quota accounting; no other dimension exists (plan §8.6,
    /// SD-12 "one budget/cost dimension list").
    struct Budget {
        /// IDL `wall_ms: DurationMs optional`.
        wall_ms: DurationMs optional;
        /// IDL `cpu_ms: DurationMs optional`.
        cpu_ms: DurationMs optional;
        /// IDL `memory_bytes: ByteCount optional`.
        memory_bytes: ByteCount optional;
        /// IDL `states: U64 optional`.
        states: U64 optional;
        /// IDL `solver_ms: DurationMs optional`.
        solver_ms: DurationMs optional;
        /// IDL `proof_ms: DurationMs optional`.
        proof_ms: DurationMs optional;
        /// Advisory and tokenizer-relative; never the enforced contract.
        tokens: U64 optional;
        /// IDL `candidates: U64 optional`.
        candidates: U64 optional;
        /// The enforced context contract (RFC 0027).
        bytes: ByteCount optional;
    }
}

protocol_struct! {
    /// Actual spend, reported in the same nine dimensions as `Budget`.
    /// A dimension the engine does not measure is absent, never zero.
    struct Cost {
        /// IDL `wall_ms: DurationMs optional`.
        wall_ms: DurationMs optional;
        /// IDL `cpu_ms: DurationMs optional`.
        cpu_ms: DurationMs optional;
        /// IDL `memory_bytes: ByteCount optional`.
        memory_bytes: ByteCount optional;
        /// IDL `states: U64 optional`.
        states: U64 optional;
        /// IDL `solver_ms: DurationMs optional`.
        solver_ms: DurationMs optional;
        /// IDL `proof_ms: DurationMs optional`.
        proof_ms: DurationMs optional;
        /// IDL `tokens: U64 optional`.
        tokens: U64 optional;
        /// IDL `candidates: U64 optional`.
        candidates: U64 optional;
        /// IDL `bytes: ByteCount optional`.
        bytes: ByteCount optional;
        /// Tokenizer identity, REQUIRED whenever `tokens` is reported.
        tokenizer_id: String optional;
    }
}

protocol_struct! {
    /// Bounds on what the daemon returns (RFC 0026 request envelope,
    /// RFC 0027 "Context policy").
    struct OutputPolicy {
        /// Enforced byte ceiling on the result payload.
        max_bytes: ByteCount optional;
        /// Advisory token ceiling; bytes remain the enforced contract.
        max_tokens: U64 optional;
        /// Ceiling on returned graph nodes.
        max_nodes: U64 optional;
        /// IDL `audience: Audience optional`.
        audience: Audience optional;
    }
}

protocol_struct! {
    /// W3C Trace Context / OpenTelemetry propagation.
    struct TraceContext {
        /// IDL `traceparent: String required`.
        traceparent: String required;
        /// IDL `tracestate: String optional`.
        tracestate: String optional;
    }
}

protocol_struct! {
    /// Page request for a list-returning operation.
    struct Page {
        /// IDL `page_size: U32 optional`.
        page_size: U32 optional;
        /// IDL `page_token: PageToken optional`.
        page_token: PageToken optional;
    }
}

protocol_struct! {
    /// The epochs a result, artifact, or continuation is pinned to.
    ///
    /// All six independently versioned epochs (docs/12 §7,
    /// `crates/continuum-value/src/epoch.rs`) MUST be named in every result:
    /// an epoch the daemon cannot pin reads `null` — a named absence, never
    /// an omitted field. `engine` is engine identity (plan §4.7
    /// "Engine-defect lifecycle"), not one of the six; it is carried here
    /// because continuations pin it.
    struct EpochSet {
        /// The negotiated protocol version; never null on a served
        /// connection.
        protocol: ProtocolVersion required;
        /// IDL `semantic: EpochIdentity nullable`.
        semantic: EpochIdentity nullable;
        /// IDL `intent: EpochIdentity nullable`.
        intent: EpochIdentity nullable;
        /// IDL `evidence: EpochIdentity nullable`.
        evidence: EpochIdentity nullable;
        /// IDL `proof: EpochIdentity nullable`.
        proof: EpochIdentity nullable;
        /// IDL `corpus: EpochIdentity nullable`.
        corpus: EpochIdentity nullable;
        /// Engine identity (plan §4.7), pinned by continuations.
        engine: EpochIdentity nullable;
    }
}

protocol_struct! {
    /// A typed `Redacted(reason, commitment)` stub (plan §4.5;
    /// `schemas/redacted.schema.json`). A redacted value is reported, never
    /// silently dropped; claims requiring the hidden data downgrade per plan
    /// §18.4.
    struct Redacted {
        /// IDL `redacted: Bool required`.
        redacted: Bool required;
        /// IDL `reason: RedactionReason required`.
        reason: RedactionReason required;
        /// IDL `commitment: Commitment required`.
        commitment: Commitment required;
        /// Artifact class of the redacted original (plan §4.4 prefix).
        original_class: String required;
    }
}

protocol_struct! {
    /// An assurance dimension that was established, and by which engine.
    struct ProducedDimension {
        /// IDL `engine: String required`.
        engine: String required;
        /// IDL `summary: String required`.
        summary: String required;
    }
}

protocol_struct! {
    /// An assurance dimension the campaign could not establish, with its typed reason.
    ///
    /// A dimension is never silently omitted (plan B11): it names a producer or it says
    /// why it has none.
    struct UnsupportedDimension {
        /// Typed reason, e.g. `sequential-consistency-only`.
        reason: String required;
    }
}

protocol_struct! {
    /// The nine-dimension assurance envelope. All nine dimensions are
    /// REQUIRED on every semantic verdict (plan B11); the member list is
    /// identical to `schemas/assurance-result.schema.json` and
    /// `schemas/context-pack.schema.json`, which the validator holds
    /// identical to each other (SD-12).
    struct AssuranceEnvelope {
        /// IDL `bounds: EnvelopeDimension required`.
        bounds: EnvelopeDimension required;
        /// IDL `faults: EnvelopeDimension required`.
        faults: EnvelopeDimension required;
        /// IDL `fairness: EnvelopeDimension required`.
        fairness: EnvelopeDimension required;
        /// IDL `values: EnvelopeDimension required`.
        values: EnvelopeDimension required;
        /// IDL `schedules: EnvelopeDimension required`.
        schedules: EnvelopeDimension required;
        /// IDL `memory_model: EnvelopeDimension required`.
        memory_model: EnvelopeDimension required;
        /// IDL `observer: EnvelopeDimension required`.
        observer: EnvelopeDimension required;
        /// IDL `proof_status: EnvelopeDimension required`.
        proof_status: EnvelopeDimension required;
        /// IDL `unknowns: EnvelopeDimension required`.
        unknowns: EnvelopeDimension required;
    }
}

protocol_struct! {
    /// A typed artifact reference in a result.
    struct ArtifactRef {
        /// Artifact class, the plan §4.4 prefix without the underscore.
        kind: String required;
        /// IDL `handle: ArtifactHandle required`.
        handle: ArtifactHandle required;
        /// Content commitment, when the artifact is content-addressed.
        commitment: Commitment optional;
        /// Present when the referenced content is redacted (plan §4.5).
        redacted: Redacted optional;
    }
}

protocol_struct! {
    /// Something the result deliberately left out (INV-007).
    struct Omission {
        /// IDL `reason: OmissionReason required`.
        reason: OmissionReason required;
        /// What was omitted, in typed terms — never free-form prose about the
        /// user's source.
        subject: String required;
        /// Handle whose expansion would recover it, when one exists.
        recoverable_by: ArtifactHandle optional;
    }
}

protocol_struct! {
    /// A typed warning. Warnings never carry interpolated source text
    /// (INV-016).
    struct Warning {
        /// IDL `code: String required`.
        code: String required;
        /// IDL `detail: String required`.
        detail: String required;
    }
}

protocol_struct! {
    /// A typed diagnostic attached to a structural operation.
    struct Diagnostic {
        /// IDL `severity: DiagnosticSeverity required`.
        severity: DiagnosticSeverity required;
        /// IDL `code: String required`.
        code: String required;
        /// IDL `detail: String required`.
        detail: String required;
        /// Source correspondence, when the diagnostic has one.
        span: SourceSpan optional;
    }
}

protocol_struct! {
    /// A half-open source region, in the snapshot's file coordinates.
    struct SourceSpan {
        /// IDL `file: String required`.
        file: String required;
        /// IDL `start_line: U32 required`.
        start_line: U32 required;
        /// IDL `start_column: U32 required`.
        start_column: U32 required;
        /// IDL `end_line: U32 required`.
        end_line: U32 required;
        /// IDL `end_column: U32 required`.
        end_column: U32 required;
    }
}

protocol_struct! {
    /// An allowed next operation with pre-filled arguments. This is the safe
    /// recovery and discovery surface (plan §0.2): a client MAY execute it
    /// as given. It is never a command string, never shell, and never
    /// contains interpolated source, log, or model text (INV-016).
    struct NextOperation {
        /// IDL `operation: OperationName required`.
        operation: OperationName required;
        /// Arguments matching that operation's request struct.
        arguments: Opaque required;
        /// Why this operation is offered, in typed terms.
        rationale: String optional;
    }
}

protocol_struct! {
    /// A typed error. `recovery` is the only recovery channel: a list of
    /// allowed operations with pre-filled arguments, never free-form
    /// commands (RFC 0026 "Rejected alternatives").
    struct Error {
        /// IDL `code: ErrorCode required`.
        code: ErrorCode required;
        /// Stable, non-interpolated explanation of the code in context.
        detail: String required;
        /// Typed, machine-readable specifics of this occurrence; the shape is
        /// determined by `code`.
        data: Opaque optional;
        /// Allowed operations from this state.
        recovery: list<NextOperation> required;
        /// Present when the failure is resumable (`BudgetExhausted`).
        continuation: ContinuationHandle optional;
        /// Present when `BudgetExhausted` is not resumable: the typed reason
        /// no continuation exists
        /// (`schemas/verification-task.schema.json`).
        non_resumable_reason: String optional;
        /// Whether an identical retry can succeed without any change by the
        /// caller.
        retryable: Bool required;
    }
}

protocol_struct! {
    /// The request envelope of every operation (RFC 0026).
    struct RequestEnvelope {
        /// The negotiated protocol version. A request naming a different
        /// version than the connection negotiated MUST be rejected with
        /// `ProtocolVersionUnsupported`.
        protocol_version: ProtocolVersion required;
        /// IDL `request_id: RequestId required`.
        request_id: RequestId required;
        /// REQUIRED for `@mutation` operations, absent for `@readonly` ones.
        idempotency_key: String optional;
        /// IDL `actor: ActorId required`.
        actor: ActorId required;
        /// Checked below the adapter, independently of handle possession.
        capability: CapabilityHandle required;
        /// IDL `operation: OperationName required`.
        operation: OperationName required;
        /// Explicit null when the operation takes no snapshot. The field name
        /// is `snapshot` everywhere, schemas included.
        snapshot: WorkspaceHandle nullable;
        /// Explicit null when the operation takes no intent.
        intent: IntentHandle nullable;
        /// The operation's request struct.
        arguments: Opaque required;
        /// REQUIRED for `@task_starting` operations.
        budget: Budget optional;
        /// IDL `output_policy: OutputPolicy optional`.
        output_policy: OutputPolicy optional;
        /// IDL `trace: TraceContext optional`.
        trace: TraceContext optional;
        /// Page request; REQUIRED to be honored by `@paginated` operations.
        page: Page optional;
    }
}

protocol_struct! {
    /// The result envelope of every operation (RFC 0026).
    struct ResultEnvelope {
        /// IDL `request_id: RequestId required`.
        request_id: RequestId required;
        /// IDL `status: ResultStatus required`.
        status: ResultStatus required;
        /// Operation-specific typed verdict, never bare prose. Null when the
        /// operation declares no `verdict` clause, and on `status = error`.
        verdict: Verdict nullable;
        /// Present exactly when `status = error`.
        error: Error optional;
        /// REQUIRED on every semantic verdict (plan B11). Every dimension
        /// names a producer or reads `Unsupported`.
        assurance: AssuranceEnvelope optional;
        /// IDL `artifacts: list<ArtifactRef> required`.
        artifacts: list<ArtifactRef> required;
        /// Present when `status` is `task_started` or `task_suspended`, and on
        /// results of task-observing operations.
        task: TaskHandle optional;
        /// Present when `status = task_suspended`, and on a resumable failure.
        continuation: ContinuationHandle optional;
        /// The INV-007 omission manifest. Empty list means nothing was
        /// omitted; the field is never absent.
        omissions: list<Omission> required;
        /// IDL `warnings: list<Warning> required`.
        warnings: list<Warning> required;
        /// Actual spend per budget dimension.
        cost: Cost required;
        /// All six epochs, each pinned or explicitly null.
        epochs: EpochSet required;
        /// Allowed operations from this state — the safe recovery and
        /// discovery surface (plan §0.2).
        next_operations: list<NextOperation> required;
        /// Present on `@paginated` operations; null on the last page.
        next_page_token: PageToken optional;
        /// The operation's response struct. Null on `status = error`.
        payload: Opaque nullable;
    }
}

protocol_struct! {
    /// The `semantic` verdict variant: a claim-level outcome and the assurance class it
    /// is supported at.
    struct SemanticVerdictValue {
        /// IDL `verdict: SemanticVerdict required`.
        verdict: SemanticVerdict required;
        /// REQUIRED when `verdict = inconclusive` (INV-008).
        inconclusive_reason: InconclusiveReason optional;
        /// The assurance class this verdict is supported at.
        assurance_class: AssuranceClass required;
    }
}

protocol_struct! {
    /// The `evaluation` verdict variant: the outcome of a run or check, and the
    /// assurance class it is supported at.
    struct EvaluationVerdictValue {
        /// IDL `verdict: EvaluationVerdict required`.
        verdict: EvaluationVerdict required;
        /// REQUIRED when `verdict = inconclusive` (INV-008).
        inconclusive_reason: InconclusiveReason optional;
        /// IDL `assurance_class: AssuranceClass required`.
        assurance_class: AssuranceClass required;
    }
}

protocol_struct! {
    /// The `policy` verdict variant: a gate or policy decision and the gates behind it.
    struct PolicyVerdictValue {
        /// IDL `decision: PolicyDecision required`.
        decision: PolicyDecision required;
        /// Gates evaluated, with their outcomes.
        gates: list<GateOutcome> required;
    }
}

protocol_struct! {
    /// The `structural` verdict variant: what a non-semantic operation did to an
    /// artifact.
    struct StructuralVerdictValue {
        /// IDL `outcome: StructuralOutcome required`.
        outcome: StructuralOutcome required;
    }
}

protocol_struct! {
    /// One repair gate's outcome and the evidence it rests on.
    struct GateOutcome {
        /// IDL `name: GateName required`.
        name: GateName required;
        /// IDL `status: GateStatus required`.
        status: GateStatus required;
        /// IDL `evidence: list<EvidenceHandle> required`.
        evidence: list<EvidenceHandle> required;
    }
}

protocol_union! {
    /// One dimension of the nine-dimension assurance envelope (plan B11;
    /// `schemas/assurance-result.schema.json`): it names its producing
    /// engine or carries a typed `Unsupported(reason)`. A dimension is never
    /// silently omitted.
    union EnvelopeDimension {
        /// IDL variant `produced`.
        produced(ProducedDimension) => Produced,
        /// IDL variant `unsupported`.
        unsupported(UnsupportedDimension) => Unsupported,
    }
}

protocol_union! {
    /// The typed verdict families. An operation's `verdict` clause names the
    /// variant it returns; an operation without a `verdict` clause returns
    /// `verdict: null`.
    union Verdict {
        /// Claim-level verdict plus its INV-008 reason when inconclusive.
        semantic(SemanticVerdictValue) => Semantic,
        /// Evaluation outcome of a run or check.
        evaluation(EvaluationVerdictValue) => Evaluation,
        /// Outcome of a policy or gate decision.
        policy(PolicyVerdictValue) => Policy,
        /// Outcome of a structural (non-semantic) operation.
        structural(StructuralVerdictValue) => Structural,
    }
}
