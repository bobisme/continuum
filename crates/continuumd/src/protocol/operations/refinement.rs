//! The `refinement` namespace: request and response structs for its 2 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `refinement.check`.
    struct RefinementCheckRequest {
        /// IDL `program: ModelHandle required`.
        program: ModelHandle required;
        /// IDL `model: ModelHandle required`.
        model: ModelHandle required;
        /// Observer projection to check under (RFC 0014).
        observer: String optional;
    }
}

protocol_struct! {
    /// The `request` body of `refinement.explain`.
    struct RefinementExplainRequest {
        /// IDL `result: EvidenceHandle required`.
        result: EvidenceHandle required;
        /// IDL `audience: Audience optional`.
        audience: Audience optional;
    }
}

protocol_struct! {
    /// The `response` body of `refinement.explain`.
    struct RefinementExplainResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
    }
}
