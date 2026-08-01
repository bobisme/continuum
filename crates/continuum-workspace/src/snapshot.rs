//! Merkle workspace snapshots: the immutable input every analysis names
//! (plan §4.2, PR 3 / IMPL-01).
//!
//! # What this module decides
//!
//! > Snapshots form a Merkle DAG. A tool call never means "whatever is currently on
//! > disk"; it means a named snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! One sentence, two obligations, and this module is exactly those two:
//!
//! - a workspace's content has a **name** — [`Snapshot::identity`], the Merkle root —
//!   that is a function of the content and of nothing else;
//! - content that two snapshots share has the **same name in both**, at every level of
//!   the tree, so a fork that edits one file re-derives one path from the root and reuses
//!   every other subtree unchanged ([`Snapshot::subtrees`], [`Snapshot::records`]).
//!
//! # Tree shape
//!
//! A snapshot is a tree of [`SnapshotNode`]s. The root is always a directory; a
//! [`FileNode`] is a leaf holding file content; a [`DirectoryNode`] holds its children
//! under their path segments in a [`BTreeMap`].
//!
//! ```text
//!                        root (directory)
//!                    ┌───────────┴───────────┐
//!                 "crates"                "README.md"
//!                    │                     (file)
//!            ┌───────┴───────┐
//!         "value"        "workspace"
//!            │               │
//!        "lib.rs"        "lib.rs"
//!         (file)          (file)
//! ```
//!
//! Every node — leaf and interior alike — carries two things:
//!
//! - a **record** ([`SnapshotNode::record`]): the node's canonical bytes. A file's record
//!   frames its content; a directory's record frames its children's *names and
//!   identities*. That is what makes the structure a Merkle tree rather than one long
//!   digest: a directory's record is small and mentions its children by name.
//! - an **identity** ([`SnapshotNode::identity`]): a `ws_*` [`ArtifactHandle`] derived
//!   from that record through the crate's existing [`ContentIdentifier`] seam.
//!
//! Interior records name children by identity, so a child's identity propagates into its
//! parent's record, into its parent's identity, and so on to the root. Changing one byte
//! of one file changes that file's record, its identity, and every record on the path to
//! the root — and changes nothing else, which is what "two snapshots sharing subtrees
//! share subtree identities" means operationally.
//!
//! # The ordering contract
//!
//! This module fixes the minimum ordering a Merkle root needs to be well defined. The
//! full deterministic file-ordering contract is PR 3 / IMPL-05; the rules below are the
//! seam it formalizes, and are stated precisely so that IMPL-05 either adopts them or
//! records a decision to change them:
//!
//! 1. A [`WorkspacePath`] is a non-empty sequence of segments. Ordering between paths is
//!    **segment-wise**, and between segments it is the byte order of their UTF-8
//!    encodings. It is *not* the byte order of the rendered `a/b/c` string: `"a/b"` and
//!    `"a.b"` order differently under the two rules, and only the segment-wise one agrees
//!    with the tree.
//! 2. Within a directory, children are ordered by segment. A directory's record is
//!    emitted in that order, so the record — and therefore the identity — is a function
//!    of the *set* of children, never of the order they were supplied in.
//! 3. Every collection here is a [`BTreeMap`] or a [`BTreeSet`], so no iteration order
//!    depends on a hash seed or a process (docs/19 §7's determinism matrix).
//! 4. Segments are compared exactly: no case folding, no Unicode normalization, no
//!    separator translation. `A.rs` and `a.rs` are two files. This matches
//!    [`artifact_path`](crate::artifact_path)'s case-sensitivity requirement on the store
//!    volume, and for the same reason: normalizing here would give one workspace two
//!    spellings, which is the failure ADR-0013 exists to prevent.
//!
//! What is deliberately *not* decided here, and is IMPL-05's: Unicode normalization
//! policy for names that differ only by composition, platform-reserved names, symlink
//! and hard-link treatment, and whether non-UTF-8 names are rejected or encoded. This
//! module takes UTF-8 segments and rejects the ones that cannot be ordered or stored
//! unambiguously ([`PathError`]).
//!
//! # Identity discipline: the tree is the identity, the root is the name
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`
//!
//! A Merkle root is a *composition* of whatever the identity seam returns. If that seam
//! is a hash — which ADR-0013 permits only in an explicitly labeled non-certified lane —
//! then two distinct workspaces can in principle carry the same root. This module
//! therefore never lets a root decide a semantic question:
//!
//! - [`PartialEq`] on [`Snapshot`] and [`SnapshotNode`] compares **content**,
//!   recursively. Identities take no part in it. Two snapshots are equal exactly when
//!   their files are, for every identity function that has ever been or will ever be
//!   installed;
//! - [`Snapshot::diff`] prunes a subtree only after an exact comparison confirms what the
//!   identities suggested, and reports every place where equal identities covered
//!   *unequal* content as [`SnapshotDiff::identity_collisions`] — data, not a silent
//!   suppression, exactly as `continuum-value`'s `Insertion::Fresh::hash_collisions` is;
//! - the store agrees. Publishing a record whose identity already names byte-different
//!   content is [`AbortReason::IdentityCollision`](crate::publication::AbortReason::IdentityCollision),
//!   not a merge.
//!
//! No second hashing discipline is introduced. The seam is
//! [`publication::ContentIdentifier`](crate::publication::ContentIdentifier), already this
//! crate's one identity boundary, and behind it `continuum-value` decides what an identity
//! *is* (ADR-0013). This module supplies bytes and composes the results.
//!
//! # How a snapshot reaches the store
//!
//! [`Snapshot::records`] yields every distinct node record, **children before parents**,
//! each paired with the identity it was derived under. Publishing them in that order
//! through [`ReferenceStore`](crate::publication::ReferenceStore) — built with the same
//! [`ContentIdentifier`] — reproduces exactly these identities, because the store derives
//! the identity itself from the bytes rather than trusting a publisher's claim. Two
//! snapshots that share a subtree publish that subtree's records to the same identities,
//! so the store's ordinary convergence *is* structural dedup; nothing here needs a rival
//! store, an index, or a second notion of what is already held.
//!
//! Children-first is not a nicety. A parent record names its children by identity, so
//! publishing a parent first would create a reachable artifact naming content the store
//! does not yet hold — the dangling-reference failure docs/35 rules out for the index and
//! that applies verbatim one level up.
//!
//! # Immutability
//!
//! > Re-derivation, never rewriting. Re-derived artifacts receive new identities linked
//! > to their predecessors by `SUPERSEDES` edges.
//! >
//! > — `notes/plan/adr/0018-semantic-versioning-and-replay.md`
//!
//! There is no method on [`Snapshot`] that mutates one. An edited workspace is a new
//! [`Snapshot`] with a new identity; the old one keeps naming the old bytes, which is
//! precisely PR 3's exit condition that "an old snapshot remains reproducible after the
//! working tree changes". [`ENCODING_VERSION`] versions the wire form of the tree and is
//! *not* one of ADR-0018's six epochs — it is an encoding contract in the sense of
//! `schemas/README.md`'s `schema_epoch`, and it pins no semantics.
//!
//! # No ambient anything
//!
//! [`Snapshot::build`] takes a [`WorkspaceContent`] — an explicit, already-ordered
//! description of what the workspace holds — and an identity seam. It opens no file,
//! reads no directory, consults no clock, and draws no randomness (INV-005, ADR-0003).
//! Importing a working tree from disk, overlaying an editor buffer, forking, and sealing
//! are PR 3 / IMPL-02, and they are *callers* of this module: each produces a
//! [`WorkspaceContent`] and asks for a snapshot.
//!
//! # Seams left open on purpose
//!
//! - **Disk import, editor overlay, fork, seal, and rich diff are IMPL-02.**
//!   [`Snapshot::diff`] here answers "which file paths differ", which is the identity-level
//!   question; rename detection, overlay provenance (`files[].overlay` in
//!   `schemas/workspace-snapshot.schema.json`), and the stale-snapshot error are IMPL-02's.
//! - **Source, dependency, toolchain, and configuration identities are IMPL-03.** Plan
//!   §4.2 lists ten components a snapshot pins; this module owns the file tree — the
//!   schema's `files` and `root_digest` — and the remaining components (`dependencies`,
//!   `epochs`, `intent`, and the named-digest lists) attach *beside* the root rather than
//!   inside it, because they are identities of things that are not files in the tree.
//! - **Deterministic file ordering is IMPL-05.** See "The ordering contract" above.
//! - **Empty directories are not representable.** A [`WorkspaceContent`] is a set of
//!   files, so a directory exists exactly when it contains one. Whether an empty
//!   directory is workspace content at all is a disk-import question (IMPL-02); if it is,
//!   the encoding gains an empty-child list and the record grammar below gains a case.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
//! use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
//!
//! // An injective identity function: the record's own bytes, in hex. Exactly ADR-0013's
//! // certified discipline — the canonical bytes *are* the identity.
//! struct HexIdentity;
//! impl ContentIdentifier for HexIdentity {
//!     fn identify(
//!         &self,
//!         class: ArtifactClass,
//!         content: &[u8],
//!     ) -> Result<ArtifactHandle, IdentityUnavailable> {
//!         let mut token = String::with_capacity(content.len() * 2);
//!         for byte in content {
//!             token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
//!             token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
//!         }
//!         ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
//!     }
//! }
//!
//! let mut content = WorkspaceContent::new();
//! content.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! content.insert(WorkspacePath::new("README.md")?, b"hello".to_vec())?;
//!
//! let snapshot = Snapshot::build(&content, &HexIdentity)?;
//! assert!(snapshot.identity().to_string().starts_with("ws_"));
//!
//! // Same content, built again: the same name.
//! assert_eq!(Snapshot::build(&content, &HexIdentity)?.identity(), snapshot.identity());
//!
//! // One byte elsewhere: a new name, and the untouched subtree keeps its own.
//! let mut edited = WorkspaceContent::new();
//! edited.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! edited.insert(WorkspacePath::new("README.md")?, b"hell0".to_vec())?;
//! let edited = Snapshot::build(&edited, &HexIdentity)?;
//!
//! assert_ne!(edited.identity(), snapshot.identity());
//! let src = WorkspacePath::new("src")?;
//! assert_eq!(
//!     edited.node(&src).map(|node| node.identity()),
//!     snapshot.node(&src).map(|node| node.identity()),
//! );
//!
//! let diff = snapshot.diff(&edited);
//! assert_eq!(diff.paths().collect::<Vec<_>>(), [&WorkspacePath::new("README.md")?]);
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;
use core::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

use crate::artifact_path::{ArtifactClass, ArtifactHandle};
use crate::publication::{ContentIdentifier, IdentityUnavailable};

// --- format constants ------------------------------------------------------------------

/// The artifact class every snapshot node is named in: `ws_*` (plan §4.4).
///
/// Interior nodes and leaves alike. A leaf is the snapshot of a single file and a
/// directory is the snapshot of a subtree, so one class names them all, they shard into
/// one store subtree ([`ArtifactPath`](crate::artifact_path::ArtifactPath)), and any node
/// can be fetched exactly the way a whole snapshot is. Leaves are kept from being
/// confused with directories by the record grammar's tag byte, not by the class.
pub const NODE_CLASS: ArtifactClass = ArtifactClass::WorkspaceSnapshot;

/// Version of the record grammar and of the tree encoding.
///
/// An encoding contract in the sense of `notes/plan/schemas/README.md`'s `schema_epoch`:
/// it says which grammar a byte string was written under, and it pins no semantics. It is
/// not one of ADR-0018's six epochs, and it MUST NOT be read as one.
///
/// Changing it changes every identity in every snapshot, so it is a store-format change,
/// not a tuning knob.
pub const ENCODING_VERSION: u8 = 1;

/// The deepest path this module accepts, in segments.
///
/// The same bound `continuum-value`'s canonical decoder uses, spelled independently
/// because the two crates share no dependency edge. It exists so that recursion over an
/// adversarial encoding terminates in a typed error rather than in a stack overflow —
/// an abort is a panic on adversarial input, which this module does not do.
pub const MAX_DEPTH: usize = 64;

/// Magic prefix of a whole-tree encoding ([`Snapshot::encode`]).
const TREE_MAGIC: &[u8] = b"cwsnap";

/// Magic prefix of a single node record ([`SnapshotNode::record`]).
///
/// Different from [`TREE_MAGIC`] so that a tree encoding and a node record can never be
/// mistaken for one another by a decoder, a store, or a reader of a hex dump.
const NODE_MAGIC: &[u8] = b"cwsnode";

/// Record and encoding tag for a file leaf.
const TAG_FILE: u8 = 0x00;

/// Record and encoding tag for a directory.
const TAG_DIRECTORY: u8 = 0x01;

/// Bytes of a file record before its content: magic, version, tag, and a `u64` length.
const FILE_RECORD_HEADER: usize = NODE_MAGIC.len() + 1 + 1 + 8;

// --- paths -----------------------------------------------------------------------------

/// Why a string is not a usable workspace path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathError {
    /// The path had no segments at all.
    ///
    /// The workspace root is not a path: it is the snapshot itself. Naming it would give
    /// the root two spellings, one of which could also name a file.
    Empty,
    /// A segment was empty — a leading, trailing, or doubled separator.
    EmptySegment {
        /// Index of the empty segment.
        index: usize,
    },
    /// A segment was `.` or `..`.
    ///
    /// The traversal rejection (docs/19 §9, docs/09 "path traversal in crashpack
    /// extraction"), made at construction so that no accepted path can denote anything
    /// outside the workspace. A snapshot is a tree, and `..` is not an edge in it.
    RelativeSegment {
        /// Index of the offending segment.
        index: usize,
        /// The segment, `.` or `..`.
        segment: String,
    },
    /// A segment contained a character that cannot appear in a portable, orderable name.
    ///
    /// Rejected: ASCII control characters including NUL, and `\`. The backslash is not
    /// forbidden because it is rare — it is forbidden because a workspace whose paths
    /// order and nest differently on two platforms has two Merkle trees, and docs/19 §7
    /// makes the OS/architecture matrix a determinism dimension. `/` cannot reach here:
    /// it is the separator.
    SegmentCharacter {
        /// Index of the offending segment.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// The path nests deeper than [`MAX_DEPTH`].
    TooDeep {
        /// The rejected depth.
        depth: usize,
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a workspace path has at least one segment"),
            Self::EmptySegment { index } => {
                write!(f, "workspace path segment {index} is empty")
            }
            Self::RelativeSegment { index, segment } => write!(
                f,
                "workspace path segment {index} is {segment:?}; a snapshot is a tree and \
                 has no relative edges"
            ),
            Self::SegmentCharacter { index, character } => write!(
                f,
                "workspace path segment {index} has a character {character:?} that is not \
                 portable in a path name"
            ),
            Self::TooDeep { depth, max } => {
                write!(
                    f,
                    "workspace path is {depth} segments deep, past the {max} bound"
                )
            }
        }
    }
}

impl core::error::Error for PathError {}

/// The separator between path segments, in the rendered form.
const SEPARATOR: char = '/';

/// A path to one file inside a workspace, relative to the workspace root.
///
/// Non-empty, segment-wise, and validated at construction: no empty segment, no `.` or
/// `..`, no control character, no `\`. See the module documentation's "ordering contract"
/// for what [`Ord`] means and why it is not string order on the rendered form.
///
/// The rendered form always uses `/`, on every platform, for the reason
/// [`ArtifactPath`](crate::artifact_path::ArtifactPath) does: the same workspace must have
/// one spelling everywhere or its snapshot identity is a function of where it was taken.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspacePath {
    /// Segment-wise; [`Ord`] on `Vec<String>` is lexicographic on segments, and [`Ord`]
    /// on `String` is byte order on its UTF-8 encoding. That composition *is* the
    /// ordering contract.
    segments: Vec<String>,
}

impl WorkspacePath {
    /// Parse a `/`-separated path.
    ///
    /// # Errors
    ///
    /// [`PathError`] naming the first violation; see its variants. The empty string is
    /// [`PathError::Empty`] rather than a one-empty-segment path: splitting it would
    /// otherwise report a segment the caller never wrote.
    pub fn new(text: &str) -> Result<Self, PathError> {
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        Self::from_segments(text.split(SEPARATOR))
    }

    /// Build a path from segments that are not `/`-joined.
    ///
    /// # Errors
    ///
    /// [`PathError`], as [`WorkspacePath::new`].
    pub fn from_segments<S: AsRef<str>, I: IntoIterator<Item = S>>(
        segments: I,
    ) -> Result<Self, PathError> {
        let segments: Vec<String> = segments
            .into_iter()
            .map(|segment| segment.as_ref().to_owned())
            .collect();
        if segments.is_empty() {
            return Err(PathError::Empty);
        }
        if segments.len() > MAX_DEPTH {
            return Err(PathError::TooDeep {
                depth: segments.len(),
                max: MAX_DEPTH,
            });
        }
        for (index, segment) in segments.iter().enumerate() {
            validate_segment(segment, index)?;
        }
        Ok(Self { segments })
    }

    /// The segments, outermost first.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(String::as_str)
    }

    /// The segment at `index`, if the path is that deep.
    #[must_use]
    pub fn segment(&self, index: usize) -> Option<&str> {
        self.segments.get(index).map(String::as_str)
    }

    /// How many segments the path has. Always at least one.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// The final segment: the file's own name.
    ///
    /// Never `None` in practice — a path has at least one segment — but reported as an
    /// [`Option`] rather than unwrapped, because a component in the trust base does not
    /// abort on its own invariant.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Whether `self` is a proper ancestor of `other` in the tree.
    #[must_use]
    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        other.segments.len() > self.segments.len()
            && other.segments[..self.segments.len()] == self.segments[..]
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                f.write_str("/")?;
            }
            f.write_str(segment)?;
        }
        Ok(())
    }
}

impl FromStr for WorkspacePath {
    type Err = PathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

/// Reject every segment that cannot be ordered, stored, or nested unambiguously.
fn validate_segment(segment: &str, index: usize) -> Result<(), PathError> {
    if segment.is_empty() {
        return Err(PathError::EmptySegment { index });
    }
    if segment == "." || segment == ".." {
        return Err(PathError::RelativeSegment {
            index,
            segment: segment.to_owned(),
        });
    }
    if let Some(character) = segment
        .chars()
        .find(|c| c.is_control() || *c == '\\' || *c == SEPARATOR)
    {
        return Err(PathError::SegmentCharacter { index, character });
    }
    Ok(())
}

// --- the explicit content description ---------------------------------------------------

/// What a workspace holds, as an explicit description rather than as a directory to walk.
///
/// A set of `(path, content)` pairs in path order. This is the whole input to
/// [`Snapshot::build`], and it is the reason that function reads nothing ambient: whoever
/// *does* read the disk — PR 3 / IMPL-02's importer — hands over a value, and a snapshot
/// built from that value is reproducible from it forever.
///
/// Insertion is order-independent: the collection is a [`BTreeMap`], so the same files
/// inserted in any order produce the same description and therefore the same snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContent {
    files: BTreeMap<WorkspacePath, Vec<u8>>,
}

impl WorkspaceContent {
    /// An empty workspace.
    ///
    /// Legal, and it has an identity: the snapshot of an empty root directory. That is
    /// the honest answer — an empty workspace is a thing one can name, fork, and diff
    /// against — and it is distinguishable from every file, because the record grammar
    /// tags directories and files differently.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one file.
    ///
    /// # Errors
    ///
    /// - [`SnapshotError::DuplicatePath`] when `path` is already present. Silently
    ///   replacing would make the description a function of insertion order, which is
    ///   exactly the nondeterminism a snapshot exists to remove.
    /// - [`SnapshotError::PathConflict`] when `path` is a file that another file needs as
    ///   a directory, or vice versa. `src/lib.rs` and `src` cannot both be files; a tree
    ///   has no node that is both.
    pub fn insert(&mut self, path: WorkspacePath, content: Vec<u8>) -> Result<(), SnapshotError> {
        if self.files.contains_key(&path) {
            return Err(SnapshotError::DuplicatePath { path });
        }
        // Every proper ancestor of `path` must be a directory, i.e. not already a file.
        for depth in 1..path.depth() {
            let ancestor = WorkspacePath {
                segments: path.segments[..depth].to_vec(),
            };
            if self.files.contains_key(&ancestor) {
                return Err(SnapshotError::PathConflict {
                    file: ancestor,
                    descendant: path,
                });
            }
        }
        // And `path` itself must not already be somebody's directory. Descendants of a
        // path sort immediately after it under the segment-wise order, so the immediate
        // successor is a descendant whenever any descendant exists.
        if let Some((next, _)) = self.files.range(path.clone()..).next() {
            if path.is_ancestor_of(next) {
                return Err(SnapshotError::PathConflict {
                    file: path.clone(),
                    descendant: next.clone(),
                });
            }
        }
        self.files.insert(path, content);
        Ok(())
    }

    /// The content at `path`, if the workspace holds a file there.
    #[must_use]
    pub fn get(&self, path: &WorkspacePath) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Whether the workspace holds a file at `path`.
    #[must_use]
    pub fn contains(&self, path: &WorkspacePath) -> bool {
        self.files.contains_key(path)
    }

    /// How many files the workspace holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the workspace holds no files.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every `(path, content)` pair, in path order.
    pub fn iter(&self) -> impl Iterator<Item = (&WorkspacePath, &[u8])> {
        self.files
            .iter()
            .map(|(path, content)| (path, content.as_slice()))
    }

    /// Every path, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &WorkspacePath> {
        self.files.keys()
    }
}

// --- errors ------------------------------------------------------------------------------

/// Why a workspace description is not a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    /// The path is already described.
    DuplicatePath {
        /// The repeated path.
        path: WorkspacePath,
    },
    /// One path is a file and another needs it as a directory.
    PathConflict {
        /// The path used as a file.
        file: WorkspacePath,
        /// A path that needs `file` to be a directory.
        descendant: WorkspacePath,
    },
    /// The tree nests deeper than [`MAX_DEPTH`].
    TooDeep {
        /// The rejected depth.
        depth: usize,
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
    /// The identity seam could not name a node.
    ///
    /// The snapshot is refused whole. A tree with one unnamed node is not a Merkle tree
    /// with a gap in it — it is not a Merkle tree.
    IdentityUnavailable {
        /// The node's path, or [`None`] for the workspace root.
        path: Option<WorkspacePath>,
    },
    /// The identity seam named a node in a class other than [`NODE_CLASS`].
    ///
    /// A snapshot node filed under another class would be fetched by another class's
    /// rules; the mismatch is refused rather than rewritten.
    IdentityClassMismatch {
        /// The node's path, or [`None`] for the workspace root.
        path: Option<WorkspacePath>,
        /// The class the seam returned.
        class: ArtifactClass,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePath { path } => {
                write!(f, "workspace path `{path}` is described twice")
            }
            Self::PathConflict { file, descendant } => write!(
                f,
                "workspace path `{file}` is a file and `{descendant}` needs it to be a directory"
            ),
            Self::TooDeep { depth, max } => {
                write!(f, "workspace tree is {depth} deep, past the {max} bound")
            }
            Self::IdentityUnavailable { path } => {
                write!(
                    f,
                    "no identity could be derived for {}",
                    render(path.as_ref())
                )
            }
            Self::IdentityClassMismatch { path, class } => write!(
                f,
                "identity for {} was derived in class `{class}`, not `{}`",
                render(path.as_ref()),
                NODE_CLASS
            ),
        }
    }
}

impl core::error::Error for SnapshotError {}

/// Name a node in an error message; the root has no path.
fn render(path: Option<&WorkspacePath>) -> String {
    path.map_or_else(
        || "the workspace root".to_owned(),
        |path| format!("`{path}`"),
    )
}

// --- nodes --------------------------------------------------------------------------------

/// Which kind of node this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// A file leaf.
    File,
    /// A directory.
    Directory,
}

impl NodeKind {
    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A file leaf: content, its record, and the identity derived from that record.
///
/// [`PartialEq`] is content equality. The identity takes no part, deliberately — see the
/// module documentation's "identity discipline".
#[derive(Debug, Clone)]
pub struct FileNode {
    identity: ArtifactHandle,
    /// `NODE_MAGIC | version | TAG_FILE | u64be(len) | content`. The content is stored
    /// inside the record rather than beside it, so there is exactly one copy and exactly
    /// one spelling of it.
    record: Vec<u8>,
}

impl FileNode {
    /// The file's content.
    #[must_use]
    pub fn content(&self) -> &[u8] {
        self.record.get(FILE_RECORD_HEADER..).unwrap_or(&[])
    }

    /// The identity this leaf contributes to its parent's record.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        &self.identity
    }

    /// The canonical record: the bytes the identity was derived from.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }
}

impl PartialEq for FileNode {
    fn eq(&self, other: &Self) -> bool {
        self.content() == other.content()
    }
}

impl Eq for FileNode {}

/// A directory: its children by segment, its record, and the identity derived from it.
///
/// [`PartialEq`] is recursive content equality over the children.
#[derive(Debug, Clone)]
pub struct DirectoryNode {
    identity: ArtifactHandle,
    /// `NODE_MAGIC | version | TAG_DIRECTORY | u64be(count) |`
    /// `(u64be(name_len) | name | u64be(handle_len) | handle)*`, children in segment
    /// order. Children appear by *identity*, which is what makes this a Merkle node: the
    /// record is small, and it still changes whenever any descendant does.
    record: Vec<u8>,
    children: BTreeMap<String, SnapshotNode>,
}

impl DirectoryNode {
    /// The identity this directory contributes to its parent's record.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        &self.identity
    }

    /// The canonical record: the bytes the identity was derived from.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }

    /// The child under `segment`, if any.
    #[must_use]
    pub fn child(&self, segment: &str) -> Option<&SnapshotNode> {
        self.children.get(segment)
    }

    /// Every `(segment, child)` pair, in segment order.
    pub fn children(&self) -> impl Iterator<Item = (&str, &SnapshotNode)> {
        self.children
            .iter()
            .map(|(segment, child)| (segment.as_str(), child))
    }

    /// How many direct children the directory has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Whether the directory has no children.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl PartialEq for DirectoryNode {
    fn eq(&self, other: &Self) -> bool {
        self.children == other.children
    }
}

impl Eq for DirectoryNode {}

/// One node of a snapshot's Merkle tree.
///
/// [`PartialEq`] is content equality, recursively, and never consults an identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotNode {
    /// A file leaf.
    File(FileNode),
    /// A directory.
    Directory(DirectoryNode),
}

impl SnapshotNode {
    /// This node's identity: a `ws_*` handle derived from [`record`](Self::record).
    ///
    /// A name, not a decision. Equality and [`Snapshot::diff`] resolve by exact
    /// comparison; see the module documentation.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        match self {
            Self::File(node) => node.identity(),
            Self::Directory(node) => node.identity(),
        }
    }

    /// The canonical record: the bytes this node's identity was derived from, and the
    /// bytes a store publishes for it.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        match self {
            Self::File(node) => node.record(),
            Self::Directory(node) => node.record(),
        }
    }

    /// Which kind of node this is.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        match self {
            Self::File(_) => NodeKind::File,
            Self::Directory(_) => NodeKind::Directory,
        }
    }

    /// The leaf, if this is one.
    #[must_use]
    pub const fn as_file(&self) -> Option<&FileNode> {
        match self {
            Self::File(node) => Some(node),
            Self::Directory(_) => None,
        }
    }

    /// The directory, if this is one.
    #[must_use]
    pub const fn as_directory(&self) -> Option<&DirectoryNode> {
        match self {
            Self::File(_) => None,
            Self::Directory(node) => Some(node),
        }
    }
}

// --- the snapshot ---------------------------------------------------------------------------

/// An immutable Merkle snapshot of a workspace's files.
///
/// Build one with [`Snapshot::build`]. There is no method that changes one: an edited
/// workspace is a different snapshot with a different identity (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Always a [`SnapshotNode::Directory`]. A workspace root is a directory, including
    /// when it is empty, so every file in a snapshot has a non-empty path.
    root: SnapshotNode,
}

impl Snapshot {
    /// Build the Merkle tree of `content`, naming every node through `identifier`.
    ///
    /// Pure: the result is a function of `content` and of `identifier`'s behavior, and of
    /// nothing else (INV-005). Nodes are named children-first, so a parent's record is
    /// derived only from identities that already exist.
    ///
    /// # Errors
    ///
    /// [`SnapshotError::IdentityUnavailable`] or [`SnapshotError::IdentityClassMismatch`]
    /// when the seam refuses or misnames a node, and [`SnapshotError::TooDeep`] /
    /// [`SnapshotError::PathConflict`] for a description that is not a tree — which a
    /// [`WorkspaceContent`] built through [`WorkspaceContent::insert`] cannot be, so those
    /// two are defense in depth rather than the expected path.
    pub fn build<I: ContentIdentifier + ?Sized>(
        content: &WorkspaceContent,
        identifier: &I,
    ) -> Result<Self, SnapshotError> {
        let mut draft = DraftDirectory::default();
        for (path, bytes) in content.iter() {
            draft.insert(path, bytes)?;
        }
        let mut trail: Vec<String> = Vec::new();
        let root = draft.seal(&mut trail, identifier)?;
        Ok(Self {
            root: SnapshotNode::Directory(root),
        })
    }

    /// The Merkle root's identity: the name of this snapshot.
    ///
    /// This is the schema's `workspace_id` / `root_digest`
    /// (`notes/plan/schemas/workspace-snapshot.schema.json`) for the file-tree component
    /// of a snapshot; the components plan §4.2 lists beside the files are PR 3 / IMPL-03's.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        self.root.identity()
    }

    /// The root directory node.
    #[must_use]
    pub const fn root(&self) -> &SnapshotNode {
        &self.root
    }

    /// The node at `path`, if the snapshot has one there.
    #[must_use]
    pub fn node(&self, path: &WorkspacePath) -> Option<&SnapshotNode> {
        let mut node = &self.root;
        for segment in path.segments() {
            node = node.as_directory()?.child(segment)?;
        }
        Some(node)
    }

    /// How many files the snapshot holds.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files().len()
    }

    /// Whether the snapshot holds no files.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file_count() == 0
    }

    /// Every non-root node with its path, in path order.
    ///
    /// The root is excluded because it has no path; reach it through
    /// [`root`](Self::root). Two snapshots that share content share the identities in this
    /// listing wherever the content coincides — that listing *is* the structural-sharing
    /// claim, in observable form.
    #[must_use]
    pub fn subtrees(&self) -> Vec<(WorkspacePath, &SnapshotNode)> {
        let mut out = Vec::new();
        let mut trail: Vec<String> = Vec::new();
        collect_subtrees(&self.root, &mut trail, &mut out);
        out
    }

    /// Every file with its path, in path order.
    ///
    /// The schema's `files` array (`path` and `digest`), minus the overlay flag that
    /// PR 3 / IMPL-02 owns.
    #[must_use]
    pub fn files(&self) -> Vec<(WorkspacePath, &FileNode)> {
        self.subtrees()
            .into_iter()
            .filter_map(|(path, node)| node.as_file().map(|file| (path, file)))
            .collect()
    }

    /// Every distinct node record with the identity it was derived under, **children
    /// before parents**.
    ///
    /// The publication order: feeding this to
    /// [`ReferenceStore::publish`](crate::publication::ReferenceStore::publish) with the
    /// same [`ContentIdentifier`] republishes exactly these identities, and a record the
    /// store already holds converges instead of being written twice.
    ///
    /// Distinctness is by *record bytes*, not by identity. Under an identity function that
    /// collides, two different records survive this listing as two entries and the store
    /// refuses the second with
    /// [`AbortReason::IdentityCollision`](crate::publication::AbortReason::IdentityCollision).
    /// Deduplicating by identity here would suppress the collision instead of reporting it,
    /// which is precisely the conflation ADR-0013 forbids.
    #[must_use]
    pub fn records(&self) -> Vec<(&ArtifactHandle, &[u8])> {
        let mut out = Vec::new();
        let mut seen: BTreeSet<&[u8]> = BTreeSet::new();
        collect_records(&self.root, &mut seen, &mut out);
        out
    }

    /// Which file paths differ between two snapshots.
    ///
    /// The identity-level diff: added, removed, and modified file paths, in path order.
    /// Subtrees are compared by identity first and pruned only after an exact comparison
    /// agrees, so a colliding identity function costs this diff time and cannot cost it
    /// correctness — the collisions it absorbed are reported as
    /// [`SnapshotDiff::identity_collisions`].
    ///
    /// Rename detection, move detection, content-level hunks, and overlay provenance are
    /// PR 3 / IMPL-02's richer diff; a path that moved appears here as a removal and an
    /// addition, which is the truthful identity-level answer.
    #[must_use]
    pub fn diff(&self, other: &Self) -> SnapshotDiff {
        let mut changes: BTreeMap<WorkspacePath, ChangeKind> = BTreeMap::new();
        let mut collisions = 0usize;
        let mut trail: Vec<String> = Vec::new();
        diff_nodes(
            &self.root,
            &other.root,
            &mut trail,
            &mut changes,
            &mut collisions,
        );
        SnapshotDiff {
            changes: changes
                .into_iter()
                .map(|(path, kind)| PathChange { path, kind })
                .collect(),
            identity_collisions: collisions,
        }
    }

    /// Encode the whole tree, wire-friendly and versioned.
    ///
    /// Grammar, version [`ENCODING_VERSION`], all integers big-endian:
    ///
    /// ```text
    /// snapshot := "cwsnap" u8(version) node
    /// node     := 0x00 u64(len) content                       -- a file
    ///           | 0x01 u64(count) entry*                      -- a directory
    /// entry    := u64(name_len) name node
    /// ```
    ///
    /// Canonical: entry names are strictly ascending, `count` and `len` are exact, and
    /// nothing follows the root node. [`Snapshot::decode`] rejects every other spelling
    /// rather than normalizing it, so an encoding round-trips to itself.
    ///
    /// Identities are deliberately **not** transmitted. A reader re-derives them from the
    /// content it actually received — ADR-0013's "proof validity still derives from
    /// decoded structure" — so a stream cannot assert an identity its bytes do not have.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(TREE_MAGIC);
        out.push(ENCODING_VERSION);
        encode_node(&self.root, &mut out);
        out
    }

    /// Decode a tree and re-derive every identity through `identifier`.
    ///
    /// # Errors
    ///
    /// [`SnapshotDecodeError`] naming the first violation. Every rejection is typed: a
    /// truncated stream, an oversized length, a non-ascending or duplicated name, an
    /// unusable name, a tree deeper than [`MAX_DEPTH`], a file at the root, trailing
    /// bytes, and a seam that refuses to name a node all land here rather than in a panic
    /// or in a partially built snapshot.
    pub fn decode<I: ContentIdentifier + ?Sized>(
        bytes: &[u8],
        identifier: &I,
    ) -> Result<Self, SnapshotDecodeError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(TREE_MAGIC.len())? != TREE_MAGIC {
            return Err(SnapshotDecodeError::Magic);
        }
        let version = cursor.byte()?;
        if version != ENCODING_VERSION {
            return Err(SnapshotDecodeError::Version { found: version });
        }
        let mut trail: Vec<String> = Vec::new();
        let root = decode_node(&mut cursor, &mut trail, identifier)?;
        if !cursor.is_empty() {
            return Err(SnapshotDecodeError::TrailingBytes { at: cursor.at });
        }
        if root.as_directory().is_none() {
            return Err(SnapshotDecodeError::RootIsNotADirectory);
        }
        Ok(Self { root })
    }
}

