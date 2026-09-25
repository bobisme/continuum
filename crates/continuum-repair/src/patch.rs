//! Patch identity (PR-20 / IMPL-02): a repair's candidate is a sealed workspace snapshot
//! derived from the base snapshot plus the transaction's declared changes, never a handle
//! the caller names.
//!
//! # What the RFC makes the patch identity
//!
//! > | 2 | `patch_application` | the patch applies cleanly to the declared base; no hidden
//! > edits | the sealed candidate snapshot's digest equals the normalized base+changes
//! > digest | smuggling edits outside the declared change set |
//! >
//! > **Gate 2 compares digests, not diffs.** The candidate snapshot is sealed and
//! > normalized; the gate compares its content identity against the identity of base +
//! > declared `changes`.
//! >
//! > — RFC 0032, "Gates"; the `apply` row: "seals a candidate snapshot from base + changes"
//!
//! So the patch identity has two halves, and the schema already has a field for each:
//!
//! - **the candidate's content identity** — `candidate_snapshot`, `^ws_…`. It is the
//!   Merkle root `continuum_workspace::snapshot::Snapshot::build` derives for the content
//!   that base + changes produces. This crate has no second snapshot notion: the tree,
//!   its ordering contract, and its record grammar are `continuum-workspace`'s. This is
//!   what gate 2 compares.
//! - **each declared change's content identity** — `changes[].digest`. It is a digest of
//!   the change's canonical record ([`DeclaredChange::canonical_bytes`]), spelled
//!   `<algorithm>:<hex>` so the hash is never a guess (`HashAlgorithm`'s rule). The
//!   schema types `digest` as a bare string; filling it with this derivation is
//!   conformance, not a shape change.
//!
//! # Normalization and the canonical order
//!
//! RFC 0032 says "normalized" and states no order of its own. The order used here is the
//! one the snapshot layer already fixes, the segment-wise path order of
//! `continuum_workspace::snapshot`'s ordering contract:
//!
//! 1. Inside one change, edits are keyed by path in a `BTreeMap`, so a change's record is
//!    a function of its *set* of edits.
//! 2. The changes of one proposal must be **path-disjoint**: no path is edited by two
//!    changes ([`PatchRefusal::OverlappingChanges`]). Every declared change is therefore
//!    commutative with every other, and the candidate is a function of the change *set*.
//! 3. The transaction's `changes` array is written in ascending order of each change's
//!    least path. Disjointness makes that order total, and it consults no hash.
//!
//! Reordering the declared changes therefore changes neither `candidate_snapshot` nor
//! `changes`, so it changes no byte of the transaction's identity preimage.
//!
//! # A change is anchored to its base
//!
//! Each edit states what it expects to find ([`FileEdit`]): `Create` expects the path to
//! be absent, and `Replace` and `Delete` name the leaf identity of the file they replace.
//! A change authored against another snapshot does not apply: it is refused typed, never
//! applied over whatever the base happens to hold. The base itself is re-derived from its
//! content and must be the transaction's frozen `base_snapshot`
//! ([`PatchRefusal::BaseMismatch`]).
//!
//! Three comparisons rest on the hasher `H`, not on exact bytes: the base identity, each
//! `before` leaf identity, and the candidate's `ws_` identity. They are as strong as
//! `H`'s collision resistance, so production uses BLAKE3; a non-cryptographic hasher is
//! accepted for tests and honest about itself in every recorded digest's algorithm
//! label. A `ws_` handle does not carry its algorithm, so a `before` computed under
//! another hasher is refused as [`PatchRefusal::StaleBefore`]: it does not name a file
//! of this base under this lineage's hasher. [`SealedCandidate::check`] is exact.
//!
//! # What normalization does not do
//!
//! A `Replace` whose new content equals the base content is accepted and recorded: the
//! candidate is unchanged, but the declared change set, and so the `rt_` identity, is
//! not the empty one. Splitting one set of edits across several changes likewise keeps
//! the candidate and moves `changes`. The recorded changes are what was declared, in
//! canonical order; normalization never merges, splits or drops a declared change.
//!
//! # Checking a supplied candidate
//!
//! [`SealedCandidate::check`] is gate 2's comparison on a candidate somebody else built:
//! the supplied content is compared exactly (`Snapshot::diff`, which prunes a subtree only
//! after an exact comparison agrees) against the sealed one, and any difference — an
//! undeclared file, a missing one, one changed byte — is [`PatchRefusal::UndeclaredEdits`]
//! naming every differing path.
//!
//! # Not here
//!
//! The daemon owns the request-size bound on changes (the IDL types `repair.apply`'s
//! `changes` as `Opaque`), resolving the base content from its snapshot store, and
//! publishing the candidate's records. Gate 2's outcome as a gate status is IMPL-03's.

