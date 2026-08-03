//! The `evidence` namespace: request and response structs for its 5 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `evidence.get`.
    struct EvidenceGetRequest {
        /// IDL `evidence: EvidenceHandle required`.
        evidence: EvidenceHandle required;
        /// Include bounded inline content, subject to `output_policy`.
        inline: Bool optional;
    }
}

protocol_struct! {
    /// The `response` body of `evidence.get`.
    struct EvidenceGetResponse {
        /// IDL `node: Opaque nullable`.
        node: Opaque nullable;
        /// IDL `edge: Opaque nullable`.
        edge: Opaque nullable;
        /// IDL `redacted: Redacted optional`.
        redacted: Redacted optional;
    }
}

protocol_struct! {
    /// The `request` body of `evidence.query`.
    struct EvidenceQueryRequest {
        /// IDL `query: EvidenceQuery required`.
        query: EvidenceQuery required;
    }
}

protocol_struct! {
    /// The `response` body of `evidence.query`.
    struct EvidenceQueryResponse {
        /// IDL `nodes: list<EvidenceHandle> required`.
        nodes: list<EvidenceHandle> required;
        /// IDL `edges: list<EvidenceHandle> required`.
        edges: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `evidence.verify`.
    struct EvidenceVerifyRequest {
        /// IDL `evidence: EvidenceHandle required`.
        evidence: EvidenceHandle required;
        /// The status the caller expects the claim to currently hold. The
        /// promotion is a compare-and-set against it; a lost CAS returns
        /// `StatusConflict`.
        expected_status: EvidenceStatus optional;
    }
}

protocol_struct! {
    /// The `response` body of `evidence.verify`.
    struct EvidenceVerifyResponse {
        /// IDL `evidence: EvidenceHandle required`.
        evidence: EvidenceHandle required;
        /// IDL `status: EvidenceStatus required`.
        status: EvidenceStatus required;
        /// The class of evidence the checker validated.
        evidence_kind: EvidenceKind required;
        /// The checker's service identity (INV-004).
        checker: String required;
        /// `checked-certificate` or `trusted-solver`.
        validation_basis: String required;
        /// Present when content the verified receipt references is redacted
        /// (plan §4.5, SD-09): the structural result and the redaction are
        /// returned together, never one in place of the other.
        redacted: Redacted optional;
    }
}

protocol_struct! {
    /// The `request` body of `evidence.subscribe`.
    struct EvidenceSubscribeRequest {
        /// IDL `scope: EvidenceQuery required`.
        scope: EvidenceQuery required;
    }
}

protocol_struct! {
    /// The `response` body of `evidence.subscribe`.
    struct EvidenceSubscribeResponse {
        /// The scope's current frontier at subscription time.
        frontier: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `evidence.link` (protocol 3.3).
    ///
    /// Three fields, and the one that is *not* here is the point: there is no
    /// `checker` field. The checker is the admitted capability's actor
    /// (RFC 0038 "Authority"), for the same reason `evidence.verify` has no
    /// status field — a field a caller can fill is a field a caller can lie in.
    struct EvidenceLinkRequest {
        /// The evidence node the check was performed over — the edge's `from`.
        subject: EvidenceHandle required;
        /// Content identity of the proof receipt the checker produced (RFC
        /// 0024). The daemon MUST already hold it; the `receipt` node this
        /// operation appends is the edge's `to` (RFC 0038 D1).
        receipt: Commitment required;
        /// The checker's tool identity, recorded as the receipt node's
        /// `provenance.tool` and inside its identity, so one receipt checked
        /// under two checker versions is two nodes.
        checker_profile: String required;
    }
}

protocol_struct! {
    /// The `response` body of `evidence.link` (protocol 3.3).
    struct EvidenceLinkResponse {
        /// The `CHECKED_BY` edge, named under `rule evidence.edge_identity`.
        edge: EvidenceHandle required;
        /// The `receipt` node the edge points at.
        receipt: EvidenceHandle required;
        /// The checker's service identity (INV-004), from the admitted
        /// capability and never from the request.
        checker: String required;
    }
}
