//! Deterministic artifact paths: handle spelling and store layout (plan §4.4,
//! PR-1 / IMPL-05).
//!
//! # What this module decides
//!
//! > Artifact classes include:
//! >
//! > ```text
//! > ws_*      workspace snapshot        rt_*      repair transaction
//! > in_*      intent contract           forge_*   synthesis archive
//! > inb_*     signed intent bundle      cont_*    resumable task continuation
//! > model_*   elaborated model          cap_*     capability
//! > cir_*     causal execution graph    diff_*    semantic/intent diff
//! > crash_*   crashpack                 defect_*  engine-defect report
//! > ctx_*     Context Pack              proof_*   proof artifact
//! > ps_*      proof state               task_*    task
//! > ev_*      evidence node or edge     dbg_*     debugger branch
//! > receipt_* signed/checked receipt
//! > ```
//! >
//! > Handles carry a kind prefix but are otherwise structureless.
//! >
//! > — `notes/plan/plan.md` §4.4 "Content-addressed artifacts"
//!
//! Two things follow from "structureless", and this module implements exactly those
//! two: the *spelling* of a handle ([`ArtifactHandle`]) and the *place* the artifact
//! with that handle is stored ([`ArtifactPath`]).
//!
//! # What this module deliberately does not decide
//!
//! It does not compute content identities. Canonical encoding, the total order, and
//! ADR-0013 content identity are PR 2's deliverable in `continuum-value`; atomic
//! publication (INV-017) is PR 2 and `continuumd`; the Merkle snapshot itself is PR 3.
//! An identity arrives here already computed and opaque, and this module only proves
//! that the same identity always lands in the same place.
//!
//! # Why the placement function must be pure
//!
//! > **INV-005 — No ambient nondeterminism.** Controlled code accesses scheduling,
//! > time, entropy, I/O, faults, and cancellation through explicit capabilities.
//! >
//! > — `notes/plan/plan.md` §2
//!
//! [`ArtifactPath::for_handle`] takes a handle and returns a path. It reads no clock,
//! no counter, no environment, no working directory, and no random source; it allocates
//! no identifier of its own. Two processes on two machines holding the same handle
//! derive the same relative path, which is what makes the store a function of content
//! rather than of history — the precondition for INV-006 replay stability and for the
//! "independently re-derivable" checker identity of plan §20.
//!
//! Ordering follows the same rule. Every collection here is a [`BTreeMap`] keyed by
//! [`ArtifactPath`], never a hash map: docs/19 §7's determinism matrix tests "different
//! hash seeds where internal structures allow", and a store listing whose order depends
//! on a per-process hash seed would fail it.
//!
//! # Path traversal
//!
//! docs/19 §9 requires validation against "path traversal in crashpacks", and docs/09
//! §"Threats" names "path traversal in crashpack extraction" outright. The defense is
//! at construction, not at use: an identity is restricted to the character class the
//! normative schemas already assign to handles — `[A-Za-z0-9_-]` — which contains
//! neither `.` nor any separator, so no accepted identity can spell `..`, an absolute
//! path, or a Windows drive or UNC prefix. A traversal attempt is a typed
//! [`ArtifactPathError`], never a path.
//!
//! # Filesystem assumptions
//!
//! Handles are case-sensitive: `ws_A` and `ws_a` are different artifacts and get
//! different paths. A case-insensitive filesystem would merge them, so the store
//! requires a case-sensitive volume. docs/19 §7 makes "supported OS/architectures" a
//! determinism-matrix dimension; adding a case-insensitive target means adding a
//! case-preserving encoding here, not relaxing the identity charset.
//!
//! # Example
//!
//! ```
//! use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle, ArtifactPath};
//!
//! let handle = ArtifactHandle::new(ArtifactClass::Crashpack, "cp7m3x9")?;
//! assert_eq!(handle.to_string(), "crash_cp7m3x9");
//!
//! let path = ArtifactPath::for_handle(&handle)?;
//! assert_eq!(path.as_str(), "crash/cp/cp7m3x9");
//! # Ok::<(), continuum_workspace::artifact_path::ArtifactPathError>(())
//! ```

