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
        /// Content identities of the source files. This list says *what* the
        /// snapshot's files are and not *where* they go; `file_components`
        /// says where (`rule snapshot.file_components`).
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
        /// The `files` components with the workspace-relative path each one sits at, so
        /// a snapshot is reconstructable from the request that creates it
        /// (`rule snapshot.file_components`, protocol 3.2).
        file_components: list<FileComponent> optional;
    }
}

protocol_struct! {
    /// One source-file component: the path the content sits at, and the content
    /// identity of the content itself.
    ///
    /// `schemas/workspace-snapshot.schema.json` requires both members of every `files`
    /// item (`path` and `digest`), so this struct is what the normative artifact class
    /// already says a file component is; before 3.2 the wire carried only the second.
    struct FileComponent {
        /// Workspace-relative path, the schema's `files[].path`.
        path: String required;
        /// Content identity of the file, the schema's `files[].digest`.
        commitment: Commitment required;
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
        /// The snapshot's toolchain *epoch*, which is what plan §4.2 and
        /// `schemas/workspace-snapshot.schema.json` both declare. It is not a content
        /// identity of a toolchain declaration file; no plan §4.2 component is.
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
    /// One `Experiment` line of a whiteboard note, compiled (protocol 3.5,
    /// RFC 0038 W3).
    ///
    /// plan §11.2's twenty node kinds have no task kind, so an experiment proposes a
    /// *task* rather than a node: it carries no claim identity, no subject, and
    /// therefore no handle — nothing publishes a task proposal and it is not a plan
    /// §4.4 artifact class.
    ///
    /// The line's own prose is deliberately not a member. It is the caller's text,
    /// the caller holds the note it sent, and echoing untrusted text back into a
    /// result is what `rule envelope.no_prose` and INV-016 forbid; `index` is how a
    /// client recovers the sentence from the document it already has.
    struct WhiteboardTaskProposal {
        /// Position in the note's `experiments` array, counting from zero.
        index: U32 required;
        /// The prior artifacts the experiment would be run against, in the note's
        /// own order. Every one resolved before the compilation happened (W4).
        references: list<Commitment> required;
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
        /// The traversal is `rule evidence.traversal`: an edge is walked in
        /// either direction, because an edge's direction is what it asserts.
        roots: list<EvidenceHandle> optional;
        /// IDL `max_depth: U32 optional`. The traversal's bound, in edges.
        /// Absent is unbounded; `0` selects the roots alone.
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
