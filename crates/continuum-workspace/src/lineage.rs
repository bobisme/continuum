//! Fork: a named divergence point, and the line that runs from it (plan §4.2,
//! PR 3 / IMPL-02).
//!
//! # What this module decides
//!
//! > Clients may create a snapshot from a working tree, overlay an in-memory editor
//! > buffer, or fork an existing snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2
//!
//! > **Exit:** two clients can fork and analyze independently.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3
//!
//! Under an immutable model, *copying* a workspace is not what forking is for — two
//! [`Snapshot`] values built from the same content are already the same artifact, share
//! every subtree identity, and publish to the same records. Independence is free. What is
//! not free, and is the whole deliverable here, is **saying so in the types**: which
//! snapshot a line diverged from, which line a head belongs to, and which head a head
//! superseded.
//!
//! [`Fork`] is that statement. It is a value; it has no owner, no registry, and no
//! lifetime tied to a store. Two [`Fork`]s created from one base cannot interfere because
//! there is nothing shared between them to interfere *through* — which is what makes "two
//! clients fork and analyze independently" a structural fact rather than a locking
//! discipline.
//!
//! # Provenance is three questions, not one
//!
//! - [`Fork::origin`] — the snapshot the line diverged from. Fixed at
//!   [`Fork::diverge`] and never moved: it is what the fork is *of*.
//! - [`Fork::parent`] — the head this head superseded, or [`None`] at the divergence
//!   point. This is `notes/plan/schemas/workspace-snapshot.schema.json`'s `parent`, and it
//!   is ADR-0018's `SUPERSEDES` edge one step at a time.
//! - [`Fork::identity`] — the current head's own name.
//!
//! Advancing a fork produces a *new* [`Fork`] value. Nothing is mutated, so a caller
//! holding the earlier value still holds the earlier line, and an analysis that named the
//! earlier head still names it (plan §4.2's "an old snapshot remains reproducible after
//! the working tree changes", at the lineage level).
//!
//! # A fork's name is a label, never an identity
//!
//! [`ForkName`] appears in no record and in no [`ArtifactHandle`]. Two forks of one
//! snapshot under two names have the *same* head identity, and that is deliberate: naming
//! a line is a human convenience, and letting it reach identity would give one workspace
//! content two artifact names — the many-to-one failure `snapshot.rs`'s byte rule refuses,
//! inverted.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::lineage::{Fork, ForkName};
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
//! let base = Snapshot::build(&content, &HexIdentity)?;
//!
//! let alice = Fork::diverge(ForkName::new("alice")?, &base);
//! let bob = Fork::diverge(ForkName::new("bob")?, &base);
//!
//! // Two names, one divergence point, one head identity: a name is not an identity.
//! assert_eq!(alice.identity(), bob.identity());
//! assert_eq!(alice.origin(), base.identity());
//! assert!(alice.parent().is_none());
//!
//! let mut edit = Overlay::new();
//! edit.write(WorkspacePath::new("src/lib.rs")?, b"fn main() { todo!() }".to_vec())?;
//! let alice = alice.advance(&edit, &HexIdentity)?;
//!
//! assert_ne!(alice.identity(), bob.identity());
//! assert_eq!(alice.origin(), base.identity());
//! assert_eq!(alice.parent(), Some(base.identity()));
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;
use core::str::FromStr;

use crate::artifact_path::ArtifactHandle;
use crate::overlay::{Overlay, OverlayError};
use crate::publication::ContentIdentifier;
use crate::snapshot::Snapshot;

/// The longest fork name, in UTF-8 bytes.
///
/// The same bound `snapshot.rs` puts on a path segment, for a weaker reason: a fork name
/// is never written to a filesystem, so nothing forces this number. It is here so that a
/// name a caller derived from untrusted input cannot become an unbounded allocation
/// carried through every error message.
pub const MAX_FORK_NAME_BYTES: usize = 255;

/// Why a string is not a fork name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InvalidForkName {
    /// The name was empty. An unnamed divergence point is not a named one.
    Empty,
    /// The name held a control character.
    ///
    /// Fork names are printed — in errors, in logs, in whatever surface shows a lineage —
    /// and a control character in a printed label is a terminal-escape hazard, not a name.
    /// The rejected set is Unicode's fixed `Cc` block, so it never grows.
    ControlCharacter {
        /// The first offending character.
        character: char,
    },
    /// The name is longer than [`MAX_FORK_NAME_BYTES`].
    TooLong {
        /// The name's length in UTF-8 bytes.
        bytes: usize,
        /// The bound, [`MAX_FORK_NAME_BYTES`].
        max: usize,
    },
}