use core::fmt;
use core::str::FromStr;
use std::collections::BTreeMap;
use std::collections::btree_map::Iter as BTreeMapIter;
use std::path::{Path, PathBuf};

/// The separator between an artifact class prefix and its identity, as plan §4.4 spells
/// every class: `ws_`, `in_`, `receipt_`.
const CLASS_SEPARATOR: char = '_';

/// How many leading identity characters form the fan-out directory.
///
/// This is the one value in this module that the dossier does not fix, and it is named
/// rather than inlined so that it is reviewable. Plan §4.5 requires the daemon to
/// "report storage attribution by artifact class" and to garbage-collect and summarize
/// whole classes, so grouping by class is dossier-driven; splitting a class across a
/// second level is ordinary filesystem hygiene for a store that plan §4.5 expects to
/// hold campaign-scale evidence. Any change to this constant changes every path and is
/// therefore a store-format change, not a tuning knob.
pub const SHARD_LEN: usize = 2;

// --- artifact classes -----------------------------------------------------------------

/// One artifact class from plan §4.4.
///
/// The variants and their declaration order are plan §4.4's list, read top to bottom.
/// That order is also this type's [`Ord`], so any sort by class is stable across
/// processes and platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactClass {
    /// `ws_*` — a workspace snapshot (plan §4.2).
    WorkspaceSnapshot,
    /// `in_*` — an Intent Contract (plan §5).
    IntentContract,
    /// `inb_*` — a signed intent bundle (plan §4.2.1).
    SignedIntentBundle,
    /// `model_*` — an elaborated model (plan §16).
    ElaboratedModel,
    /// `cir_*` — a causal execution graph (RFC 0001).
    CausalExecutionGraph,
    /// `crash_*` — a crashpack (plan §7).
    Crashpack,
    /// `ctx_*` — a Context Pack (plan §6, RFC 0028).
    ContextPack,
    /// `proof_*` — a proof artifact (plan §15).
    ProofArtifact,
    /// `ps_*` — a proof state (plan §15).
    ProofState,
    /// `task_*` — a verification task (plan §4.5).
    Task,
    /// `ev_*` — an evidence node or edge (plan §11).
    Evidence,
    /// `dbg_*` — a debugger branch (plan §7).
    DebuggerBranch,
    /// `receipt_*` — a signed or checked receipt (RFC 0024).
    Receipt,
    /// `rt_*` — a repair transaction (plan §8).
    RepairTransaction,
    /// `forge_*` — a synthesis archive (plan §14).
    ForgeArchive,
    /// `cont_*` — a resumable task continuation (plan §9.6).
    Continuation,
    /// `cap_*` — a capability token (plan §18.2).
    ///
    /// The one class in plan §4.4 that is **not** content-addressed: capability tokens
    /// "are minted randomly and do confer authority". [`ArtifactPath::for_handle`]
    /// refuses it — see [`ArtifactClass::is_content_addressed`].
    Capability,
    /// `diff_*` — a semantic or intent diff (plan §5.3, RFC 0031).
    Diff,
    /// `defect_*` — an engine-defect report (plan §4.7).
    EngineDefect,
}

impl ArtifactClass {
    /// Every artifact class, in plan §4.4's order.
    pub const ALL: [Self; 19] = [
        Self::WorkspaceSnapshot,
        Self::IntentContract,
        Self::SignedIntentBundle,
        Self::ElaboratedModel,
        Self::CausalExecutionGraph,
        Self::Crashpack,
        Self::ContextPack,
        Self::ProofArtifact,
        Self::ProofState,
        Self::Task,
        Self::Evidence,
        Self::DebuggerBranch,
        Self::Receipt,
        Self::RepairTransaction,
        Self::ForgeArchive,
        Self::Continuation,
        Self::Capability,
        Self::Diff,
        Self::EngineDefect,
    ];

