//! The `repair` namespace: request and response structs for its 8 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `repair.begin`.
    struct RepairBeginRequest {
        /// IDL `failure: CrashpackHandle required`.
        failure: CrashpackHandle required;
        /// IDL `gate_profile: GateProfile required`.
        gate_profile: GateProfile required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.begin`.
    struct RepairBeginResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.apply`.
    struct RepairApplyRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// Typed changes: rust, model, proof, correspondence, domain,
        /// intent.
        changes: Opaque required;
        /// IDL `hypothesis: String required`.
        hypothesis: String required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.apply`.
    struct RepairApplyResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `candidate_snapshot: WorkspaceHandle required`.
        candidate_snapshot: WorkspaceHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.attach`.
    struct RepairAttachRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `artifacts: list<ArtifactHandle> required`.
        artifacts: list<ArtifactHandle> required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.attach`.
    struct RepairAttachResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.evaluate`.
    struct RepairEvaluateRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.evaluate`.
    struct RepairEvaluateResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `gates: list<GateOutcome> required`.
        gates: list<GateOutcome> required;
        /// IDL `continuation: ContinuationHandle optional`.
        continuation: ContinuationHandle optional;
    }
}

protocol_struct! {
    /// The `request` body of `repair.resume`.
    struct RepairResumeRequest {
        /// IDL `continuation: ContinuationHandle required`.
        continuation: ContinuationHandle required;
        /// IDL `budget: Budget optional`.
        budget: Budget optional;
    }
}

protocol_struct! {
    /// The `response` body of `repair.resume`.
    struct RepairResumeResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `gates: list<GateOutcome> required`.
        gates: list<GateOutcome> required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.review`.
    struct RepairReviewRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.review`.
    struct RepairReviewResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `semantic_diff: DiffHandle nullable`.
        semantic_diff: DiffHandle nullable;
        /// IDL `gates: list<GateOutcome> required`.
        gates: list<GateOutcome> required;
        /// IDL `evidence: list<EvidenceHandle> required`.
        evidence: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.promote`.
    struct RepairPromoteRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.promote`.
    struct RepairPromoteResponse {
        /// IDL `receipt: ReceiptHandle required`.
        receipt: ReceiptHandle required;
        /// The promotion receipt
        /// (`schemas/promotion-receipt.schema.json`).
        promotion_receipt: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `repair.reject`.
    struct RepairRejectRequest {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `reason: String required`.
        reason: String required;
    }
}

protocol_struct! {
    /// The `response` body of `repair.reject`.
    struct RepairRejectResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
    }
}
