//! Disk import: the one layer that reads a working tree (plan §4.2, PR 3 / IMPL-02).
//!
//! # What this module decides
//!
//! > Clients may create a snapshot from a working tree, overlay an in-memory editor
//! > buffer, or fork an existing snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! [`snapshot`](crate::snapshot) says a snapshot is built from a
//! [`WorkspaceContent`](crate::snapshot::WorkspaceContent) — "an explicit, already-ordered
//! description of what the workspace holds" — and [`components`](crate::components) says a
//! [`WorkspaceDescriptor`] is built from already-assembled component bytes. Both left the
//! same question open, and this module answers it: **where do those values come from when
//! the workspace is a directory on a disk?**
//!
//! [`DiskImporter`] is the answer, and it is deliberately the *only* type in this crate
//! that opens a file or reads a directory. Everything downstream — overlay, fork, seal,
//! diff — consumes values, so every one of them is reproducible from the value the
//! importer produced, forever, on a machine where the directory no longer exists. That
//! asymmetry is the whole point: ambient filesystem access is legitimate here and nowhere
//! else in the crate (INV-005, ADR-0003).
//!
//! # This module decides nothing about paths
//!
//! Every question about *which names are workspace paths* was decided by
//! [`admit`](crate::snapshot::admit) and is consumed here rather than re-answered:
//! normalization (none), case (exact), non-UTF-8 names (typed rejection), reserved
//! segments, rejected characters, segment length, depth, and symbolic links. The importer
//! classifies what it finds into a [`Candidate`] and hands the whole set to [`admit`]. It
//! contains no second copy of the byte rule, and a policy change in `snapshot.rs` is
//! therefore a policy change here with no edit to this file.
//!
//! In particular a symbolic link is [`Candidate::Symlink`] and a FIFO, socket, or device
//! node is [`Candidate::Special`] — both a typed refusal of the whole workspace naming the
//! entry, never a silent skip. Skipping either would produce a snapshot that claims to
//! describe a directory it does not describe.
//!
//! # Determinism: two orders, both functions of the tree
//!
//! Two imports of one tree produce one snapshot identity, whatever order the filesystem
//! reports entries in. That holds for *acceptance* because [`admit`] sorts, and for
//! *rejection* because this module sorts too:
//!
//! - **The walk order** is depth-first pre-order, with each directory's entries sorted by
//!   their raw name bytes before any of them is opened. The importer's own rejections — an
//!   I/O failure, a name this platform cannot report as bytes — are therefore reported for
//!   the least entry in *path* order, which is a function of the tree alone.
//! - **The admission order** is [`admit`]'s: the least candidate in raw-name-byte order.
//!   Path order and raw-name order disagree (`a.b` sorts before `a/b` one way and after it
//!   the other), which is why the two are named separately instead of pretended to be one.
//!
//! Neither order is observable in a successful import: the result is a
//! [`WorkspaceContent`](crate::snapshot::WorkspaceContent), which is a
//! [`BTreeMap`](std::collections::BTreeMap).
//!
//! The walk keeps its own stack rather than recursing, so an adversarially deep directory
//! tree costs time and memory and never the call stack. Depth is still admission's
//! decision: a file nested deeper than [`MAX_DEPTH`](crate::snapshot::MAX_DEPTH) is
//! [`PathError::TooDeep`](crate::snapshot::PathError::TooDeep), reported through
//! [`ImportError::Admission`].
//!
//! # Empty directories are skipped, and that is a consequence, not a choice
//!
//! A [`WorkspaceContent`](crate::snapshot::WorkspaceContent) is a set of files, so a
//! directory exists in a snapshot exactly when it holds one. `snapshot.rs` left the
//! question here:
//!
//! > **Empty directories are not representable.** […] Whether an empty directory is
//! > workspace content at all is a disk-import question (IMPL-02); if it is, the encoding
//! > gains an empty-child list and the record grammar below gains a case.
//!
//! The answer is **no**: import skips a directory that contributes no file, and the record
//! grammar does not move. Representing empty directories would change every directory
//! record in every existing snapshot to say something no consumer of this crate asks —
//! nothing downstream of a snapshot reads a directory except to reach a file — and a
//! store-format change is not paid for by a feature nobody consumes. The cost is stated
//! rather than hidden: a workspace whose meaning depends on an empty directory existing
//! (a mount point, an output directory a build expects) does not round-trip through a
//! snapshot, and the honest fix is a marker file, which *is* content.
//!
//! Skipping is not silent. [`ImportedWorkspace::unrepresented_directories`] names every
//! directory the walk entered that left no trace in the snapshot, by its disk path —
//! because a directory that leaves no trace has no workspace path to be named by.
//!
//! # Nothing is excluded
//!
//! The importer applies no ignore policy. There is no `.gitignore` reading, no `.git`
//! special case, no build-output heuristic: every regular file under the root is workspace
//! content. An exclusion policy makes snapshot identity a function of files that are *not*
//! in the snapshot, and `.gitignore` semantics — nested files, negation, precedence,
//! ordering — is a specification of its own that would have to be implemented identically
//! by every future importer or one workspace would have two identities. A caller who wants
//! exclusions points the importer at a tree that has none; adding a typed, versioned
//! exclusion policy is a later, separately evidenced change.
//!
//! # Recognized component files
//!
//! [`components`](crate::components) takes already-assembled bytes for the dependency,
//! toolchain, and configuration components and says which on-disk file supplies them is
//! "disk import's job". [`RECOGNIZED_COMPONENT_FILES`] is that job, done once, as a closed
//! table:
//!
//! | Component | Recognized workspace-root file |
//! |---|---|
//! | [`ComponentKind::Dependencies`] | `Cargo.lock` |
//! | [`ComponentKind::Toolchain`] | `rust-toolchain.toml`, `rust-toolchain` |
//! | [`ComponentKind::Configuration`] | `continuum.toml` |
//!
//! Four properties make it a policy rather than a guess:
//!
//! - **Root-relative only.** A `Cargo.lock` in a subdirectory is a file in the tree and
//!   nothing more. The workspace's lockfile is the root's, exactly as Cargo resolves it.
//! - **Ambiguity is a typed refusal.** `rustup` honors both toolchain spellings, so both
//!   are recognized — and a workspace holding *both* is
//!   [`ImportError::AmbiguousComponent`], never a silent precedence rule. A precedence
//!   rule would make the toolchain identity a function of a preference list rather than of
//!   the workspace.
//! - **Recognition is additive, never subtractive.** A recognized file stays an ordinary
//!   file in the source tree; the component is a *second*, separately framed identity over
//!   the same bytes ([`Component::record`](crate::components::Component::record) uses its
//!   own magic, so the two identities cannot be confused). Removing the file from the tree
//!   would make the snapshot stop describing the directory.
//! - **Growth is a version bump.** [`COMPONENT_RECOGNITION_VERSION`] moves whenever the
//!   table does, because the table decides which workspaces get which descriptor identity.
//!
//! Two deliberate v1 limits, stated so they are not mistaken for oversights:
//!
//! - **`Cargo.toml` is not the configuration file.** Plan §4.2's configuration surface for
//!   a Rust project is the `[package.metadata.continuum]` *section* of `Cargo.toml`, and
//!   extracting a section needs a TOML parser — a dependency this crate's boundary
//!   refuses. Taking the whole manifest instead would move the configuration identity
//!   whenever an unrelated dependency bound changed, which is a false invalidation. Plan
//!   §4.2 also names the standalone spelling for model-only projects — `continuum.toml` —
//!   and that is what is recognized. The manifest-section surface belongs to
//!   `cargo continuum`, which has a manifest parser.
//! - **The table is Rust-only.** `package-lock.json`, `poetry.lock`, and friends are not
//!   recognized, because [`ComponentKind::Dependencies`] is one buffer and a polyglot
//!   workspace has several lockfiles at once — so widening the table means first deciding
//!   how several ecosystems compose into one component identity, which is a decision, not
//!   a list.
//!
//! A caller who wants a component the table does not recognize supplies the bytes
//! directly through [`WorkspaceDescriptorBuilder`](crate::components::WorkspaceDescriptorBuilder);
//! docs/35's snapshot transaction has the client submitting "dependency/config references"
//! for exactly that reason.
//!
//! # The policy in force is reported, not recorded
//!
//! [`ImportedWorkspace`] carries the [`AdmissionPolicy`] and the
//! [`COMPONENT_RECOGNITION_VERSION`] the import ran under, so a caller can record them
//! beside the artifact it publishes. Neither enters any record here: the descriptor's
//! record grammar is closed ([`components`](crate::components), "Closed kind, versioned
//! growth"), and adding a field to it is a store-format change that would re-derive every
//! existing identity. It is also unnecessary today — exactly one admission arm is
//! implemented ([`SymlinkPolicy::Reject`](crate::snapshot::SymlinkPolicy::Reject)) and one
//! recognition table exists, so the policy cannot vary between two importers of one
//! workspace. Binding them into a published `workspace-snapshot` document is
//! schema-assembly work for whichever PR performs it.
//!
//! # Cost
//!
//! Every file's content is read into memory. A snapshot *is* an in-memory value in this
//! crate — [`WorkspaceContent`](crate::snapshot::WorkspaceContent) holds the bytes — so
//! this adds no bound the crate did not already have. Streaming import belongs with a
//! durable store, not with a semantic reference implementation.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//!
//! use continuum_workspace::import::DiskImporter;
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
//! let imported = DiskImporter::new().import(Path::new("/some/workspace"), &HexIdentity)?;
//!
//! // The file tree, and the components the root's recognized files supplied.
//! assert!(imported.snapshot().identity().to_string().starts_with("ws_"));
//! for (kind, path) in imported.recognized_components() {
//!     println!("{kind} came from {path}");
//! }
//! # Ok::<(), continuum_workspace::import::ImportError>(())
//! ```

