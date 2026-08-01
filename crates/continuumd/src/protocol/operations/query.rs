//! The `query` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `query.explain_reuse`.
    struct QueryExplainReuseRequest {
        /// IDL `derivation: ArtifactHandle required`.
        derivation: ArtifactHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `query.explain_reuse`.
    struct QueryExplainReuseResponse {
        /// IDL `reused: list<ArtifactHandle> required`.
        reused: list<ArtifactHandle> required;
        /// IDL `recomputed: list<ArtifactHandle> required`.
        recomputed: list<ArtifactHandle> required;
        /// IDL `reasons: map<String,String> required`.
        reasons: map<String,String> required;
    }
}

protocol_struct! {
    /// The `request` body of `query.explain_invalidation`.
    struct QueryExplainInvalidationRequest {
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `query.explain_invalidation`.
    struct QueryExplainInvalidationResponse {
        /// IDL `invalidated: list<ArtifactHandle> required`.
        invalidated: list<ArtifactHandle> required;
        /// IDL `unknown: list<ArtifactHandle> required`.
        unknown: list<ArtifactHandle> required;
        /// IDL `edges: list<String> required`.
        edges: list<String> required;
    }
}

protocol_struct! {
    /// The `request` body of `query.clean_compare`.
    struct QueryCleanCompareRequest {
        /// IDL `derivation: ArtifactHandle required`.
        derivation: ArtifactHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `query.clean_compare`.
    struct QueryCleanCompareResponse {
        /// IDL `parity: Bool required`.
        parity: Bool required;
        /// Engine-defect report, present when parity fails (plan §4.7).
        defect: DefectHandle optional;
    }
}
