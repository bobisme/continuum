//! The `verification` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `verification.start`.
    struct VerificationStartRequest {
        /// IDL `target: Target required`.
        target: Target required;
        /// IDL `portfolio: Portfolio required`.
        portfolio: Portfolio required;
        /// IDL `context_policy: ContextPolicy optional`.
        context_policy: ContextPolicy optional;
        /// IDL `priority_class: PriorityClass optional`.
        priority_class: PriorityClass optional;
    }
}

protocol_struct! {
    /// The `response` body of `verification.start`.
    struct VerificationStartResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// Present instead of `task` when a cached result is returned.
        result: VerificationResult optional;
    }
}

protocol_struct! {
    /// The `request` body of `verification.result`.
    struct VerificationResultRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `verification.await`.
    struct VerificationAwaitRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `timeout_ms: DurationMs optional`.
        timeout_ms: DurationMs optional;
    }
}