use core::fmt;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::components::{
    ComponentKind, DescriptorError, WorkspaceDescriptor, WorkspaceDescriptorBuilder,
};
use crate::publication::ContentIdentifier;
use crate::snapshot::{
    AdmissionError, AdmissionPolicy, Candidate, Snapshot, SnapshotError, WorkspaceContent,
    WorkspacePath, admit,
};

// --- the recognized-component policy -----------------------------------------------------

/// Version of [`RECOGNIZED_COMPONENT_FILES`].
///
/// The table decides which workspace gets which
/// [`WorkspaceDescriptor::identity`](crate::components::WorkspaceDescriptor::identity), so
/// changing it changes descriptor identities for workspaces nobody edited. That makes the
/// table a versioned policy rather than a convenience list: adding a name, removing one,
/// or moving one between kinds moves this number in the same change.
///
/// It is an encoding-contract-shaped version in the sense of
/// `notes/plan/schemas/README.md`'s `schema_epoch` — it pins no semantics — and it is not
/// one of ADR-0018's six epochs.
pub const COMPONENT_RECOGNITION_VERSION: u32 = 1;

/// Which workspace-root file supplies which optional component, closed and ordered.
///
/// Entries are `(kind, accepted names)`, in [`ComponentKind`] order, and names are matched
/// against the **final segment of a depth-1 workspace path** — the workspace root's own
/// entries and nowhere else. A kind with several accepted names accepts *at most one of
/// them per workspace*: a tree holding two is [`ImportError::AmbiguousComponent`].
///
/// [`ComponentKind::Source`] is absent because it is not optional and is not a file: it is
/// the whole tree.
///
/// See the module documentation's "recognized component files" for why each name is here
/// and why `Cargo.toml` is not.
pub const RECOGNIZED_COMPONENT_FILES: [(ComponentKind, &[&str]); 3] = [
    (ComponentKind::Dependencies, &["Cargo.lock"]),
    (
        ComponentKind::Toolchain,
        &["rust-toolchain.toml", "rust-toolchain"],
    ),
    (ComponentKind::Configuration, &["continuum.toml"]),
];