use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;

use continuum_value::identity::ContentHasher;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
use continuum_workspace::snapshot::{Snapshot, SnapshotError, WorkspaceContent, WorkspacePath};

use crate::handle::SnapshotId;
use crate::hypothesis::{Change, ChangeKind};

/// Domain tag of a change record. It keeps a change record from being read as any other
/// canonical record this workspace hashes.
const CHANGE_TAG: &[u8] = b"continuum-repair/change";

/// Version of the change record grammar. Changing it changes every change digest.
pub const CHANGE_ENCODING_VERSION: u8 = 1;

/// The snapshot-layer identity seam, under the transaction's hasher `H`.
///
/// `continuum-workspace` takes identity through its `ContentIdentifier` trait and
/// `continuum-value` owns the hash; this is the composition, the same one the daemon's
/// `Blake3Identity` makes, generic so that one lineage names its snapshots and its
/// `rt_` handles under one algorithm.
pub struct HashedIdentifier<H: ContentHasher>(PhantomData<fn() -> H>);

impl<H: ContentHasher> HashedIdentifier<H> {
    /// The identifier.
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<H: ContentHasher> Default for HashedIdentifier<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: ContentHasher> ContentIdentifier for HashedIdentifier<H> {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &H::hash(content).to_token()).map_err(|_| IdentityUnavailable)
    }
}

/// One edit to one file, with what the edit expects to find there.
#[derive(Clone, PartialEq, Eq)]
pub enum FileEdit {
    /// Add a file where the base has none.
    Create {
        /// The new content.
        content: Vec<u8>,
    },
    /// Replace the base's file, which must have leaf identity `before`.
    Replace {
        /// The leaf identity of the file being replaced, in the base snapshot.
        before: SnapshotId,
        /// The new content.
        content: Vec<u8>,
    },
    /// Remove the base's file, which must have leaf identity `before`.
    Delete {
        /// The leaf identity of the file being removed, in the base snapshot.
        before: SnapshotId,
    },
}

impl fmt::Debug for FileEdit {
    /// Content lengths only: file content can be large, and it is caller data.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { content } => write!(f, "Create(<{} bytes>)", content.len()),
            Self::Replace { before, content } => {
                write!(f, "Replace({before} -> <{} bytes>)", content.len())
            }
            Self::Delete { before } => write!(f, "Delete({before})"),
        }
    }
}

/// Why a change is not well formed. No variant carries file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MalformedChange {
    /// A change must edit at least one file. An empty change has no content to name.
    Empty,
    /// One change edits one path twice.
    DuplicatePath {
        /// The repeated path.
        path: WorkspacePath,
    },
}

impl fmt::Display for MalformedChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a change edits no file"),
            Self::DuplicatePath { path } => write!(f, "a change edits `{path}` twice"),
        }
    }
}

impl std::error::Error for MalformedChange {}

/// One entry of the proposal's changes: a kind from the closed set and the file edits it
/// makes. Its `changes[].digest` is derived from these, never supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredChange {
    kind: ChangeKind,
    edits: BTreeMap<WorkspacePath, FileEdit>,
}

impl DeclaredChange {
    /// A change of `kind` making `edits`.
    ///
    /// # Errors
    ///
    /// [`MalformedChange::Empty`] for no edits, and [`MalformedChange::DuplicatePath`]
    /// when two edits name one path: keeping either would make the change a function of
    /// input order.
    pub fn new(
        kind: ChangeKind,
        edits: impl IntoIterator<Item = (WorkspacePath, FileEdit)>,
    ) -> Result<Self, MalformedChange> {
        let mut map = BTreeMap::new();
        for (path, edit) in edits {
            if map.contains_key(&path) {
                return Err(MalformedChange::DuplicatePath { path });
            }
            map.insert(path, edit);
        }
        if map.is_empty() {
            return Err(MalformedChange::Empty);
        }
        Ok(Self { kind, edits: map })
    }