// --- diff ------------------------------------------------------------------------------------

/// How one file path differs between two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    /// Present only in the right-hand snapshot.
    Added,
    /// Present only in the left-hand snapshot.
    Removed,
    /// Present in both, with different content.
    Modified,
}

impl ChangeKind {
    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
        }
    }
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One changed file path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathChange {
    path: WorkspacePath,
    kind: ChangeKind,
}

impl PathChange {
    /// The path that changed.
    #[must_use]
    pub const fn path(&self) -> &WorkspacePath {
        &self.path
    }

    /// How it changed.
    #[must_use]
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }
}

impl fmt::Display for PathChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.path)
    }
}

/// Which file paths differ between two snapshots, and what the comparison had to survive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotDiff {
    changes: Vec<PathChange>,
    identity_collisions: usize,
}

impl SnapshotDiff {
    /// Every change, in path order.
    pub fn changes(&self) -> impl Iterator<Item = &PathChange> {
        self.changes.iter()
    }

    /// Every changed path, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &WorkspacePath> {
        self.changes.iter().map(PathChange::path)
    }

    /// How many changes there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether the two snapshots hold the same files with the same content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// How many subtree pairs carried equal identities over unequal content.
    ///
    /// Zero under any injective identity function, and expected to stay zero under a
    /// cryptographic one. Non-zero is data, not a warning to suppress: it means the
    /// identity seam collided and exact comparison resolved it, which is ADR-0013's
    /// certified-lane behavior observed rather than assumed. It never changes what the
    /// diff reports.
    #[must_use]
    pub const fn identity_collisions(&self) -> usize {
        self.identity_collisions
    }
}