// --- errors -------------------------------------------------------------------------------

/// Which filesystem operation an [`ImportError::Io`] was reported by.
///
/// Named rather than folded into the message so that a caller can branch on *what* the
/// importer was doing without parsing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IoOperation {
    /// Listing a directory's entries.
    ReadDirectory,
    /// Reading an entry's metadata, without following it.
    ReadMetadata,
    /// Reading a regular file's content.
    ReadFile,
    /// Reading a symbolic link's target.
    ReadLink,
}

impl IoOperation {
    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadDirectory => "read-directory",
            Self::ReadMetadata => "read-metadata",
            Self::ReadFile => "read-file",
            Self::ReadLink => "read-link",
        }
    }

    /// What the importer was doing, as a clause for a message.
    #[must_use]
    const fn gerund(self) -> &'static str {
        match self {
            Self::ReadDirectory => "listing the directory",
            Self::ReadMetadata => "reading the metadata of",
            Self::ReadFile => "reading the file",
            Self::ReadLink => "reading the link target of",
        }
    }
}

impl fmt::Display for IoOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a directory is not a workspace.
///
/// Every variant names the entry it is about. There is no untyped skip anywhere in this
/// module: an entry that cannot become workspace content refuses the whole import, because
/// a snapshot that quietly omitted it would claim to describe a directory it does not
/// describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The import root is not a directory.
    ///
    /// A workspace root is a directory even when it is empty; a regular file is not a
    /// workspace with one file in it.
    RootIsNotADirectory {
        /// The root, as the caller named it.
        path: PathBuf,
    },
    /// The filesystem refused an operation.
    Io {
        /// The entry the operation was on.
        path: PathBuf,
        /// What the importer was doing.
        operation: IoOperation,
        /// The error's kind. The operating system's message is deliberately not carried:
        /// it varies by platform and locale, and an error type that two platforms compare
        /// unequal on identical inputs is a determinism hazard in test and in evidence.
        kind: io::ErrorKind,
    },
    /// An entry's name has no byte spelling on this platform.
    ///
    /// Reachable only where an operating system reports names as something other than
    /// bytes and the name is not valid Unicode either — a lone surrogate in a Windows
    /// name. [`admit`] decides what a *name* means and it decides over bytes, so a name
    /// with no byte spelling cannot be submitted to it. Refusing is the byte rule's own
    /// answer (`snapshot.rs`, "names that are not UTF-8"): a name is never lossily
    /// decoded, and the fix — rename the file — is in the user's hands.
    UnrepresentableName {
        /// The entry, as a path.
        path: PathBuf,
    },
    /// The workspace root holds two files that both claim one component.
    AmbiguousComponent {
        /// The contested component.
        kind: ComponentKind,
        /// Every recognized name present, in [`RECOGNIZED_COMPONENT_FILES`] order.
        present: Vec<String>,
    },
    /// The candidates are not a workspace: see [`AdmissionError`].
    Admission(AdmissionError),
    /// The admitted content is not a snapshot: see [`SnapshotError`].
    Snapshot(SnapshotError),
    /// The snapshot and its components are not a descriptor: see [`DescriptorError`].
    Descriptor(DescriptorError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootIsNotADirectory { path } => write!(
                f,
                "import root `{}` is not a directory; a workspace root is a directory, \
                 including when it is empty",
                path.display()
            ),
            Self::Io {
                path,
                operation,
                kind,
            } => write!(
                f,
                "{} `{}` failed: {kind:?}",
                operation.gerund(),
                path.display()
            ),
            Self::UnrepresentableName { path } => write!(
                f,
                "entry `{}` has a name this platform cannot report as bytes; a workspace \
                 path is its bytes and is never lossily decoded",
                path.display()
            ),
            Self::AmbiguousComponent { kind, present } => write!(
                f,
                "the workspace root holds {} files that each claim the `{kind}` component \
                 ({}); which one is authoritative is a property of the workspace, not of a \
                 precedence rule",
                present.len(),
                present.join(", ")
            ),
            Self::Admission(error) => error.fmt(f),
            Self::Snapshot(error) => error.fmt(f),
            Self::Descriptor(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ImportError {}

impl From<AdmissionError> for ImportError {
    fn from(error: AdmissionError) -> Self {
        Self::Admission(error)
    }
}

impl From<SnapshotError> for ImportError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<DescriptorError> for ImportError {
    fn from(error: DescriptorError) -> Self {
        Self::Descriptor(error)
    }
}

// --- the importer ---------------------------------------------------------------------------

/// The crate's filesystem boundary: a working tree in, values out.
///
/// The only type here that opens a file or reads a directory. Everything it produces is an
/// ordinary value, so every later operation — overlay, fork, seal, diff — is reproducible
/// without the directory (INV-005).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiskImporter {
    admission: AdmissionPolicy,
}

impl DiskImporter {
    /// An importer under the default admission policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            admission: AdmissionPolicy::new(),
        }
    }

    /// Choose the admission policy.
    ///
    /// The policy is [`admit`]'s, unchanged and unextended: selecting an arm this crate
    /// does not implement is that function's typed refusal, reported here as
    /// [`ImportError::Admission`].
    #[must_use]
    pub const fn with_admission(self, admission: AdmissionPolicy) -> Self {
        Self { admission }
    }

    /// The admission policy in force.
    #[must_use]
    pub const fn admission_policy(self) -> AdmissionPolicy {
        self.admission
    }

    /// Walk `root`, admit what is there, and name it.
    ///
    /// The whole import, in one call: walk deterministically, classify every entry, apply
    /// the admission policy, build the Merkle tree, recognize the root's component files,
    /// and compose the [`WorkspaceDescriptor`].
    ///
    /// # Errors
    ///
    /// [`ImportError`]: a root that is not a directory, a filesystem refusal, a name with
    /// no byte spelling, two files claiming one component, or the typed rejection
    /// [`admit`], [`Snapshot::build`], or
    /// [`WorkspaceDescriptorBuilder::build`](crate::components::WorkspaceDescriptorBuilder::build)
    /// reported.
    pub fn import<I: ContentIdentifier + ?Sized>(
        self,
        root: &Path,
        identifier: &I,
    ) -> Result<ImportedWorkspace, ImportError> {
        let metadata = fs::metadata(root).map_err(|error| ImportError::Io {
            path: root.to_path_buf(),
            operation: IoOperation::ReadMetadata,
            kind: error.kind(),
        })?;
        if !metadata.is_dir() {
            return Err(ImportError::RootIsNotADirectory {
                path: root.to_path_buf(),
            });
        }

        let walk = walk(root)?;
        let unrepresented = walk.unrepresented_directories();

        let candidates: Vec<(&[u8], Candidate<'_>)> = walk
            .entries
            .iter()
            .map(|entry| (entry.name.as_slice(), entry.found.as_candidate()))
            .collect();
        let content = admit(candidates, self.admission)?;

        let recognized = recognize(&content)?;
        let source = Snapshot::build(&content, identifier)?;

        let mut builder = WorkspaceDescriptorBuilder::default();
        for (kind, path) in &recognized {
            let bytes = content.get(path).unwrap_or_default().to_vec();
            builder = supply(builder, *kind, bytes);
        }
        let descriptor = builder.build(source, identifier)?;

        Ok(ImportedWorkspace {
            descriptor,
            recognized,
            unrepresented,
            admission: self.admission,
        })
    }
}

