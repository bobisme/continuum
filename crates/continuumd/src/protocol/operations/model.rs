//! The `model` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `model.check`.
    struct ModelCheckRequest {
        /// IDL `model: ModelHandle required`.
        model: ModelHandle required;
        /// IDL `target: Target required`.
        target: Target required;
        /// IDL `context_policy: ContextPolicy optional`.
        context_policy: ContextPolicy optional;
    }
}

protocol_struct! {
    /// The `request` body of `model.explore`.
    struct ModelExploreRequest {
        /// IDL `model: ModelHandle required`.
        model: ModelHandle required;
        /// IDL `strategy: ExplorationStrategy required`.
        strategy: ExplorationStrategy required;
    }
}

protocol_struct! {
    /// The `response` body of `model.explore`.
    struct ModelExploreResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `evidence: list<EvidenceHandle> required`.
        evidence: list<EvidenceHandle> required;
        /// IDL `causal_graph: CausalGraphHandle optional`.
        causal_graph: CausalGraphHandle optional;
    }
}

protocol_struct! {
    /// The `request` body of `model.compare`.
    struct ModelCompareRequest {
        /// IDL `before: ModelHandle required`.
        before: ModelHandle required;
        /// IDL `after: ModelHandle required`.
        after: ModelHandle required;
        /// IDL `layers: list<DiffLayer> required`.
        layers: list<DiffLayer> required;
    }
}

protocol_struct! {
    /// The `response` body of `model.compare`.
    struct ModelCompareResponse {
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
        /// IDL `summary: Opaque required`.
        summary: Opaque required;
    }
}