    /// The change kind.
    #[must_use]
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// The edits, in path order.
    pub fn edits(&self) -> impl Iterator<Item = (&WorkspacePath, &FileEdit)> {
        self.edits.iter()
    }

    /// The least path this change edits: its position in the canonical order.
    fn least_path(&self) -> &WorkspacePath {
        // `new` refuses an empty change, so the first key exists.
        self.edits
            .keys()
            .next()
            .unwrap_or_else(|| unreachable!("a declared change has at least one edit"))
    }

    /// The change's canonical record: the preimage of its `changes[].digest`.
    ///
    /// Grammar, version [`CHANGE_ENCODING_VERSION`], integers big-endian `u64`:
    ///
    /// ```text
    /// change := u64(len) "continuum-repair/change" u8(version)
    ///           bytes(kind) u64(edit_count) edit*          -- edits in path order
    /// edit   := u64(segment_count) bytes(segment)* op
    /// op     := 0x00 bytes(content)                       -- Create
    ///         | 0x01 bytes(before) bytes(content)         -- Replace
    ///         | 0x02 bytes(before)                        -- Delete
    /// bytes  := u64(len) byte*
    /// ```
    ///
    /// Every variable-length field is length-prefixed and every alternative is tagged, so
    /// two different changes have two different records.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_bytes(&mut out, CHANGE_TAG);
        out.push(CHANGE_ENCODING_VERSION);
        put_bytes(&mut out, self.kind.token().as_bytes());
        put_len(&mut out, self.edits.len());
        for (path, edit) in &self.edits {
            put_len(&mut out, path.depth());
            for segment in path.segments() {
                put_bytes(&mut out, segment.as_bytes());
            }
            match edit {
                FileEdit::Create { content } => {
                    out.push(0x00);
                    put_bytes(&mut out, content);
                }
                FileEdit::Replace { before, content } => {
                    out.push(0x01);
                    put_bytes(&mut out, before.as_str().as_bytes());
                    put_bytes(&mut out, content);
                }
                FileEdit::Delete { before } => {
                    out.push(0x02);
                    put_bytes(&mut out, before.as_str().as_bytes());
                }
            }
        }
        out
    }

    /// The `changes[]` entry this change is recorded as under `H`: its kind and the
    /// `<algorithm>:<hex>` digest of [`Self::canonical_bytes`].
    #[must_use]
    pub fn recorded<H: ContentHasher>(&self) -> Change {
        let digest = format!(
            "{}:{}",
            H::ALGORITHM.token(),
            H::hash(&self.canonical_bytes()).to_token()
        );
        Change::new(self.kind, digest)
    }
}

fn put_len(out: &mut Vec<u8>, len: usize) {
    // `usize` is at most 64 bits on every supported target.
    out.extend_from_slice(&(len as u64).to_be_bytes());
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_len(out, bytes.len());
    out.extend_from_slice(bytes);
}

/// Why no candidate could be sealed, or why a supplied candidate is not the sealed one.
///
/// No variant carries file content or the hypothesis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchRefusal {
    /// The base content does not have the transaction's frozen base identity. A change
    /// against a different base is a different transaction (RFC 0032).
    BaseMismatch {
        /// The transaction's `base_snapshot`.
        expected: SnapshotId,
        /// The identity the supplied base content has.
        found: SnapshotId,
    },
    /// Two declared changes edit one path. Changes must be disjoint so that they
    /// commute; one change may make both edits.
    OverlappingChanges {
        /// The path both edit.
        path: WorkspacePath,
    },
    /// A `Create` names a path the base already holds a file at.
    CreateOverExisting {
        /// The path.
        path: WorkspacePath,
    },
    /// A `Replace` or `Delete` names a path the base holds no file at.
    MissingFile {
        /// The path.
        path: WorkspacePath,
    },
    /// A `Replace` or `Delete` names a path that is a directory in the base.
    NotAFile {
        /// The path.
        path: WorkspacePath,
    },
    /// A `Replace` or `Delete` expects a file the base does not hold: the change was
    /// authored against another snapshot.
    StaleBefore {
        /// The path.
        path: WorkspacePath,
    },
    /// The edited content is not a workspace tree (a file where a directory is needed,
    /// or the identity seam refused a node).
    Tree(SnapshotError),
    /// A supplied candidate differs from base + declared changes: every path that differs
    /// is a hidden edit (gate 2, "smuggling edits outside the declared change set").
    UndeclaredEdits {
        /// Every differing path, in path order.
        paths: Vec<WorkspacePath>,
    },
    /// The identity seam could not name a snapshot as a `ws_` handle.
    IdentityUnavailable,
}

