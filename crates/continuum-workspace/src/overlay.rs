//! Editor overlay: unsaved buffers layered over a snapshot (plan §4.2, PR 3 / IMPL-02).
//!
//! # What this module decides
//!
//! > Clients may create a snapshot from a working tree, overlay an in-memory editor
//! > buffer, or fork an existing snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2
//!
//! > An editor may create many cheap overlay snapshots.
//! >
//! > — `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md`, "Snapshot transaction"
//!
//! An editor holds buffers that are not on disk. Analysis of those buffers has to name a
//! snapshot like every other analysis (plan §4.2: "a tool call never means 'whatever is
//! currently on disk'"), so the buffers have to *become* a snapshot — without the base
//! snapshot changing, because artifacts are immutable and an analysis already running
//! against the base must keep meaning what it meant (ADR-0018).
//!
//! [`Overlay`] is the buffers as a value and [`Overlay::derive`] is the derivation. It
//! takes `&Snapshot` and returns a new one: there is no interior mutability, no cell, and
//! no `&mut` anywhere in this module, so the base's immutability is a property of the
//! signatures rather than a promise in prose.
//!
//! # An overlay is a set, not a script
//!
//! [`Overlay::write`] and [`Overlay::remove`] refuse to touch a path twice
//! ([`OverlayError::DuplicateWrite`], [`OverlayError::DuplicateRemoval`]) and refuse to
//! both write and remove one ([`OverlayError::WrittenAndRemoved`]). An overlay is therefore
//! a function of the *set* of calls that built it and never of their order — the same
//! discipline [`WorkspaceContent::insert`] applies for the same reason. An editor has one
//! buffer per path; presenting two is a bug in the editor, and last-write-wins would hide
//! it behind an order-dependent result.
//!
//! # Shadowing the base is the point; removing what is not there is not
//!
//! Writing a path the base already holds is an ordinary modification: that is what an
//! overlay *is*, and it is not a duplicate. Removing a path the base does not hold is
//! [`OverlayError::RemovedPathAbsent`]: the editor believes it is closing a file the base
//! never had, which is a disagreement about what the base contains, and this crate reports
//! disagreements rather than absorbing them.
//!
//! # Provenance: the schema's `files[].overlay`
//!
//! [`OverlaySnapshot`] carries the base's identity and the set of paths the overlay
//! supplied, so [`OverlaySnapshot::is_overlaid`] answers
//! `notes/plan/schemas/workspace-snapshot.schema.json`'s `files[].overlay` for every file
//! in the derived tree. The flag is *provenance*, never identity: it takes no part in any
//! record, so a file written by an overlay and the same bytes imported from disk have one
//! identity — which is what makes an overlay snapshot a first-class snapshot rather than a
//! second kind of thing.
//!
//! # Cheap
//!
//! Deriving re-derives the records on the path from each changed file to the root and
//! reuses every other subtree's identity unchanged
//! ([`Snapshot::subtrees`](crate::snapshot::Snapshot::subtrees)). Publishing a derived
//! snapshot therefore publishes only what changed, because the store converges on
//! identities it already holds — "many cheap overlay snapshots", with no cache and no
//! second notion of what is already stored.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::overlay::Overlay;
//! use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
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
//! content.insert(WorkspacePath::new("README.md")?, b"hello".to_vec())?;
//! let base = Snapshot::build(&content, &HexIdentity)?;
//! let before = base.identity().clone();
//!
//! let mut overlay = Overlay::new();
//! overlay.write(WorkspacePath::new("src/lib.rs")?, b"fn main() { todo!() }".to_vec())?;
//! let derived = overlay.derive(&base, &HexIdentity)?;
//!
//! assert_ne!(derived.snapshot().identity(), base.identity());
//! assert!(derived.is_overlaid(&WorkspacePath::new("src/lib.rs")?));
//! assert!(!derived.is_overlaid(&WorkspacePath::new("README.md")?));
//!
//! // The base is untouched: same identity, same content, still usable.
//! assert_eq!(base.identity(), &before);
//! assert_eq!(base.file_count(), 2);
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::artifact_path::ArtifactHandle;
use crate::publication::ContentIdentifier;
use crate::snapshot::{Snapshot, SnapshotError, WorkspaceContent, WorkspacePath};