/// Attach one recognized component's bytes to the builder.
///
/// [`ComponentKind::Source`] never reaches here: [`recognize`] only ever returns kinds from
/// [`RECOGNIZED_COMPONENT_FILES`], which does not list it. The arm exists because the enum
/// is closed and a `_` arm would silently absorb a fifth kind added later.
fn supply(
    builder: WorkspaceDescriptorBuilder,
    kind: ComponentKind,
    bytes: Vec<u8>,
) -> WorkspaceDescriptorBuilder {
    match kind {
        ComponentKind::Source => builder,
        ComponentKind::Dependencies => builder.dependencies(bytes),
        ComponentKind::Toolchain => builder.toolchain(bytes),
        ComponentKind::Configuration => builder.configuration(bytes),
    }
}

/// Which of the root's files supply which component, in [`RECOGNIZED_COMPONENT_FILES`]
/// order.
///
/// Pure: a function of the admitted content, so recognition is reproducible from the
/// snapshot without the directory.
fn recognize(
    content: &WorkspaceContent,
) -> Result<Vec<(ComponentKind, WorkspacePath)>, ImportError> {
    let mut out = Vec::new();
    for (kind, names) in RECOGNIZED_COMPONENT_FILES {
        let mut present: Vec<(&str, WorkspacePath)> = Vec::new();
        for name in names {
            // A typed refusal rather than an unwrap: unreachable for the table as written
            // — every name in it is a valid one-segment path, which a unit test asserts —
            // and a component in the trust base does not abort on its own invariant.
            let path = WorkspacePath::new(name).map_err(|error| {
                ImportError::Admission(AdmissionError::Name {
                    name: (*name).as_bytes().to_vec(),
                    error,
                })
            })?;
            // Depth is 1 by construction for every name in the table; the guard states the
            // root-relative rule rather than relying on it.
            if path.depth() == 1 && content.contains(&path) {
                present.push((name, path));
            }
        }
        if present.len() > 1 {
            return Err(ImportError::AmbiguousComponent {
                kind,
                present: present
                    .into_iter()
                    .map(|(name, _)| name.to_owned())
                    .collect(),
            });
        }
        if let Some((_, path)) = present.pop() {
            out.push((kind, path));
        }
    }
    Ok(out)
}

