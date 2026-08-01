//! The `program` namespace: request and response structs for its 3 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `program.extract`.
    struct ProgramExtractRequest {
        /// Crate roots within the snapshot; empty means the whole snapshot.
        roots: list<String> required;
    }
}

protocol_struct! {
    /// The `response` body of `program.extract`.
    struct ProgramExtractResponse {
        /// IDL `model: ModelHandle required`.
        model: ModelHandle required;
        /// Generated correspondence between source and model (plan §16).
        correspondence: list<Commitment> required;
        /// IDL `diagnostics: list<Diagnostic> required`.
        diagnostics: list<Diagnostic> required;
    }
}

protocol_struct! {
    /// The `request` body of `program.run`.
    struct ProgramRunRequest {
        /// IDL `entry: String required`.
        entry: String required;
        /// Controlled-effects configuration (RFC 0002 domain packs).
        configuration: Opaque optional;
    }
}

protocol_struct! {
    /// The `response` body of `program.run`.
    struct ProgramRunResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `causal_graph: CausalGraphHandle optional`.
        causal_graph: CausalGraphHandle optional;
        /// IDL `crashpack: CrashpackHandle optional`.
        crashpack: CrashpackHandle optional;
    }
}

protocol_struct! {
    /// The `request` body of `program.replay`.
    struct ProgramReplayRequest {
        /// IDL `recording: ArtifactHandle required`.
        recording: ArtifactHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `program.replay`.
    struct ProgramReplayResponse {
        /// IDL `causal_graph: CausalGraphHandle required`.
        causal_graph: CausalGraphHandle required;
        /// IDL `divergence: Opaque optional`.
        divergence: Opaque optional;
        /// Engine-defect report, present on divergence (plan §4.7,
        /// `rule errors.defect_emission`).
        defect: DefectHandle optional;
    }
}