/// Why an overlay is not applicable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayError {
    /// The overlay already holds a buffer for this path.
    DuplicateWrite {
        /// The repeated path.
        path: WorkspacePath,
    },
    /// The overlay already removes this path.
    DuplicateRemoval {
        /// The repeated path.
        path: WorkspacePath,
    },
    /// The overlay both writes and removes this path.
    WrittenAndRemoved {
        /// The contested path.
        path: WorkspacePath,
    },
    /// The overlay removes a path the base does not hold as a file.
    RemovedPathAbsent {
        /// The path that is not there.
        path: WorkspacePath,
    },
    /// Base and overlay together are not a tree: a written path is a file another path
    /// needs as a directory, or the reverse.
    NotATree(SnapshotError),
    /// The derived description is not a snapshot: see [`SnapshotError`].
    Snapshot(SnapshotError),
}

impl fmt::Display for OverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateWrite { path } => write!(
                f,
                "the overlay already holds a buffer for `{path}`; an overlay is a set of \
                 buffers, and one path has one buffer"
            ),
            Self::DuplicateRemoval { path } => {
                write!(f, "the overlay already removes `{path}`")
            }
            Self::WrittenAndRemoved { path } => write!(
                f,
                "the overlay both writes and removes `{path}`; the derived snapshot would \
                 depend on which was applied last"
            ),
            Self::RemovedPathAbsent { path } => write!(
                f,
                "the overlay removes `{path}`, which the base snapshot does not hold; the \
                 overlay and the base disagree about what the workspace contains"
            ),
            Self::NotATree(error) | Self::Snapshot(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for OverlayError {}

/// Unsaved editor state as a value: buffers to layer over a base, and paths to drop.
///
/// Built independently of any base — an editor's buffers are not *about* a snapshot until
/// they are applied to one — and applied by [`Overlay::derive`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overlay {
    writes: BTreeMap<WorkspacePath, Vec<u8>>,
    removals: BTreeSet<WorkspacePath>,
}

impl Overlay {
    /// An overlay that changes nothing. Deriving with it reproduces the base exactly,
    /// identity included.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Layer one buffer at `path`.
    ///
    /// # Errors
    ///
    /// [`OverlayError::DuplicateWrite`] when the overlay already holds a buffer for
    /// `path`, and [`OverlayError::WrittenAndRemoved`] when it already removes it.
    pub fn write(&mut self, path: WorkspacePath, buffer: Vec<u8>) -> Result<(), OverlayError> {
        if self.removals.contains(&path) {
            return Err(OverlayError::WrittenAndRemoved { path });
        }
        if self.writes.contains_key(&path) {
            return Err(OverlayError::DuplicateWrite { path });
        }
        self.writes.insert(path, buffer);
        Ok(())
    }

    /// Drop `path` from the derived snapshot.
    ///
    /// # Errors
    ///
    /// [`OverlayError::DuplicateRemoval`] when the overlay already removes `path`, and
    /// [`OverlayError::WrittenAndRemoved`] when it already holds a buffer for it.
    pub fn remove(&mut self, path: WorkspacePath) -> Result<(), OverlayError> {
        if self.writes.contains_key(&path) {
            return Err(OverlayError::WrittenAndRemoved { path });
        }
        if self.removals.contains(&path) {
            return Err(OverlayError::DuplicateRemoval { path });
        }
        self.removals.insert(path);
        Ok(())
    }

    /// Every `(path, buffer)` pair, in path order.
    pub fn writes(&self) -> impl Iterator<Item = (&WorkspacePath, &[u8])> {
        self.writes
            .iter()
            .map(|(path, buffer)| (path, buffer.as_slice()))
    }

    /// Every removed path, in path order.
    pub fn removals(&self) -> impl Iterator<Item = &WorkspacePath> {
        self.removals.iter()
    }

    /// How many paths the overlay touches.
    #[must_use]
    pub fn len(&self) -> usize {
        self.writes.len().saturating_add(self.removals.len())
    }

