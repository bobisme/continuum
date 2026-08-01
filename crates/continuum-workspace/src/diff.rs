//! Diff: which files differ between two snapshots, and what they are named
//! (plan §4.2, PR 3 / IMPL-02).
//!
//! # What this module decides
//!
//! [`Snapshot::diff`] answers "which *paths* changed". That is the identity-level question
//! and it is already correct — including under a colliding identity seam, which it resolves
//! by exact comparison and reports rather than absorbs. What it cannot answer, because a
//! [`SnapshotDiff`] holds no handles, is "changed **from what to what**": the pair of
//! content identities a caller needs to fetch either side, cache a comparison, or record
//! the change in evidence.
//!
//! [`WorkspaceDiff`] is that answer. Every change carries the file's identity before and
//! after — `Some`/`None` exactly as the change kind implies — and the diff carries
//! [`SnapshotDiff::identity_collisions`] forward unchanged, because a number that is
//! dropped one layer up is a number that was suppressed.
//!
//! # Path-level, deliberately
//!
//! No rename detection, no move detection, no content hunks, no semantic reading of what
//! changed inside a file. A path that moved appears here as a removal and an addition,
//! which is the truthful identity-level answer: this layer knows bytes and names, and
//! *nothing* about what a byte means. Semantic diff is `continuum-semantic-diff`'s
//! (plan §5.3, docs/40) and it will consume artifacts this layer names — inferring
//! intent-bearing structure here would put a second, weaker semantic diff below the real
//! one.
//!
//! # Collisions are carried, not laundered
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`
//!
//! [`WorkspaceDiff::identity_collisions`] counts the subtree pairs whose identities were
//! equal over unequal content. It is zero under any injective seam and is expected to stay
//! zero under a cryptographic one; non-zero is data about the seam, never a reason to
//! change what the diff reports. The changes themselves are decided by exact comparison in
//! [`Snapshot::diff`] either way.
//!
//! # Overlay against base
//!
//! [`WorkspaceDiff::of_overlay`] diffs a derived snapshot against the base it was derived
//! from, and checks that it *is* that base ([`BaseMismatch`]). The check is not the
//! stale-snapshot error — that one is about a working tree moving under a snapshot, and it
//! is a separate deliverable — it is the narrower category error of handing this function
//! two values that were never related, where every reported change would be meaningless.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::diff::WorkspaceDiff;
//! use continuum_workspace::overlay::Overlay;
//! use continuum_workspace::snapshot::{ChangeKind, Snapshot, WorkspaceContent, WorkspacePath};
//! # use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! # use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
//! # struct HexIdentity;
//! # impl ContentIdentifier for HexIdentity {
//! #     fn identify(&self, class: ArtifactClass, content: &[u8])
//! #         -> Result<ArtifactHandle, IdentityUnavailable> {
//! #         let mut token = String::with_capacity(content.len() * 2);
//! #         for byte in content {
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
//! #             token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
//! #         }
//! #         ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
//! #     }
//! # }
//! let mut content = WorkspaceContent::new();
//! content.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! content.insert(WorkspacePath::new("gone.md")?, b"bye".to_vec())?;
//! let base = Snapshot::build(&content, &HexIdentity)?;
//!
//! let mut overlay = Overlay::new();
//! overlay.write(WorkspacePath::new("src/lib.rs")?, b"fn main() { todo!() }".to_vec())?;
//! overlay.write(WorkspacePath::new("new.md")?, b"hi".to_vec())?;
//! overlay.remove(WorkspacePath::new("gone.md")?)?;
//! let derived = overlay.derive(&base, &HexIdentity)?;
//!
//! let diff = WorkspaceDiff::of_overlay(&base, &derived)?;
//! assert_eq!(
//!     diff.changes().map(|change| (change.path().to_string(), change.kind())).collect::<Vec<_>>(),
//!     [
//!         ("gone.md".to_owned(), ChangeKind::Removed),
//!         ("new.md".to_owned(), ChangeKind::Added),
//!         ("src/lib.rs".to_owned(), ChangeKind::Modified),
//!     ],
//! );
//!
//! // A modification names both sides, so either can be fetched.
//! let modified = diff.modified().next().expect("one modification");
//! assert!(modified.before().is_some() && modified.after().is_some());
//! assert_ne!(modified.before(), modified.after());
//! assert_eq!(diff.identity_collisions(), 0);
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;

use crate::artifact_path::ArtifactHandle;
use crate::overlay::OverlaySnapshot;
use crate::snapshot::{ChangeKind, FileNode, Snapshot, SnapshotNode, WorkspacePath};

/// A derived snapshot was diffed against a snapshot it was not derived from.
///
/// Not the stale-snapshot error: this is the narrower "these two values were never
/// related" refusal. See the module documentation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BaseMismatch {
    /// The base the derived snapshot was actually derived from.
    expected: ArtifactHandle,
    /// The base it was offered.
    found: ArtifactHandle,
}

impl BaseMismatch {
    /// The base the derived snapshot records.
    #[must_use]
    pub const fn expected(&self) -> &ArtifactHandle {
        &self.expected
    }

    /// The base it was offered.
    #[must_use]
    pub const fn found(&self) -> &ArtifactHandle {
        &self.found
    }
}

impl fmt::Display for BaseMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the derived snapshot was derived from `{}`, not from `{}`; every change \
             reported against the wrong base would be meaningless",
            self.expected, self.found
        )
    }
}

impl core::error::Error for BaseMismatch {}

/// One changed file, named on both sides.
///
/// `before` and `after` follow the kind exactly: an addition has only an `after`, a removal
/// only a `before`, and a modification both — and for a modification they are never equal,
/// because equal content is not a change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileChange {
    path: WorkspacePath,
    kind: ChangeKind,
    before: Option<ArtifactHandle>,
    after: Option<ArtifactHandle>,
}

impl FileChange {
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

    /// The file's identity in the left-hand snapshot, if it was there.
    #[must_use]
    pub const fn before(&self) -> Option<&ArtifactHandle> {
        self.before.as_ref()
    }

    /// The file's identity in the right-hand snapshot, if it is there.
    #[must_use]
    pub const fn after(&self) -> Option<&ArtifactHandle> {
        self.after.as_ref()
    }
}

impl fmt::Display for FileChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.path)
    }
}

/// Which files differ between two snapshots, with the identities on both sides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceDiff {
    changes: Vec<FileChange>,
    identity_collisions: usize,
}

impl WorkspaceDiff {
    /// Diff two snapshots.
    ///
    /// Path order, and a function of the two snapshots alone.
    #[must_use]
    pub fn between(before: &Snapshot, after: &Snapshot) -> Self {
        let identities = before.diff(after);
        let changes = identities
            .changes()
            .map(|change| {
                let path = change.path();
                FileChange {
                    path: path.clone(),
                    kind: change.kind(),
                    before: match change.kind() {
                        ChangeKind::Added => None,
                        ChangeKind::Removed | ChangeKind::Modified => file_identity(before, path),
                    },
                    after: match change.kind() {
                        ChangeKind::Removed => None,
                        ChangeKind::Added | ChangeKind::Modified => file_identity(after, path),
                    },
                }
            })
            .collect();
        Self {
            changes,
            identity_collisions: identities.identity_collisions(),
        }
    }

    /// Diff a derived snapshot against the base it was derived from.
    ///
    /// # Errors
    ///
    /// [`BaseMismatch`] when `base` is not the snapshot `derived` records as its base.
    pub fn of_overlay(base: &Snapshot, derived: &OverlaySnapshot) -> Result<Self, BaseMismatch> {
        if derived.base() != base.identity() {
            return Err(BaseMismatch {
                expected: derived.base().clone(),
                found: base.identity().clone(),
            });
        }
        Ok(Self::between(base, derived.snapshot()))
    }

    /// Every change, in path order.
    pub fn changes(&self) -> impl Iterator<Item = &FileChange> {
        self.changes.iter()
    }

    /// Every added file, in path order.
    pub fn added(&self) -> impl Iterator<Item = &FileChange> {
        self.of_kind(ChangeKind::Added)
    }

    /// Every removed file, in path order.
    pub fn removed(&self) -> impl Iterator<Item = &FileChange> {
        self.of_kind(ChangeKind::Removed)
    }

    /// Every modified file, in path order.
    pub fn modified(&self) -> impl Iterator<Item = &FileChange> {
        self.of_kind(ChangeKind::Modified)
    }

    /// Every change of one kind, in path order.
    fn of_kind(&self, kind: ChangeKind) -> impl Iterator<Item = &FileChange> {
        self.changes
            .iter()
            .filter(move |change| change.kind == kind)
    }

    /// How many files changed.
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
    /// Carried from [`SnapshotDiff::identity_collisions`](crate::snapshot::SnapshotDiff::identity_collisions)
    /// unchanged. See the module documentation.
    #[must_use]
    pub const fn identity_collisions(&self) -> usize {
        self.identity_collisions
    }
}

/// The identity of the file at `path`, or [`None`] where the snapshot has no file there.
///
/// [`Snapshot::diff`] reports only file paths, and reports each one on the side it exists
/// on, so this is `Some` for every lookup this module makes. It is an [`Option`] anyway,
/// because a component in the trust base does not abort on its own invariant.
fn file_identity(snapshot: &Snapshot, path: &WorkspacePath) -> Option<ArtifactHandle> {
    snapshot
        .node(path)
        .and_then(SnapshotNode::as_file)
        .map(|file| FileNode::identity(file).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_path::ArtifactClass;
    use crate::overlay::Overlay;
    use crate::publication::{ContentIdentifier, IdentityUnavailable};
    use crate::snapshot::{WorkspaceContent, WorkspacePath};

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

    /// Names every record `ws_same`. The worst identity function that exists, and the only
    /// way to observe collision handling as a fact rather than a probability.
    struct ConstantIdentity;

    impl ContentIdentifier for ConstantIdentity {
        fn identify(
            &self,
            class: ArtifactClass,
            _content: &[u8],
        ) -> Result<ArtifactHandle, IdentityUnavailable> {
            ArtifactHandle::new(class, "same").map_err(|_| IdentityUnavailable)
        }
    }

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("a test path is valid")
    }

    fn snapshot<I: ContentIdentifier>(files: &[(&str, &[u8])], identifier: &I) -> Snapshot {
        let mut content = WorkspaceContent::new();
        for (text, bytes) in files {
            content
                .insert(path(text), (*bytes).to_vec())
                .expect("insert");
        }
        Snapshot::build(&content, identifier).expect("build")
    }

    #[test]
    fn each_kind_names_the_sides_it_has() {
        let before = snapshot(
            &[("a.rs", b"a"), ("gone.rs", b"g"), ("same.rs", b"s")],
            &HexIdentity,
        );
        let after = snapshot(
            &[("a.rs", b"a2"), ("new.rs", b"n"), ("same.rs", b"s")],
            &HexIdentity,
        );
        let diff = WorkspaceDiff::between(&before, &after);

        assert_eq!(diff.len(), 3);
        assert!(!diff.is_empty());

        let modified = diff.modified().next().expect("a.rs");
        assert_eq!(modified.path(), &path("a.rs"));
        assert_eq!(
            modified.before(),
            before.node(&path("a.rs")).map(SnapshotNode::identity)
        );
        assert_eq!(
            modified.after(),
            after.node(&path("a.rs")).map(SnapshotNode::identity)
        );
        assert_ne!(modified.before(), modified.after());

        let removed = diff.removed().next().expect("gone.rs");
        assert_eq!(removed.path(), &path("gone.rs"));
        assert!(removed.after().is_none());
        assert_eq!(
            removed.before(),
            before.node(&path("gone.rs")).map(SnapshotNode::identity)
        );

        let added = diff.added().next().expect("new.rs");
        assert_eq!(added.path(), &path("new.rs"));
        assert!(added.before().is_none());
        assert_eq!(
            added.after(),
            after.node(&path("new.rs")).map(SnapshotNode::identity)
        );

        // An unchanged file is not a change.
        assert!(
            diff.changes()
                .all(|change| change.path() != &path("same.rs"))
        );
    }

    #[test]
    fn an_unchanged_pair_has_no_changes() {
        let one = snapshot(&[("a.rs", b"a")], &HexIdentity);
        let two = snapshot(&[("a.rs", b"a")], &HexIdentity);
        let diff = WorkspaceDiff::between(&one, &two);
        assert!(diff.is_empty());
        assert_eq!(diff.identity_collisions(), 0);
        assert_eq!(diff, WorkspaceDiff::default());
    }

    #[test]
    fn a_move_is_a_removal_and_an_addition() {
        let before = snapshot(&[("old/a.rs", b"a")], &HexIdentity);
        let after = snapshot(&[("new/a.rs", b"a")], &HexIdentity);
        let diff = WorkspaceDiff::between(&before, &after);

        assert_eq!(diff.removed().count(), 1);
        assert_eq!(diff.added().count(), 1);
        assert_eq!(diff.modified().count(), 0);
        // The bytes did not change, so the two sides carry one identity under a different
        // name — the truthful identity-level reading of a move.
        assert_eq!(
            diff.removed().next().and_then(FileChange::before),
            diff.added().next().and_then(FileChange::after),
        );
    }

    #[test]
    fn a_totally_colliding_seam_is_reported_and_never_absorbed() {
        let before = snapshot(&[("a.rs", b"a")], &ConstantIdentity);
        let after = snapshot(&[("a.rs", b"b")], &ConstantIdentity);
        let diff = WorkspaceDiff::between(&before, &after);

        assert_eq!(diff.len(), 1);
        assert_eq!(diff.modified().count(), 1);
        assert!(diff.identity_collisions() > 0);
        // Under this seam every identity is equal, and the change is still reported,
        // because exact comparison decided it.
        assert_eq!(
            diff.modified().next().and_then(FileChange::before),
            diff.modified().next().and_then(FileChange::after),
        );
    }

    #[test]
    fn an_overlay_diff_checks_its_base() {
        let base = snapshot(&[("a.rs", b"a")], &HexIdentity);
        let other = snapshot(&[("b.rs", b"b")], &HexIdentity);

        let mut overlay = Overlay::new();
        overlay.write(path("a.rs"), b"a2".to_vec()).expect("write");
        let derived = overlay.derive(&base, &HexIdentity).expect("derive");

        assert_eq!(
            WorkspaceDiff::of_overlay(&base, &derived)
                .expect("diff")
                .len(),
            1
        );
        assert_eq!(
            WorkspaceDiff::of_overlay(&other, &derived),
            Err(BaseMismatch {
                expected: base.identity().clone(),
                found: other.identity().clone(),
            }),
        );
    }
}