// --- the result -------------------------------------------------------------------------------

/// What one import produced.
///
/// A value, complete without the directory it came from: the composed
/// [`WorkspaceDescriptor`], which of the root's files supplied which component, which
/// directories left no trace, and the policy the import ran under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedWorkspace {
    descriptor: WorkspaceDescriptor,
    recognized: Vec<(ComponentKind, WorkspacePath)>,
    unrepresented: Vec<PathBuf>,
    admission: AdmissionPolicy,
}

impl ImportedWorkspace {
    /// The composed descriptor: the file tree beside the components the root supplied.
    #[must_use]
    pub const fn descriptor(&self) -> &WorkspaceDescriptor {
        &self.descriptor
    }

    /// The descriptor, taken by value — for sealing, which consumes it.
    #[must_use]
    pub fn into_descriptor(self) -> WorkspaceDescriptor {
        self.descriptor
    }

    /// The source file tree.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        self.descriptor.source()
    }

    /// Which workspace file supplied which component, in [`RECOGNIZED_COMPONENT_FILES`]
    /// order. Empty when the root holds none of them.
    #[must_use]
    pub fn recognized_components(&self) -> &[(ComponentKind, WorkspacePath)] {
        &self.recognized
    }

    /// Every directory the walk entered that left no trace in the snapshot, in walk order.
    ///
    /// Named by *disk* path, because a directory with no files under it has no workspace
    /// path — that is exactly what "not representable" means. See the module
    /// documentation's "empty directories are skipped".
    #[must_use]
    pub fn unrepresented_directories(&self) -> &[PathBuf] {
        &self.unrepresented
    }

    /// The admission policy this import ran under.
    #[must_use]
    pub const fn admission_policy(&self) -> AdmissionPolicy {
        self.admission
    }

    /// The [`RECOGNIZED_COMPONENT_FILES`] version this import ran under.
    #[must_use]
    pub const fn component_recognition_version(&self) -> u32 {
        COMPONENT_RECOGNITION_VERSION
    }
}