    /// The class token: the plan §4.4 prefix without its trailing `_`.
    ///
    /// This doubles as the first path segment, so it is deliberately a bare lowercase
    /// ASCII word with no separator in it — see [`ArtifactHandle::from_str`].
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::WorkspaceSnapshot => "ws",
            Self::IntentContract => "in",
            Self::SignedIntentBundle => "inb",
            Self::ElaboratedModel => "model",
            Self::CausalExecutionGraph => "cir",
            Self::Crashpack => "crash",
            Self::ContextPack => "ctx",
            Self::ProofArtifact => "proof",
            Self::ProofState => "ps",
            Self::Task => "task",
            Self::Evidence => "ev",
            Self::DebuggerBranch => "dbg",
            Self::Receipt => "receipt",
            Self::RepairTransaction => "rt",
            Self::ForgeArchive => "forge",
            Self::Continuation => "cont",
            Self::Capability => "cap",
            Self::Diff => "diff",
            Self::EngineDefect => "defect",
        }
    }

    /// The class prefix exactly as plan §4.4 spells it, trailing `_` included.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::WorkspaceSnapshot => "ws_",
            Self::IntentContract => "in_",
            Self::SignedIntentBundle => "inb_",
            Self::ElaboratedModel => "model_",
            Self::CausalExecutionGraph => "cir_",
            Self::Crashpack => "crash_",
            Self::ContextPack => "ctx_",
            Self::ProofArtifact => "proof_",
            Self::ProofState => "ps_",
            Self::Task => "task_",
            Self::Evidence => "ev_",
            Self::DebuggerBranch => "dbg_",
            Self::Receipt => "receipt_",
            Self::RepairTransaction => "rt_",
            Self::ForgeArchive => "forge_",
            Self::Continuation => "cont_",
            Self::Capability => "cap_",
            Self::Diff => "diff_",
            Self::EngineDefect => "defect_",
        }
    }

    /// Whether artifacts of this class are content-addressed.
    ///
    /// > Content-addressed identities are derivable by anyone holding the content […]
    /// > so handles are identifiers, not secrets, and confer no authority. Capability
    /// > tokens (`cap_*`) are minted randomly and do confer authority.
    /// >
    /// > — plan §4.4
    ///
    /// A random bearer token is not a content identity, and filing one under a path
    /// derived from itself would publish the secret into the store's directory
    /// namespace, where a listing is enough to steal it. Every other class is content-
    /// addressed and therefore placeable.
    #[must_use]
    pub const fn is_content_addressed(self) -> bool {
        !matches!(self, Self::Capability)
    }

    /// Recover a class from its [`token`](Self::token).
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.token() == token)
    }
}

impl fmt::Display for ArtifactClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

// --- errors ---------------------------------------------------------------------------

/// Why a string is not a usable handle, or a handle has no store path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPathError {
    /// The text had no `<class>_` prefix at all.
    MissingClassPrefix,
    /// The text carried a prefix that plan §4.4 does not list.
    UnknownClass,
    /// The identity after the prefix was empty. The normative schemas spell the handle
    /// pattern `^ws_[A-Za-z0-9_-]+$`: at least one identity character is required.
    EmptyIdentity,
    /// The identity contained a character outside `[A-Za-z0-9_-]`.
    ///
    /// This is the path-traversal rejection (docs/19 §9, docs/09): `.`, `/`, `\`, `:`,
    /// NUL, and every non-ASCII character land here.
    IdentityCharacter {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// The class is not content-addressed, so it has no content-derived path.
    ///
    /// Raised only for [`ArtifactClass::Capability`] (plan §4.4).
    NotContentAddressed(ArtifactClass),
}