// --- building ------------------------------------------------------------------------------

/// A mutable tree under construction. Never observable: sealing consumes it.
#[derive(Debug, Default)]
struct DraftDirectory {
    children: BTreeMap<String, DraftNode>,
}

#[derive(Debug)]
enum DraftNode {
    File(Vec<u8>),
    Directory(DraftDirectory),
}

impl DraftDirectory {
    /// Place one file, creating the directories on the way.
    fn insert(&mut self, path: &WorkspacePath, content: &[u8]) -> Result<(), SnapshotError> {
        let depth = path.depth();
        if depth > MAX_DEPTH {
            return Err(SnapshotError::TooDeep {
                depth,
                max: MAX_DEPTH,
            });
        }
        let mut cursor = self;
        for (index, segment) in path.segments().enumerate() {
            let last = index + 1 == depth;
            if last {
                if cursor.children.contains_key(segment) {
                    // Reached only when the same description holds a path twice or holds
                    // a path that another path needs as a directory; `WorkspaceContent`
                    // rejects both at insertion, so this is defense in depth.
                    return Err(SnapshotError::DuplicatePath { path: path.clone() });
                }
                cursor
                    .children
                    .insert(segment.to_owned(), DraftNode::File(content.to_vec()));
                return Ok(());
            }
            let next = cursor
                .children
                .entry(segment.to_owned())
                .or_insert_with(|| DraftNode::Directory(DraftDirectory::default()));
            match next {
                DraftNode::Directory(directory) => cursor = directory,
                DraftNode::File(_) => {
                    let ancestor = WorkspacePath {
                        segments: path.segments.get(..=index).unwrap_or_default().to_vec(),
                    };
                    return Err(SnapshotError::PathConflict {
                        file: ancestor,
                        descendant: path.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Name every node, children first, and freeze the tree.
    ///
    /// `trail` is the path of the directory being sealed, and is used only to name a node
    /// in an error. Recursion is bounded by [`MAX_DEPTH`], which `insert` enforces.
    fn seal<I: ContentIdentifier + ?Sized>(
        self,
        trail: &mut Vec<String>,
        identifier: &I,
    ) -> Result<DirectoryNode, SnapshotError> {
        let mut children: BTreeMap<String, SnapshotNode> = BTreeMap::new();
        for (segment, draft) in self.children {
            trail.push(segment.clone());
            let sealed = match draft {
                DraftNode::File(content) => {
                    SnapshotNode::File(seal_file(&content, trail, identifier)?)
                }
                DraftNode::Directory(directory) => {
                    SnapshotNode::Directory(directory.seal(trail, identifier)?)
                }
            };
            trail.pop();
            children.insert(segment, sealed);
        }

        let mut record = Vec::new();
        record.extend_from_slice(NODE_MAGIC);
        record.push(ENCODING_VERSION);
        record.push(TAG_DIRECTORY);
        record.extend_from_slice(&length_of(children.len()).to_be_bytes());
        for (segment, child) in &children {
            let handle = child.identity().to_string();
            record.extend_from_slice(&length_of(segment.len()).to_be_bytes());
            record.extend_from_slice(segment.as_bytes());
            record.extend_from_slice(&length_of(handle.len()).to_be_bytes());
            record.extend_from_slice(handle.as_bytes());
        }
        let identity = derive(identifier, &record, trail)?;
        Ok(DirectoryNode {
            identity,
            record,
            children,
        })
    }
}

/// Frame a file's content and name it.
fn seal_file<I: ContentIdentifier + ?Sized>(
    content: &[u8],
    trail: &[String],
    identifier: &I,
) -> Result<FileNode, SnapshotError> {
    let mut record = Vec::with_capacity(FILE_RECORD_HEADER + content.len());
    record.extend_from_slice(NODE_MAGIC);
    record.push(ENCODING_VERSION);
    record.push(TAG_FILE);
    record.extend_from_slice(&length_of(content.len()).to_be_bytes());
    record.extend_from_slice(content);
    let identity = derive(identifier, &record, trail)?;
    Ok(FileNode { identity, record })
}

/// Ask the seam for a node's identity and check the class it came back in.
fn derive<I: ContentIdentifier + ?Sized>(
    identifier: &I,
    record: &[u8],
    trail: &[String],
) -> Result<ArtifactHandle, SnapshotError> {
    let path = trail_path(trail);
    let handle = identifier
        .identify(NODE_CLASS, record)
        .map_err(|IdentityUnavailable| SnapshotError::IdentityUnavailable { path: path.clone() })?;
    if handle.class() == NODE_CLASS {
        Ok(handle)
    } else {
        Err(SnapshotError::IdentityClassMismatch {
            path,
            class: handle.class(),
        })
    }
}

/// The path a trail names, or [`None`] at the root.
fn trail_path(trail: &[String]) -> Option<WorkspacePath> {
    if trail.is_empty() {
        None
    } else {
        Some(WorkspacePath {
            segments: trail.to_vec(),
        })
    }
}

/// A length as it appears in a record. Saturating rather than panicking: a length that
/// does not fit in 64 bits cannot be produced by an in-memory tree on any target this
/// workspace supports, and a saturating conversion is a wrong number where a panic is a
/// crash on adversarial input.
fn length_of(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

// --- traversal -------------------------------------------------------------------------------

/// Collect every non-root node with its path, pre-order, which under the segment-wise
/// order is path order.
fn collect_subtrees<'a>(
    node: &'a SnapshotNode,
    trail: &mut Vec<String>,
    out: &mut Vec<(WorkspacePath, &'a SnapshotNode)>,
) {
    let SnapshotNode::Directory(directory) = node else {
        return;
    };
    for (segment, child) in directory.children() {
        trail.push(segment.to_owned());
        if let Some(path) = trail_path(trail) {
            out.push((path, child));
        }
        collect_subtrees(child, trail, out);
        trail.pop();
    }
}

/// Collect distinct records, children before parents.
fn collect_records<'a>(
    node: &'a SnapshotNode,
    seen: &mut BTreeSet<&'a [u8]>,
    out: &mut Vec<(&'a ArtifactHandle, &'a [u8])>,
) {
    if let SnapshotNode::Directory(directory) = node {
        for (_, child) in directory.children() {
            collect_records(child, seen, out);
        }
    }
    if seen.insert(node.record()) {
        out.push((node.identity(), node.record()));
    }
}

/// Compare two nodes at the same location.
fn diff_nodes(
    left: &SnapshotNode,
    right: &SnapshotNode,
    trail: &mut Vec<String>,
    out: &mut BTreeMap<WorkspacePath, ChangeKind>,
    collisions: &mut usize,
) {
    // ADR-0013, applied to a subtree: the identity partitions, the exact comparison
    // decides. Equal identities over unequal content are counted, never trusted.
    let same_identity = left.identity() == right.identity();
    let same_content = left == right;
    if same_identity && !same_content {
        *collisions += 1;
    }
    if same_content {
        return;
    }

    match (left, right) {
        (SnapshotNode::File(_), SnapshotNode::File(_)) => {
            if let Some(path) = trail_path(trail) {
                out.insert(path, ChangeKind::Modified);
            }
        }
        (SnapshotNode::Directory(left), SnapshotNode::Directory(right)) => {
            let segments: BTreeSet<&str> = left
                .children()
                .map(|(segment, _)| segment)
                .chain(right.children().map(|(segment, _)| segment))
                .collect();
            for segment in segments {
                trail.push(segment.to_owned());
                match (left.child(segment), right.child(segment)) {
                    (Some(left), Some(right)) => diff_nodes(left, right, trail, out, collisions),
                    (Some(left), None) => mark_subtree(left, trail, out, ChangeKind::Removed),
                    (None, Some(right)) => mark_subtree(right, trail, out, ChangeKind::Added),
                    (None, None) => {}
                }
                trail.pop();
            }
        }
        // A path that is a file on one side and a directory on the other: the file went
        // away and every file under the directory arrived, which is the truthful
        // identity-level reading of a replacement.
        (SnapshotNode::File(_), SnapshotNode::Directory(_)) => {
            mark_subtree(left, trail, out, ChangeKind::Removed);
            mark_subtree(right, trail, out, ChangeKind::Added);
        }
        (SnapshotNode::Directory(_), SnapshotNode::File(_)) => {
            mark_subtree(left, trail, out, ChangeKind::Removed);
            mark_subtree(right, trail, out, ChangeKind::Added);
        }
    }
}

/// Record every file under `node` as `kind`.
fn mark_subtree(
    node: &SnapshotNode,
    trail: &mut Vec<String>,
    out: &mut BTreeMap<WorkspacePath, ChangeKind>,
    kind: ChangeKind,
) {
    match node {
        SnapshotNode::File(_) => {
            if let Some(path) = trail_path(trail) {
                out.insert(path, kind);
            }
        }
        SnapshotNode::Directory(directory) => {
            for (segment, child) in directory.children() {
                trail.push(segment.to_owned());
                mark_subtree(child, trail, out, kind);
                trail.pop();
            }
        }
    }
}

// --- encoding --------------------------------------------------------------------------------

/// Append a node to the tree encoding.
fn encode_node(node: &SnapshotNode, out: &mut Vec<u8>) {
    match node {
        SnapshotNode::File(file) => {
            let content = file.content();
            out.push(TAG_FILE);
            out.extend_from_slice(&length_of(content.len()).to_be_bytes());
            out.extend_from_slice(content);
        }
        SnapshotNode::Directory(directory) => {
            out.push(TAG_DIRECTORY);
            out.extend_from_slice(&length_of(directory.len()).to_be_bytes());
            for (segment, child) in directory.children() {
                out.extend_from_slice(&length_of(segment.len()).to_be_bytes());
                out.extend_from_slice(segment.as_bytes());
                encode_node(child, out);
            }
        }
    }
}

/// Why a byte string is not a snapshot encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotDecodeError {
    /// The stream did not start with the tree magic.
    Magic,
    /// The stream declared an encoding version this build does not implement.
    ///
    /// Rejected outright rather than best-effort decoded: docs/09 T13 and ADR-0018 both
    /// require an artifact declaring an unimplemented contract to be refused.
    Version {
        /// The version the stream declared.
        found: u8,
    },
    /// The stream ended inside a field.
    UnexpectedEnd {
        /// Byte offset the read started at.
        at: usize,
        /// How many bytes were needed.
        needed: usize,
    },
    /// A node tag that is neither a file nor a directory.
    UnknownTag {
        /// The tag byte.
        tag: u8,
        /// Byte offset it was read at.
        at: usize,
    },
    /// A declared length exceeds what the stream can hold, or what this target can index.
    ///
    /// Checked against the remaining input before anything is allocated, so an
    /// adversarial length is a typed error rather than an allocation.
    LengthOutOfRange {
        /// Byte offset of the length field.
        at: usize,
        /// The declared length.
        declared: u64,
    },
    /// An entry name was not UTF-8.
    NameEncoding {
        /// Byte offset of the name.
        at: usize,
    },
    /// An entry name is not a usable path segment.
    Name {
        /// Byte offset of the name.
        at: usize,
        /// Why the segment was rejected.
        error: PathError,
    },
    /// Entry names were not strictly ascending — out of order, or repeated.
    ///
    /// The canonical order is the ordering contract, so a stream that lists children in
    /// any other order is a second spelling of a snapshot that already has one.
    NameOrder {
        /// Byte offset of the offending name.
        at: usize,
        /// The name that did not follow its predecessor.
        name: String,
    },
    /// The tree nests deeper than [`MAX_DEPTH`].
    TooDeep {
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
    /// The root node was a file. A workspace root is a directory.
    RootIsNotADirectory,
    /// Bytes followed the root node.
    TrailingBytes {
        /// Byte offset of the first trailing byte.
        at: usize,
    },
    /// The identity seam refused or misnamed a node of the decoded tree.
    Identity(SnapshotError),
}

impl fmt::Display for SnapshotDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Magic => f.write_str("not a workspace-snapshot encoding"),
            Self::Version { found } => write!(
                f,
                "snapshot encoding version {found} is not {ENCODING_VERSION}; an artifact \
                 declaring an unimplemented contract is refused, never best-effort decoded"
            ),
            Self::UnexpectedEnd { at, needed } => {
                write!(
                    f,
                    "snapshot encoding ends at byte {at}, needing {needed} more"
                )
            }
            Self::UnknownTag { tag, at } => {
                write!(f, "unknown snapshot node tag {tag:#04x} at byte {at}")
            }
            Self::LengthOutOfRange { at, declared } => write!(
                f,
                "snapshot encoding declares a length of {declared} at byte {at}, past the \
                 end of the stream"
            ),
            Self::NameEncoding { at } => {
                write!(f, "snapshot entry name at byte {at} is not UTF-8")
            }
            Self::Name { at, error } => {
                write!(f, "snapshot entry name at byte {at}: {error}")
            }
            Self::NameOrder { at, name } => write!(
                f,
                "snapshot entry name {name:?} at byte {at} does not strictly follow its \
                 predecessor; entries are canonically ordered"
            ),
            Self::TooDeep { max } => {
                write!(f, "snapshot encoding nests past the {max} bound")
            }
            Self::RootIsNotADirectory => f.write_str("a snapshot root is a directory, not a file"),
            Self::TrailingBytes { at } => {
                write!(f, "snapshot encoding has trailing bytes from {at}")
            }
            Self::Identity(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for SnapshotDecodeError {}

impl From<SnapshotError> for SnapshotDecodeError {
    fn from(error: SnapshotError) -> Self {
        Self::Identity(error)
    }
}

/// A bounds-checked reader. Every read is fallible; nothing here can panic.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn is_empty(&self) -> bool {
        self.at >= self.bytes.len()
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], SnapshotDecodeError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(SnapshotDecodeError::UnexpectedEnd {
                at: self.at,
                needed: count,
            })?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(SnapshotDecodeError::UnexpectedEnd {
                at: self.at,
                needed: count,
            })?;
        self.at = end;
        Ok(slice)
    }

    fn byte(&mut self) -> Result<u8, SnapshotDecodeError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(SnapshotDecodeError::UnexpectedEnd {
                at: self.at,
                needed: 1,
            })
    }

    /// Read a length and check it against what is left, before anything is allocated.
    fn length(&mut self) -> Result<usize, SnapshotDecodeError> {
        let at = self.at;
        let raw = self.take(8)?;
        let declared = <[u8; 8]>::try_from(raw).map_or(u64::MAX, u64::from_be_bytes);
        let len = usize::try_from(declared)
            .map_err(|_| SnapshotDecodeError::LengthOutOfRange { at, declared })?;
        if len > self.remaining() {
            return Err(SnapshotDecodeError::LengthOutOfRange { at, declared });
        }
        Ok(len)
    }
}

/// Decode one node and name it.
///
/// Recursion is bounded: the depth check runs before descending, so an adversarially
/// nested stream is a [`SnapshotDecodeError::TooDeep`] and never a stack overflow.
fn decode_node<I: ContentIdentifier + ?Sized>(
    cursor: &mut Cursor<'_>,
    trail: &mut Vec<String>,
    identifier: &I,
) -> Result<SnapshotNode, SnapshotDecodeError> {
    if trail.len() > MAX_DEPTH {
        return Err(SnapshotDecodeError::TooDeep { max: MAX_DEPTH });
    }
    let at = cursor.at;
    let tag = cursor.byte()?;
    match tag {
        TAG_FILE => {
            let len = cursor.length()?;
            let content = cursor.take(len)?;
            Ok(SnapshotNode::File(seal_file(content, trail, identifier)?))
        }
        TAG_DIRECTORY => {
            // A child costs at least one tag byte plus a name length, so a declared count
            // beyond the remaining input is rejected before a single entry is read.
            let count = cursor.length()?;
            let mut children: BTreeMap<String, SnapshotNode> = BTreeMap::new();
            let mut previous: Option<String> = None;
            for _ in 0..count {
                let name_at = cursor.at;
                let name_len = cursor.length()?;
                let raw = cursor.take(name_len)?;
                let name = core::str::from_utf8(raw)
                    .map_err(|_| SnapshotDecodeError::NameEncoding { at: name_at })?;
                validate_segment(name, trail.len())
                    .map_err(|error| SnapshotDecodeError::Name { at: name_at, error })?;
                if previous.as_deref().is_some_and(|last| last >= name) {
                    return Err(SnapshotDecodeError::NameOrder {
                        at: name_at,
                        name: name.to_owned(),
                    });
                }
                previous = Some(name.to_owned());

                trail.push(name.to_owned());
                let child = decode_node(cursor, trail, identifier);
                trail.pop();
                children.insert(name.to_owned(), child?);
            }
            Ok(SnapshotNode::Directory(seal_directory(
                children, trail, identifier,
            )?))
        }
        _ => Err(SnapshotDecodeError::UnknownTag { tag, at }),
    }
}

/// Re-derive a directory's record and identity from already-sealed children.
///
/// The one place a directory record is written for a decoded tree; it must produce byte-
/// for-byte what [`DraftDirectory::seal`] produces, and
/// `an_encoding_round_trips_to_the_same_identities` pins that it does.
fn seal_directory<I: ContentIdentifier + ?Sized>(
    children: BTreeMap<String, SnapshotNode>,
    trail: &[String],
    identifier: &I,
) -> Result<DirectoryNode, SnapshotError> {
    let mut record = Vec::new();
    record.extend_from_slice(NODE_MAGIC);
    record.push(ENCODING_VERSION);
    record.push(TAG_DIRECTORY);
    record.extend_from_slice(&length_of(children.len()).to_be_bytes());
    for (segment, child) in &children {
        let handle = child.identity().to_string();
        record.extend_from_slice(&length_of(segment.len()).to_be_bytes());
        record.extend_from_slice(segment.as_bytes());
        record.extend_from_slice(&length_of(handle.len()).to_be_bytes());
        record.extend_from_slice(handle.as_bytes());
    }
    let identity = derive(identifier, &record, trail)?;
    Ok(DirectoryNode {
        identity,
        record,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The injective identity function: a record's own bytes, in hex.
    ///
    /// ADR-0013's certified discipline written as a [`ContentIdentifier`] — the canonical
    /// bytes *are* the identity — so "distinct content, distinct identity" is a fact about
    /// this seam and not a probabilistic claim. Identities grow with the subtree, which is
    /// why the adversarial and wide-tree tests use the smaller seams below.
    struct HexIdentity;

    impl ContentIdentifier for HexIdentity {
        fn identify(
            &self,
            class: ArtifactClass,
            content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            let mut token = String::with_capacity(content.len() * 2);
            for byte in content {
                token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
                token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
            }
            ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
        }
    }

    /// Every record gets the same name. The worst identity function that exists.
    struct ConstantIdentity;

    impl ContentIdentifier for ConstantIdentity {
        fn identify(
            &self,
            class: ArtifactClass,
            _content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            ArtifactHandle::new(class, "collide").map_err(|_| IdentityUnavailable)
        }
    }

    /// Refuses to name anything.
    struct NoIdentity;

    impl ContentIdentifier for NoIdentity {
        fn identify(
            &self,
            _class: ArtifactClass,
            _content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            Err(IdentityUnavailable)
        }
    }

    /// Names nodes in the wrong artifact class.
    struct WrongClassIdentity;

    impl ContentIdentifier for WrongClassIdentity {
        fn identify(
            &self,
            _class: ArtifactClass,
            _content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            ArtifactHandle::new(ArtifactClass::Evidence, "wrong").map_err(|_| IdentityUnavailable)
        }
    }

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("test path is well formed")
    }

    fn content(files: &[(&str, &[u8])]) -> WorkspaceContent {
        let mut out = WorkspaceContent::new();
        for (text, bytes) in files {
            out.insert(path(text), (*bytes).to_vec())
                .expect("test description is a tree");
        }
        out
    }

    fn snapshot(files: &[(&str, &[u8])]) -> Snapshot {
        Snapshot::build(&content(files), &HexIdentity).expect("HexIdentity names every node")
    }

    const SAMPLE: [(&str, &[u8]); 5] = [
        ("Cargo.toml", b"[package]"),
        ("README.md", b"hello"),
        ("crates/value/lib.rs", b"pub fn v() {}"),
        ("crates/workspace/lib.rs", b"pub fn w() {}"),
        ("crates/workspace/snapshot.rs", b"pub struct S;"),
    ];

    // --- the ordering contract ----------------------------------------------------------

    #[test]
    fn path_order_is_segment_wise_not_string_order() {
        // `.` (0x2e) sorts before `/` (0x2f), so string order puts `a.b` between `a` and
        // `a/b`; segment-wise order does not, and only segment-wise order agrees with a
        // tree in which `a/b` is a child of `a`.
        assert!("a.b" < "a/b");
        assert!(path("a/b") < path("a.b"));
        assert!(path("a") < path("a/b"));
        assert!(path("a/b") < path("a/b/c"));
        assert!(path("a/z") > path("a/b/c"));
    }

    #[test]
    fn segments_are_compared_exactly() {
        assert_ne!(path("A.rs"), path("a.rs"));
        assert!(path("A.rs") < path("a.rs"));
    }

    #[test]
    fn unusable_paths_are_typed_errors() {
        assert_eq!(WorkspacePath::new(""), Err(PathError::Empty));
        assert_eq!(
            WorkspacePath::from_segments(Vec::<String>::new()),
            Err(PathError::Empty)
        );
        assert_eq!(
            WorkspacePath::new("/a"),
            Err(PathError::EmptySegment { index: 0 })
        );
        assert_eq!(
            WorkspacePath::new("a/"),
            Err(PathError::EmptySegment { index: 1 })
        );
        assert_eq!(
            WorkspacePath::new("a//b"),
            Err(PathError::EmptySegment { index: 1 })
        );
        assert_eq!(
            WorkspacePath::new("../etc/passwd"),
            Err(PathError::RelativeSegment {
                index: 0,
                segment: "..".to_owned()
            })
        );
        assert_eq!(
            WorkspacePath::new("a/./b"),
            Err(PathError::RelativeSegment {
                index: 1,
                segment: ".".to_owned()
            })
        );
        assert_eq!(
            WorkspacePath::new("a\0b"),
            Err(PathError::SegmentCharacter {
                index: 0,
                character: '\0'
            })
        );
        assert_eq!(
            WorkspacePath::new("a\\b"),
            Err(PathError::SegmentCharacter {
                index: 0,
                character: '\\'
            })
        );
        assert!(matches!(
            WorkspacePath::from_segments(vec!["x"; MAX_DEPTH + 1]),
            Err(PathError::TooDeep { .. })
        ));
        // A separator inside an explicit segment would silently deepen the tree.
        assert_eq!(
            WorkspacePath::from_segments(["a/b"]),
            Err(PathError::SegmentCharacter {
                index: 0,
                character: '/'
            })
        );
    }

    #[test]
    fn a_path_renders_and_reparses() {
        for text in ["a", "a/b", "crates/continuum-workspace/src/snapshot.rs"] {
            let parsed = path(text);
            assert_eq!(parsed.to_string(), text);
            assert_eq!(text.parse::<WorkspacePath>(), Ok(parsed));
        }
    }

    // --- the description ------------------------------------------------------------------

    #[test]
    fn a_duplicate_path_is_refused() {
        let mut description = WorkspaceContent::new();
        description
            .insert(path("a/b"), b"1".to_vec())
            .expect("first");
        assert_eq!(
            description.insert(path("a/b"), b"2".to_vec()),
            Err(SnapshotError::DuplicatePath { path: path("a/b") })
        );
        // And the first content is untouched: a refusal changes nothing.
        assert_eq!(description.get(&path("a/b")), Some(b"1".as_slice()));
    }

    #[test]
    fn a_path_cannot_be_a_file_and_a_directory() {
        let mut file_first = WorkspaceContent::new();
        file_first.insert(path("a/b"), b"1".to_vec()).expect("file");
        assert_eq!(
            file_first.insert(path("a/b/c"), b"2".to_vec()),
            Err(SnapshotError::PathConflict {
                file: path("a/b"),
                descendant: path("a/b/c"),
            })
        );

        let mut directory_first = WorkspaceContent::new();
        directory_first
            .insert(path("a/b/c"), b"2".to_vec())
            .expect("file");
        assert_eq!(
            directory_first.insert(path("a/b"), b"1".to_vec()),
            Err(SnapshotError::PathConflict {
                file: path("a/b"),
                descendant: path("a/b/c"),
            })
        );
    }

    #[test]
    fn a_conflict_check_does_not_reject_a_sibling() {
        let mut description = WorkspaceContent::new();
        description.insert(path("a/b"), b"1".to_vec()).expect("b");
        description.insert(path("a/bb"), b"2".to_vec()).expect("bb");
        description.insert(path("ab"), b"3".to_vec()).expect("ab");
        assert_eq!(description.len(), 3);
    }

    // --- determinism -----------------------------------------------------------------------

    #[test]
    fn the_same_content_twice_is_the_same_root() {
        let first = snapshot(&SAMPLE);
        let second = snapshot(&SAMPLE);
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first, second);
    }

    #[test]
    fn permuting_the_input_order_does_not_move_the_root() {
        let forward = snapshot(&SAMPLE);

        let mut reversed: Vec<(&str, &[u8])> = SAMPLE.to_vec();
        reversed.reverse();
        assert_eq!(snapshot(&reversed).identity(), forward.identity());

        let mut rotated: Vec<(&str, &[u8])> = SAMPLE.to_vec();
        rotated.rotate_left(3);
        assert_eq!(snapshot(&rotated).identity(), forward.identity());
    }

    #[test]
    fn an_empty_workspace_has_an_identity_of_its_own() {
        let empty = Snapshot::build(&WorkspaceContent::new(), &HexIdentity).expect("named");
        assert!(empty.is_empty());
        assert_eq!(empty.file_count(), 0);
        assert_eq!(empty.subtrees().len(), 0);
        assert_eq!(empty.root().kind(), NodeKind::Directory);
        // Rebuilt, it is the same snapshot; and it is not any file's identity, because
        // the record grammar tags directories and files apart.
        let again = Snapshot::build(&WorkspaceContent::new(), &HexIdentity).expect("named");
        assert_eq!(empty.identity(), again.identity());
        let one_empty_file = snapshot(&[("a", b"")]);
        let leaf = one_empty_file.node(&path("a")).expect("present");
        assert_ne!(leaf.identity(), empty.identity());
    }

    // --- sensitivity -------------------------------------------------------------------------

    #[test]
    fn one_byte_moves_the_root() {
        let before = snapshot(&[("a/b.rs", b"xy")]);
        let after = snapshot(&[("a/b.rs", b"xz")]);
        assert_ne!(before.identity(), after.identity());
        assert_ne!(before, after);
    }

    #[test]
    fn a_rename_moves_the_root() {
        let before = snapshot(&[("a/b.rs", b"same")]);
        let after = snapshot(&[("a/c.rs", b"same")]);
        assert_ne!(before.identity(), after.identity());
        // The leaf itself is unchanged — content is content — which is exactly why the
        // *directory* record has to name its children, and does.
        assert_eq!(
            before.node(&path("a/b.rs")).map(SnapshotNode::identity),
            after.node(&path("a/c.rs")).map(SnapshotNode::identity),
        );
        assert_ne!(
            before.node(&path("a")).map(SnapshotNode::identity),
            after.node(&path("a")).map(SnapshotNode::identity),
        );
    }

    #[test]
    fn moving_a_file_between_directories_moves_the_root() {
        let before = snapshot(&[("a/x", b"c"), ("b/y", b"d")]);
        let after = snapshot(&[("a/y", b"d"), ("b/x", b"c")]);
        assert_ne!(before.identity(), after.identity());
    }

    #[test]
    fn adding_and_removing_a_file_moves_the_root() {
        let base = snapshot(&SAMPLE);
        let mut extended: Vec<(&str, &[u8])> = SAMPLE.to_vec();
        extended.push(("crates/workspace/extra.rs", b"more"));
        let extended = snapshot(&extended);
        assert_ne!(base.identity(), extended.identity());
        assert_eq!(extended.file_count(), base.file_count() + 1);
    }

    // --- structural sharing ---------------------------------------------------------------

    #[test]
    fn two_snapshots_differing_in_one_file_share_every_other_subtree() {
        let base = snapshot(&SAMPLE);
        let mut edited_files: Vec<(&str, &[u8])> = SAMPLE.to_vec();
        edited_files[3] = ("crates/workspace/lib.rs", b"pub fn w2() {}");
        let edited = snapshot(&edited_files);

        let touched = [
            path("crates"),
            path("crates/workspace"),
            path("crates/workspace/lib.rs"),
        ];

        let base_nodes: BTreeMap<WorkspacePath, &ArtifactHandle> = base
            .subtrees()
            .into_iter()
            .map(|(path, node)| (path, node.identity()))
            .collect();
        let edited_nodes: BTreeMap<WorkspacePath, &ArtifactHandle> = edited
            .subtrees()
            .into_iter()
            .map(|(path, node)| (path, node.identity()))
            .collect();

        assert_eq!(
            base_nodes.keys().collect::<Vec<_>>(),
            edited_nodes.keys().collect::<Vec<_>>()
        );
        for (path, identity) in &base_nodes {
            let other = edited_nodes.get(path).expect("same shape");
            if touched.contains(path) {
                assert_ne!(identity, other, "{path} should have been re-derived");
            } else {
                assert_eq!(identity, other, "{path} should have been shared");
            }
        }
        assert_ne!(base.identity(), edited.identity());
    }

    #[test]
    fn identical_subtrees_in_one_snapshot_share_one_record() {
        // Two directories with byte-identical contents are one subtree, named once.
        let snapshot = snapshot(&[("left/f", b"same"), ("right/f", b"same")]);
        let left = snapshot.node(&path("left")).expect("present");
        let right = snapshot.node(&path("right")).expect("present");
        assert_eq!(left.identity(), right.identity());
        assert_eq!(left, right);

        // The publication listing therefore carries the shared subtree once: root, the
        // shared directory, and the shared leaf.
        assert_eq!(snapshot.records().len(), 3);
    }

    #[test]
    fn records_are_listed_children_before_parents() {
        let snapshot = snapshot(&SAMPLE);
        let mut position: BTreeMap<&[u8], usize> = BTreeMap::new();
        for (index, (_, record)) in snapshot.records().iter().enumerate() {
            position.insert(record, index);
        }
        // Every parent appears after every one of its children.
        for (_, node) in snapshot.subtrees() {
            if let Some(directory) = node.as_directory() {
                let parent = position.get(node.record()).copied().unwrap_or_default();
                for (_, child) in directory.children() {
                    let child = position.get(child.record()).copied().unwrap_or_default();
                    assert!(child < parent);
                }
            }
        }
        let root = position.get(snapshot.root().record()).copied();
        assert_eq!(root, Some(snapshot.records().len() - 1));
    }

    // --- diff ----------------------------------------------------------------------------

    #[test]
    fn an_unchanged_snapshot_diffs_to_nothing() {
        let diff = snapshot(&SAMPLE).diff(&snapshot(&SAMPLE));
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
        assert_eq!(diff.identity_collisions(), 0);
    }

    #[test]
    fn a_diff_names_exactly_the_changed_paths() {
        let before = snapshot(&[("a/x", b"1"), ("a/y", b"2"), ("gone", b"3")]);
        let after = snapshot(&[("a/x", b"1"), ("a/y", b"changed"), ("new", b"4")]);
        let diff = before.diff(&after);
        let observed: Vec<(String, ChangeKind)> = diff
            .changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect();
        assert_eq!(
            observed,
            [
                ("a/y".to_owned(), ChangeKind::Modified),
                ("gone".to_owned(), ChangeKind::Removed),
                ("new".to_owned(), ChangeKind::Added),
            ]
        );
        // Reversed, additions and removals swap and modifications stay.
        let reverse: Vec<(String, ChangeKind)> = after
            .diff(&before)
            .changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect();
        assert_eq!(
            reverse,
            [
                ("a/y".to_owned(), ChangeKind::Modified),
                ("gone".to_owned(), ChangeKind::Added),
                ("new".to_owned(), ChangeKind::Removed),
            ]
        );
    }

    #[test]
    fn a_file_replaced_by_a_directory_is_a_removal_and_additions() {
        let before = snapshot(&[("a", b"file")]);
        let after = snapshot(&[("a/b", b"1"), ("a/c", b"2")]);
        let observed: Vec<(String, ChangeKind)> = before
            .diff(&after)
            .changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect();
        assert_eq!(
            observed,
            [
                ("a".to_owned(), ChangeKind::Removed),
                ("a/b".to_owned(), ChangeKind::Added),
                ("a/c".to_owned(), ChangeKind::Added),
            ]
        );
    }

    #[test]
    fn a_whole_directory_removal_lists_its_files() {
        let before = snapshot(&SAMPLE);
        let after = snapshot(&[("Cargo.toml", b"[package]"), ("README.md", b"hello")]);
        let observed: Vec<String> = before
            .diff(&after)
            .paths()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            observed,
            [
                "crates/value/lib.rs".to_owned(),
                "crates/workspace/lib.rs".to_owned(),
                "crates/workspace/snapshot.rs".to_owned(),
            ]
        );
    }

    // --- adversarial identities --------------------------------------------------------------

    #[test]
    fn a_colliding_identity_never_conflates_distinct_content() {
        let left = Snapshot::build(&content(&[("a", b"one")]), &ConstantIdentity).expect("named");
        let right = Snapshot::build(&content(&[("a", b"two")]), &ConstantIdentity).expect("named");

        // The collision is total and real: every node in both trees has one name.
        assert_eq!(left.identity(), right.identity());
        assert_eq!(
            left.node(&path("a")).map(SnapshotNode::identity),
            right.node(&path("a")).map(SnapshotNode::identity),
        );

        // And nothing is conflated.
        assert_ne!(left, right);
        let diff = left.diff(&right);
        assert_eq!(
            diff.changes()
                .map(|change| (change.path().to_string(), change.kind()))
                .collect::<Vec<_>>(),
            [("a".to_owned(), ChangeKind::Modified)]
        );
        // The collision is counted, not hidden: the root and the leaf both collided.
        assert_eq!(diff.identity_collisions(), 2);
    }

    #[test]
    fn a_colliding_identity_does_not_invent_changes_either() {
        let left = Snapshot::build(&content(&[("a", b"same")]), &ConstantIdentity).expect("named");
        let right = Snapshot::build(&content(&[("a", b"same")]), &ConstantIdentity).expect("named");
        let diff = left.diff(&right);
        assert!(diff.is_empty());
        assert_eq!(diff.identity_collisions(), 0);
        assert_eq!(left, right);
    }

    #[test]
    fn colliding_records_are_listed_separately_so_a_store_can_refuse_them() {
        // Dedup is by record bytes. Two distinct records that collide on identity survive
        // as two entries, which is what lets the store's exact comparison fire.
        let snapshot =
            Snapshot::build(&content(&[("a", b"one"), ("b", b"two")]), &ConstantIdentity)
                .expect("named");
        let records = snapshot.records();
        assert_eq!(records.len(), 3);
        let names: BTreeSet<String> = records
            .iter()
            .map(|(identity, _)| identity.to_string())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn a_seam_that_refuses_refuses_the_whole_snapshot() {
        assert_eq!(
            Snapshot::build(&content(&[("a/b", b"x")]), &NoIdentity),
            Err(SnapshotError::IdentityUnavailable {
                path: Some(path("a/b"))
            })
        );
        assert_eq!(
            Snapshot::build(&WorkspaceContent::new(), &NoIdentity),
            Err(SnapshotError::IdentityUnavailable { path: None })
        );
    }

    #[test]
    fn a_seam_that_misnames_the_class_is_refused() {
        assert_eq!(
            Snapshot::build(&content(&[("a", b"x")]), &WrongClassIdentity),
            Err(SnapshotError::IdentityClassMismatch {
                path: Some(path("a")),
                class: ArtifactClass::Evidence,
            })
        );
    }

    // --- serialization -------------------------------------------------------------------------

    #[test]
    fn an_encoding_round_trips_to_the_same_identities() {
        for files in [&SAMPLE[..], &[("a", b"" as &[u8])][..], &[][..]] {
            let original = Snapshot::build(&content(files), &HexIdentity).expect("named");
            let bytes = original.encode();
            let decoded = Snapshot::decode(&bytes, &HexIdentity).expect("round trip");
            assert_eq!(decoded, original);
            assert_eq!(decoded.identity(), original.identity());
            for ((left_path, left), (right_path, right)) in
                decoded.subtrees().into_iter().zip(original.subtrees())
            {
                assert_eq!(left_path, right_path);
                assert_eq!(left.identity(), right.identity());
                assert_eq!(left.record(), right.record());
            }
            // And the encoding itself is stable.
            assert_eq!(decoded.encode(), bytes);
        }
    }

    #[test]
    fn an_encoding_starts_with_its_magic_and_version() {
        let bytes = snapshot(&SAMPLE).encode();
        assert!(bytes.starts_with(TREE_MAGIC));
        assert_eq!(bytes.get(TREE_MAGIC.len()), Some(&ENCODING_VERSION));
    }

    #[test]
    fn malformed_encodings_are_typed_errors() {
        let good = snapshot(&SAMPLE).encode();

        assert_eq!(
            Snapshot::decode(b"", &HexIdentity),
            Err(SnapshotDecodeError::UnexpectedEnd {
                at: 0,
                needed: TREE_MAGIC.len()
            })
        );
        assert_eq!(
            Snapshot::decode(b"nope!!\x01\x01\0\0\0\0\0\0\0\0", &HexIdentity),
            Err(SnapshotDecodeError::Magic)
        );

        let mut wrong_version = good.clone();
        wrong_version[TREE_MAGIC.len()] = ENCODING_VERSION.wrapping_add(1);
        assert_eq!(
            Snapshot::decode(&wrong_version, &HexIdentity),
            Err(SnapshotDecodeError::Version {
                found: ENCODING_VERSION.wrapping_add(1)
            })
        );

        let mut trailing = good.clone();
        trailing.push(0x00);
        assert!(matches!(
            Snapshot::decode(&trailing, &HexIdentity),
            Err(SnapshotDecodeError::TrailingBytes { .. })
        ));

        let truncated = &good[..good.len() - 1];
        assert!(Snapshot::decode(truncated, &HexIdentity).is_err());

        // A file at the root.
        let mut file_root = Vec::from(TREE_MAGIC);
        file_root.push(ENCODING_VERSION);
        file_root.push(TAG_FILE);
        file_root.extend_from_slice(&0u64.to_be_bytes());
        assert_eq!(
            Snapshot::decode(&file_root, &HexIdentity),
            Err(SnapshotDecodeError::RootIsNotADirectory)
        );

        // An unknown tag.
        let mut bad_tag = Vec::from(TREE_MAGIC);
        bad_tag.push(ENCODING_VERSION);
        bad_tag.push(0x7f);
        assert!(matches!(
            Snapshot::decode(&bad_tag, &HexIdentity),
            Err(SnapshotDecodeError::UnknownTag { tag: 0x7f, .. })
        ));

        // A length past the end of the stream: rejected before anything is allocated.
        let mut huge = Vec::from(TREE_MAGIC);
        huge.push(ENCODING_VERSION);
        huge.push(TAG_FILE);
        huge.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(
            Snapshot::decode(&huge, &HexIdentity),
            Err(SnapshotDecodeError::LengthOutOfRange { .. })
        ));
    }

    /// Build a one-directory encoding with the given entry names, in the given order.
    fn directory_encoding(names: &[&str]) -> Vec<u8> {
        let mut out = Vec::from(TREE_MAGIC);
        out.push(ENCODING_VERSION);
        out.push(TAG_DIRECTORY);
        out.extend_from_slice(&length_of(names.len()).to_be_bytes());
        for name in names {
            out.extend_from_slice(&length_of(name.len()).to_be_bytes());
            out.extend_from_slice(name.as_bytes());
            out.push(TAG_FILE);
            out.extend_from_slice(&0u64.to_be_bytes());
        }
        out
    }

    #[test]
    fn non_canonical_orderings_are_rejected_rather_than_normalized() {
        assert!(Snapshot::decode(&directory_encoding(&["a", "b"]), &HexIdentity).is_ok());
        assert!(matches!(
            Snapshot::decode(&directory_encoding(&["b", "a"]), &HexIdentity),
            Err(SnapshotDecodeError::NameOrder { .. })
        ));
        assert!(matches!(
            Snapshot::decode(&directory_encoding(&["a", "a"]), &HexIdentity),
            Err(SnapshotDecodeError::NameOrder { .. })
        ));
    }

    #[test]
    fn unusable_entry_names_are_rejected() {
        for name in ["", ".", "..", "a/b", "a\\b"] {
            assert!(
                matches!(
                    Snapshot::decode(&directory_encoding(&[name]), &HexIdentity),
                    Err(SnapshotDecodeError::Name { .. })
                ),
                "entry name {name:?} was accepted"
            );
        }
        // Not UTF-8.
        let mut invalid = Vec::from(TREE_MAGIC);
        invalid.push(ENCODING_VERSION);
        invalid.push(TAG_DIRECTORY);
        invalid.extend_from_slice(&1u64.to_be_bytes());
        invalid.extend_from_slice(&1u64.to_be_bytes());
        invalid.push(0xff);
        invalid.push(TAG_FILE);
        invalid.extend_from_slice(&0u64.to_be_bytes());
        assert!(matches!(
            Snapshot::decode(&invalid, &HexIdentity),
            Err(SnapshotDecodeError::NameEncoding { .. })
        ));
    }

    #[test]
    fn an_adversarially_nested_encoding_is_a_typed_error_not_a_stack_overflow() {
        // MAX_DEPTH + 8 nested directories, each with one child.
        let depth = MAX_DEPTH + 8;
        let mut out = Vec::from(TREE_MAGIC);
        out.push(ENCODING_VERSION);
        for _ in 0..depth {
            out.push(TAG_DIRECTORY);
            out.extend_from_slice(&1u64.to_be_bytes());
            out.extend_from_slice(&1u64.to_be_bytes());
            out.push(b'x');
        }
        out.push(TAG_FILE);
        out.extend_from_slice(&0u64.to_be_bytes());
        assert_eq!(
            Snapshot::decode(&out, &HexIdentity),
            Err(SnapshotDecodeError::TooDeep { max: MAX_DEPTH })
        );
    }

    #[test]
    fn a_decoded_tree_is_named_by_the_reader_not_by_the_stream() {
        // The encoding carries no identities at all, so the same bytes under two seams
        // produce two names — and neither can be asserted by a sender.
        let bytes = snapshot(&SAMPLE).encode();
        let honest = Snapshot::decode(&bytes, &HexIdentity).expect("round trip");
        let colliding = Snapshot::decode(&bytes, &ConstantIdentity).expect("round trip");
        assert_eq!(honest, colliding);
        assert_ne!(honest.identity(), colliding.identity());
    }
}