    /// Whether the overlay touches nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.removals.is_empty()
    }

    /// Derive the snapshot `base` would be if these buffers were saved.
    ///
    /// Purely functional: `base` is borrowed, nothing in it is written, and the result is a
    /// function of `base`, `self`, and `identifier`'s behavior alone.
    ///
    /// # Errors
    ///
    /// [`OverlayError::RemovedPathAbsent`] when a removal names a path the base does not
    /// hold, [`OverlayError::NotATree`] when a written path collides with a directory (or
    /// the reverse), and [`OverlayError::Snapshot`] when the identity seam refuses to name
    /// a node.
    pub fn derive<I: ContentIdentifier + ?Sized>(
        &self,
        base: &Snapshot,
        identifier: &I,
    ) -> Result<OverlaySnapshot, OverlayError> {
        let mut files: BTreeMap<WorkspacePath, Vec<u8>> = base
            .files()
            .into_iter()
            .map(|(path, node)| (path, node.content().to_vec()))
            .collect();

        for path in &self.removals {
            if files.remove(path).is_none() {
                return Err(OverlayError::RemovedPathAbsent { path: path.clone() });
            }
        }
        for (path, buffer) in &self.writes {
            // Shadowing a base file is the overlay's whole purpose, so this insert
            // replaces rather than refuses — and it is order-independent, because the
            // overlay holds one buffer per path and the base holds one file per path.
            files.insert(path.clone(), buffer.clone());
        }

        let mut content = WorkspaceContent::new();
        for (path, bytes) in files {
            content
                .insert(path, bytes)
                .map_err(OverlayError::NotATree)?;
        }

        let snapshot = Snapshot::build(&content, identifier).map_err(OverlayError::Snapshot)?;
        Ok(OverlaySnapshot {
            snapshot,
            base: base.identity().clone(),
            overlaid: self.writes.keys().cloned().collect(),
            removed: self.removals.clone(),
        })
    }
}

/// A snapshot derived from a base by an overlay, with the provenance that derivation had.
///
/// The snapshot inside is an ordinary [`Snapshot`] — same type, same identity discipline,
/// publishable through the same records. What this wrapper adds is the answer to "where
/// did this file come from", which the base snapshot cannot be asked and the derived one
/// does not carry, because provenance is not content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySnapshot {
    snapshot: Snapshot,
    base: ArtifactHandle,
    overlaid: BTreeSet<WorkspacePath>,
    removed: BTreeSet<WorkspacePath>,
}

impl OverlaySnapshot {
    /// The derived snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// The derived snapshot, taken by value.
    #[must_use]
    pub fn into_snapshot(self) -> Snapshot {
        self.snapshot
    }

    /// The identity of the snapshot this was derived from.
    ///
    /// The schema's `parent` (`notes/plan/schemas/workspace-snapshot.schema.json`) for an
    /// overlay derivation.
    #[must_use]
    pub const fn base(&self) -> &ArtifactHandle {
        &self.base
    }

    /// Whether the overlay supplied this path's content.
    ///
    /// The schema's `files[].overlay`. True for every path the overlay wrote, *including*
    /// one whose buffer happened to equal the base's bytes: the flag records where the
    /// content came from, not whether it differs. What differs is
    /// [`WorkspaceDiff`](crate::diff::WorkspaceDiff)'s question.
    #[must_use]
    pub fn is_overlaid(&self, path: &WorkspacePath) -> bool {
        self.overlaid.contains(path)
    }

    /// Every path the overlay supplied, in path order.
    pub fn overlaid_paths(&self) -> impl Iterator<Item = &WorkspacePath> {
        self.overlaid.iter()
    }

    /// Every path the overlay dropped, in path order. None of them is in the derived
    /// snapshot; they are named because provenance includes what was taken away.
    pub fn removed_paths(&self) -> impl Iterator<Item = &WorkspacePath> {
        self.removed.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_path::{ArtifactClass, ArtifactHandle};
    use crate::publication::IdentityUnavailable;

    /// The record's own bytes in hex: injective, so every claim below is a fact rather
    /// than a probability (ADR-0013's certified discipline).
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

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("a test path is valid")
    }

    fn base() -> Snapshot {
        let mut content = WorkspaceContent::new();
        content
            .insert(path("src/lib.rs"), b"fn main() {}".to_vec())
            .expect("insert");
        content
            .insert(path("README.md"), b"hello".to_vec())
            .expect("insert");
        Snapshot::build(&content, &HexIdentity).expect("build")
    }

    #[test]
    fn an_empty_overlay_reproduces_the_base_exactly() {
        let base = base();
        let derived = Overlay::new().derive(&base, &HexIdentity).expect("derive");
        assert_eq!(derived.snapshot().identity(), base.identity());
        assert_eq!(derived.snapshot(), &base);
        assert!(Overlay::new().is_empty());
    }

