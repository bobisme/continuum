//! The `context` namespace: request and response structs for its 2 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `context.compile`.
    struct ContextCompileRequest {
        /// IDL `evidence_root: ArtifactHandle required`.
        evidence_root: ArtifactHandle required;
        /// IDL `question: String required`.
        question: String required;
        /// IDL `audience: Audience optional`.
        audience: Audience optional;
        /// Required guarantees; the pack declares which it achieved.
        guarantees: list<String> optional;
    }
}

protocol_struct! {
    /// The `response` body of `context.compile`.
    struct ContextCompileResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
        /// The pack (`schemas/context-pack.schema.json`).
        pack: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `context.expand`.
    struct ContextExpandRequest {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
        /// IDL `anchor: String required`.
        anchor: String required;
        /// IDL `relation: ExpansionRelation required`.
        relation: ExpansionRelation required;
        /// IDL `depth: U32 optional`.
        depth: U32 optional;
    }
}

protocol_struct! {
    /// The `response` body of `context.expand`.
    struct ContextExpandResponse {
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
        /// IDL `parent: ContextHandle required`.
        parent: ContextHandle required;
        /// IDL `pack: Opaque required`.
        pack: Opaque required;
    }
}
