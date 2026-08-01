//! The `forge` namespace: request and response structs for its 4 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `forge.create`.
    struct ForgeCreateRequest {
        /// IDL `sketch: Opaque required`.
        sketch: Opaque required;
        /// IDL `objectives: list<String> required`.
        objectives: list<String> required;
        /// IDL `diversity_descriptors: list<String> optional`.
        diversity_descriptors: list<String> optional;
        /// IDL `assurance_target: AssuranceClass required`.
        assurance_target: AssuranceClass required;
    }
}

protocol_struct! {
    /// The `response` body of `forge.create`.
    struct ForgeCreateResponse {
        /// IDL `forge: ForgeHandle required`.
        forge: ForgeHandle required;
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
    }
}

protocol_struct! {
    /// The `request` body of `forge.step`.
    struct ForgeStepRequest {
        /// IDL `forge: ForgeHandle required`.
        forge: ForgeHandle required;
        /// IDL `iterations: U32 optional`.
        iterations: U32 optional;
    }
}

protocol_struct! {
    /// The `response` body of `forge.step`.
    struct ForgeStepResponse {
        /// IDL `forge: ForgeHandle required`.
        forge: ForgeHandle required;
        /// IDL `candidates: list<EvidenceHandle> required`.
        candidates: list<EvidenceHandle> required;
        /// IDL `counterexamples: list<CrashpackHandle> required`.
        counterexamples: list<CrashpackHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `forge.archive`.
    struct ForgeArchiveRequest {
        /// IDL `forge: ForgeHandle required`.
        forge: ForgeHandle required;
        /// IDL `statuses: list<EvidenceStatus> optional`.
        statuses: list<EvidenceStatus> optional;
        /// IDL `descriptors: list<String> optional`.
        descriptors: list<String> optional;
    }
}

protocol_struct! {
    /// The `response` body of `forge.archive`.
    struct ForgeArchiveResponse {
        /// IDL `candidates: list<EvidenceHandle> required`.
        candidates: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `forge.materialize`.
    struct ForgeMaterializeRequest {
        /// IDL `forge: ForgeHandle required`.
        forge: ForgeHandle required;
        /// IDL `candidate: EvidenceHandle required`.
        candidate: EvidenceHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `forge.materialize`.
    struct ForgeMaterializeResponse {
        /// IDL `repair: RepairHandle required`.
        repair: RepairHandle required;
        /// IDL `snapshot: WorkspaceHandle required`.
        snapshot: WorkspaceHandle required;
    }
}