impl fmt::Display for InvalidForkName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a fork name has at least one character"),
            Self::ControlCharacter { character } => {
                write!(
                    f,
                    "a fork name may not hold the control character {character:?}"
                )
            }
            Self::TooLong { bytes, max } => {
                write!(f, "a fork name is {bytes} bytes, past the {max}-byte bound")
            }
        }
    }
}

impl core::error::Error for InvalidForkName {}

/// A label for one line of workspace evolution.
///
/// Validated at construction so that a [`Fork`] cannot carry a name no surface can print.
/// It takes no part in any identity: see the module documentation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForkName(String);

impl ForkName {
    /// Validate a fork name.
    ///
    /// # Errors
    ///
    /// [`InvalidForkName`]: empty, holding a control character, or past
    /// [`MAX_FORK_NAME_BYTES`].
    pub fn new(text: &str) -> Result<Self, InvalidForkName> {
        if text.is_empty() {
            return Err(InvalidForkName::Empty);
        }
        if let Some(character) = text.chars().find(|character| character.is_control()) {
            return Err(InvalidForkName::ControlCharacter { character });
        }
        if text.len() > MAX_FORK_NAME_BYTES {
            return Err(InvalidForkName::TooLong {
                bytes: text.len(),
                max: MAX_FORK_NAME_BYTES,
            });
        }
        Ok(Self(text.to_owned()))
    }

    /// The name, as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ForkName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ForkName {
    type Err = InvalidForkName;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

/// One named line of workspace evolution: where it diverged, what it superseded, and where
/// it is now.
///
/// Immutable. [`Fork::advance`] and [`Fork::advance_to`] return a new value rather than
/// changing this one, so every earlier state of the line stays exactly as reachable as it
/// was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fork {
    name: ForkName,
    origin: ArtifactHandle,
    parent: Option<ArtifactHandle>,
    head: Snapshot,
}

impl Fork {
    /// Open a line at `base`.
    ///
    /// Cheap in the sense that matters: the head *is* `base` — same content, same identity,
    /// same subtree identities, so publishing this fork's head after publishing `base`
    /// writes nothing new. The [`Snapshot`] value is cloned because a [`Fork`] is an owned
    /// value; the cloned tree names exactly the artifacts the original does.
    #[must_use]
    pub fn diverge(name: ForkName, base: &Snapshot) -> Self {
        Self {
            name,
            origin: base.identity().clone(),
            parent: None,
            head: base.clone(),
        }
    }

    /// The line's label.
    #[must_use]
    pub const fn name(&self) -> &ForkName {
        &self.name
    }

    /// The snapshot this line diverged from. Fixed forever at [`Fork::diverge`].
    #[must_use]
    pub const fn origin(&self) -> &ArtifactHandle {
        &self.origin
    }

    /// The head this head superseded, or [`None`] at the divergence point.
    ///
    /// The schema's `parent`.
    #[must_use]
    pub const fn parent(&self) -> Option<&ArtifactHandle> {
        self.parent.as_ref()
    }

    /// The current head.
    #[must_use]
    pub const fn head(&self) -> &Snapshot {
        &self.head
    }

