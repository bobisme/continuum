//! The `failure` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `failure.explain`.
    struct FailureExplainRequest {
        /// IDL `failure: CrashpackHandle required`.
        failure: CrashpackHandle required;
        /// IDL `level: ExplanationLevel required`.
        level: ExplanationLevel required;
        /// IDL `audience: Audience optional`.
        audience: Audience optional;
    }
}

protocol_struct! {
    /// The `response` body of `failure.explain`.
    struct FailureExplainResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `failure.minimize`.
    struct FailureMinimizeRequest {
        /// IDL `failure: CrashpackHandle required`.
        failure: CrashpackHandle required;
        /// Minimality class to target
        /// (`schemas/context-pack.schema.json` guarantees).
        guarantee: String optional;
    }
}

protocol_struct! {
    /// The `response` body of `failure.minimize`.
    struct FailureMinimizeResponse {
        /// IDL `crashpack: CrashpackHandle required`.
        crashpack: CrashpackHandle required;
        /// IDL `guarantees: list<String> required`.
        guarantees: list<String> required;
    }
}

protocol_struct! {
    /// The `request` body of `failure.branch`.
    struct FailureBranchRequest {
        /// IDL `failure: CrashpackHandle required`.
        failure: CrashpackHandle required;
        /// IDL `alternate: String required`.
        alternate: String required;
    }
}

protocol_struct! {
    /// The `response` body of `failure.branch`.
    struct FailureBranchResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `context: ContextHandle optional`.
        context: ContextHandle optional;
    }
}