    #[test]
    fn an_overlay_is_a_function_of_its_calls_as_a_set() {
        let mut forward = Overlay::new();
        forward.write(path("a.rs"), b"a".to_vec()).expect("write");
        forward.write(path("b.rs"), b"b".to_vec()).expect("write");
        forward.remove(path("c.rs")).expect("remove");

        let mut backward = Overlay::new();
        backward.remove(path("c.rs")).expect("remove");
        backward.write(path("b.rs"), b"b".to_vec()).expect("write");
        backward.write(path("a.rs"), b"a".to_vec()).expect("write");

        assert_eq!(forward, backward);
        assert_eq!(forward.len(), 3);
    }

    #[test]
    fn one_path_has_one_buffer() {
        let mut overlay = Overlay::new();
        overlay.write(path("a.rs"), b"a".to_vec()).expect("write");
        assert_eq!(
            overlay.write(path("a.rs"), b"a2".to_vec()),
            Err(OverlayError::DuplicateWrite { path: path("a.rs") }),
        );
        assert_eq!(
            overlay.remove(path("a.rs")),
            Err(OverlayError::WrittenAndRemoved { path: path("a.rs") }),
        );

        let mut other = Overlay::new();
        other.remove(path("b.rs")).expect("remove");
        assert_eq!(
            other.remove(path("b.rs")),
            Err(OverlayError::DuplicateRemoval { path: path("b.rs") }),
        );
        assert_eq!(
            other.write(path("b.rs"), b"b".to_vec()),
            Err(OverlayError::WrittenAndRemoved { path: path("b.rs") }),
        );
    }

    #[test]
    fn removing_what_the_base_does_not_hold_is_refused() {
        let mut overlay = Overlay::new();
        overlay.remove(path("nowhere.rs")).expect("remove");
        assert_eq!(
            overlay.derive(&base(), &HexIdentity),
            Err(OverlayError::RemovedPathAbsent {
                path: path("nowhere.rs")
            }),
        );
    }

    #[test]
    fn a_buffer_that_collides_with_a_directory_is_refused() {
        let mut overlay = Overlay::new();
        overlay
            .write(path("src"), b"not a directory".to_vec())
            .expect("write");
        assert!(matches!(
            overlay.derive(&base(), &HexIdentity),
            Err(OverlayError::NotATree(SnapshotError::PathConflict { .. })),
        ));
    }

    #[test]
    fn a_removal_and_a_write_compose() {
        let base = base();
        let mut overlay = Overlay::new();
        overlay.remove(path("README.md")).expect("remove");
        overlay
            .write(path("src/main.rs"), b"fn main() {}".to_vec())
            .expect("write");
        let derived = overlay.derive(&base, &HexIdentity).expect("derive");

        let paths: Vec<String> = derived
            .snapshot()
            .files()
            .into_iter()
            .map(|(path, _)| path.to_string())
            .collect();
        assert_eq!(paths, ["src/lib.rs", "src/main.rs"]);
        assert!(derived.is_overlaid(&path("src/main.rs")));
        assert!(!derived.is_overlaid(&path("src/lib.rs")));
        assert_eq!(
            derived.removed_paths().collect::<Vec<_>>(),
            [&path("README.md")]
        );
        assert_eq!(derived.base(), base.identity());
    }

    #[test]
    fn an_overlaid_file_shares_its_identity_with_the_same_bytes_imported() {
        let base = base();
        let mut overlay = Overlay::new();
        overlay
            .write(path("README.md"), b"goodbye".to_vec())
            .expect("write");
        let derived = overlay.derive(&base, &HexIdentity).expect("derive");

        let mut direct = WorkspaceContent::new();
        direct
            .insert(path("src/lib.rs"), b"fn main() {}".to_vec())
            .expect("insert");
        direct
            .insert(path("README.md"), b"goodbye".to_vec())
            .expect("insert");
        let direct = Snapshot::build(&direct, &HexIdentity).expect("build");

        // Provenance is not identity: the overlay flag is nowhere in the record.
        assert_eq!(derived.snapshot().identity(), direct.identity());
    }

    #[test]
    fn an_untouched_subtree_keeps_its_identity() {
        let base = base();
        let mut overlay = Overlay::new();
        overlay
            .write(path("README.md"), b"goodbye".to_vec())
            .expect("write");
        let derived = overlay.derive(&base, &HexIdentity).expect("derive");
        assert_eq!(
            derived
                .snapshot()
                .node(&path("src"))
                .map(|node| node.identity()),
            base.node(&path("src")).map(|node| node.identity()),
        );
    }
}
