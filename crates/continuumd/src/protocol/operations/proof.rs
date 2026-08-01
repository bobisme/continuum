//! The `proof` namespace: request and response structs for its 4 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `proof.goal`.
    struct ProofGoalRequest {
        /// IDL `obligation: String required`.
        obligation: String required;
    }
}

protocol_struct! {
    /// The `response` body of `proof.goal`.
    struct ProofGoalResponse {
        /// IDL `proof_state: ProofStateHandle required`.
        proof_state: ProofStateHandle required;
        /// IDL `goal: Opaque required`.
        goal: Opaque required;
        /// IDL `context: ContextHandle required`.
        context: ContextHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `proof.attempt`.
    struct ProofAttemptRequest {
        /// IDL `proof_state: ProofStateHandle required`.
        proof_state: ProofStateHandle required;
        /// Tactic script or term, treated strictly as data.
        step: String required;
    }
}

protocol_struct! {
    /// The `response` body of `proof.attempt`.
    struct ProofAttemptResponse {
        /// IDL `children: list<ProofStateHandle> required`.
        children: list<ProofStateHandle> required;
        /// IDL `diagnostics: list<Diagnostic> required`.
        diagnostics: list<Diagnostic> required;
        /// IDL `closed: Bool required`.
        closed: Bool required;
    }
}

protocol_struct! {
    /// The `request` body of `proof.check`.
    struct ProofCheckRequest {
        /// IDL `candidate: ProofArtifactHandle required`.
        candidate: ProofArtifactHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `proof.check`.
    struct ProofCheckResponse {
        /// IDL `receipt: ReceiptHandle required`.
        receipt: ReceiptHandle required;
        /// The proof receipt (`schemas/proof-receipt.schema.json`).
        proof_receipt: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `proof.slice`.
    struct ProofSliceRequest {
        /// IDL `proof_state: ProofStateHandle required`.
        proof_state: ProofStateHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `proof.slice`.
    struct ProofSliceResponse {
        /// IDL `declarations: list<String> required`.
        declarations: list<String> required;
        /// IDL `axioms: list<String> required`.
        axioms: list<String> required;
    }
}
