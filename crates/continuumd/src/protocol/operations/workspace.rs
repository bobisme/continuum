//! The `workspace` namespace: request and response structs for its 4 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `workspace.create`.
    struct WorkspaceCreateRequest {
        /// IDL `components: SnapshotComponents required`.
        components: SnapshotComponents required;
        /// Editor buffers overlaid on `components`.
        overlay: list<FileOverlay> optional;
        /// Seal the snapshot on creation.
        seal: Bool optional;
    }
}

protocol_struct! {
    /// The `response` body of `workspace.create`.
    struct WorkspaceCreateResponse {
        /// IDL `snapshot: WorkspaceHandle required`.
        snapshot: WorkspaceHandle required;
        /// IDL `sealed: Bool required`.
        sealed: Bool required;
        /// IDL `diagnostics: list<Diagnostic> required`.
        diagnostics: list<Diagnostic> required;
    }
}

protocol_struct! {
    /// The `request` body of `workspace.fork`.
    struct WorkspaceForkRequest {
        /// IDL `base: WorkspaceHandle required`.
        base: WorkspaceHandle required;
        /// IDL `overlay: list<FileOverlay> optional`.
        overlay: list<FileOverlay> optional;
        /// Patch content identities to apply to the base.
        patches: list<Commitment> optional;
    }
}

protocol_struct! {
    /// The `response` body of `workspace.fork`.
    struct WorkspaceForkResponse {
        /// IDL `snapshot: WorkspaceHandle required`.
        snapshot: WorkspaceHandle required;
        /// The preserved intent binding.
        intent: IntentHandle required;
        /// Pre-computed diff against the base, when available without
        /// evaluation.
        pre_diff: DiffHandle optional;
        /// IDL `diagnostics: list<Diagnostic> required`.
        diagnostics: list<Diagnostic> required;
    }
}

protocol_struct! {
    /// The `request` body of `workspace.diff`.
    struct WorkspaceDiffRequest {
        /// IDL `before: WorkspaceHandle required`.
        before: WorkspaceHandle required;
        /// IDL `after: WorkspaceHandle required`.
        after: WorkspaceHandle required;
        /// Layers to compute: textual, structural, semantic, intent.
        layers: list<DiffLayer> required;
    }
}

protocol_struct! {
    /// The `response` body of `workspace.diff`.
    struct WorkspaceDiffResponse {
        /// IDL `diff: DiffHandle required`.
        diff: DiffHandle required;
        /// The diff artifact, inline, in the form of
        /// `schemas/semantic-diff.schema.json`.
        summary: Opaque required;
    }
}

protocol_struct! {
    /// The `request` body of `workspace.seal`.
    struct WorkspaceSealRequest {
        /// IDL `snapshot: WorkspaceHandle required`.
        snapshot: WorkspaceHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `workspace.seal`.
    struct WorkspaceSealResponse {
        /// IDL `snapshot: WorkspaceHandle required`.
        snapshot: WorkspaceHandle required;
        /// IDL `root_digest: Commitment required`.
        root_digest: Commitment required;
    }
}