impl fmt::Display for ArtifactPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingClassPrefix => {
                f.write_str("artifact handle has no `<class>_` prefix (plan §4.4)")
            }
            Self::UnknownClass => {
                f.write_str("artifact handle names a class plan §4.4 does not list")
            }
            Self::EmptyIdentity => f.write_str("artifact handle has an empty identity"),
            Self::IdentityCharacter { index, character } => write!(
                f,
                "artifact identity has a character {character:?} outside [A-Za-z0-9_-] at byte {index}"
            ),
            Self::NotContentAddressed(class) => write!(
                f,
                "artifact class `{class}` is not content-addressed and has no derived path"
            ),
        }
    }
}

impl core::error::Error for ArtifactPathError {}

// --- handles --------------------------------------------------------------------------

/// A content-addressed artifact handle: a plan §4.4 class plus an opaque identity.
///
/// The wire spelling is `<prefix><identity>` — `ws_7m3x9`, `receipt_2c…` — which is the
/// form the normative schemas match with `^ws_[A-Za-z0-9_-]+$`
/// (`notes/plan/schemas/workspace-snapshot.schema.json`, `verification-task.schema.json`,
/// `evidence-graph-node.schema.json`, and six others).
///
/// The identity is stored verbatim and compared exactly. Plan §4.4 calls handles
/// "structureless": nothing here parses, normalizes, or interprets the identity, because
/// under ADR-0013 an identity has exactly one canonical spelling and any normalization
/// applied here would be a second one.
///
/// [`Ord`] is by class in plan §4.4 order, then by identity bytes — total, and equal on
/// every platform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactHandle {
    class: ArtifactClass,
    identity: String,
}

impl ArtifactHandle {
    /// Build a handle from a class and an identity token.
    ///
    /// # Errors
    ///
    /// [`ArtifactPathError::EmptyIdentity`] when `identity` is empty, and
    /// [`ArtifactPathError::IdentityCharacter`] when it contains anything outside
    /// `[A-Za-z0-9_-]`.
    pub fn new(class: ArtifactClass, identity: &str) -> Result<Self, ArtifactPathError> {
        validate_identity(identity)?;
        Ok(Self {
            class,
            identity: identity.to_owned(),
        })
    }

    /// This handle's artifact class.
    #[must_use]
    pub const fn class(&self) -> ArtifactClass {
        self.class
    }

    /// This handle's identity, without the class prefix.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

impl fmt::Display for ArtifactHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.class.prefix())?;
        f.write_str(&self.identity)
    }
}

impl FromStr for ArtifactHandle {
    type Err = ArtifactPathError;

    /// Parse a handle from its wire spelling.
    ///
    /// The split is at the *first* `_`. That is unambiguous rather than merely
    /// convenient: no class token contains `_`, so a handle has exactly one possible
    /// decomposition even though identities may contain `_` freely. In particular
    /// `inb_x` cannot also be read as class `in` with identity `b_x`, because that
    /// reading requires the third character to be `_` and it is `b`.
    ///
    /// # Errors
    ///
    /// [`ArtifactPathError::MissingClassPrefix`], [`ArtifactPathError::UnknownClass`],
    /// or the identity errors of [`ArtifactHandle::new`].
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (token, identity) = text
            .split_once(CLASS_SEPARATOR)
            .ok_or(ArtifactPathError::MissingClassPrefix)?;
        let class = ArtifactClass::from_token(token).ok_or(ArtifactPathError::UnknownClass)?;
        Self::new(class, identity)
    }
}

/// Reject every identity that is not exactly one canonical, traversal-free token.
fn validate_identity(identity: &str) -> Result<(), ArtifactPathError> {
    if identity.is_empty() {
        return Err(ArtifactPathError::EmptyIdentity);
    }
    if let Some((index, character)) = identity
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        return Err(ArtifactPathError::IdentityCharacter { index, character });
    }
    Ok(())
}

// --- paths ----------------------------------------------------------------------------