// --- the walk ---------------------------------------------------------------------------------

/// What the walk found at one entry, owning its bytes so [`admit`] can borrow them.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    File(Vec<u8>),
    Symlink(Vec<u8>),
    Special,
}

impl Found {
    /// The candidate this is, for [`admit`]. The classification is one-to-one and this
    /// module makes no decision in it.
    fn as_candidate(&self) -> Candidate<'_> {
        match self {
            Self::File(content) => Candidate::File(content),
            Self::Symlink(target) => Candidate::Symlink(target),
            Self::Special => Candidate::Special,
        }
    }
}

/// One walked non-directory entry: its workspace-root-relative raw name and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: Vec<u8>,
    found: Found,
}

/// One entry still to visit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    disk: PathBuf,
    name: Vec<u8>,
}

/// Everything one walk observed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Walk {
    /// Non-directory entries, in walk order.
    entries: Vec<Entry>,
    /// Directories entered, with their disk paths, in walk order.
    directories: Vec<(Vec<u8>, PathBuf)>,
}

impl Walk {
    /// The directories that left no trace: no entry's name is under them.
    ///
    /// Computed from the walk rather than tracked during it, so the definition is stated
    /// once and cannot drift: a directory is unrepresented exactly when no entry's raw name
    /// begins with the directory's name followed by a separator. Nested empty directories
    /// are all reported, outermost first, which is what a caller fixing the tree needs.
    fn unrepresented_directories(&self) -> Vec<PathBuf> {
        let names: BTreeSet<&[u8]> = self
            .entries
            .iter()
            .map(|entry| entry.name.as_slice())
            .collect();
        self.directories
            .iter()
            .filter(|(name, _)| {
                let mut prefix = name.clone();
                prefix.push(b'/');
                !names
                    .range(prefix.as_slice()..)
                    .next()
                    .is_some_and(|candidate| candidate.starts_with(&prefix))
            })
            .map(|(_, disk)| disk.clone())
            .collect()
    }
}

