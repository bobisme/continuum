//! The `workspace` namespace: request and response structs for its 5 operations.
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
    /// The `request` body of `workspace.create_by_reference` (protocol 3.6).
    ///
    /// Four members where the inline form has three, and the arithmetic is the point:
    /// `components` replaces an eleven-member `SnapshotComponents` with one
    /// `Commitment`, and `epochs` and `intent` are the two members of that struct which
    /// do *not* come from the reference. `rule snapshot.by_reference` states why each
    /// travels — admission reads the governing intent from the request before any
    /// handler runs (INV-015), and the epochs are the caller's declaration checked
    /// against the epochs the deployment serves, not a fact the store may supply to
    /// itself.
    ///
    /// There is no `overlay`. An overlay is editor buffers laid over the components,
    /// which is content inline again, and a caller wanting one creates and then forks —
    /// which is what `workspace.fork` is for and what the DX-10 instrument's own
    /// trajectory already does.
    struct WorkspaceCreateByReferenceRequest {
        /// Content identity of a `SnapshotComponents` value this daemon holds.
        components: Commitment required;
        /// IDL `epochs: SnapshotEpochs required`.
        epochs: SnapshotEpochs required;
        /// IDL `intent: IntentHandle required`.
        intent: IntentHandle required;
        /// Seal the snapshot on creation.
        seal: Bool optional;
    }
}

protocol_struct! {
    /// The `response` body of `workspace.create_by_reference` (protocol 3.6).
    ///
    /// Member for member what `workspace.create` answers, and that is a contract rather
    /// than a coincidence: the two operations resolve to one `create` over one
    /// `SnapshotComponents` value, so equal components answer with an equal handle and
    /// an equal frame (`rule snapshot.by_reference`).
    struct WorkspaceCreateByReferenceResponse {
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