impl fmt::Display for PatchRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseMismatch { expected, found } => write!(
                f,
                "the base content is {found}, not the transaction's base {expected}"
            ),
            Self::OverlappingChanges { path } => {
                write!(f, "two declared changes edit `{path}`")
            }
            Self::CreateOverExisting { path } => {
                write!(f, "a change creates `{path}`, which the base already holds")
            }
            Self::MissingFile { path } => {
                write!(f, "a change edits `{path}`, which the base does not hold")
            }
            Self::NotAFile { path } => {
                write!(
                    f,
                    "a change edits `{path}` as a file, and it is a directory in the base"
                )
            }
            Self::StaleBefore { path } => write!(
                f,
                "a change edits `{path}` as a file the base does not hold: it was authored against another snapshot"
            ),
            Self::Tree(error) => write!(f, "the candidate is not a workspace tree: {error}"),
            Self::UndeclaredEdits { paths } => write!(
                f,
                "the candidate differs from base + declared changes at {} path(s)",
                paths.len()
            ),
            Self::IdentityUnavailable => f.write_str("no snapshot identity could be derived"),
        }
    }
}

impl std::error::Error for PatchRefusal {}

impl From<SnapshotError> for PatchRefusal {
    fn from(error: SnapshotError) -> Self {
        Self::Tree(error)
    }
}

/// A candidate snapshot sealed from a base snapshot and declared changes under `H`.
///
/// [`Self::seal`] is the only constructor, so a value of this type always names content
/// that base + changes produces. It carries both halves of the patch identity: the
/// candidate's `ws_` identity and the changes as recorded, in canonical order.
pub struct SealedCandidate<H: ContentHasher> {
    hasher: PhantomData<fn() -> H>,
    base: SnapshotId,
    changes: Vec<Change>,
    snapshot: Snapshot,
    identity: SnapshotId,
}

impl<H: ContentHasher> Clone for SealedCandidate<H> {
    fn clone(&self) -> Self {
        Self {
            hasher: PhantomData,
            base: self.base.clone(),
            changes: self.changes.clone(),
            snapshot: self.snapshot.clone(),
            identity: self.identity.clone(),
        }
    }
}

impl<H: ContentHasher> PartialEq for SealedCandidate<H> {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.changes == other.changes && self.snapshot == other.snapshot
    }
}

impl<H: ContentHasher> Eq for SealedCandidate<H> {}

impl<H: ContentHasher> fmt::Debug for SealedCandidate<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedCandidate")
            .field("base", &self.base)
            .field("changes", &self.changes)
            .field("identity", &self.identity)
            .field("files", &self.snapshot.file_count())
            .finish()
    }
}

fn snapshot_id(handle: &ArtifactHandle) -> Result<SnapshotId, PatchRefusal> {
    SnapshotId::new(&handle.to_string()).map_err(|_| PatchRefusal::IdentityUnavailable)
}

