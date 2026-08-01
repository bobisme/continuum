//! Workspace-level composite identity: source, dependency, toolchain, and configuration
//! (plan §4.2, PR 3 / IMPL-03).
//!
//! # What this module decides
//!
//! > A workspace snapshot contains content identities for: source files; CML modules;
//! > Rust semantic extraction; domain-pack manifests; dependency lockfiles; toolchain and
//! > semantic epochs; the content identity of the governing Intent Contract (a reference,
//! > not the contract itself); generated correspondence; proof environment;
//! > configuration.
//! >
//! > — `notes/plan/plan.md` §4.2 "Workspace snapshots"
//!
//! Plan §4.2 names ten components. [`snapshot`](crate::snapshot) delivered the first —
//! source files, as a Merkle tree — in PR 3 / IMPL-01, and its module documentation left
//! this note:
//!
//! > Source, dependency, toolchain, and configuration identities are IMPL-03. […] this
//! > module owns the file tree — the schema's `files` and `root_digest` — and the
//! > remaining components (`dependencies`, `epochs`, `intent`, and the named-digest
//! > lists) attach *beside* the root rather than inside it, because they are identities
//! > of things that are not files in the tree.
//!
//! This module is that seam, for exactly four of the ten: **source** (by reference to
//! [`Snapshot`]), **dependencies**, **toolchain**, and **configuration** — the bone
//! `PR-3-IMPL-03` names. CML modules, Rust semantic extraction, domain-pack manifests,
//! generated correspondence, and proof environment are the remaining named-digest-list
//! components of `notes/plan/schemas/workspace-snapshot.schema.json`; the governing
//! Intent Contract reference needs PR 4's types. None of those five are touched here —
//! adding them is a later, separately evidenced change, not a silent extension of
//! [`ComponentKind`] (see "Closed kind, versioned growth" below).
//!
//! [`WorkspaceDescriptor`] is the composite: it binds a [`Snapshot`] identity beside a
//! dependency-lockfile identity, a toolchain identity, and a configuration identity, and
//! names itself from the four. "Beside the root rather than inside it" is not a metaphor
//! here — [`WorkspaceDescriptor`] never edits or re-derives anything about the
//! [`Snapshot`] it holds; it only asks the tree for [`Snapshot::identity`] and composes.
//!
//! # Why the design mirrors `snapshot.rs`
//!
//! `snapshot.rs` is not touched by this module — a sibling change owns its ordering
//! policy — but its identity discipline is the one this crate has already committed to,
//! and this module follows it exactly rather than inventing a second one:
//!
//! - a **record** ([`Component::record`], [`WorkspaceDescriptor::record`]): canonical
//!   bytes — magic, version, tagged fields — that an identity is derived from;
//! - an **identity** ([`Component::identity`], [`WorkspaceDescriptor::identity`]): a
//!   `ws_*` [`ArtifactHandle`] derived from that record through the crate's
//!   [`ContentIdentifier`] seam ([`crate::publication::ContentIdentifier`]), never
//!   computed locally;
//! - **content equality, never identity equality** ([`PartialEq`] on [`Component`] and
//!   [`WorkspaceDescriptor`]): ADR-0013 — "canonical structural encodings define
//!   identity; hashes index and partition; collisions resolve by exact comparison" — so
//!   two descriptors are equal exactly when their source trees and component bytes are,
//!   for every identity function that has ever been or will ever be installed. A
//!   [`WorkspaceDescriptor`]'s `record` and `identity` fields are therefore *excluded*
//!   from its [`PartialEq`], exactly as [`DirectoryNode`](crate::snapshot::DirectoryNode)
//!   excludes its own;
//! - **presence is structural, not an empty encoding.** A workspace without a lockfile
//!   has no `Dependencies` entry in the record at all — not a zero-length one. Two
//!   descriptors that differ only in whether a component is present therefore have
//!   different records, and different identities, exactly as `Snapshot`'s "an empty
//!   workspace has an identity of its own" is a *directory with no children*, never a
//!   directory conflated with "no directory";
//! - **identities are never trusted from the wire.** [`WorkspaceDescriptor::decode`]
//!   re-derives every identity — the descriptor's own and every component's — from the
//!   bytes it actually reads, through the caller's [`ContentIdentifier`]. The wire form
//!   carries no identity at all, matching [`Snapshot::encode`]'s "a reader re-derives
//!   them from the content it actually received".
//!
//! # Closed kind, versioned growth
//!
//! [`ComponentKind`] has exactly four variants. Adding a fifth — one of the other five
//! plan §4.2 components, or something new — changes [`ComponentKind::ALL`], the record
//! grammar's tag space, and [`ENCODING_VERSION`], all at once: it is a store-format
//! change, exactly as growing `snapshot.rs`'s record grammar would be, and MUST NOT be
//! done by silently reusing an existing tag or by encoding a new component as
//! configuration-shaped bytes. This mirrors [`crate::snapshot::ENCODING_VERSION`]'s own
//! stance: an encoding contract in the sense of `notes/plan/schemas/README.md`'s
//! `schema_epoch`, not one of ADR-0018's six independently versioned epochs
//! ([`continuum_value::epoch::EpochKind`], out of this crate's dependency closure).
//!
//! # Two artifact shapes, one class
//!
//! Every identity this module derives — a component's and the descriptor's own — is
//! requested in [`crate::snapshot::NODE_CLASS`] (`ws_*`), the same class
//! `snapshot.rs` names every file leaf and directory in. A [`WorkspaceDescriptor`] is
//! "the workspace" one level up from "the file tree", so it shares the tree's namespace
//! rather than inventing a second `ArtifactClass` for what
//! `notes/plan/schemas/workspace-snapshot.schema.json` already calls `workspace_id`
//! (`^ws_[A-Za-z0-9_-]+$`) beside the tree's own `root_digest`. Domain separation between
//! a component record, a descriptor record, and a `snapshot.rs` node record is carried by
//! distinct magic prefixes and tag bytes instead ([`COMPONENT_RECORD_MAGIC`],
//! [`DESCRIPTOR_RECORD_MAGIC`], [`DESCRIPTOR_MAGIC`]), the same way `snapshot.rs`
//! separates `TREE_MAGIC` from `NODE_MAGIC`.
//!
//! # No ambient anything
//!
//! [`WorkspaceDescriptorBuilder::build`] takes an already-built [`Snapshot`] and raw
//! bytes for each optional component — it opens no file, reads no directory, and decides
//! nothing about *which* file on disk is "the" lockfile, toolchain declaration, or
//! configuration (INV-005, ADR-0003, mirroring [`Snapshot::build`]'s "No ambient
//! anything"). That discovery is disk import's job.
//!
//! # Seams left open on purpose
//!
//! - **Disk import is PR 3 / IMPL-02's.** This module is the pure composite; the code
//!   that walks a working tree, decides that `Cargo.lock` is *the* dependency file for a
//!   Cargo workspace (or that a workspace has none), reads `rust-toolchain.toml`, and
//!   reads whatever files constitute "workspace configuration" is IMPL-02's, and it is a
//!   *caller* of [`WorkspaceDescriptorBuilder`]: it produces up to three optional byte
//!   buffers and a [`Snapshot`], and asks for a descriptor. Multi-file configuration
//!   (concatenation policy, canonical ordering across files) is also IMPL-02's; this
//!   module takes configuration as one already-assembled buffer, exactly as
//!   [`Snapshot::build`] takes an already-assembled [`WorkspaceContent`](crate::snapshot::WorkspaceContent)
//!   rather than walking a directory itself.
//! - **The other five plan §4.2 components are out of scope.** See "What this module
//!   decides" above.
//! - **The `epochs.semantic` / `epochs.proof` fields of the workspace-snapshot schema are
//!   not this module's.** They are two of ADR-0018's six independently versioned epochs
//!   (`continuum-value`'s `EpochKind`, PR 1 / IMPL-02), not content identities of
//!   workspace bytes, and binding them into a published `workspace-snapshot` artifact is
//!   schema-assembly work for whichever PR performs it.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
//! use continuum_workspace::components::WorkspaceDescriptor;
//! use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
//! use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
//!
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
//! let source = Snapshot::build(&content, &HexIdentity)?;
//!
//! // A workspace with a lockfile but no toolchain pin and no configuration.
//! let descriptor = WorkspaceDescriptor::builder()
//!     .dependencies(b"[[package]]\nname = \"a\"".to_vec())
//!     .build(source, &HexIdentity)?;
//!
//! assert!(descriptor.identity().to_string().starts_with("ws_"));
//! assert!(descriptor.dependencies().is_some());
//! assert!(descriptor.toolchain().is_none());
//! assert!(descriptor.configuration().is_none());
//!
//! // Absence is structural: adding the same-shaped-but-absent toolchain component moves
//! // the identity even though no byte of the lockfile or the source tree changed.
//! let mut with_content = WorkspaceContent::new();
//! with_content.insert(WorkspacePath::new("src/lib.rs")?, b"fn main() {}".to_vec())?;
//! let source_again = Snapshot::build(&with_content, &HexIdentity)?;
//! let with_toolchain = WorkspaceDescriptor::builder()
//!     .dependencies(b"[[package]]\nname = \"a\"".to_vec())
//!     .toolchain(b"[toolchain]\nchannel = \"1.75\"".to_vec())
//!     .build(source_again, &HexIdentity)?;
//! assert_ne!(with_toolchain.identity(), descriptor.identity());
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

