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
//! A Merkle root is well defined only when a directory's children have exactly one order.
//! A *workspace* has exactly one Merkle root only when a name has exactly one spelling.
//! PR 3 / IMPL-01 fixed the first; PR 3 / IMPL-05 fixes the second, in "the admission
//! policy" below. The order is:
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
//! # The admission policy: which names are workspace paths (PR 3 / IMPL-05)
//!
//! > Snapshots are content-addressed and immutable […] re-derived artifacts receive new
//! > identities.
//! >
//! > — plan §4.2, `notes/plan/adr/0018-semantic-versioning-and-replay.md`
//!
//! The constitutional requirement is that the same workspace content has exactly one
//! snapshot identity, everywhere, forever. One rule decides every open question below,
//! and it decides them all the same way. Call it the **byte rule**:
//!
//! > A workspace path *is* its bytes. Nothing between the bytes an operating system
//! > reports and the bytes of a Merkle record may rewrite them, and which names are
//! > accepted may not be a function of anything that changes over time — not a Unicode
//! > version, not a locale, not a platform.
//!
//! The byte rule is not a preference for simplicity. Every rewrite refused below is
//! **many-to-one**: it maps two names the operating system distinguishes onto one path.
//! A many-to-one map is fatal twice over. It gives one identity two workspaces, so the
//! identity stops determining the content it names; and it forces the importer to pick a
//! winner between two files that collided, which is a choice made in *input order* —
//! exactly the nondeterminism a snapshot exists to remove.
//!
//! ## Unicode normalization: none
//!
//! Two paths that differ only in normalization form are **two distinct paths**. `café`
//! spelled NFC (`caf\u{e9}`) and NFD (`cafe\u{301}`) are two files, they order by their
//! bytes like any other two names, and neither is rewritten into the other.
//!
//! Beyond the byte rule, normalizing would make snapshot identity a function of *the
//! Unicode version the importer was built against*. Unicode's stability policy fixes the
//! canonical decomposition of an already-assigned character, but says nothing about code
//! points that are unassigned today: a name containing one normalizes to itself now and
//! may normalize to something else after the next Unicode release. A snapshot identity
//! that moves when a library is upgraded is not an identity, and INV-006 replay would
//! fail against artifacts nobody edited.
//!
//! An importer that wants normalized names normalizes *the working tree*, which is a
//! visible, reviewable change to the workspace, and then takes a snapshot of the result.
//!
//! ## Case: exact, never folded
//!
//! `A.rs` and `a.rs` are two files. The byte rule again, plus two reinforcements: case
//! folding is locale-dependent (Turkish `I`/`ı`) and Unicode-version-dependent, and full
//! folding is not even length-preserving (`ß` → `ss`), so it is many-to-one in the most
//! literal way. This matches [`artifact_path`](crate::artifact_path)'s case-sensitivity
//! requirement on the store volume, and for the same reason (ADR-0013).
//!
//! ## Names that are not UTF-8: rejected, typed
//!
//! [`WorkspacePath::from_bytes`] is the boundary an importer crosses, and a segment whose
//! bytes are not UTF-8 is [`PathError::SegmentEncoding`] — never escaped, never replaced,
//! never lossily decoded.
//!
//! Escaping was the alternative and it is refused on the schemas' authority, not on
//! taste. INV-003 makes `notes/plan/schemas/` normative for artifact shape, and
//! `workspace-snapshot.schema.json` types `files[].path` as a JSON string, which is
//! Unicode text by definition. A byte sequence that is not UTF-8 has no representation
//! there, so an escaped path would be a name whose *artifact* spells one thing and whose
//! *filesystem* spells another, with a decoder in between that every consumer — store,
//! diff, CLI, agent — would have to implement identically forever. One decoder that
//! disagrees is one workspace with two identities. Rejection has none of that surface:
//! the failure is loud, typed, and at the boundary, and the fix (rename the file) is in
//! the user's hands and visible in their working tree.
//!
//! Lossy decoding is refused for a sharper reason: `String::from_utf8_lossy` maps every
//! invalid sequence to `U+FFFD`, so two different unreadable names become one path. That
//! is the many-to-one failure exactly.
//!
//! ## Rejected characters
//!
//! [`PathError::SegmentCharacter`] covers ASCII control characters (including NUL), the
//! separator `/`, and the eight characters no ordinary Win32 path may contain:
//! `\ < > : " | ? *`. The rule is the byte rule's platform half: a name that a supported
//! platform cannot hold *at all* would make the workspace unmaterializable there, and a
//! workspace that exists on one platform and not another has two snapshots — docs/19 §7
//! makes the OS/architecture matrix a determinism dimension. `:` is also the
//! path-traversal rejection [`artifact_path`](crate::artifact_path) makes for drive-
//! relative paths (`c:x`), and `\` is the separator half of it (docs/19 §9, docs/09).
//!
//! Each rejected character is a *fixed code point*, and control characters are the fixed
//! `Cc` ranges `U+0000..=U+001F` and `U+007F..=U+009F`, which Unicode can never extend.
//! So the accepted set is not a function of the Unicode version — the byte rule's second
//! clause. That is also why bidirectional-override and other format (`Cf`) characters are
//! **accepted**: rejecting them would key the accepted set to a general category whose
//! membership grows with each Unicode release. A name that renders deceptively is a
//! display concern for whatever shows it, not a licence to make identity unstable.
//!
//! ## Reserved segments
//!
//! [`PathError::ReservedSegment`], with a [`ReservedReason`] naming which rule fired:
//!
//! - **`TrailingDot` / `TrailingSpace`.** Win32 strips a trailing `.` or space when it
//!   opens a path, so `a.` and `a` are one file there and two files on Linux. Writing out
//!   such a workspace and reading it back changes its identity, on a platform where
//!   nothing was edited.
//! - **`DeviceName`.** `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, matched
//!   against the stem before the first `.` (Win32 resolves `CON.txt` to the console too),
//!   under an **ASCII-only** uppercase fold. Opening one of these on Windows reaches a
//!   device rather than a file, so such a workspace cannot be materialized at all.
//!
//!   The fold looks like the case folding decision three headings up refuses, and is not:
//!   it decides *rejection* only, never identity, and it is ASCII-only, so it is fixed
//!   forever. No two accepted names are ever compared under it.
//!
//! Trailing dots and device names cannot be enabled by any volume option, which is what
//! separates them from case: see "what is required of the filesystem instead" below.
//!
//! ## Segment length
//!
//! At most [`MAX_SEGMENT_BYTES`] UTF-8 bytes ([`PathError::SegmentTooLong`]). 255 bytes
//! is `NAME_MAX` on the filesystems in the supported matrix, and since a UTF-8 sequence
//! is never shorter than its UTF-16 encoding in code units, 255 bytes also stays inside
//! NTFS's 255-unit bound. A longer segment names a file no supported filesystem can
//! hold. The *total* rendered length is deliberately not bounded here: it depends on the
//! workspace root's own prefix, which this module does not know and IMPL-02 does.
//!
//! ## Symbolic links: not content, at this layer
//!
//! A symlink is not file content; it is a name that refers to another name. Snapshotting
//! its target inlines a file that has its own path (so one byte string enters the tree
//! twice and a later edit desynchronizes the copies), and snapshotting the link's target
//! *string* stores a path that may point outside the workspace — the traversal
//! [`WorkspacePath`] rejects at construction, smuggled in as content.
//!
//! [`SymlinkPolicy`] is therefore the typed surface IMPL-02 must consult, and
//! [`SymlinkPolicy::Reject`] — a typed [`AdmissionError::Symlink`] naming the link and
//! its target — is the only arm this layer implements.
//! [`SymlinkPolicy::FollowWithCycleDetection`] is named because it is the only *other*
//! defensible arm and naming it keeps it from being invented ad hoc; selecting it today
//! is [`AdmissionError::UnsupportedSymlinkPolicy`], so an importer physically cannot
//! follow a link without coming back to this module. It would have to specify what a
//! cycle is, what a link escaping the workspace root means, and how the resulting
//! duplicate content is recorded — three decisions, not a flag.
//!
//! **Hard links need no policy.** A hard link is indistinguishable from a regular file,
//! two paths sharing an inode are simply two files with equal content, and equal content
//! already shares one identity and one record ([`Snapshot::records`]). Nothing to decide.
//!
//! **Everything else is refused.** A FIFO, socket, or device node has no content to
//! snapshot — reading one yields whatever was in flight, which is ambient nondeterminism
//! (INV-005) — so [`Candidate::Special`] is [`AdmissionError::Special`]. Directories are
//! not a candidate kind: a directory exists exactly when it holds a file, and whether an
//! *empty* directory is workspace content at all is still IMPL-02's question (see "seams
//! left open on purpose").
//!
//! ## What is required of the filesystem instead
//!
//! Exactly one requirement, and it is the one the store already makes: **a case-sensitive
//! volume**. A workspace holding both `A.rs` and `a.rs` is a legal snapshot and cannot be
//! written to a case-insensitive volume — precisely as
//! [`artifact_path`](crate::artifact_path) says of `ws_A` and `ws_a`. The line between
//! "declared requirement" and "typed rejection" is whether a *conforming volume exists*:
//! case sensitivity is a volume property one can choose (ext4, case-sensitive APFS, NTFS
//! with the flag set), while no volume option makes Win32 stop swallowing a trailing dot
//! or stop resolving `NUL`. (The `\\?\` extended-path form does bypass Win32's parser,
//! but it is a per-call spelling, not a property of the workspace, and the tools a user
//! points at their own checkout do not use it.)
//!
//! ## What reserves nothing, and why that is worth saying
//!
//! Neither the record grammar nor the store layout reserves a single name.
//!
//! - The directory record is length-prefixed (`u64(name_len) | name | …`), so it is
//!   injective over arbitrary segment bytes: no name can be confused with a delimiter,
//!   because there are no delimiters. A name-escaping scheme would be pure liability.
//! - Store paths are derived from *handles*
//!   ([`ArtifactPath::for_handle`](crate::artifact_path::ArtifactPath::for_handle)), and a
//!   handle's identity charset is `[A-Za-z0-9_-]`. No workspace path segment ever appears
//!   in a store path, so no segment can collide with one.
//!
//! Every rejection above is therefore a *materialization* rejection, and IMPL-02 should
//! read it that way: the tree does not need these rules, round-tripping a workspace
//! through a filesystem does.
//!
//! ## Why [`ENCODING_VERSION`] does not move
//!
//! The grammar, the byte layout, and the order are all unchanged; the accepted set
//! narrowed. Every snapshot that is still representable keeps exactly the identity it
//! had, so bumping the version would re-derive every artifact in the store to say
//! nothing. A name this policy now refuses could never have been imported deterministically
//! in the first place, which is why narrowing is not a semantic change (docs/12 §1,
//! GOV-1-09).
//!
//! ## The entry point
//!
//! [`admit`] — pure, total, and order-independent in both directions. It takes candidate
//! `(name bytes, `[`Candidate`]`)` pairs in any order and returns either the canonical
//! ordered [`WorkspaceContent`] or the *first* typed rejection, where "first" is by the
//! candidate's raw name bytes and not by the order the importer happened to walk the
//! disk in. A rejection that depended on walk order would leave two importers of one
//! workspace disagreeing about *why* it was refused, which is the same failure as
//! disagreeing about its identity, one level up.
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
//! - **Deterministic file ordering is IMPL-05, and is here.** See "the ordering contract"
//!   and "the admission policy" above; [`admit`] is the entry point an importer uses.
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