impl<H: ContentHasher> SealedCandidate<H> {
    /// Seal the candidate that `changes` make of `base`, whose identity must be
    /// `base_snapshot`.
    ///
    /// Pure: the result is a function of the arguments (INV-005). The order of `changes`
    /// does not reach it (see the module docs).
    ///
    /// # Errors
    ///
    /// [`PatchRefusal`]: the base is not `base_snapshot`; two changes overlap; an edit
    /// does not apply to the base; or the result is not a tree.
    pub fn seal(
        base_snapshot: &SnapshotId,
        base: &WorkspaceContent,
        changes: &[DeclaredChange],
    ) -> Result<Self, PatchRefusal> {
        // Disjointness first: it needs no snapshot, and it is linear in the edits.
        let mut edits: BTreeMap<&WorkspacePath, &FileEdit> = BTreeMap::new();
        for change in changes {
            for (path, edit) in change.edits() {
                if edits.insert(path, edit).is_some() {
                    return Err(PatchRefusal::OverlappingChanges { path: path.clone() });
                }
            }
        }

        let identifier = HashedIdentifier::<H>::new();
        let base_tree = Snapshot::build(base, &identifier)?;
        let found = snapshot_id(base_tree.identity())?;
        if &found != base_snapshot {
            return Err(PatchRefusal::BaseMismatch {
                expected: base_snapshot.clone(),
                found,
            });
        }

        for (path, edit) in &edits {
            match edit {
                FileEdit::Create { .. } => {
                    if base.contains(path) {
                        return Err(PatchRefusal::CreateOverExisting {
                            path: (*path).clone(),
                        });
                    }
                }
                FileEdit::Replace { before, .. } | FileEdit::Delete { before } => {
                    let node = base_tree
                        .node(path)
                        .ok_or_else(|| PatchRefusal::MissingFile {
                            path: (*path).clone(),
                        })?;
                    let leaf = node.as_file().ok_or_else(|| PatchRefusal::NotAFile {
                        path: (*path).clone(),
                    })?;
                    if snapshot_id(leaf.identity())? != *before {
                        return Err(PatchRefusal::StaleBefore {
                            path: (*path).clone(),
                        });
                    }
                }
            }
        }

        let mut candidate = WorkspaceContent::new();
        for (path, bytes) in base.iter() {
            match edits.get(path) {
                None => candidate.insert(path.clone(), bytes.to_vec())?,
                Some(FileEdit::Replace { content, .. }) => {
                    candidate.insert(path.clone(), content.clone())?;
                }
                // A delete drops the file; a create on a present path was refused above.
                Some(FileEdit::Delete { .. } | FileEdit::Create { .. }) => {}
            }
        }
        for (path, edit) in &edits {
            if let FileEdit::Create { content } = edit {
                candidate.insert((*path).clone(), content.clone())?;
            }
        }
        let snapshot = Snapshot::build(&candidate, &identifier)?;
        let identity = snapshot_id(snapshot.identity())?;

        let mut ordered: Vec<&DeclaredChange> = changes.iter().collect();
        ordered.sort_by(|a, b| a.least_path().cmp(b.least_path()));
        Ok(Self {
            hasher: PhantomData,
            base: base_snapshot.clone(),
            changes: ordered
                .into_iter()
                .map(DeclaredChange::recorded::<H>)
                .collect(),
            snapshot,
            identity,
        })
    }

    /// Gate 2's comparison: whether `supplied` is exactly this candidate's content.
    ///
    /// # Errors
    ///
    /// [`PatchRefusal::UndeclaredEdits`] naming every path at which `supplied` differs
    /// from base + declared changes, and [`PatchRefusal::Tree`] when the seam cannot name
    /// the supplied content.
    pub fn check(&self, supplied: &WorkspaceContent) -> Result<(), PatchRefusal> {
        let supplied = Snapshot::build(supplied, &HashedIdentifier::<H>::new())?;
        let diff = self.snapshot.diff(&supplied);
        if diff.is_empty() {
            Ok(())
        } else {
            Err(PatchRefusal::UndeclaredEdits {
                paths: diff.paths().cloned().collect(),
            })
        }
    }

    /// The base snapshot the candidate was sealed from.
    #[must_use]
    pub const fn base(&self) -> &SnapshotId {
        &self.base
    }

    /// `candidate_snapshot`: the candidate's content identity.
    #[must_use]
    pub const fn identity(&self) -> &SnapshotId {
        &self.identity
    }

    /// `changes`, as recorded, in canonical order.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// The sealed snapshot, whose records the daemon publishes.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
}