use core::fmt;

use crate::artifact_path::{ArtifactClass, ArtifactHandle};
use crate::publication::{ContentIdentifier, IdentityUnavailable};
use crate::snapshot::{NODE_CLASS, Snapshot, SnapshotDecodeError};

// --- format constants ------------------------------------------------------------------

/// Version of the record and encoding grammars this module defines.
///
/// An encoding contract in the sense of `notes/plan/schemas/README.md`'s `schema_epoch`,
/// exactly as [`crate::snapshot::ENCODING_VERSION`] is: it says which grammar a byte
/// string was written under and pins no semantics. It is independent of
/// `crate::snapshot::ENCODING_VERSION` — the two modules may version their grammars on
/// different schedules — and it is not one of ADR-0018's six epochs.
pub const ENCODING_VERSION: u8 = 1;

/// Magic prefix of one [`Component`]'s record.
const COMPONENT_RECORD_MAGIC: &[u8] = b"cwcrec";

/// Magic prefix of a [`WorkspaceDescriptor`]'s own record.
const DESCRIPTOR_RECORD_MAGIC: &[u8] = b"cwdrec";

/// Magic prefix of a whole-descriptor encoding ([`WorkspaceDescriptor::encode`]).
///
/// Distinct from [`COMPONENT_RECORD_MAGIC`] and [`DESCRIPTOR_RECORD_MAGIC`] for the same
/// reason `snapshot.rs` keeps `TREE_MAGIC` and `NODE_MAGIC` apart: three different byte
/// strings must never be mistaken for one another by a decoder or by a reader of a hex
/// dump.
const DESCRIPTOR_MAGIC: &[u8] = b"cwdesc";