/// The longest path segment this module accepts, in UTF-8 bytes.
///
/// `NAME_MAX` on every filesystem in docs/19 §7's supported matrix: 255 bytes on ext4,
/// APFS, and XFS, and 255 UTF-16 code units on NTFS — which this bound also respects,
/// because a character's UTF-16 encoding is never *longer* in code units than its UTF-8
/// encoding is in bytes. A longer segment names a file no supported filesystem can hold,
/// so a workspace containing one could not be written out and read back unchanged.
///
/// The *rendered path* length is deliberately not bounded here: it depends on the
/// workspace root's own prefix, which this module does not know (PR 3 / IMPL-02 does).
pub const MAX_SEGMENT_BYTES: usize = 255;

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

/// Which reservation rule a segment ran into ([`PathError::ReservedSegment`]).
///
/// Every variant is a name a supported platform *mangles or diverts* rather than stores,
/// so accepting it would let one workspace round-trip through a filesystem into a
/// different workspace. See the module documentation's "reserved segments".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReservedReason {
    /// A Win32 device name: `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`,
    /// matched against the stem before the first `.` under an ASCII-only uppercase fold.
    ///
    /// Opening one of these on Windows reaches a device, not a file. The fold decides
    /// rejection only and never identity: two *accepted* names are never compared under
    /// it, and being ASCII-only it is fixed forever.
    DeviceName,
    /// The segment ends in `.`, which Win32 strips when it opens a path.
    TrailingDot,
    /// The segment ends in a space, which Win32 strips when it opens a path.
    TrailingSpace,
}

