//! The `correspondence` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `correspondence.bind`.
    struct CorrespondenceBindRequest {
        /// IDL `source: SourceSpan required`.
        source: SourceSpan required;
        /// IDL `element: String required`.
        element: String required;
    }
}

protocol_struct! {
    /// The `response` body of `correspondence.bind`.
    struct CorrespondenceBindResponse {
        /// IDL `correspondence: Commitment required`.
        correspondence: Commitment required;
    }
}

protocol_struct! {
    /// The `request` body of `correspondence.status`.
    struct CorrespondenceStatusRequest {
        /// IDL `element: String optional`.
        element: String optional;
    }
}

protocol_struct! {
    /// The `response` body of `correspondence.status`.
    struct CorrespondenceStatusResponse {
        /// IDL `bound: U64 required`.
        bound: U64 required;
        /// IDL `unbound: U64 required`.
        unbound: U64 required;
        /// IDL `ambiguous: U64 required`.
        ambiguous: U64 required;
    }
}

protocol_struct! {
    /// The `request` body of `correspondence.drift`.
    struct CorrespondenceDriftRequest {
        /// IDL `before: WorkspaceHandle required`.
        before: WorkspaceHandle required;
        /// IDL `after: WorkspaceHandle required`.
        after: WorkspaceHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `correspondence.drift`.
    struct CorrespondenceDriftResponse {
        /// IDL `drifted: list<String> required`.
        drifted: list<String> required;
        /// IDL `diff: DiffHandle optional`.
        diff: DiffHandle optional;
    }
}