/// The store location of one artifact, relative to the store root.
///
/// The shape is `<class>/<shard>/<identity>` — three `/`-separated segments — where
/// `<shard>` is the first [`SHARD_LEN`] characters of the identity (or the whole
/// identity when it is shorter).
///
/// Class first because plan §4.5 requires the daemon to "report storage attribution by
/// artifact class" and to make whole classes GC-eligible under retention policy; a
/// class-rooted subtree makes both a directory operation instead of an index scan.
///
/// The stored form always uses `/`. [`ArtifactPath::resolve`] joins segment by segment,
/// so the platform separator appears only in the [`PathBuf`] and never in the identity
/// the store records — the same artifact has one spelling on every OS, which is what
/// docs/19 §7's OS/architecture matrix dimension requires.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactPath(String);

impl ArtifactPath {
    /// Derive the store path of a content-addressed artifact.
    ///
    /// Pure: the result is a function of `handle` alone (INV-005 — see the module
    /// documentation).
    ///
    /// # Errors
    ///
    /// [`ArtifactPathError::NotContentAddressed`] when the handle's class is not
    /// content-addressed, which under plan §4.4 means [`ArtifactClass::Capability`].
    pub fn for_handle(handle: &ArtifactHandle) -> Result<Self, ArtifactPathError> {
        let class = handle.class();
        if !class.is_content_addressed() {
            return Err(ArtifactPathError::NotContentAddressed(class));
        }
        let identity = handle.identity();
        // `[A-Za-z0-9_-]` is single-byte ASCII throughout, so a byte split cannot land
        // inside a character.
        let shard = &identity[..SHARD_LEN.min(identity.len())];
        Ok(Self(format!("{}/{shard}/{identity}", class.token())))
    }

    /// The relative path, `/`-separated.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path segments, outermost first.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Join this path under a store root.
    ///
    /// Joins segment by segment, so the result stays inside `root` for every path this
    /// module can produce.
    #[must_use]
    pub fn resolve(&self, root: &Path) -> PathBuf {
        let mut resolved = root.to_path_buf();
        for segment in self.segments() {
            resolved.push(segment);
        }
        resolved
    }
}

impl fmt::Display for ArtifactPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --- layout ---------------------------------------------------------------------------

/// A set of placed artifacts, ordered by path.
///
/// Iteration order is the [`BTreeMap`] order of [`ArtifactPath`], so it depends on the
/// artifacts present and on nothing else — not on insertion order, not on a hash seed,
/// not on the process. docs/19 §7 tests retained scenarios across "different hash seeds
/// where internal structures allow" and across "process restarts", and a store listing
/// is exactly the kind of semantic artifact §7 requires to come out identical.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactLayout {
    entries: BTreeMap<ArtifactPath, ArtifactHandle>,
}

impl ArtifactLayout {
    /// An empty layout.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Place `handle` and return the path it was filed under.
    ///
    /// Placing the same handle twice is idempotent: the path is a function of the
    /// handle, so the second call rewrites the entry with an equal one.
    ///
    /// # Errors
    ///
    /// The errors of [`ArtifactPath::for_handle`].
    pub fn insert(&mut self, handle: ArtifactHandle) -> Result<ArtifactPath, ArtifactPathError> {
        let path = ArtifactPath::for_handle(&handle)?;
        self.entries.insert(path.clone(), handle);
        Ok(path)
    }

    /// The handle filed at `path`, if any.
    #[must_use]
    pub fn get(&self, path: &ArtifactPath) -> Option<&ArtifactHandle> {
        self.entries.get(path)
    }

    /// Whether anything is filed at `path`.
    #[must_use]
    pub fn contains(&self, path: &ArtifactPath) -> bool {
        self.entries.contains_key(path)
    }