/// Bytes of a component record before its content: magic, version, kind tag, and a `u64`
/// length.
const COMPONENT_RECORD_HEADER: usize = COMPONENT_RECORD_MAGIC.len() + 1 + 1 + 8;

// --- component kinds ---------------------------------------------------------------------

/// Which of the four PR-3/IMPL-03 workspace components a value names.
///
/// Closed by design — see the module documentation's "Closed kind, versioned growth".
/// [`Source`](Self::Source) is the one kind that is never absent: every workspace has a
/// file tree, even an empty one ([`Snapshot`]'s own "an empty workspace has an identity
/// of its own"). The other three are optional ([`WorkspaceDescriptor::dependencies`],
/// [`WorkspaceDescriptor::toolchain`], [`WorkspaceDescriptor::configuration`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentKind {
    /// The source file tree — a reference to a [`Snapshot`]'s own identity.
    Source,
    /// A dependency-lockfile identity. `Cargo.lock` is the motivating case; the kind
    /// names no filename.
    Dependencies,
    /// A toolchain-declaration identity. `rust-toolchain.toml` is the motivating case.
    Toolchain,
    /// A workspace-configuration identity.
    Configuration,
}

impl ComponentKind {
    /// Every kind, in the order the record grammar and [`WorkspaceDescriptor::encode`]
    /// use — also this type's declaration order and its [`Ord`].
    pub const ALL: [Self; 4] = [
        Self::Source,
        Self::Dependencies,
        Self::Toolchain,
        Self::Configuration,
    ];

    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Dependencies => "dependencies",
            Self::Toolchain => "toolchain",
            Self::Configuration => "configuration",
        }
    }

    /// The tag byte this kind is framed under in every record and encoding this module
    /// writes.
    const fn tag(self) -> u8 {
        match self {
            Self::Source => 0x00,
            Self::Dependencies => 0x01,
            Self::Toolchain => 0x02,
            Self::Configuration => 0x03,
        }
    }

    /// Recover a kind from its [`tag`](Self::tag).
    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0x00 => Some(Self::Source),
            0x01 => Some(Self::Dependencies),
            0x02 => Some(Self::Toolchain),
            0x03 => Some(Self::Configuration),
            _ => None,
        }
    }
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- components ---------------------------------------------------------------------------

/// One optional workspace component: raw bytes, framed into a record, and the identity
/// derived from that record.
///
/// [`PartialEq`] is content equality — the kind and the framed bytes — never the
/// identity, for the same reason [`FileNode`](crate::snapshot::FileNode) excludes its own.
#[derive(Debug, Clone)]
pub struct Component {
    kind: ComponentKind,
    identity: ArtifactHandle,
    /// `COMPONENT_RECORD_MAGIC | version | kind.tag() | u64be(len) | content`.
    record: Vec<u8>,
}

impl Component {
    /// Which component this is.
    #[must_use]
    pub const fn kind(&self) -> ComponentKind {
        self.kind
    }

    /// The identity this component contributes to a [`WorkspaceDescriptor`]'s record.
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        &self.identity
    }

    /// The canonical record: the bytes the identity was derived from, and the bytes a
    /// store publishes for it.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }

    /// The component's raw content, without the record framing.
    #[must_use]
    pub fn content(&self) -> &[u8] {
        self.record.get(COMPONENT_RECORD_HEADER..).unwrap_or(&[])
    }
}

impl PartialEq for Component {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.content() == other.content()
    }
}

impl Eq for Component {}

/// Frame one component's content and name it.
///
/// # Panics
///
/// Never. `kind` is expected to be one of the three optional kinds; called with
/// [`ComponentKind::Source`] it would simply frame that tag, which no caller in this
/// module does — [`Source`](ComponentKind::Source) is always sealed through [`Snapshot`]
/// instead.
fn seal_component<I: ContentIdentifier + ?Sized>(
    kind: ComponentKind,
    content: Vec<u8>,
    identifier: &I,
) -> Result<Component, DescriptorError> {
    let mut record = Vec::with_capacity(COMPONENT_RECORD_HEADER + content.len());
    record.extend_from_slice(COMPONENT_RECORD_MAGIC);
    record.push(ENCODING_VERSION);
    record.push(kind.tag());
    record.extend_from_slice(&length_of(content.len()).to_be_bytes());
    record.extend_from_slice(&content);
    let identity = derive_identity(identifier, &record, Some(kind))?;
    Ok(Component {
        kind,
        identity,
        record,
    })
}