impl ReservedReason {
    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeviceName => "device-name",
            Self::TrailingDot => "trailing-dot",
            Self::TrailingSpace => "trailing-space",
        }
    }

    /// Why the platform cannot store a segment under this rule, in one clause.
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::DeviceName => "names a Win32 device rather than a file",
            Self::TrailingDot => "ends in `.`, which Win32 strips when opening a path",
            Self::TrailingSpace => "ends in a space, which Win32 strips when opening a path",
        }
    }
}

impl fmt::Display for ReservedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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
    /// Rejected: ASCII control characters including NUL, and the eight characters no
    /// ordinary Win32 path may hold — `\ < > : " | ? *`. The backslash is not forbidden
    /// because it is rare — it is forbidden because a workspace whose paths order and
    /// nest differently on two platforms has two Merkle trees, and docs/19 §7 makes the
    /// OS/architecture matrix a determinism dimension. `:` carries a second reason: it
    /// spells a drive-relative Win32 path, which is the traversal
    /// [`artifact_path`](crate::artifact_path) rejects for the same handle (docs/19 §9,
    /// docs/09). `/` reaches here only from [`WorkspacePath::from_segments`], where a
    /// separator inside an explicit segment would silently deepen the tree.
    ///
    /// Each rejected character is a fixed code point and the control range is Unicode's
    /// fixed `Cc` block, so the accepted set is not a function of the Unicode version —
    /// which is also why format (`Cf`) characters are accepted. See the module
    /// documentation's "rejected characters".
    SegmentCharacter {
        /// Index of the offending segment.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// A segment's bytes are not UTF-8 ([`WorkspacePath::from_bytes`]).
    ///
    /// Refused rather than escaped or lossily decoded: `files[].path` is a JSON string in
    /// `notes/plan/schemas/workspace-snapshot.schema.json`, which INV-003 makes normative,
    /// so a non-Unicode name has no representation in the artifact that would name it —
    /// and a lossy decode maps every unreadable name onto `U+FFFD`, merging distinct
    /// files. See the module documentation's "names that are not UTF-8".
    SegmentEncoding {
        /// Index of the offending segment.
        index: usize,
        /// Byte offset of the first invalid byte, within the segment.
        offset: usize,
        /// The first invalid byte.
        byte: u8,
    },
    /// A segment is a name a supported platform mangles or diverts rather than stores.
    ReservedSegment {
        /// Index of the offending segment.
        index: usize,
        /// The segment, as written.
        segment: String,
        /// Which reservation rule fired.
        reason: ReservedReason,
    },
    /// A segment is longer than [`MAX_SEGMENT_BYTES`] UTF-8 bytes.
    SegmentTooLong {
        /// Index of the offending segment.
        index: usize,
        /// The segment's length in UTF-8 bytes.
        bytes: usize,
        /// The bound, [`MAX_SEGMENT_BYTES`].
        max: usize,
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
            Self::SegmentEncoding {
                index,
                offset,
                byte,
            } => write!(
                f,
                "workspace path segment {index} is not UTF-8: byte {byte:#04x} at offset \
                 {offset}. A workspace path is Unicode text or it is refused; it is never \
                 escaped or lossily decoded"
            ),
            Self::ReservedSegment {
                index,
                segment,
                reason,
            } => write!(
                f,
                "workspace path segment {index} is {segment:?}, which {}",
                reason.explanation()
            ),
            Self::SegmentTooLong { index, bytes, max } => write!(
                f,
                "workspace path segment {index} is {bytes} bytes, past the {max}-byte bound"
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

/// The separator as one byte, for splitting raw operating-system bytes.
///
/// Splitting bytes on `0x2f` agrees with splitting text on `/` because `/` is ASCII and
/// UTF-8 never uses a byte below `0x80` inside a multi-byte sequence. An *overlong*
/// encoding of `/` (`0xc0 0xaf`) is therefore not a separator here — it is invalid UTF-8,
/// and [`PathError::SegmentEncoding`] refuses it before it can be mistaken for one.
const SEPARATOR_BYTE: u8 = b'/';

/// Characters no ordinary Win32 path may contain, `\` included.
///
/// Fixed code points, deliberately: see [`PathError::SegmentCharacter`].
const NON_PORTABLE_CHARACTERS: [char; 8] = ['\\', '<', '>', ':', '"', '|', '?', '*'];

/// Win32 device names, compared against a segment's stem under an ASCII-only fold.
///
/// The historical DOS device set, which every Windows release still resolves ahead of the
/// filesystem. Spelled in one fixed array so the reserved set is auditable in one glance
/// and cannot drift.
const RESERVED_DEVICE_STEMS: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

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

    /// Parse a `/`-separated path from the raw bytes an operating system reported.
    ///
    /// The byte-level boundary of the admission policy, and the only place a name that is
    /// not Unicode text can be met. Pure: the result is a function of `raw` alone.
    ///
    /// Injective on the names it accepts — no normalization, no folding, no separator
    /// collapsing — so two distinct accepted byte strings are two distinct paths, and
    /// `path.to_string().as_bytes()` is `raw` again. That is what makes the constitutional
    /// requirement provable rather than hoped for: one workspace, one identity, because
    /// one name has one spelling.
    ///
    /// # Errors
    ///
    /// [`PathError::SegmentEncoding`] for a segment that is not UTF-8, and otherwise every
    /// [`PathError`] [`WorkspacePath::new`] reports.
    ///
    /// ```
    /// use continuum_workspace::snapshot::{PathError, WorkspacePath};
    ///
    /// let path = WorkspacePath::from_bytes(b"src/lib.rs")?;
    /// assert_eq!(path.to_string(), "src/lib.rs");
    ///
    /// // An overlong encoding of `/` is not a separator; it is not UTF-8 at all.
    /// assert!(matches!(
    ///     WorkspacePath::from_bytes(b"a\xc0\xafb"),
    ///     Err(PathError::SegmentEncoding { index: 0, .. }),
    /// ));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn from_bytes(raw: &[u8]) -> Result<Self, PathError> {
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        // Counted before anything is allocated, so an adversarially deep name costs a
        // scan rather than one `String` per segment.
        let depth = raw
            .iter()
            .filter(|byte| **byte == SEPARATOR_BYTE)
            .count()
            .saturating_add(1);
        if depth > MAX_DEPTH {
            return Err(PathError::TooDeep {
                depth,
                max: MAX_DEPTH,
            });
        }
        let mut segments: Vec<String> = Vec::with_capacity(depth);
        for (index, chunk) in raw.split(|byte| *byte == SEPARATOR_BYTE).enumerate() {
            let text = core::str::from_utf8(chunk).map_err(|error| {
                let offset = error.valid_up_to();
                PathError::SegmentEncoding {
                    index,
                    offset,
                    byte: chunk.get(offset).copied().unwrap_or_default(),
                }
            })?;
            segments.push(text.to_owned());
        }
        Self::from_segments(segments)
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
///
/// The checks run in a fixed order — empty, relative, character, reserved, length — so a
/// segment with two defects always reports the same one. "First violation" has to be a
/// function of the segment and of nothing else, for the same reason [`admit`]'s "first
/// rejection" does.
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
    if let Some(character) = segment.chars().find(|c| !is_portable_character(*c)) {
        return Err(PathError::SegmentCharacter { index, character });
    }
    if let Some(reason) = reserved_reason(segment) {
        return Err(PathError::ReservedSegment {
            index,
            segment: segment.to_owned(),
            reason,
        });
    }
    if segment.len() > MAX_SEGMENT_BYTES {
        return Err(PathError::SegmentTooLong {
            index,
            bytes: segment.len(),
            max: MAX_SEGMENT_BYTES,
        });
    }
    Ok(())
}

/// Whether a character may appear in a segment.
///
/// A predicate over fixed code points: Unicode's `Cc` block never grows, and the eight
/// non-portable characters are literals. Nothing here consults a Unicode general category
/// that a future release could extend, because the set of accepted names must not be a
/// function of which Unicode version an importer was built against.
fn is_portable_character(character: char) -> bool {
    !character.is_control()
        && character != SEPARATOR
        && !NON_PORTABLE_CHARACTERS.contains(&character)
}

/// Which reservation rule a segment runs into, if any.
///
/// The ASCII-only fold decides rejection and never identity: no two *accepted* segments
/// are ever compared through it, so `README.md` and `readme.md` remain two files.
fn reserved_reason(segment: &str) -> Option<ReservedReason> {
    if segment.ends_with('.') {
        return Some(ReservedReason::TrailingDot);
    }
    if segment.ends_with(' ') {
        return Some(ReservedReason::TrailingSpace);
    }
    // Win32 resolves `CON.txt` to the console as readily as `CON`, so the stem before the
    // first `.` is what has to be compared.
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED_DEVICE_STEMS
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        .then_some(ReservedReason::DeviceName)
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

// --- admission: the importer's boundary (PR 3 / IMPL-05) ----------------------------------

/// What an importer found at one candidate path.
///
/// Deliberately not a filesystem type: it is the *content model's* reading of one. A
/// regular file is content; a symbolic link is a name, not content; everything else has
/// no content to snapshot at all. Directories are absent because a directory exists
/// exactly when it holds a file, and whether an empty one is workspace content remains
/// PR 3 / IMPL-02's question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Candidate<'a> {
    /// A regular file, with its content.
    File(&'a [u8]),
    /// A symbolic link, with the bytes of its target exactly as the operating system
    /// reported them — unresolved, and not assumed to be UTF-8.
    Symlink(&'a [u8]),
    /// A FIFO, socket, device node, or anything else that is neither of the above.
    ///
    /// Reading one does not yield content; it yields whatever happened to be in flight,
    /// which is ambient nondeterminism (INV-005) wearing a path.
    Special,
}

/// What an importer does when it meets a symbolic link.
///
/// A policy *axis*, named so that it cannot be decided ad hoc twice. Exactly one arm is
/// implemented, so the axis adds no nondeterminism today: see
/// [`SymlinkPolicy::is_implemented`] and the module documentation's "symbolic links".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymlinkPolicy {
    /// Refuse the workspace, naming the link and its target
    /// ([`AdmissionError::Symlink`]). The default, and the only implemented arm.
    #[default]
    Reject,
    /// Resolve links to the files they name, refusing a cycle.
    ///
    /// Named because it is the only other defensible arm and naming it keeps it from
    /// being invented ad hoc; **not implemented**. Selecting it is
    /// [`AdmissionError::UnsupportedSymlinkPolicy`], because implementing it means
    /// answering three questions this layer has not answered — what a cycle is, what a
    /// link escaping the workspace root means, and how the duplicated content is
    /// recorded — and a flag is not an answer to any of them.
    FollowWithCycleDetection,
}

impl SymlinkPolicy {
    /// Whether this arm is implemented. Only [`SymlinkPolicy::Reject`] is.
    #[must_use]
    pub const fn is_implemented(self) -> bool {
        matches!(self, Self::Reject)
    }

    /// The stable machine name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::FollowWithCycleDetection => "follow-with-cycle-detection",
        }
    }
}

impl fmt::Display for SymlinkPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The policy [`admit`] applies, as an explicit value rather than an ambient default.
///
/// A policy that varies between two importers of one workspace is a workspace with two
/// identities, so this type exists to make the axis *visible* and to keep the set of
/// implementable values a singleton until a decision widens it. When PR 3 / IMPL-03
/// records the components a snapshot pins, the policy in force belongs beside them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdmissionPolicy {
    symlinks: SymlinkPolicy,
}