    /// How many artifacts are placed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the layout is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every `(path, handle)` pair, in path order.
    pub fn iter(&self) -> BTreeMapIter<'_, ArtifactPath, ArtifactHandle> {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a ArtifactLayout {
    type Item = (&'a ArtifactPath, &'a ArtifactHandle);
    type IntoIter = BTreeMapIter<'a, ArtifactPath, ArtifactHandle>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(text: &str) -> ArtifactHandle {
        text.parse().expect("test handle is well formed")
    }

    fn path_of(text: &str) -> String {
        ArtifactPath::for_handle(&handle(text))
            .expect("test handle is content-addressed")
            .as_str()
            .to_owned()
    }

    /// A spread of handles wide enough to exercise every class, both shard cases, and
    /// every legal identity character.
    fn sample_handles() -> Vec<ArtifactHandle> {
        let mut handles: Vec<ArtifactHandle> = ArtifactClass::ALL
            .into_iter()
            .filter(|class| class.is_content_addressed())
            .map(|class| ArtifactHandle::new(class, "7m3x9").expect("valid"))
            .collect();
        for identity in ["a", "ab", "A", "Z_9-0", "0", "__", "--", "zzzzzzzzzzzzzzzz"] {
            handles.push(
                ArtifactHandle::new(ArtifactClass::Evidence, identity).expect("valid identity"),
            );
        }
        handles
    }

    // --- plan §4.4 fidelity ------------------------------------------------------------

    #[test]
    fn classes_are_plan_4_4_in_order() {
        let prefixes: Vec<&str> = ArtifactClass::ALL
            .iter()
            .map(|class| class.prefix())
            .collect();
        assert_eq!(
            prefixes,
            [
                "ws_", "in_", "inb_", "model_", "cir_", "crash_", "ctx_", "proof_", "ps_", "task_",
                "ev_", "dbg_", "receipt_", "rt_", "forge_", "cont_", "cap_", "diff_", "defect_",
            ]
        );
    }

    #[test]
    fn class_tokens_are_distinct_and_separator_free() {
        // The `from_str` split at the first `_` is only unambiguous while this holds.
        for (index, class) in ArtifactClass::ALL.iter().enumerate() {
            assert!(!class.token().contains(CLASS_SEPARATOR), "{class}");
            assert_eq!(
                class.prefix(),
                format!("{}{CLASS_SEPARATOR}", class.token())
            );
            for other in &ArtifactClass::ALL[index + 1..] {
                assert_ne!(class.token(), other.token());
            }
        }
    }

    #[test]
    fn only_capabilities_are_not_content_addressed() {
        // plan §4.4: "Capability tokens (`cap_*`) are minted randomly and do confer
        // authority."
        let excluded: Vec<ArtifactClass> = ArtifactClass::ALL
            .into_iter()
            .filter(|class| !class.is_content_addressed())
            .collect();
        assert_eq!(excluded, [ArtifactClass::Capability]);
    }

    #[test]
    fn a_capability_token_gets_no_content_derived_path() {
        let cap = ArtifactHandle::new(ArtifactClass::Capability, "s3cret").expect("valid");
        assert_eq!(cap.to_string(), "cap_s3cret");
        assert_eq!(
            ArtifactPath::for_handle(&cap),
            Err(ArtifactPathError::NotContentAddressed(
                ArtifactClass::Capability
            ))
        );
        assert_eq!(
            ArtifactLayout::new().insert(cap),
            Err(ArtifactPathError::NotContentAddressed(
                ArtifactClass::Capability
            ))
        );
    }

    // --- handle spelling ---------------------------------------------------------------

    #[test]
    fn handles_round_trip_through_their_wire_form() {
        for class in ArtifactClass::ALL {
            let original = ArtifactHandle::new(class, "7m3x9").expect("valid identity");
            let reparsed: ArtifactHandle = original.to_string().parse().expect("round trip");
            assert_eq!(reparsed, original);
            assert_eq!(reparsed.class(), class);
            assert_eq!(reparsed.identity(), "7m3x9");
        }
    }

    #[test]
    fn wire_form_matches_the_normative_schema_pattern() {
        // `^<class>_[A-Za-z0-9_-]+$`, e.g. schemas/workspace-snapshot.schema.json.
        for handle in sample_handles() {
            let text = handle.to_string();
            let rest = text
                .strip_prefix(handle.class().prefix())
                .expect("handle starts with its class prefix");
            assert!(!rest.is_empty());
            assert!(
                rest.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "{text}"
            );
        }
    }

    #[test]
    fn the_in_inb_prefix_pair_is_unambiguous() {
        let bundle = handle("inb_b_x");
        assert_eq!(bundle.class(), ArtifactClass::SignedIntentBundle);
        assert_eq!(bundle.identity(), "b_x");

        let contract = handle("in_b_x");
        assert_eq!(contract.class(), ArtifactClass::IntentContract);
        assert_eq!(contract.identity(), "b_x");

        assert_ne!(
            ArtifactPath::for_handle(&bundle),
            ArtifactPath::for_handle(&contract)
        );
    }

    #[test]
    fn identities_may_contain_underscores() {
        let h = handle("receipt_2c_ab-9_z");
        assert_eq!(h.class(), ArtifactClass::Receipt);
        assert_eq!(h.identity(), "2c_ab-9_z");
        assert_eq!(h.to_string(), "receipt_2c_ab-9_z");
    }

    #[test]
    fn malformed_handles_are_typed_errors() {
        assert_eq!(
            "ws7m3x9".parse::<ArtifactHandle>(),
            Err(ArtifactPathError::MissingClassPrefix)
        );
        assert_eq!(
            "".parse::<ArtifactHandle>(),
            Err(ArtifactPathError::MissingClassPrefix)
        );
        assert_eq!(
            "snapshot_7m3".parse::<ArtifactHandle>(),
            Err(ArtifactPathError::UnknownClass)
        );
        assert_eq!(
            "WS_7m3".parse::<ArtifactHandle>(),
            Err(ArtifactPathError::UnknownClass)
        );
        assert_eq!(
            "ws_".parse::<ArtifactHandle>(),
            Err(ArtifactPathError::EmptyIdentity)
        );
    }

    // --- path traversal (docs/19 §9, docs/09) ------------------------------------------

    #[test]
    fn traversal_and_separator_attempts_are_rejected() {
        for identity in [
            "..",
            ".",
            "../../etc/passwd",
            "a/b",
            "a\\b",
            "C:",
            "\\\\server\\share",
            "a\0b",
            "a b",
            "a\nb",
            "a.b",
            "héllo",
            "ws_\u{202e}",
        ] {
            let result = ArtifactHandle::new(ArtifactClass::WorkspaceSnapshot, identity);
            assert!(
                matches!(result, Err(ArtifactPathError::IdentityCharacter { .. })),
                "identity {identity:?} was not rejected: {result:?}"
            );
        }
    }

    #[test]
    fn no_derived_path_has_a_relative_or_absolute_segment() {
        for handle in sample_handles() {
            let path = ArtifactPath::for_handle(&handle).expect("content-addressed");
            let segments: Vec<&str> = path.segments().collect();
            assert_eq!(segments.len(), 3, "{path}");
            for segment in segments {
                assert!(!segment.is_empty(), "{path}");
                assert_ne!(segment, ".", "{path}");
                assert_ne!(segment, "..", "{path}");
                assert!(!segment.contains(['/', '\\', ':']), "{path}");
            }
        }
    }

    #[test]
    fn resolve_stays_under_the_root() {
        let root = Path::new("/var/lib/continuum/cas");
        let resolved = ArtifactPath::for_handle(&handle("ws_7m3x9"))
            .expect("content-addressed")
            .resolve(root);
        assert_eq!(
            resolved,
            PathBuf::from("/var/lib/continuum/cas/ws/7m/7m3x9")
        );
        assert!(resolved.starts_with(root));
    }

    // --- determinism ------------------------------------------------------------------

    #[test]
    fn golden_paths_are_stable() {
        // Literals, not recomputation: this is the assertion that survives a rebuild,
        // a new process, and a different machine (docs/19 §7 "process restarts").
        assert_eq!(path_of("ws_7m3x9"), "ws/7m/7m3x9");
        assert_eq!(path_of("crash_cp7m3x9"), "crash/cp/cp7m3x9");
        assert_eq!(path_of("receipt_2c_ab-9"), "receipt/2c/2c_ab-9");
        assert_eq!(path_of("defect_ZZ_01-b"), "defect/ZZ/ZZ_01-b");
        assert_eq!(path_of("inb_b_x"), "inb/b_/b_x");
        // Identities shorter than the shard use themselves as the shard.
        assert_eq!(path_of("ev_a"), "ev/a/a");
        assert_eq!(path_of("ev_ab"), "ev/ab/ab");
    }

    #[test]
    fn placement_is_a_pure_function_of_the_handle() {
        for handle in sample_handles() {
            let first = ArtifactPath::for_handle(&handle).expect("content-addressed");
            let second = ArtifactPath::for_handle(&handle).expect("content-addressed");
            // Same handle rebuilt from its wire form, i.e. reached by a different route.
            let reparsed: ArtifactHandle = handle.to_string().parse().expect("round trip");
            let third = ArtifactPath::for_handle(&reparsed).expect("content-addressed");
            assert_eq!(first, second);
            assert_eq!(first, third);
        }
    }

    #[test]
    fn distinct_handles_never_share_a_path() {
        let mut seen: BTreeMap<ArtifactPath, ArtifactHandle> = BTreeMap::new();
        for handle in sample_handles() {
            let path = ArtifactPath::for_handle(&handle).expect("content-addressed");
            if let Some(previous) = seen.insert(path.clone(), handle.clone()) {
                assert_eq!(previous, handle, "{path} is shared by two handles");
            }
        }
        // Case is significant: `ev_A` and `ev_a` are different artifacts.
        assert_ne!(path_of("ev_A"), path_of("ev_a"));
    }

    #[test]
    fn layout_order_is_independent_of_insertion_order() {
        let handles = sample_handles();

        let build = |order: Vec<ArtifactHandle>| -> Vec<(String, String)> {
            let mut layout = ArtifactLayout::new();
            for handle in order {
                layout.insert(handle).expect("content-addressed");
            }
            layout
                .iter()
                .map(|(path, handle)| (path.to_string(), handle.to_string()))
                .collect()
        };

        let forward = build(handles.clone());

        let mut reversed = handles.clone();
        reversed.reverse();
        assert_eq!(build(reversed), forward);

        let mut rotated = handles.clone();
        rotated.rotate_left(7);
        assert_eq!(build(rotated), forward);

        // Interleaved insertion with a duplicate pass — placement is idempotent.
        let mut doubled = handles.clone();
        doubled.extend(handles.iter().rev().cloned());
        let doubled_listing = build(doubled);
        assert_eq!(doubled_listing, forward);
        assert_eq!(forward.len(), handles.len());
    }

    #[test]
    fn layout_listing_is_sorted_by_path() {
        let mut layout = ArtifactLayout::new();
        for handle in sample_handles() {
            layout.insert(handle).expect("content-addressed");
        }
        let paths: Vec<&ArtifactPath> = layout.iter().map(|(path, _)| path).collect();
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(layout.len(), sample_handles().len());
        assert!(!layout.is_empty());

        let ws = ArtifactPath::for_handle(&handle("ws_7m3x9")).expect("content-addressed");
        assert!(layout.contains(&ws));
        assert_eq!(layout.get(&ws), Some(&handle("ws_7m3x9")));
        assert_eq!((&layout).into_iter().count(), layout.len());
    }

    #[test]
    fn class_tokens_recover_their_class() {
        for class in ArtifactClass::ALL {
            assert_eq!(ArtifactClass::from_token(class.token()), Some(class));
        }
        assert_eq!(ArtifactClass::from_token("snapshot"), None);
        assert_eq!(ArtifactClass::from_token("ws_"), None);
    }
}