    /// The current head's identity.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        self.head.identity()
    }

    /// The head, taken by value.
    #[must_use]
    pub fn into_head(self) -> Snapshot {
        self.head
    }

    /// The same line, one step on, at `head`.
    ///
    /// The typed lineage step: the name and the origin carry over, and the head being
    /// replaced becomes the new head's [`parent`](Self::parent). Advancing to a snapshot
    /// equal to the current head is legal and is not a no-op in provenance — the line
    /// records that a step was taken, and the two heads share one identity because they
    /// share their content.
    #[must_use]
    pub fn advance_to(&self, head: Snapshot) -> Self {
        Self {
            name: self.name.clone(),
            origin: self.origin.clone(),
            parent: Some(self.head.identity().clone()),
            head,
        }
    }

    /// The same line, one step on, with `overlay` applied to the current head.
    ///
    /// The two-call spelling — [`Overlay::derive`] then [`Fork::advance_to`] — is what a
    /// caller uses when it also wants the derivation's provenance
    /// ([`OverlaySnapshot`](crate::overlay::OverlaySnapshot)'s overlay flags); this is the
    /// same thing with that provenance dropped.
    ///
    /// # Errors
    ///
    /// [`OverlayError`], exactly as [`Overlay::derive`] reports it.
    pub fn advance<I: ContentIdentifier + ?Sized>(
        &self,
        overlay: &Overlay,
        identifier: &I,
    ) -> Result<Self, OverlayError> {
        let derived = overlay.derive(&self.head, identifier)?;
        Ok(self.advance_to(derived.into_snapshot()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_path::ArtifactClass;
    use crate::publication::IdentityUnavailable;
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

    fn path(text: &str) -> WorkspacePath {
        WorkspacePath::new(text).expect("a test path is valid")
    }

    fn base() -> Snapshot {
        let mut content = WorkspaceContent::new();
        content
            .insert(path("src/lib.rs"), b"fn main() {}".to_vec())
            .expect("insert");
        Snapshot::build(&content, &HexIdentity).expect("build")
    }

    fn name(text: &str) -> ForkName {
        ForkName::new(text).expect("a test name is valid")
    }

    #[test]
    fn a_fork_name_is_validated() {
        assert_eq!(ForkName::new(""), Err(InvalidForkName::Empty));
        assert_eq!(
            ForkName::new("a\u{7}b"),
            Err(InvalidForkName::ControlCharacter { character: '\u{7}' }),
        );
        let long = "x".repeat(MAX_FORK_NAME_BYTES + 1);
        assert_eq!(
            ForkName::new(&long),
            Err(InvalidForkName::TooLong {
                bytes: MAX_FORK_NAME_BYTES + 1,
                max: MAX_FORK_NAME_BYTES,
            }),
        );
        assert_eq!(name("feature/x").as_str(), "feature/x");
        assert_eq!("feature/x".parse::<ForkName>(), Ok(name("feature/x")));
    }

    #[test]
    fn diverging_records_the_origin_and_no_parent() {
        let base = base();
        let fork = Fork::diverge(name("alice"), &base);
        assert_eq!(fork.origin(), base.identity());
        assert_eq!(fork.identity(), base.identity());
        assert_eq!(fork.parent(), None);
        assert_eq!(fork.head(), &base);
        assert_eq!(fork.name(), &name("alice"));
    }

    #[test]
    fn advancing_leaves_the_earlier_value_intact() {
        let base = base();
        let fork = Fork::diverge(name("alice"), &base);

        let mut overlay = Overlay::new();
        overlay
            .write(path("src/lib.rs"), b"fn main() { todo!() }".to_vec())
            .expect("write");
        let advanced = fork.advance(&overlay, &HexIdentity).expect("advance");

        assert_eq!(advanced.parent(), Some(base.identity()));
        assert_eq!(advanced.origin(), base.identity());
        assert_ne!(advanced.identity(), base.identity());

        // The earlier value is untouched, and so is the base it was taken from.
        assert_eq!(fork.identity(), base.identity());
        assert_eq!(fork.parent(), None);
        assert_eq!(base.file_count(), 1);
    }

    #[test]
    fn two_lines_from_one_base_evolve_independently() {
        let base = base();
        let alice = Fork::diverge(name("alice"), &base);
        let bob = Fork::diverge(name("bob"), &base);
        assert_eq!(alice.identity(), bob.identity());

        let mut one = Overlay::new();
        one.write(path("src/lib.rs"), b"alice".to_vec())
            .expect("write");
        let mut two = Overlay::new();
        two.write(path("src/lib.rs"), b"bob".to_vec())
            .expect("write");

        let alice = alice.advance(&one, &HexIdentity).expect("advance");
        let bob = bob.advance(&two, &HexIdentity).expect("advance");

        assert_ne!(alice.identity(), bob.identity());
        assert_eq!(alice.origin(), bob.origin());
        assert_eq!(alice.parent(), bob.parent());
        assert_eq!(base.identity(), alice.origin());
    }

    #[test]
    fn a_name_is_not_an_identity() {
        let base = base();
        let one = Fork::diverge(name("alice"), &base);
        let two = Fork::diverge(name("bob"), &base);
        assert_eq!(one.identity(), two.identity());
        assert_ne!(one.name(), two.name());
    }

    #[test]
    fn advancing_to_an_equal_head_still_records_the_step() {
        let base = base();
        let fork = Fork::diverge(name("alice"), &base);
        let stepped = fork.advance_to(base.clone());
        assert_eq!(stepped.identity(), base.identity());
        assert_eq!(stepped.parent(), Some(base.identity()));
    }
}