/// Ask the seam for an identity and check the class it came back in.
fn derive_identity<I: ContentIdentifier + ?Sized>(
    identifier: &I,
    record: &[u8],
    kind: Option<ComponentKind>,
) -> Result<ArtifactHandle, DescriptorError> {
    let handle = identifier
        .identify(NODE_CLASS, record)
        .map_err(|IdentityUnavailable| DescriptorError::IdentityUnavailable { kind })?;
    if handle.class() == NODE_CLASS {
        Ok(handle)
    } else {
        Err(DescriptorError::IdentityClassMismatch {
            kind,
            class: handle.class(),
        })
    }
}

// --- errors ------------------------------------------------------------------------------

/// Why a set of components could not be sealed into a [`WorkspaceDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// The identity seam could not name a component, or the descriptor's own record.
    IdentityUnavailable {
        /// The component that could not be named, or [`None`] for the descriptor's own
        /// record.
        kind: Option<ComponentKind>,
    },
    /// The identity seam named something outside [`NODE_CLASS`].
    IdentityClassMismatch {
        /// The component that was misnamed, or [`None`] for the descriptor's own record.
        kind: Option<ComponentKind>,
        /// The class the seam returned.
        class: ArtifactClass,
    },
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityUnavailable { kind } => {
                write!(f, "no identity could be derived for {}", render_kind(*kind))
            }
            Self::IdentityClassMismatch { kind, class } => write!(
                f,
                "identity for {} was derived in class `{class}`, not `{NODE_CLASS}`",
                render_kind(*kind)
            ),
        }
    }
}

impl core::error::Error for DescriptorError {}

/// Name a component (or the descriptor itself) in an error message.
fn render_kind(kind: Option<ComponentKind>) -> String {
    kind.map_or_else(
        || "the descriptor's own record".to_owned(),
        |kind| format!("the `{kind}` component"),
    )
}

/// Why a byte string is not a [`WorkspaceDescriptor`] encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorDecodeError {
    /// The stream did not start with [`DESCRIPTOR_MAGIC`].
    Magic,
    /// The stream declared an encoding version this build does not implement.
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
    /// A declared length exceeds what the stream can hold.
    LengthOutOfRange {
        /// Byte offset of the length field.
        at: usize,
        /// The declared length.
        declared: u64,
    },
    /// An entry's tag byte named no [`ComponentKind`].
    UnknownTag {
        /// The tag byte.
        tag: u8,
        /// Byte offset it was read at.
        at: usize,
    },
    /// Entries were not strictly ascending by tag — reordered, or a duplicate kind.
    ///
    /// The canonical order is [`ComponentKind::ALL`], so a stream that lists entries in
    /// any other order — or lists one kind twice — is a second spelling of a descriptor
    /// that already has one, exactly as `snapshot.rs` rejects a non-canonically-ordered
    /// directory rather than normalizing it.
    TagOrder {
        /// Byte offset of the offending tag.
        at: usize,
        /// The tag that did not strictly follow its predecessor.
        tag: u8,
    },
    /// No [`ComponentKind::Source`] entry was present.
    ///
    /// Source is the one component that is never absent (the module documentation's
    /// "Closed kind, versioned growth"), so a stream without one is not a descriptor of a
    /// workspace that merely has no source — it is not a descriptor at all.
    MissingSource,
    /// Bytes followed the last entry.
    TrailingBytes {
        /// Byte offset of the first trailing byte.
        at: usize,
    },
    /// The nested [`Snapshot`] encoding for the source component failed to decode.
    Source(SnapshotDecodeError),
    /// The identity seam refused or misnamed a decoded component, or the descriptor
    /// itself.
    Identity(DescriptorError),
}

