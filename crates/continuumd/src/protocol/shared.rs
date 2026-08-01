//! Types shared by several operations (IDL §9, plus [`VerificationResult`], which
//! the IDL declares in §10 as the named response of six operations).

use super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// What an operation is aimed at.
    struct Target {
        /// IDL `kind: TargetKind required`.
        kind: TargetKind required;
        /// IDL `id: String required`.
        id: String required;
    }
}

protocol_struct! {
    /// Content identities of the ten plan §4.2 snapshot components
    /// (`schemas/workspace-snapshot.schema.json`).
    struct SnapshotComponents {
        /// IDL `files: list<Commitment> required`.
        files: list<Commitment> required;
        /// IDL `cml_modules: list<Commitment> required`.
        cml_modules: list<Commitment> required;
        /// IDL `rust_extraction: list<Commitment> required`.
        rust_extraction: list<Commitment> required;
        /// IDL `domain_packs: list<Commitment> required`.
        domain_packs: list<Commitment> required;
        /// IDL `dependencies: list<Commitment> required`.
        dependencies: list<Commitment> required;
        /// Toolchain and semantic epochs of the snapshot. The protocol epoch
        /// is a connection property and is NOT part of snapshot identity
        /// (plan §4.6; SD-13).
        epochs: SnapshotEpochs required;
        /// Reference to the governing Intent Contract — the identity only,
        /// never the contract itself (plan §4.2, INV-001).
        intent: IntentHandle required;
        /// IDL `correspondence: list<Commitment> required`.
        correspondence: list<Commitment> required;
        /// IDL `proof_environment: list<Commitment> required`.
        proof_environment: list<Commitment> required;
        /// IDL `configuration: list<Commitment> required`.
        configuration: list<Commitment> required;
    }
}

protocol_struct! {
    /// The epochs a snapshot pins. The protocol epoch is deliberately absent: it is a
    /// connection property, not part of snapshot identity (plan §4.6; SD-13).
    struct SnapshotEpochs {
        /// IDL `semantic: EpochIdentity required`.
        semantic: EpochIdentity required;
        /// IDL `proof: EpochIdentity required`.
        proof: EpochIdentity required;
        /// IDL `toolchain: EpochIdentity optional`.
        toolchain: EpochIdentity optional;
    }
}

protocol_struct! {
    /// An in-memory editor buffer overlaid on a snapshot's files.
    struct FileOverlay {
        /// IDL `path: String required`.
        path: String required;
        /// IDL `content: Bytes required`.
        content: Bytes required;
    }
}

protocol_struct! {
    /// Policy governing whether and how a Context Pack is compiled with a
    /// result (RFC 0027 "Context policy").
    struct ContextPolicy {
        /// Compile a Context Pack on failure. Default behavior when absent is
        /// to compile one pack plus `next_operations`.
        compile_on_failure: Bool required;
        /// IDL `audience: Audience optional`.
        audience: Audience optional;
        /// IDL `budget: Budget optional`.
        budget: Budget optional;
    }
}

protocol_struct! {
    /// A change proposed to an Intent Contract.
    struct IntentChangeSet {
        /// The contract fields being changed, with their proposed values, in
        /// the form of `schemas/intent-contract.schema.json`.
        changes: Opaque required;
        /// Free of untrusted interpolation; typed rationale for reviewers.
        rationale: String required;
    }
}

protocol_struct! {
    /// A filter over the evidence graph.
    struct EvidenceQuery {
        /// IDL `node_kinds: list<EvidenceNodeKind> optional`.
        node_kinds: list<EvidenceNodeKind> optional;
        /// IDL `edge_kinds: list<EvidenceEdgeKind> optional`.
        edge_kinds: list<EvidenceEdgeKind> optional;
        /// IDL `statuses: list<EvidenceStatus> optional`.
        statuses: list<EvidenceStatus> optional;
        /// Claim identity or property identifier to scope the query to.
        claim_id: String optional;
        /// Roots to traverse from; empty means the whole graph in scope.
        roots: list<EvidenceHandle> optional;
        /// IDL `max_depth: U32 optional`.
        max_depth: U32 optional;
    }
}

protocol_struct! {
    /// The typed result of a verification campaign: the fragments it covered, the
    /// evidence it rests on, and the artifacts it produced.
    ///
    /// Six operations declare it as their named response body, so it is declared once
    /// here rather than six times as an anonymous body.
    struct VerificationResult {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `target: Target required`.
        target: Target required;
        /// The intent-scope fragments this result covers. A fragment the
        /// campaign did not cover is reported in `omissions`, never implied.
        fragments: list<Fragment> required;
        /// Evidence roots this verdict rests on.
        evidence: list<EvidenceHandle> required;
        /// Counterexample, when the verdict is `refuted`.
        crashpack: CrashpackHandle optional;
        /// Compiled Context Pack, per `ContextPolicy`.
        context: ContextHandle optional;
        /// IDL `continuation: ContinuationHandle optional`.
        continuation: ContinuationHandle optional;
    }
}