impl AdmissionPolicy {
    /// The policy every implemented arm agrees on: reject symbolic links.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            symlinks: SymlinkPolicy::Reject,
        }
    }

    /// Choose the symbolic-link arm.
    #[must_use]
    pub const fn with_symlinks(self, symlinks: SymlinkPolicy) -> Self {
        Self { symlinks }
    }

    /// The symbolic-link arm in force.
    #[must_use]
    pub const fn symlinks(self) -> SymlinkPolicy {
        self.symlinks
    }
}

/// Why a set of candidates is not a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// A candidate's name is not a workspace path.
    Name {
        /// The raw name, as the operating system reported it.
        name: Vec<u8>,
        /// Why it was refused.
        error: PathError,
    },
    /// A candidate is a symbolic link and the policy is [`SymlinkPolicy::Reject`].
    Symlink {
        /// The link's raw name.
        name: Vec<u8>,
        /// The link's raw target.
        target: Vec<u8>,
    },
    /// A candidate is neither a regular file nor a symbolic link.
    Special {
        /// The entry's raw name.
        name: Vec<u8>,
    },
    /// The requested symbolic-link arm is not implemented.
    UnsupportedSymlinkPolicy {
        /// The arm that was asked for.
        policy: SymlinkPolicy,
    },
    /// The candidates name paths that are not a tree: a repeat, or a file another
    /// candidate needs as a directory.
    NotATree(SnapshotError),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { name, error } => {
                write!(f, "candidate `{}`: {error}", render_name(name))
            }
            Self::Symlink { name, target } => write!(
                f,
                "candidate `{}` is a symbolic link to `{}`; a link is a name, not content, \
                 and this layer stores content",
                render_name(name),
                render_name(target)
            ),
            Self::Special { name } => write!(
                f,
                "candidate `{}` is neither a regular file nor a symbolic link, so it has \
                 no content to snapshot",
                render_name(name)
            ),
            Self::UnsupportedSymlinkPolicy { policy } => write!(
                f,
                "symbolic-link policy `{policy}` is not implemented; only `{}` is",
                SymlinkPolicy::Reject
            ),
            Self::NotATree(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for AdmissionError {}

impl From<SnapshotError> for AdmissionError {
    fn from(error: SnapshotError) -> Self {
        Self::NotATree(error)
    }
}

/// Render a raw name for a message only.
///
/// Lossy on purpose and *only* here: a message is prose, and the name it quotes has
/// already been refused. No accepted path is ever produced this way — that is
/// [`WorkspacePath::from_bytes`], which refuses what this function would flatten.
fn render_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

/// Validate and order candidate entries: the one entry point an importer needs.
///
/// Takes `(raw name, `[`Candidate`]`)` pairs in any order and returns the canonical
/// ordered [`WorkspaceContent`] — the input [`Snapshot::build`] wants — or the first
/// typed rejection.
///
/// Pure, and order-independent **in both directions**. Acceptance is order-independent
/// because [`WorkspaceContent`] is a [`BTreeMap`]; rejection is order-independent because
/// candidates are examined in the order of their *raw name bytes*, which is a total order
/// defined on every input including the ones that are not paths at all. Walking a
/// directory yields entries in whatever order the filesystem felt like, so a rejection
/// that depended on that order would have two importers of one workspace disagreeing
/// about why it was refused — the same failure as disagreeing about its identity, one
/// level up (INV-005, docs/19 §7).
///
/// The examination order is fixed and worth stating, because "first" has to mean
/// something:
///
/// 1. the policy itself, before any candidate is read;
/// 2. every candidate in raw-name order — name first, then kind;
/// 3. every accepted path in *path* order, where a repeat or a file-versus-directory
///    conflict is found. Path order is not raw-name order (`a.b` and `a/b` disagree), so
///    this pass is sorted again rather than reusing the first pass's order.
///
/// # Errors
///
/// [`AdmissionError`]: an unimplemented policy, a name that is not a workspace path, a
/// symbolic link, an entry that is not a regular file, or candidates that are not a tree.
///
/// # Example
///
/// ```
/// use continuum_workspace::snapshot::{admit, AdmissionPolicy, Candidate};
///
/// // Whatever order the walk produced.
/// let found = [
///     (b"src/lib.rs".as_slice(), Candidate::File(b"fn main() {}")),
///     (b"README.md".as_slice(), Candidate::File(b"hello")),
/// ];
/// let content = admit(found, AdmissionPolicy::new())?;
/// assert_eq!(
///     content.paths().map(ToString::to_string).collect::<Vec<_>>(),
///     ["README.md", "src/lib.rs"],
/// );
///
/// // Reversed, it is the same description — and therefore the same snapshot.
/// let mut reversed = found;
/// reversed.reverse();
/// assert_eq!(admit(reversed, AdmissionPolicy::new())?, content);
/// # Ok::<(), continuum_workspace::snapshot::AdmissionError>(())
/// ```
pub fn admit<'a, I>(
    candidates: I,
    policy: AdmissionPolicy,
) -> Result<WorkspaceContent, AdmissionError>
where
    I: IntoIterator<Item = (&'a [u8], Candidate<'a>)>,
{
    if !policy.symlinks().is_implemented() {
        return Err(AdmissionError::UnsupportedSymlinkPolicy {
            policy: policy.symlinks(),
        });
    }

    let mut found: Vec<(&[u8], Candidate<'_>)> = candidates.into_iter().collect();
    found.sort_by(|left, right| left.0.cmp(right.0));

    let mut admitted: Vec<(WorkspacePath, &[u8])> = Vec::with_capacity(found.len());
    for (name, candidate) in found {
        let path = WorkspacePath::from_bytes(name).map_err(|error| AdmissionError::Name {
            name: name.to_vec(),
            error,
        })?;
        match candidate {
            Candidate::File(content) => admitted.push((path, content)),
            Candidate::Symlink(target) => {
                return Err(AdmissionError::Symlink {
                    name: name.to_vec(),
                    target: target.to_vec(),
                });
            }
            Candidate::Special => {
                return Err(AdmissionError::Special {
                    name: name.to_vec(),
                });
            }
        }
    }

    admitted.sort_by(|left, right| left.0.cmp(&right.0));
    let mut content = WorkspaceContent::new();
    for (path, bytes) in admitted {
        content.insert(path, bytes.to_vec())?;
    }
    Ok(content)
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

    // --- the admission policy (PR 3 / IMPL-05) --------------------------------------------

    /// A deterministic xorshift. Every permutation these tests try is a function of a
    /// written-down seed and of nothing else; INV-005 does not stop at `src/`.
    struct Shuffle(u64);

    impl Shuffle {
        const fn new(seed: u64) -> Self {
            // Zero is xorshift's fixed point, and a test whose corpus never moves is a
            // test that proves nothing.
            Self(seed | 1)
        }

        fn step(&mut self) -> u64 {
            let mut state = self.0;
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            self.0 = state;
            state
        }

        fn below(&mut self, bound: usize) -> usize {
            let bound = u64::try_from(bound).unwrap_or(1).max(1);
            usize::try_from(self.step() % bound).unwrap_or(0)
        }

        fn permute<T>(&mut self, items: &mut [T]) {
            for index in (1..items.len()).rev() {
                let other = self.below(index + 1);
                items.swap(index, other);
            }
        }
    }

    /// `café` in NFC: one precomposed `é`.
    const NFC: &str = "caf\u{e9}";
    /// `café` in NFD: `e` followed by a combining acute accent.
    const NFD: &str = "cafe\u{301}";

    #[test]
    fn the_separator_byte_agrees_with_the_separator_character() {
        // `from_bytes` splits on the byte and `Display` joins on the character; if these
        // two ever disagreed, a path would render as something that does not reparse.
        let mut buffer = [0u8; 4];
        assert_eq!(
            SEPARATOR.encode_utf8(&mut buffer).as_bytes(),
            [SEPARATOR_BYTE]
        );
    }

    #[test]
    fn normalization_forms_are_two_distinct_paths() {
        // The decision: none. Two spellings of one grapheme cluster are two names, they
        // order by their bytes, and neither is rewritten into the other.
        assert_ne!(NFC, NFD);
        assert_ne!(path(NFC), path(NFD));
        assert_ne!(
            WorkspacePath::from_bytes(NFC.as_bytes()),
            WorkspacePath::from_bytes(NFD.as_bytes())
        );

        // Both are admissible, in one workspace, as two files.
        let content = admit(
            [
                (NFC.as_bytes(), Candidate::File(b"precomposed")),
                (NFD.as_bytes(), Candidate::File(b"decomposed")),
            ],
            AdmissionPolicy::new(),
        )
        .expect("two names, two files");
        assert_eq!(content.len(), 2);
        assert_eq!(content.get(&path(NFC)), Some(b"precomposed".as_slice()));
        assert_eq!(content.get(&path(NFD)), Some(b"decomposed".as_slice()));

        // And they are two snapshots, not one: the identity follows the bytes.
        assert_ne!(
            snapshot(&[(NFC, b"same")]).identity(),
            snapshot(&[(NFD, b"same")]).identity()
        );
    }

    #[test]
    fn case_pairs_are_distinct_however_they_would_fold() {
        // Simple folding.
        assert_ne!(path("A.rs"), path("a.rs"));
        // Locale-dependent folding: Turkish maps `I` to `ı`, every other locale to `i`.
        assert_ne!(path("\u{131}.rs"), path("i.rs"));
        assert_ne!(path("I.rs"), path("\u{130}.rs"));
        // Full folding is not even length-preserving: `ß` folds to `ss`.
        assert_ne!(path("stra\u{df}e"), path("strasse"));
        // Three distinct files, in one workspace, under one directory.
        let content = admit(
            [
                ("A.rs".as_bytes(), Candidate::File(b"upper")),
                ("a.rs".as_bytes(), Candidate::File(b"lower")),
                ("stra\u{df}e".as_bytes(), Candidate::File(b"sharp s")),
            ],
            AdmissionPolicy::new(),
        )
        .expect("three names, three files");
        assert_eq!(content.len(), 3);
    }

    #[test]
    fn bytes_that_are_not_utf8_are_a_typed_rejection() {
        // (raw name, offset of the first invalid byte, that byte)
        let cases: [(&[u8], usize, u8); 5] = [
            // An overlong encoding of `/`: the classic separator smuggle.
            (b"a\xc0\xafb", 1, 0xc0),
            // A lone surrogate half, which UTF-8 may not encode at all.
            (b"\xed\xa0\x80", 0, 0xed),
            // A truncated three-byte sequence.
            (b"ab\xe2\x82", 2, 0xe2),
            // A bare continuation byte.
            (b"\x80x", 0, 0x80),
            // A byte that begins no sequence.
            (b"x\xff", 1, 0xff),
        ];
        for (raw, offset, byte) in cases {
            assert_eq!(
                WorkspacePath::from_bytes(raw),
                Err(PathError::SegmentEncoding {
                    index: 0,
                    offset,
                    byte
                }),
                "{raw:?} was not refused as expected"
            );
            assert_eq!(
                admit([(raw, Candidate::File(b""))], AdmissionPolicy::new()),
                Err(AdmissionError::Name {
                    name: raw.to_vec(),
                    error: PathError::SegmentEncoding {
                        index: 0,
                        offset,
                        byte
                    }
                })
            );
        }

        // The rejection is per segment, and the index says which.
        assert_eq!(
            WorkspacePath::from_bytes(b"ok/\xff"),
            Err(PathError::SegmentEncoding {
                index: 1,
                offset: 0,
                byte: 0xff
            })
        );
    }

    #[test]
    fn reserved_segments_are_typed_rejections() {
        let cases: [(&str, ReservedReason); 8] = [
            ("CON", ReservedReason::DeviceName),
            ("con", ReservedReason::DeviceName),
            ("CoN.txt", ReservedReason::DeviceName),
            ("NUL", ReservedReason::DeviceName),
            ("com1", ReservedReason::DeviceName),
            ("LPT9.tar.gz", ReservedReason::DeviceName),
            ("trailing.", ReservedReason::TrailingDot),
            ("trailing ", ReservedReason::TrailingSpace),
        ];
        for (segment, reason) in cases {
            assert_eq!(
                WorkspacePath::new(segment),
                Err(PathError::ReservedSegment {
                    index: 0,
                    segment: segment.to_owned(),
                    reason
                }),
                "{segment:?} was accepted"
            );
            // At depth, too: every segment is a name the platform has to store.
            assert_eq!(
                WorkspacePath::new(&format!("src/{segment}")),
                Err(PathError::ReservedSegment {
                    index: 1,
                    segment: segment.to_owned(),
                    reason
                })
            );
        }
    }

    #[test]
    fn names_near_the_reserved_set_are_still_accepted() {
        // The set is exactly the device names, matched on the stem. Nothing wider: a
        // reservation that swallowed ordinary names would be its own failure.
        for segment in [
            "CONS",
            "console.txt",
            "COM0",
            "COM10",
            "LPT",
            "a.CON",
            "con-fig",
            ".con",
            "concon",
            ".hidden",
            "a.b.c",
        ] {
            assert!(
                WorkspacePath::new(segment).is_ok(),
                "{segment:?} was refused"
            );
        }
    }

    #[test]
    fn non_portable_characters_are_typed_rejections() {
        for character in NON_PORTABLE_CHARACTERS {
            let segment = format!("a{character}b");
            assert_eq!(
                WorkspacePath::new(&segment),
                Err(PathError::SegmentCharacter {
                    index: 0,
                    character
                }),
                "{segment:?} was accepted"
            );
        }
        // Control characters, at both ends of the two `Cc` ranges.
        for character in ['\u{0}', '\u{1f}', '\u{7f}', '\u{9f}'] {
            assert_eq!(
                WorkspacePath::new(&format!("a{character}b")),
                Err(PathError::SegmentCharacter {
                    index: 0,
                    character
                })
            );
        }
        // Format characters are *accepted*: rejecting them would key the accepted set to
        // a Unicode general category whose membership grows with every release.
        for character in ['\u{200e}', '\u{202e}', '\u{200b}', '\u{feff}'] {
            assert!(
                WorkspacePath::new(&format!("a{character}b")).is_ok(),
                "U+{:04X} was refused",
                u32::from(character)
            );
        }
    }

    #[test]
    fn a_segment_past_the_byte_bound_is_a_typed_rejection() {
        let at_bound = "x".repeat(MAX_SEGMENT_BYTES);
        assert!(WorkspacePath::new(&at_bound).is_ok());
        let past_bound = "x".repeat(MAX_SEGMENT_BYTES + 1);
        assert_eq!(
            WorkspacePath::new(&past_bound),
            Err(PathError::SegmentTooLong {
                index: 0,
                bytes: MAX_SEGMENT_BYTES + 1,
                max: MAX_SEGMENT_BYTES
            })
        );
        // The bound is bytes, not characters: `é` is two bytes, and a filesystem counts
        // the same two.
        let multibyte = "\u{e9}".repeat(MAX_SEGMENT_BYTES / 2 + 1);
        assert_eq!(
            WorkspacePath::new(&multibyte),
            Err(PathError::SegmentTooLong {
                index: 0,
                bytes: multibyte.len(),
                max: MAX_SEGMENT_BYTES
            })
        );
    }

    #[test]
    fn parsing_bytes_is_injective_and_round_trips() {
        // No normalization, no folding, no separator collapsing, so the map from accepted
        // byte strings to paths is injective — and its inverse is `Display`.
        let names: [&[u8]; 10] = [
            b"a",
            b"a/b",
            b"a.b",
            b"A",
            NFC.as_bytes(),
            NFD.as_bytes(),
            b"stra\xc3\x9fe",
            b"strasse",
            "\u{202e}gpj.exe".as_bytes(),
            b"a/b/c",
        ];
        let mut seen: BTreeSet<WorkspacePath> = BTreeSet::new();
        for raw in names {
            let parsed = WorkspacePath::from_bytes(raw).expect("accepted");
            assert_eq!(parsed.to_string().as_bytes(), raw);
            assert_eq!(WorkspacePath::from_bytes(raw), Ok(parsed.clone()));
            assert!(seen.insert(parsed), "{raw:?} collided with another name");
        }
        assert_eq!(seen.len(), names.len());

        // And the byte parser agrees with the text parser wherever both accept.
        for raw in names {
            let text = core::str::from_utf8(raw).expect("these are all UTF-8");
            assert_eq!(WorkspacePath::from_bytes(raw), WorkspacePath::new(text));
        }
    }

    #[test]
    fn parsing_bytes_reports_the_same_refusals_as_parsing_text() {
        assert_eq!(WorkspacePath::from_bytes(b""), Err(PathError::Empty));
        assert_eq!(
            WorkspacePath::from_bytes(b"/a"),
            Err(PathError::EmptySegment { index: 0 })
        );
        assert_eq!(
            WorkspacePath::from_bytes(b"a/../b"),
            Err(PathError::RelativeSegment {
                index: 1,
                segment: "..".to_owned()
            })
        );
        assert_eq!(
            WorkspacePath::from_bytes(b"a\\b"),
            Err(PathError::SegmentCharacter {
                index: 0,
                character: '\\'
            })
        );
        // Deep enough to be refused before a single segment is allocated.
        let deep = ["x"; MAX_DEPTH * 2].join("/");
        assert_eq!(
            WorkspacePath::from_bytes(deep.as_bytes()),
            Err(PathError::TooDeep {
                depth: MAX_DEPTH * 2,
                max: MAX_DEPTH
            })
        );
    }

    #[test]
    fn path_order_is_a_total_order() {
        // Reflexive, antisymmetric, transitive, and total, over exactly the cases that
        // make a naive implementation wrong: the segment/string divergence, prefixes,
        // case pairs, and normalization pairs.
        let corpus: Vec<WorkspacePath> = [
            "a",
            "a.b",
            "a/b",
            "a/b/c",
            "a/bb",
            "ab",
            "A",
            "A/b",
            NFC,
            NFD,
            "stra\u{df}e",
            "strasse",
            "z",
            "\u{202e}z",
        ]
        .into_iter()
        .map(path)
        .collect();

        for left in &corpus {
            assert_eq!(left.cmp(left), core::cmp::Ordering::Equal);
            for right in &corpus {
                // Totality and antisymmetry.
                assert_eq!(
                    left.cmp(right),
                    right.cmp(left).reverse(),
                    "{left} vs {right}"
                );
                assert_eq!(left == right, left.cmp(right) == core::cmp::Ordering::Equal);
                for third in &corpus {
                    if left <= right && right <= third {
                        assert!(left <= third, "{left} <= {right} <= {third}");
                    }
                }
            }
        }

        // The divergence the module documentation names, pinned once more: string order
        // and segment order disagree, and only segment order agrees with the tree.
        assert!("a.b" < "a/b");
        assert!(path("a/b") < path("a.b"));
        // A path sorts immediately before its own descendants, which is what
        // `WorkspaceContent`'s conflict check relies on.
        assert!(path("a") < path("a/b"));
        assert!(path("a/b") < path("a/bb"));
    }

    // --- admission ------------------------------------------------------------------------

    fn files<'a>(names: &[&'a str]) -> Vec<(&'a [u8], Candidate<'a>)> {
        names
            .iter()
            .map(|name| (name.as_bytes(), Candidate::File(b"x")))
            .collect()
    }

    #[test]
    fn admission_yields_the_canonical_order() {
        let content = admit(
            files(&["src/lib.rs", "README.md", "src/a.rs", "a.rs"]),
            AdmissionPolicy::new(),
        )
        .expect("a tree");
        assert_eq!(
            content.paths().map(ToString::to_string).collect::<Vec<_>>(),
            ["README.md", "a.rs", "src/a.rs", "src/lib.rs"]
        );
    }

    #[test]
    fn admission_is_invariant_under_permutation() {
        // Acceptance *and* rejection: a walk order that changed either would be a
        // workspace with two answers.
        let accepted = files(&[
            "Cargo.toml",
            "a.b",
            "a/b",
            "a/b/c",
            NFC,
            NFD,
            "src/lib.rs",
            "z",
        ]);
        let refused = {
            let mut refused = files(&["ok", "src/lib.rs", "zz"]);
            refused.push((b"z\\bad".as_slice(), Candidate::File(b"x")));
            refused.push((b"a\\bad".as_slice(), Candidate::File(b"x")));
            refused
        };

        for corpus in [accepted, refused] {
            let expected = admit(corpus.clone(), AdmissionPolicy::new());
            let mut shuffle = Shuffle::new(0x9e37_79b9_7f4a_7c15);
            for _ in 0..64 {
                let mut permuted = corpus.clone();
                shuffle.permute(&mut permuted);
                assert_eq!(admit(permuted, AdmissionPolicy::new()), expected);
            }
        }
    }

    #[test]
    fn admission_reports_the_least_named_rejection() {
        // Two bad names: the answer is the one that is least in raw-name order, never the
        // one the walk happened to reach first.
        let corpus: Vec<(&[u8], Candidate<'_>)> = vec![
            (b"z\\bad", Candidate::File(b"x")),
            (b"a\\bad", Candidate::File(b"x")),
        ];
        let expected = Err(AdmissionError::Name {
            name: b"a\\bad".to_vec(),
            error: PathError::SegmentCharacter {
                index: 0,
                character: '\\',
            },
        });
        assert_eq!(admit(corpus.clone(), AdmissionPolicy::new()), expected);
        let mut reversed = corpus;
        reversed.reverse();
        assert_eq!(admit(reversed, AdmissionPolicy::new()), expected);
    }

    #[test]
    fn admission_refuses_a_symbolic_link() {
        let corpus: Vec<(&[u8], Candidate<'_>)> = vec![
            (b"src/lib.rs", Candidate::File(b"fn main() {}")),
            (b"src/link.rs", Candidate::Symlink(b"../../etc/passwd")),
        ];
        assert_eq!(
            admit(corpus, AdmissionPolicy::new()),
            Err(AdmissionError::Symlink {
                name: b"src/link.rs".to_vec(),
                target: b"../../etc/passwd".to_vec(),
            })
        );
    }

    #[test]
    fn admission_refuses_an_entry_that_is_not_a_file() {
        let corpus: Vec<(&[u8], Candidate<'_>)> =
            vec![(b"a", Candidate::File(b"x")), (b"pipe", Candidate::Special)];
        assert_eq!(
            admit(corpus, AdmissionPolicy::new()),
            Err(AdmissionError::Special {
                name: b"pipe".to_vec()
            })
        );
    }

    #[test]
    fn admission_refuses_an_unimplemented_symlink_policy() {
        let policy = AdmissionPolicy::new().with_symlinks(SymlinkPolicy::FollowWithCycleDetection);
        assert!(!policy.symlinks().is_implemented());
        // Refused before a single candidate is read: the policy is wrong, not the input.
        assert_eq!(
            admit(files(&["a"]), policy),
            Err(AdmissionError::UnsupportedSymlinkPolicy {
                policy: SymlinkPolicy::FollowWithCycleDetection
            })
        );
        assert_eq!(
            admit(Vec::new(), policy),
            Err(AdmissionError::UnsupportedSymlinkPolicy {
                policy: SymlinkPolicy::FollowWithCycleDetection
            })
        );
        assert_eq!(AdmissionPolicy::default(), AdmissionPolicy::new());
        assert_eq!(AdmissionPolicy::new().symlinks(), SymlinkPolicy::Reject);
    }

    #[test]
    fn admission_refuses_candidates_that_are_not_a_tree() {
        // The same raw name twice.
        let repeated: Vec<(&[u8], Candidate<'_>)> = vec![
            (b"a/b", Candidate::File(b"one")),
            (b"a/b", Candidate::File(b"two")),
        ];
        assert_eq!(
            admit(repeated, AdmissionPolicy::new()),
            Err(AdmissionError::NotATree(SnapshotError::DuplicatePath {
                path: path("a/b")
            }))
        );

        // A file another candidate needs as a directory, reported the same way whichever
        // order the walk produced them in.
        let conflicting: Vec<(&[u8], Candidate<'_>)> = vec![
            (b"a/b/c", Candidate::File(b"deep")),
            (b"a/b", Candidate::File(b"shallow")),
        ];
        let expected = Err(AdmissionError::NotATree(SnapshotError::PathConflict {
            file: path("a/b"),
            descendant: path("a/b/c"),
        }));
        assert_eq!(admit(conflicting.clone(), AdmissionPolicy::new()), expected);
        let mut reversed = conflicting;
        reversed.reverse();
        assert_eq!(admit(reversed, AdmissionPolicy::new()), expected);
    }

    #[test]
    fn an_admitted_workspace_builds_one_snapshot_whatever_the_walk_order() {
        let corpus = files(&[
            "Cargo.toml",
            "README.md",
            "crates/value/lib.rs",
            "crates/workspace/lib.rs",
            "crates/workspace/snapshot.rs",
            NFC,
            NFD,
        ]);
        let first = Snapshot::build(
            &admit(corpus.clone(), AdmissionPolicy::new()).expect("a tree"),
            &HexIdentity,
        )
        .expect("named");

        let mut shuffle = Shuffle::new(0xd1b5_4a32_d192_ed03);
        for _ in 0..32 {
            let mut permuted = corpus.clone();
            shuffle.permute(&mut permuted);
            let other = Snapshot::build(
                &admit(permuted, AdmissionPolicy::new()).expect("a tree"),
                &HexIdentity,
            )
            .expect("named");
            assert_eq!(other.identity(), first.identity());
            assert_eq!(other.encode(), first.encode());
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