impl fmt::Display for DescriptorDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Magic => f.write_str("not a workspace-descriptor encoding"),
            Self::Version { found } => write!(
                f,
                "descriptor encoding version {found} is not {ENCODING_VERSION}; an artifact \
                 declaring an unimplemented contract is refused, never best-effort decoded"
            ),
            Self::UnexpectedEnd { at, needed } => write!(
                f,
                "descriptor encoding ends at byte {at}, needing {needed} more"
            ),
            Self::LengthOutOfRange { at, declared } => write!(
                f,
                "descriptor encoding declares a length of {declared} at byte {at}, past the \
                 end of the stream"
            ),
            Self::UnknownTag { tag, at } => {
                write!(f, "unknown component tag {tag:#04x} at byte {at}")
            }
            Self::TagOrder { at, tag } => write!(
                f,
                "component tag {tag:#04x} at byte {at} does not strictly follow its \
                 predecessor; entries are canonically ordered"
            ),
            Self::MissingSource => f.write_str("descriptor encoding has no source component"),
            Self::TrailingBytes { at } => {
                write!(f, "descriptor encoding has trailing bytes from {at}")
            }
            Self::Source(error) => write!(f, "source component: {error}"),
            Self::Identity(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for DescriptorDecodeError {}

impl From<DescriptorError> for DescriptorDecodeError {
    fn from(error: DescriptorError) -> Self {
        Self::Identity(error)
    }
}

// --- the descriptor ------------------------------------------------------------------------

/// The workspace-level composite identity: a [`Snapshot`] identity beside a dependency, a
/// toolchain, and a configuration identity.
///
/// Build one with [`WorkspaceDescriptor::builder`]. There is no method that mutates one:
/// a workspace whose lockfile, toolchain pin, or configuration changed is a new
/// [`WorkspaceDescriptor`] with a new identity, exactly as an edited [`Snapshot`] is a new
/// [`Snapshot`] (ADR-0018).
#[derive(Debug, Clone)]
pub struct WorkspaceDescriptor {
    identity: ArtifactHandle,
    /// `DESCRIPTOR_RECORD_MAGIC | version | u64be(count) | (tag | u64be(len) | handle)*`,
    /// present components only, in [`ComponentKind`] order.
    record: Vec<u8>,
    source: Snapshot,
    dependencies: Option<Component>,
    toolchain: Option<Component>,
    configuration: Option<Component>,
}

impl WorkspaceDescriptor {
    /// Start building a descriptor.
    #[must_use]
    pub fn builder() -> WorkspaceDescriptorBuilder {
        WorkspaceDescriptorBuilder::default()
    }

    /// The descriptor's own identity: the name of this workspace, one level up from its
    /// file tree's [`Snapshot::identity`].
    #[must_use]
    pub const fn identity(&self) -> &ArtifactHandle {
        &self.identity
    }

    /// The canonical record: the bytes this descriptor's identity was derived from.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }

    /// The source file tree.
    #[must_use]
    pub const fn source(&self) -> &Snapshot {
        &self.source
    }

    /// The dependency-lockfile component, if the workspace has one.
    #[must_use]
    pub fn dependencies(&self) -> Option<&Component> {
        self.dependencies.as_ref()
    }

    /// The toolchain-declaration component, if the workspace has one.
    #[must_use]
    pub fn toolchain(&self) -> Option<&Component> {
        self.toolchain.as_ref()
    }

    /// The configuration component, if the workspace has one.
    #[must_use]
    pub fn configuration(&self) -> Option<&Component> {
        self.configuration.as_ref()
    }

    /// Every kind this descriptor actually carries, in [`ComponentKind`] order. Always
    /// starts with [`ComponentKind::Source`].
    #[must_use]
    pub fn present_kinds(&self) -> Vec<ComponentKind> {
        let mut out = vec![ComponentKind::Source];
        if self.dependencies.is_some() {
            out.push(ComponentKind::Dependencies);
        }
        if self.toolchain.is_some() {
            out.push(ComponentKind::Toolchain);
        }
        if self.configuration.is_some() {
            out.push(ComponentKind::Configuration);
        }
        out
    }

    /// Every distinct record this descriptor is made of, **children before parents**:
    /// every record [`Snapshot::records`] would publish for the source tree, then each
    /// present component's own record, then this descriptor's own record last.
    ///
    /// Publishing them in that order through
    /// [`ReferenceStore::publish`](crate::publication::ReferenceStore::publish) with the
    /// same [`ContentIdentifier`] reproduces exactly these identities and converges with
    /// any other descriptor that shares a component or a source subtree — the same
    /// structural-sharing claim [`Snapshot::records`] makes, one level up.
    #[must_use]
    pub fn records(&self) -> Vec<(&ArtifactHandle, &[u8])> {
        let mut out = self.source.records();
        for component in [&self.dependencies, &self.toolchain, &self.configuration]
            .into_iter()
            .flatten()
        {
            out.push((component.identity(), component.record()));
        }
        out.push((&self.identity, self.record.as_slice()));
        out
    }

    /// Encode the whole descriptor, wire-friendly and versioned.
    ///
    /// Grammar, version [`ENCODING_VERSION`], all integers big-endian:
    ///
    /// ```text
    /// descriptor := "cwdesc" u8(version) u64(count) entry*
    /// entry       := u8(kind_tag) u64(len) bytes
    /// ```
    ///
    /// `count` is between 1 and 4: [`ComponentKind::Source`] is always present, its
    /// `bytes` is [`Snapshot::encode`]'s own output, and entries are in strictly
    /// ascending tag order — [`ComponentKind::ALL`]'s order, present kinds only.
    /// [`WorkspaceDescriptor::decode`] rejects every other spelling rather than
    /// normalizing it.
    ///
    /// Identities are deliberately **not** transmitted, for the same reason
    /// [`Snapshot::encode`] omits them: a reader re-derives every identity from the
    /// content it actually received.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut entries: Vec<(u8, Vec<u8>)> =
            vec![(ComponentKind::Source.tag(), self.source.encode())];
        if let Some(component) = &self.dependencies {
            entries.push((
                ComponentKind::Dependencies.tag(),
                component.content().to_vec(),
            ));
        }
        if let Some(component) = &self.toolchain {
            entries.push((ComponentKind::Toolchain.tag(), component.content().to_vec()));
        }
        if let Some(component) = &self.configuration {
            entries.push((
                ComponentKind::Configuration.tag(),
                component.content().to_vec(),
            ));
        }

        let mut out = Vec::new();
        out.extend_from_slice(DESCRIPTOR_MAGIC);
        out.push(ENCODING_VERSION);
        out.extend_from_slice(&length_of(entries.len()).to_be_bytes());
        for (tag, bytes) in &entries {
            out.push(*tag);
            out.extend_from_slice(&length_of(bytes.len()).to_be_bytes());
            out.extend_from_slice(bytes);
        }
        out
    }

    /// Decode a descriptor and re-derive every identity — the source tree's, each
    /// component's, and the descriptor's own — through `identifier`.
    ///
    /// # Errors
    ///
    /// [`DescriptorDecodeError`] naming the first violation: a truncated stream, an
    /// oversized length, an unknown or reordered or duplicated component tag, a missing
    /// source component, trailing bytes, a malformed nested source encoding, and a seam
    /// that refuses to name a node all land here rather than in a panic or a partially
    /// built descriptor.
    pub fn decode<I: ContentIdentifier + ?Sized>(
        bytes: &[u8],
        identifier: &I,
    ) -> Result<Self, DescriptorDecodeError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(DESCRIPTOR_MAGIC.len())? != DESCRIPTOR_MAGIC {
            return Err(DescriptorDecodeError::Magic);
        }
        let version = cursor.byte()?;
        if version != ENCODING_VERSION {
            return Err(DescriptorDecodeError::Version { found: version });
        }

        let count = cursor.length()?;
        let mut source: Option<Snapshot> = None;
        let mut dependencies: Option<Component> = None;
        let mut toolchain: Option<Component> = None;
        let mut configuration: Option<Component> = None;
        let mut previous_tag: Option<u8> = None;

        for _ in 0..count {
            let tag_at = cursor.at;
            let tag = cursor.byte()?;
            let kind = ComponentKind::from_tag(tag)
                .ok_or(DescriptorDecodeError::UnknownTag { tag, at: tag_at })?;
            if previous_tag.is_some_and(|previous| previous >= tag) {
                return Err(DescriptorDecodeError::TagOrder { at: tag_at, tag });
            }
            previous_tag = Some(tag);

            let len = cursor.length()?;
            let content = cursor.take(len)?;

            match kind {
                ComponentKind::Source => {
                    source = Some(
                        Snapshot::decode(content, identifier)
                            .map_err(DescriptorDecodeError::Source)?,
                    );
                }
                ComponentKind::Dependencies => {
                    dependencies = Some(seal_component(kind, content.to_vec(), identifier)?);
                }
                ComponentKind::Toolchain => {
                    toolchain = Some(seal_component(kind, content.to_vec(), identifier)?);
                }
                ComponentKind::Configuration => {
                    configuration = Some(seal_component(kind, content.to_vec(), identifier)?);
                }
            }
        }

        if !cursor.is_empty() {
            return Err(DescriptorDecodeError::TrailingBytes { at: cursor.at });
        }
        let source = source.ok_or(DescriptorDecodeError::MissingSource)?;

        Ok(assemble(
            source,
            dependencies,
            toolchain,
            configuration,
            identifier,
        )?)
    }
}

