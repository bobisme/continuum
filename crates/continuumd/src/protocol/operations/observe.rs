//! The `observe` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `observe.ingest`.
    struct ObserveIngestRequest {
        /// Trace bundle content identity.
        trace: Commitment required;
        /// Instrumentation profile the trace was captured under.
        instrumentation_profile: String required;
    }
}

protocol_struct! {
    /// The `response` body of `observe.ingest`.
    struct ObserveIngestResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `evidence: list<EvidenceHandle> required`.
        evidence: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `observe.classify`.
    struct ObserveClassifyRequest {
        /// IDL `evidence: EvidenceHandle required`.
        evidence: EvidenceHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `observe.classify`.
    struct ObserveClassifyResponse {
        /// IDL `classification: Opaque required`.
        classification: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `observe.result`.
    struct ObserveResultRequest {
        /// IDL `evidence: EvidenceHandle required`.
        evidence: EvidenceHandle required;
    }
}
