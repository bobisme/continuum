//! The `debug` namespace: request and response structs for its 11 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `debug.open`.
    struct DebugOpenRequest {
        /// IDL `subject: ArtifactHandle required`.
        subject: ArtifactHandle required;
        /// IDL `observer: String optional`.
        observer: String optional;
    }
}

protocol_struct! {
    /// The `response` body of `debug.open`.
    struct DebugOpenResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `frontier: Opaque required`.
        frontier: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.state`.
    struct DebugStateRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `observer: String optional`.
        observer: String optional;
    }
}

protocol_struct! {
    /// The `response` body of `debug.state`.
    struct DebugStateResponse {
        /// IDL `state: Opaque required`.
        state: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.enabled`.
    struct DebugEnabledRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.enabled`.
    struct DebugEnabledResponse {
        /// IDL `events: list<String> required`.
        events: list<String> required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.step_event`.
    struct DebugStepEventRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `event: String required`.
        event: String required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.step_event`.
    struct DebugStepEventResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `delta: Opaque required`.
        delta: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.step_abstract`.
    struct DebugStepAbstractRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.step_abstract`.
    struct DebugStepAbstractResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `delta: Opaque required`.
        delta: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.reverse_causal`.
    struct DebugReverseCausalRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.reverse_causal`.
    struct DebugReverseCausalResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `delta: Opaque required`.
        delta: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.branch`.
    struct DebugBranchRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `alternate: String required`.
        alternate: String required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.branch`.
    struct DebugBranchResponse {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.compare`.
    struct DebugCompareRequest {
        /// IDL `left: DebugHandle required`.
        left: DebugHandle required;
        /// IDL `right: DebugHandle required`.
        right: DebugHandle required;
        /// IDL `observer: String optional`.
        observer: String optional;
    }
}

protocol_struct! {
    /// The `response` body of `debug.compare`.
    struct DebugCompareResponse {
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.why_enabled`.
    struct DebugWhyEnabledRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `event: String required`.
        event: String required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.why_enabled`.
    struct DebugWhyEnabledResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.why_blocked`.
    struct DebugWhyBlockedRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `event: String required`.
        event: String required;
    }
}

protocol_struct! {
    /// The `response` body of `debug.why_blocked`.
    struct DebugWhyBlockedResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `debug.export`.
    struct DebugExportRequest {
        /// IDL `branch: DebugHandle required`.
        branch: DebugHandle required;
        /// IDL `as_regression: Bool optional`.
        as_regression: Bool optional;
    }
}

protocol_struct! {
    /// The `response` body of `debug.export`.
    struct DebugExportResponse {
        /// IDL `crashpack: CrashpackHandle required`.
        crashpack: CrashpackHandle required;
    }
}