impl PartialEq for WorkspaceDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.dependencies == other.dependencies
            && self.toolchain == other.toolchain
            && self.configuration == other.configuration
    }
}

impl Eq for WorkspaceDescriptor {}

/// Assemble the descriptor's own record from already-sealed components and name it.
fn assemble<I: ContentIdentifier + ?Sized>(
    source: Snapshot,
    dependencies: Option<Component>,
    toolchain: Option<Component>,
    configuration: Option<Component>,
    identifier: &I,
) -> Result<WorkspaceDescriptor, DescriptorError> {
    let mut present: Vec<(ComponentKind, String)> =
        vec![(ComponentKind::Source, source.identity().to_string())];
    if let Some(component) = &dependencies {
        present.push((
            ComponentKind::Dependencies,
            component.identity().to_string(),
        ));
    }
    if let Some(component) = &toolchain {
        present.push((ComponentKind::Toolchain, component.identity().to_string()));
    }
    if let Some(component) = &configuration {
        present.push((
            ComponentKind::Configuration,
            component.identity().to_string(),
        ));
    }

    let mut record = Vec::new();
    record.extend_from_slice(DESCRIPTOR_RECORD_MAGIC);
    record.push(ENCODING_VERSION);
    record.extend_from_slice(&length_of(present.len()).to_be_bytes());
    for (kind, handle_text) in &present {
        record.push(kind.tag());
        record.extend_from_slice(&length_of(handle_text.len()).to_be_bytes());
        record.extend_from_slice(handle_text.as_bytes());
    }

    let identity = derive_identity(identifier, &record, None)?;
    Ok(WorkspaceDescriptor {
        identity,
        record,
        source,
        dependencies,
        toolchain,
        configuration,
    })
}

/// A length as it appears in a record or encoding. Saturating rather than panicking,
/// exactly as `crate::snapshot`'s own `length_of` is: a length that does not fit in 64
/// bits cannot be produced by an in-memory value on any target this workspace supports.
fn length_of(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

// --- builder --------------------------------------------------------------------------------

/// Builder for [`WorkspaceDescriptor`].
///
/// The three optional components are set here; the mandatory source tree is supplied to
/// [`build`](Self::build) directly, so a descriptor without one cannot be constructed —
/// there is no "forgot to call `.source(..)`" failure mode to report at run time.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceDescriptorBuilder {
    dependencies: Option<Vec<u8>>,
    toolchain: Option<Vec<u8>>,
    configuration: Option<Vec<u8>>,
}

impl WorkspaceDescriptorBuilder {
    /// Set the dependency-lockfile bytes. `Cargo.lock`'s content is the motivating case.
    #[must_use]
    pub fn dependencies(mut self, content: Vec<u8>) -> Self {
        self.dependencies = Some(content);
        self
    }

    /// Set the toolchain-declaration bytes. `rust-toolchain.toml`'s content is the
    /// motivating case.
    #[must_use]
    pub fn toolchain(mut self, content: Vec<u8>) -> Self {
        self.toolchain = Some(content);
        self
    }

