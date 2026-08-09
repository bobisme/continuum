//! The `whiteboard` namespace: request and response structs for its 1 operation.
//!
//! One operation in a namespace of its own is not a novelty — `benchmark.run` has been
//! exactly that since 3.0. Folding `compile` into `evidence` was rejected because RFC
//! 0027's argument for that namespace's levels ("`evidence` is `read` except for
//! `evidence.link`") is about a graph's own read/verify/link surface and does not
//! transfer to a note compiler at `propose`.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `whiteboard.compile` (protocol 3.5).
    ///
    /// One field, and its type is the decision: the note crosses as an `Opaque`
    /// governed by `schemas/whiteboard-note.schema.json`, not as a struct declared in
    /// the IDL. INV-003 makes that schema normative for a note's shape, and declaring
    /// the same twelve-member document a second time would be the two-authority
    /// disagreement `rule conformance.registry_agreement` exists to prevent one
    /// namespace down (RFC 0038 W11). `IntentChangeSet.changes` is the precedent.
    ///
    /// There is no `author` field, no `epochs` field, and no status field. The author
    /// is a member of the note and MUST equal the admitted capability's actor (W12);
    /// the pinning is the connection's; and the note format has no status member at
    /// all (W5).
    struct WhiteboardCompileRequest {
        /// The note, in the form of `schemas/whiteboard-note.schema.json`.
        note: Opaque required;
    }
}

protocol_struct! {
    /// The `response` body of `whiteboard.compile` (protocol 3.5).
    struct WhiteboardCompileResponse {
        /// The proposed nodes, in the note's own section and line order.
        nodes: list<EvidenceHandle> required;
        /// The `SUPPORTS` edges the note's decisions drew (RFC 0038 W6).
        edges: list<EvidenceHandle> required;
        /// One entry per `Experiment` line (RFC 0038 W3). Present and empty when the
        /// note had none: an absent list and an empty list would be two spellings of
        /// one answer.
        tasks: list<WhiteboardTaskProposal> required;
    }
}