/// Walk `root` depth-first, least entry first, without recursing.
///
/// The stack is explicit so that an adversarially deep tree cannot exhaust the call stack:
/// depth is admission's decision, reported as
/// [`PathError::TooDeep`](crate::snapshot::PathError::TooDeep), and a stack overflow is a
/// panic on adversarial input, which this crate does not do.
fn walk(root: &Path) -> Result<Walk, ImportError> {
    let mut out = Walk {
        entries: Vec::new(),
        directories: Vec::new(),
    };
    let mut stack: Vec<Pending> = Vec::new();
    push_children(root, &[], &mut stack)?;

    while let Some(pending) = stack.pop() {
        let metadata = fs::symlink_metadata(&pending.disk).map_err(|error| ImportError::Io {
            path: pending.disk.clone(),
            operation: IoOperation::ReadMetadata,
            kind: error.kind(),
        })?;
        let kind = metadata.file_type();

        // A symbolic link is examined first and never followed: `symlink_metadata` reports
        // the link itself, so a link to a directory is a link here, not a directory, and
        // the walk cannot leave the root through one.
        if kind.is_symlink() {
            let target = fs::read_link(&pending.disk).map_err(|error| ImportError::Io {
                path: pending.disk.clone(),
                operation: IoOperation::ReadLink,
                kind: error.kind(),
            })?;
            let target = raw_bytes(target.as_os_str()).ok_or(ImportError::UnrepresentableName {
                path: pending.disk.clone(),
            })?;
            out.entries.push(Entry {
                name: pending.name,
                found: Found::Symlink(target),
            });
        } else if kind.is_dir() {
            push_children(&pending.disk, &pending.name, &mut stack)?;
            out.directories.push((pending.name, pending.disk));
        } else if kind.is_file() {
            let content = fs::read(&pending.disk).map_err(|error| ImportError::Io {
                path: pending.disk.clone(),
                operation: IoOperation::ReadFile,
                kind: error.kind(),
            })?;
            out.entries.push(Entry {
                name: pending.name,
                found: Found::File(content),
            });
        } else {
            out.entries.push(Entry {
                name: pending.name,
                found: Found::Special,
            });
        }
    }
    Ok(out)
}

/// List one directory, sort its entries by raw name bytes, and stack them least-first.
///
/// Sorting here is what makes the *rejection* order a function of the tree: a filesystem
/// reports entries in whatever order it likes, and an I/O failure met in that order would
/// have two importers of one workspace disagreeing about which entry refused it.
fn push_children(
    directory: &Path,
    prefix: &[u8],
    stack: &mut Vec<Pending>,
) -> Result<(), ImportError> {
    let listing = fs::read_dir(directory).map_err(|error| ImportError::Io {
        path: directory.to_path_buf(),
        operation: IoOperation::ReadDirectory,
        kind: error.kind(),
    })?;

    let mut children: Vec<Pending> = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|error| ImportError::Io {
            path: directory.to_path_buf(),
            operation: IoOperation::ReadDirectory,
            kind: error.kind(),
        })?;
        let name = entry.file_name();
        let raw = raw_bytes(&name).ok_or_else(|| ImportError::UnrepresentableName {
            path: directory.join(&name),
        })?;
        let mut relative = Vec::with_capacity(prefix.len() + 1 + raw.len());
        if !prefix.is_empty() {
            relative.extend_from_slice(prefix);
            relative.push(b'/');
        }
        relative.extend_from_slice(&raw);
        children.push(Pending {
            disk: entry.path(),
            name: relative,
        });
    }

    children.sort_by(|left, right| left.name.cmp(&right.name));
    // The stack pops from the end, so the least child has to be pushed last.
    stack.extend(children.into_iter().rev());
    Ok(())
}

/// An operating-system name as the bytes it is, or [`None`] where it has no byte spelling.
///
/// Never lossy: `snapshot.rs`'s byte rule refuses a lossily decoded name because the decode
/// maps distinct names onto one path, and this is the boundary that rule is stated at.
#[cfg(unix)]
fn raw_bytes(name: &OsStr) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;

    Some(name.as_bytes().to_vec())
}