    /// Set the workspace-configuration bytes.
    #[must_use]
    pub fn configuration(mut self, content: Vec<u8>) -> Self {
        self.configuration = Some(content);
        self
    }

    /// Seal every set component, bind `source` beside them, and name the result.
    ///
    /// # Errors
    ///
    /// [`DescriptorError::IdentityUnavailable`] or
    /// [`DescriptorError::IdentityClassMismatch`] when `identifier` refuses or misnames a
    /// component or the descriptor's own record. `source`'s own identity is never
    /// re-derived here — it was already fixed when `source` was built.
    pub fn build<I: ContentIdentifier + ?Sized>(
        self,
        source: Snapshot,
        identifier: &I,
    ) -> Result<WorkspaceDescriptor, DescriptorError> {
        let dependencies = self
            .dependencies
            .map(|content| seal_component(ComponentKind::Dependencies, content, identifier))
            .transpose()?;
        let toolchain = self
            .toolchain
            .map(|content| seal_component(ComponentKind::Toolchain, content, identifier))
            .transpose()?;
        let configuration = self
            .configuration
            .map(|content| seal_component(ComponentKind::Configuration, content, identifier))
            .transpose()?;
        assemble(source, dependencies, toolchain, configuration, identifier)
    }
}

// --- decoding ------------------------------------------------------------------------------

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

    fn take(&mut self, count: usize) -> Result<&'a [u8], DescriptorDecodeError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(DescriptorDecodeError::UnexpectedEnd {
                at: self.at,
                needed: count,
            })?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(DescriptorDecodeError::UnexpectedEnd {
                at: self.at,
                needed: count,
            })?;
        self.at = end;
        Ok(slice)
    }

    fn byte(&mut self) -> Result<u8, DescriptorDecodeError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(DescriptorDecodeError::UnexpectedEnd {
                at: self.at,
                needed: 1,
            })
    }

    /// Read a length and check it against what is left, before anything is allocated.
    fn length(&mut self) -> Result<usize, DescriptorDecodeError> {
        let at = self.at;
        let raw = self.take(8)?;
        let declared = <[u8; 8]>::try_from(raw).map_or(u64::MAX, u64::from_be_bytes);
        let len = usize::try_from(declared)
            .map_err(|_| DescriptorDecodeError::LengthOutOfRange { at, declared })?;
        if len > self.remaining() {
            return Err(DescriptorDecodeError::LengthOutOfRange { at, declared });
        }
        Ok(len)
    }
}

// --- tests -----------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::WorkspaceContent;

    /// The injective identity function: a record's own bytes, in hex. ADR-0013's
    /// certified discipline written as a [`ContentIdentifier`].
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

    /// Names records in the wrong artifact class.
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

    fn source(files: &[(&str, &[u8])]) -> Snapshot {
        let mut content = WorkspaceContent::new();
        for (path, bytes) in files {
            content
                .insert(
                    crate::snapshot::WorkspacePath::new(path).expect("test path"),
                    (*bytes).to_vec(),
                )
                .expect("test description is a tree");
        }
        Snapshot::build(&content, &HexIdentity).expect("HexIdentity names every node")
    }

    fn bare_source() -> Snapshot {
        source(&[("src/lib.rs", b"fn main() {}")])
    }

    // --- presence and determinism --------------------------------------------------------

    #[test]
    fn absent_components_are_absent_not_empty() {
        let descriptor = WorkspaceDescriptor::builder()
            .build(bare_source(), &HexIdentity)
            .expect("named");
        assert!(descriptor.dependencies().is_none());
        assert!(descriptor.toolchain().is_none());
        assert!(descriptor.configuration().is_none());
        assert_eq!(descriptor.present_kinds(), vec![ComponentKind::Source]);

        // Distinct from a workspace with a present-but-empty lockfile: presence is
        // structural, so an empty buffer still moves the identity.
        let with_empty_lockfile = WorkspaceDescriptor::builder()
            .dependencies(Vec::new())
            .build(bare_source(), &HexIdentity)
            .expect("named");
        assert_ne!(descriptor.identity(), with_empty_lockfile.identity());
        assert_ne!(descriptor, with_empty_lockfile);
        assert!(with_empty_lockfile.dependencies().is_some());
    }

    #[test]
    fn same_components_are_the_same_identity() {
        let first = WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .toolchain(b"toolchain".to_vec())
            .configuration(b"config".to_vec())
            .build(bare_source(), &HexIdentity)
            .expect("named");
        let second = WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .toolchain(b"toolchain".to_vec())
            .configuration(b"config".to_vec())
            .build(bare_source(), &HexIdentity)
            .expect("named");
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first, second);
    }

    #[test]
    fn a_changed_component_moves_the_descriptor_identity() {
        let base = WorkspaceDescriptor::builder()
            .dependencies(b"lock-v1".to_vec())
            .build(bare_source(), &HexIdentity)
            .expect("named");
        let changed = WorkspaceDescriptor::builder()
            .dependencies(b"lock-v2".to_vec())
            .build(bare_source(), &HexIdentity)
            .expect("named");
        assert_ne!(base.identity(), changed.identity());
        assert_ne!(base, changed);
    }

    #[test]
    fn component_independence_the_source_identity_does_not_move() {
        let base_source = bare_source();
        let base_source_identity = base_source.identity().clone();
        let base = WorkspaceDescriptor::builder()
            .dependencies(b"lock-v1".to_vec())
            .build(base_source, &HexIdentity)
            .expect("named");

        let other_source = bare_source();
        assert_eq!(other_source.identity(), &base_source_identity);
        let changed = WorkspaceDescriptor::builder()
            .dependencies(b"lock-v2".to_vec())
            .build(other_source, &HexIdentity)
            .expect("named");

        // The descriptor identity moved, but the source tree's own identity did not.
        assert_ne!(base.identity(), changed.identity());
        assert_eq!(base.source().identity(), &base_source_identity);
        assert_eq!(changed.source().identity(), &base_source_identity);
        assert_eq!(base.source(), changed.source());
    }

    // --- PartialEq discipline (ADR-0013) --------------------------------------------------

    #[test]
    fn partial_eq_never_consults_identity() {
        let left = WorkspaceDescriptor::builder()
            .dependencies(b"one".to_vec())
            .build(bare_source(), &ConstantIdentity)
            .expect("named");
        let right = WorkspaceDescriptor::builder()
            .dependencies(b"two".to_vec())
            .build(bare_source(), &ConstantIdentity)
            .expect("named");

        // The collision is total: every identity in both descriptors is the same name.
        assert_eq!(left.identity(), right.identity());
        assert_eq!(
            left.dependencies().map(Component::identity),
            right.dependencies().map(Component::identity)
        );

        // And nothing is conflated: content equality still tells them apart.
        assert_ne!(left, right);
        assert_ne!(left.dependencies(), right.dependencies());
    }

    #[test]
    fn identical_content_under_a_colliding_seam_is_still_equal() {
        let left = WorkspaceDescriptor::builder()
            .dependencies(b"same".to_vec())
            .build(bare_source(), &ConstantIdentity)
            .expect("named");
        let right = WorkspaceDescriptor::builder()
            .dependencies(b"same".to_vec())
            .build(bare_source(), &ConstantIdentity)
            .expect("named");
        assert_eq!(left, right);
        assert_eq!(left.identity(), right.identity());
    }

    // --- refusals ----------------------------------------------------------------------------

    #[test]
    fn a_seam_that_refuses_refuses_the_whole_descriptor() {
        assert_eq!(
            WorkspaceDescriptor::builder()
                .dependencies(b"x".to_vec())
                .build(bare_source(), &NoIdentity),
            Err(DescriptorError::IdentityUnavailable {
                kind: Some(ComponentKind::Dependencies)
            })
        );
    }

    #[test]
    fn a_seam_that_misnames_the_class_is_refused() {
        assert_eq!(
            WorkspaceDescriptor::builder()
                .toolchain(b"x".to_vec())
                .build(bare_source(), &WrongClassIdentity),
            Err(DescriptorError::IdentityClassMismatch {
                kind: Some(ComponentKind::Toolchain),
                class: ArtifactClass::Evidence,
            })
        );
    }

    // --- round trip --------------------------------------------------------------------------

    #[test]
    fn an_encoding_round_trips_to_the_same_identity() {
        for descriptor in [
            WorkspaceDescriptor::builder()
                .build(bare_source(), &HexIdentity)
                .expect("named"),
            WorkspaceDescriptor::builder()
                .dependencies(b"lock".to_vec())
                .toolchain(b"toolchain".to_vec())
                .configuration(b"config".to_vec())
                .build(source(&[("a", b""), ("b/c", b"x")]), &HexIdentity)
                .expect("named"),
        ] {
            let bytes = descriptor.encode();
            let decoded = WorkspaceDescriptor::decode(&bytes, &HexIdentity).expect("round trip");
            assert_eq!(decoded, descriptor);
            assert_eq!(decoded.identity(), descriptor.identity());
            assert_eq!(decoded.encode(), bytes);
        }
    }

    // --- malformed encodings ------------------------------------------------------------------

    #[test]
    fn malformed_encodings_are_typed_errors() {
        let good = WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .build(bare_source(), &HexIdentity)
            .expect("named")
            .encode();

        assert_eq!(
            WorkspaceDescriptor::decode(b"", &HexIdentity),
            Err(DescriptorDecodeError::UnexpectedEnd {
                at: 0,
                needed: DESCRIPTOR_MAGIC.len()
            })
        );
        assert_eq!(
            WorkspaceDescriptor::decode(b"nope!!\x01\0\0\0\0\0\0\0\0", &HexIdentity),
            Err(DescriptorDecodeError::Magic)
        );

        let mut wrong_version = good.clone();
        wrong_version[DESCRIPTOR_MAGIC.len()] = ENCODING_VERSION.wrapping_add(1);
        assert_eq!(
            WorkspaceDescriptor::decode(&wrong_version, &HexIdentity),
            Err(DescriptorDecodeError::Version {
                found: ENCODING_VERSION.wrapping_add(1)
            })
        );

        let mut trailing = good.clone();
        trailing.push(0x00);
        assert!(matches!(
            WorkspaceDescriptor::decode(&trailing, &HexIdentity),
            Err(DescriptorDecodeError::TrailingBytes { .. })
        ));

        for len in 0..good.len() {
            assert!(
                WorkspaceDescriptor::decode(&good[..len], &HexIdentity).is_err(),
                "a proper prefix of length {len} decoded"
            );
        }
    }
}