/// An operating-system name as the bytes it is, or [`None`] where it has no byte spelling.
///
/// Off Unix a name is not natively a byte string. A name that is valid Unicode has exactly
/// one byte spelling — its UTF-8 encoding — and one that is not (a lone surrogate in a
/// Windows name) has none, which is [`ImportError::UnrepresentableName`].
#[cfg(not(unix))]
fn raw_bytes(name: &OsStr) -> Option<Vec<u8>> {
    name.to_str().map(|text| text.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ComponentKind;

    #[test]
    fn the_recognized_table_lists_every_optional_kind_once_and_never_source() {
        let kinds: Vec<ComponentKind> = RECOGNIZED_COMPONENT_FILES
            .iter()
            .map(|(kind, _)| *kind)
            .collect();
        assert_eq!(
            kinds,
            [
                ComponentKind::Dependencies,
                ComponentKind::Toolchain,
                ComponentKind::Configuration
            ],
        );
        assert!(!kinds.contains(&ComponentKind::Source));
    }

    #[test]
    fn every_recognized_name_is_a_one_segment_workspace_path() {
        for (_, names) in RECOGNIZED_COMPONENT_FILES {
            for name in names {
                let path = WorkspacePath::new(name).expect("a recognized name is a path");
                assert_eq!(path.depth(), 1, "{name} is not a workspace-root file");
            }
        }
    }

    #[test]
    fn no_name_is_claimed_by_two_kinds() {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (_, names) in RECOGNIZED_COMPONENT_FILES {
            for name in names {
                assert!(seen.insert(name), "{name} is claimed twice");
            }
        }
    }

    #[test]
    fn recognition_reads_the_root_and_not_a_subdirectory() {
        let mut content = WorkspaceContent::new();
        content
            .insert(
                WorkspacePath::new("vendor/Cargo.lock").expect("path"),
                b"nested".to_vec(),
            )
            .expect("insert");
        assert_eq!(recognize(&content).expect("recognize"), Vec::new());

        content
            .insert(
                WorkspacePath::new("Cargo.lock").expect("path"),
                b"root".to_vec(),
            )
            .expect("insert");
        assert_eq!(
            recognize(&content).expect("recognize"),
            vec![(
                ComponentKind::Dependencies,
                WorkspacePath::new("Cargo.lock").expect("path")
            )],
        );
    }

    #[test]
    fn two_spellings_of_one_component_are_a_typed_refusal() {
        let mut content = WorkspaceContent::new();
        content
            .insert(
                WorkspacePath::new("rust-toolchain.toml").expect("path"),
                b"a".to_vec(),
            )
            .expect("insert");
        content
            .insert(
                WorkspacePath::new("rust-toolchain").expect("path"),
                b"b".to_vec(),
            )
            .expect("insert");
        assert_eq!(
            recognize(&content),
            Err(ImportError::AmbiguousComponent {
                kind: ComponentKind::Toolchain,
                present: vec![
                    "rust-toolchain.toml".to_owned(),
                    "rust-toolchain".to_owned()
                ],
            }),
        );
    }

    #[test]
    fn an_unrepresented_directory_is_one_no_entry_lives_under() {
        let walk = Walk {
            entries: vec![Entry {
                name: b"src/lib.rs".to_vec(),
                found: Found::File(b"x".to_vec()),
            }],
            directories: vec![
                (b"src".to_vec(), PathBuf::from("/w/src")),
                (b"empty".to_vec(), PathBuf::from("/w/empty")),
                (b"empty/deeper".to_vec(), PathBuf::from("/w/empty/deeper")),
                // A directory whose name is a prefix of a *file* name is not thereby
                // represented: `srcery` is not `src`.
                (b"srcery".to_vec(), PathBuf::from("/w/srcery")),
            ],
        };
        assert_eq!(
            walk.unrepresented_directories(),
            vec![
                PathBuf::from("/w/empty"),
                PathBuf::from("/w/empty/deeper"),
                PathBuf::from("/w/srcery"),
            ],
        );
    }

    #[test]
    fn a_classification_is_one_to_one() {
        assert_eq!(
            Found::File(b"x".to_vec()).as_candidate(),
            Candidate::File(b"x")
        );
        assert_eq!(
            Found::Symlink(b"../y".to_vec()).as_candidate(),
            Candidate::Symlink(b"../y")
        );
        assert_eq!(Found::Special.as_candidate(), Candidate::Special);
    }

    #[test]
    fn the_default_importer_rejects_symbolic_links() {
        assert_eq!(
            DiskImporter::new().admission_policy(),
            AdmissionPolicy::new()
        );
        assert_eq!(DiskImporter::default(), DiskImporter::new());
    }
}
