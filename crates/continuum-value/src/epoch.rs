//! Compatibility epochs: protocol, semantic, intent, evidence, proof, and corpus
//! (PR-1 / IMPL-02).
//!
//! # What an epoch is
//!
//! > Epochs are content identities; advancing one never mutates existing artifacts
//! > (ADR-0018).
//! >
//! > - evidence is epoch-scoped: a receipt remains verifiable under its pinned epoch
//! >   indefinitely (INV-006, INV-014); an epoch advance never silently revalidates or
//! >   invalidates a published receipt;
//! > - each advance publishes a typed per-artifact-class compatibility statement —
//! >   `Preserved | Revalidate | Incompatible` — and an estimated invalidation blast
//! >   radius by artifact class before it is applied;
//! > - the daemon may hold at most two epochs concurrently during migration; new work
//! >   defaults to the newest; continuations resume only under their pinned epoch
//! >   (§9.6) and are forked, never migrated in place;
//! > - re-derived artifacts receive new identities linked to their predecessors by
//! >   `SUPERSEDES` edges; nothing is rewritten in place.
//! >
//! > — `notes/plan/plan.md` §4.6 "Semantic epoch advance"
//!
//! # Why there are six, and why they are six *types*
//!
//! > Protocol, semantic, intent, evidence, proof, and corpus epochs are versioned
//! > independently (docs/12 §7) and MUST NOT be conflated.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "IDL and versioning"
//!
//! `MUST NOT be conflated` is enforced here by the type system rather than by review:
//! [`SemanticEpoch`], [`IntentEpoch`], [`EvidenceEpoch`], [`ProofEpoch`], and
//! [`CorpusEpoch`] are distinct newtypes with no conversions between them, and
//! [`ProtocolEpoch`] is a different shape entirely. Passing a proof epoch where a
//! semantic epoch is expected does not compile.
//!
//! # Equality and ordering
//!
//! Plan §4.6 defines epochs as *content identities*, not as version numbers, so five of
//! the six carry equality and hashing but deliberately **no ordering**: a content
//! identity such as `tla-examples@91c22ea…`
//! (`notes/plan/rfcs/0019-corpus-port-manifest.md`) or `leanprover/lean4:v4.32.1`
//! (`notes/plan/docs/23_LEAN4_FORMALIZATION_PROGRAM.md`) has no successor relation that
//! can be computed from the identity. Succession is a published event, modeled by
//! [`EpochAdvance`], not a comparison.
//!
//! [`ProtocolEpoch`] is the exception, and the dossier says so outright: the daemon
//! "selects the highest common version" at connection open and "MUST serve protocol
//! majors N and N−1" (RFC 0026). Both statements require a total order over
//! `major.minor`, so [`ProtocolEpoch`] implements [`Ord`].
//!
//! # Scope note
//!
//! RFC 0026's result-envelope table also mentions an *engine* epoch. That is engine
//! identity (plan §4.7 "Engine-defect lifecycle"), not one of the six independently
//! versioned compatibility epochs of docs/12 §7, and is out of scope for this module.
//!
//! Mapping a rejected resume onto the wire error codes `ContinuationEpochMismatch`,
//! `EpochUnsupported`, and `ProtocolVersionUnsupported` (plan §10.3, RFC 0026) belongs
//! to `continuumd` (PR 5); this module supplies the typed decision those codes report.

use core::fmt;
use core::str::FromStr;

/// Every compatibility epoch Continuum versions independently.
///
/// The variants and their order are
/// `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 1: "protocol, semantic, intent,
/// evidence, proof, and corpus epochs".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EpochKind {
    /// Wire compatibility of the `continuumd` native protocol (RFC 0026).
    Protocol,
    /// The meaning of evaluation: what a model, property, or trace *says*
    /// (plan §4.6, ADR-0018).
    Semantic,
    /// The Intent Contract vocabulary and its policy tables (plan §5, §4.2.1).
    Intent,
    /// The evidence-graph and receipt schemas an artifact declares itself under
    /// (plan §4.3, `notes/plan/schemas/README.md` "Schema epochs and compatibility").
    Evidence,
    /// The Lean toolchain and theorem-package closure (ADR-0035, docs/23).
    Proof,
    /// The pinned upstream corpus revision and oracle toolchain
    /// (`notes/plan/corpus/tla-examples/CORPUS_POLICY.md`).
    Corpus,
}

impl EpochKind {
    /// Every epoch kind, in the order PR 1 names them.
    ///
    /// A machine result must name all of these — an epoch a result cannot pin is
    /// reported as [`EpochBinding::Unpinned`], never omitted.
    pub const ALL: [Self; 6] = [
        Self::Protocol,
        Self::Semantic,
        Self::Intent,
        Self::Evidence,
        Self::Proof,
        Self::Corpus,
    ];

    /// The stable machine name of this epoch kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protocol => "protocol",
            Self::Semantic => "semantic",
            Self::Intent => "intent",
            Self::Evidence => "evidence",
            Self::Proof => "proof",
            Self::Corpus => "corpus",
        }
    }
}

impl fmt::Display for EpochKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- epoch identity ------------------------------------------------------------------

/// The content identity of one epoch (plan §4.6: "Epochs are content identities").
///
/// The dossier's epoch identities are opaque tokens whose *spelling is the identity*:
/// `cml/0.2` and `tla-examples@91c22ea…`
/// (`notes/plan/rfcs/0019-corpus-port-manifest.md`), `leanprover/lean4:v4.32.1`
/// (`notes/plan/docs/23_LEAN4_FORMALIZATION_PROGRAM.md`). This type therefore stores the
/// token verbatim and compares it exactly.
///
/// Construction restricts the token to printable ASCII with no spaces. Under ADR-0013 an
/// identity has exactly one canonical spelling; permitting whitespace, control
/// characters, or Unicode that normalizes several ways would let two spellings denote
/// one epoch, which is precisely what a content identity may not do.
///
/// [`Ord`] is deliberately not implemented — see the module documentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EpochIdentity(String);

impl EpochIdentity {
    /// Build an epoch identity from its canonical token.
    ///
    /// # Errors
    ///
    /// Returns [`EpochIdentityError`] when the token is empty or is not printable ASCII.
    pub fn new(token: &str) -> Result<Self, EpochIdentityError> {
        if token.is_empty() {
            return Err(EpochIdentityError::Empty);
        }
        if let Some((index, character)) = token.char_indices().find(|(_, c)| !c.is_ascii_graphic())
        {
            return Err(EpochIdentityError::NonCanonical { index, character });
        }
        Ok(Self(token.to_owned()))
    }

    /// The canonical token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EpochIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a string is not a usable epoch identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochIdentityError {
    /// The token was empty. An artifact cannot declare an unnamed epoch; the typed
    /// absence is [`EpochBinding::Unpinned`], not an empty identity.
    Empty,
    /// The token contained a character outside printable ASCII, so it has more than one
    /// possible spelling (ADR-0013 canonical identity).
    NonCanonical {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
}

impl fmt::Display for EpochIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("epoch identity is empty"),
            Self::NonCanonical { index, character } => write!(
                f,
                "epoch identity has a non-canonical character {character:?} at byte {index}"
            ),
        }
    }
}

impl core::error::Error for EpochIdentityError {}

// --- the five content-identity epochs -------------------------------------------------

/// Declare one content-identity epoch newtype.
///
/// Five of the six epochs are structurally identical — an [`EpochIdentity`] tagged with
/// its [`EpochKind`] — and differ only in what they version. They are separate types,
/// with no conversions between them, because RFC 0026 requires that they never be
/// conflated.
macro_rules! content_identity_epoch {
    ($(#[$meta:meta])* $name:ident => $kind:expr) => {
        $(#[$meta])*
        ///
        /// Equality is exact identity equality; there is no ordering (plan §4.6 — see
        /// the module documentation). Succession is [`EpochAdvance`].
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(EpochIdentity);

        impl $name {
            /// Which of the six epochs this type is.
            pub const KIND: EpochKind = $kind;

            /// Build this epoch from its canonical token.
            ///
            /// # Errors
            ///
            /// Returns [`EpochIdentityError`] when the token is not a canonical epoch
            /// identity.
            pub fn new(token: &str) -> Result<Self, EpochIdentityError> {
                EpochIdentity::new(token).map(Self)
            }

            /// Build this epoch from an already-validated identity.
            #[must_use]
            pub const fn from_identity(identity: EpochIdentity) -> Self {
                Self(identity)
            }

            /// This epoch's content identity.
            #[must_use]
            pub const fn identity(&self) -> &EpochIdentity {
                &self.0
            }

            /// This epoch's canonical token.
            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Record an advance from this epoch to `next`, carrying the typed
            /// compatibility statement plan §4.6 requires each advance to publish.
            ///
            /// # Errors
            ///
            /// Returns [`EpochAdvanceError::NotAnAdvance`] when `next` is this epoch.
            /// An advance produces a *new* identity; nothing is rewritten in place.
            pub fn advance_to(
                &self,
                next: &Self,
                compatibility: Compatibility,
            ) -> Result<EpochAdvance, EpochAdvanceError> {
                EpochAdvance::new(Self::KIND, self.identity(), next.identity(), compatibility)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

content_identity_epoch! {
    /// The meaning of evaluation.
    ///
    /// A workspace snapshot pins one (plan §4.2), a continuation resumes only under its
    /// own (plan §9.6), and an incremental result claiming exactness "must match clean
    /// evaluation for the same snapshot and semantic epoch" (plan §3). RFC 0019 spells
    /// one `cml/0.2`.
    SemanticEpoch => EpochKind::Semantic
}

content_identity_epoch! {
    /// The Intent Contract vocabulary and the policy tables interpreting it.
    ///
    /// Named as an independently versioned epoch by RFC 0026 ("IDL and versioning").
    /// Intent Contracts live in the intent registry outside every writable snapshot and
    /// converge through signed bundles whose acceptance chains are policy-checked (plan
    /// §4.2, §4.2.1); this epoch is the identity of the vocabulary those contracts and
    /// tables are written against. Its advance policy is expanded by PR 0, which turns
    /// the RFC summaries into normative specifications; nothing beyond RFC 0026's
    /// independence requirement is asserted here.
    IntentEpoch => EpochKind::Intent
}

content_identity_epoch! {
    /// The evidence-graph and receipt schemas an artifact declares itself under.
    ///
    /// > Evidence and receipt schemas MUST remain readable by every future verifier for
    /// > their declared schema epoch, regardless of which protocol majors the daemon
    /// > still serves […] Retiring a protocol major never orphans an artifact.
    /// >
    /// > — RFC 0026, "Version window, budget updates, and evidence subscriptions"
    ///
    /// An artifact declaring an unknown or incompatible epoch is rejected with the typed
    /// `EpochUnsupported`, "never best-effort decoding (docs/09 T13)" — which is why
    /// this type has no lenient constructor and no ordering to guess compatibility from.
    EvidenceEpoch => EpochKind::Evidence
}

content_identity_epoch! {
    /// The Lean toolchain and theorem-package closure.
    ///
    /// > The dossier pins `v4.32.1` […] The pin advances only through a proof epoch that
    /// > rebuilds all theorem packages and records any changed axioms or performance.
    /// >
    /// > — `notes/plan/docs/23_LEAN4_FORMALIZATION_PROGRAM.md`
    ///
    /// Every strongest-assurance receipt records it alongside the semantic epoch
    /// (ADR-0035, RFC 0024), and INV-014 makes checker identity independently
    /// re-derivable from it.
    ProofEpoch => EpochKind::Proof
}

content_identity_epoch! {
    /// The pinned upstream corpus revision and oracle toolchain.
    ///
    /// > Each corpus epoch pins: upstream repository and commit; all submodule commits;
    /// > exact TLA+ toolchain/TLC version; exact Apalache and TLAPS versions where used;
    /// > expected model configurations and resource budgets; generated oracle artifacts
    /// > and their hashes.
    /// >
    /// > Upstream movement creates a proposed epoch. It does not mutate historical
    /// > evidence.
    /// >
    /// > — `notes/plan/corpus/tla-examples/CORPUS_POLICY.md`
    ///
    /// RFC 0019 spells one `tla-examples@91c22ea…`; RFC 0024 binds it into proof
    /// receipts.
    CorpusEpoch => EpochKind::Corpus
}

// --- the protocol epoch ---------------------------------------------------------------

/// Wire compatibility of the `continuumd` native protocol.
///
/// > `protocol_version` (e.g. `"3.0"`) is negotiated at connection open: the client
/// > sends its supported range; the daemon selects the highest common version or rejects
/// > with `ProtocolVersionUnsupported`.
/// >
/// > — RFC 0026, "IDL and versioning"
///
/// Unlike the other five epochs this one is ordered: "the highest common version" and
/// the majors `N`/`N−1` service window are both order predicates. It is also *not* part
/// of workspace-snapshot identity — it belongs to the connection and result envelope
/// (plan §25 SD debt entry; `plan.review.5` A11–A12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolEpoch {
    major: u32,
    minor: u32,
}

impl ProtocolEpoch {
    /// Build a protocol epoch from its major and minor components.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// The major component. The daemon serves majors `N` and `N−1`.
    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    /// The minor component.
    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }

    /// This epoch's content identity, which is its canonical `major.minor` spelling.
    ///
    /// # Panics
    ///
    /// Never: the rendered form is always printable ASCII.
    #[must_use]
    pub fn identity(self) -> EpochIdentity {
        EpochIdentity::new(&self.to_string()).expect("a rendered protocol epoch is printable ASCII")
    }

    /// Select the highest version the daemon supports that the client also accepts.
    ///
    /// `supported` is the daemon's set; `offered` is the client's supported range. A
    /// [`None`] result is the `ProtocolVersionUnsupported` rejection of RFC 0026 — the
    /// daemon never falls back to a version outside the client's range.
    #[must_use]
    pub fn negotiate(supported: &[Self], offered: ProtocolRange) -> Option<Self> {
        supported
            .iter()
            .copied()
            .filter(|candidate| offered.contains(*candidate))
            .max()
    }

    /// Record an advance from this epoch to `next` with its compatibility statement.
    ///
    /// # Errors
    ///
    /// Returns [`EpochAdvanceError::NotAnAdvance`] when `next` is this epoch.
    pub fn advance_to(
        self,
        next: Self,
        compatibility: Compatibility,
    ) -> Result<EpochAdvance, EpochAdvanceError> {
        EpochAdvance::new(
            EpochKind::Protocol,
            &self.identity(),
            &next.identity(),
            compatibility,
        )
    }
}

impl fmt::Display for ProtocolEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for ProtocolEpoch {
    type Err = ProtocolEpochError;

    /// Parse the wire spelling `major.minor`.
    ///
    /// Exactly one separator, decimal digits only, and no leading zeros, so that parsing
    /// and [`Display`](fmt::Display) round-trip: an epoch has one spelling (ADR-0013).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (major, minor) = text.split_once('.').ok_or(ProtocolEpochError::Malformed)?;
        Ok(Self {
            major: parse_component(major)?,
            minor: parse_component(minor)?,
        })
    }
}

fn parse_component(text: &str) -> Result<u32, ProtocolEpochError> {
    if text.is_empty() || (text.len() > 1 && text.starts_with('0')) {
        return Err(ProtocolEpochError::Malformed);
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ProtocolEpochError::Malformed);
    }
    text.parse().map_err(|_| ProtocolEpochError::Malformed)
}

/// Why a string is not a protocol epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolEpochError {
    /// The text is not a canonical `major.minor` spelling.
    Malformed,
}

impl fmt::Display for ProtocolEpochError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("protocol epoch is not a canonical major.minor version")
    }
}

impl core::error::Error for ProtocolEpochError {}

/// The inclusive range of protocol epochs a client accepts at connection open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolRange {
    low: ProtocolEpoch,
    high: ProtocolEpoch,
}

impl ProtocolRange {
    /// Build a range, returning [`None`] when `high` precedes `low`.
    #[must_use]
    pub fn new(low: ProtocolEpoch, high: ProtocolEpoch) -> Option<Self> {
        (low <= high).then_some(Self { low, high })
    }

    /// The lowest accepted epoch.
    #[must_use]
    pub const fn low(self) -> ProtocolEpoch {
        self.low
    }

    /// The highest accepted epoch.
    #[must_use]
    pub const fn high(self) -> ProtocolEpoch {
        self.high
    }

    /// Whether `candidate` falls inside the range.
    #[must_use]
    pub fn contains(self, candidate: ProtocolEpoch) -> bool {
        self.low <= candidate && candidate <= self.high
    }
}

/// The protocol-major service window.
///
/// > The daemon MUST serve protocol majors N and N−1 concurrently; a client on the
/// > previous major is rejected only when it falls outside this window
/// > (`ProtocolVersionUnsupported`).
/// >
/// > — RFC 0026, "Version window, budget updates, and evidence subscriptions"
///
/// The window governs *service*, not artifact readability: "Retiring a protocol major
/// never orphans an artifact" — that is the [`EvidenceEpoch`]'s job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolWindow {
    current_major: u32,
}

impl ProtocolWindow {
    /// The window a daemon at `current_major` serves.
    #[must_use]
    pub const fn new(current_major: u32) -> Self {
        Self { current_major }
    }

    /// The newest served major, `N`.
    #[must_use]
    pub const fn current_major(self) -> u32 {
        self.current_major
    }

    /// Whether the daemon serves `epoch`: its major is `N` or `N−1`.
    #[must_use]
    pub const fn serves(self, epoch: ProtocolEpoch) -> bool {
        epoch.major == self.current_major
            || (self.current_major > 0 && epoch.major == self.current_major - 1)
    }
}

// --- advancing an epoch ---------------------------------------------------------------

/// The typed compatibility statement an epoch advance publishes per artifact class.
///
/// > each advance publishes a typed per-artifact-class compatibility statement —
/// > `Preserved | Revalidate | Incompatible` — and an estimated invalidation blast radius
/// > by artifact class before it is applied
/// >
/// > — plan §4.6
///
/// The artifact classes themselves are plan §4.4's handle taxonomy and land with PR 2's
/// content-addressed artifacts; this type is the per-class verdict those statements
/// carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compatibility {
    /// Artifacts of the class keep their meaning and their evidence standing.
    Preserved,
    /// Artifacts remain readable but their claims must be re-established under the new
    /// epoch before they are relied on again.
    Revalidate,
    /// Artifacts of the class do not carry across the advance. Nothing is rewritten in
    /// place: re-derivation produces new identities linked by `SUPERSEDES` edges.
    Incompatible,
}

/// A published advance from one epoch identity to its successor.
///
/// This is the only successor relation the module defines. Plan §4.6 makes advancing an
/// *event* with a published statement rather than a computation over identities, so
/// [`EpochAdvance`] records who advanced, from what, to what, and with which
/// compatibility verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochAdvance {
    kind: EpochKind,
    from: EpochIdentity,
    to: EpochIdentity,
    compatibility: Compatibility,
}

impl EpochAdvance {
    /// Record an advance.
    ///
    /// # Errors
    ///
    /// Returns [`EpochAdvanceError::NotAnAdvance`] when `from` and `to` are the same
    /// identity. Advancing never mutates an existing artifact, so it always yields a new
    /// identity.
    pub fn new(
        kind: EpochKind,
        from: &EpochIdentity,
        to: &EpochIdentity,
        compatibility: Compatibility,
    ) -> Result<Self, EpochAdvanceError> {
        if from == to {
            return Err(EpochAdvanceError::NotAnAdvance);
        }
        Ok(Self {
            kind,
            from: from.clone(),
            to: to.clone(),
            compatibility,
        })
    }

    /// Which epoch advanced.
    #[must_use]
    pub const fn kind(&self) -> EpochKind {
        self.kind
    }

    /// The predecessor identity, which remains valid for artifacts pinned to it.
    ///
    /// Plan §4.6: re-derived artifacts are "linked to their predecessors by `SUPERSEDES`
    /// edges".
    #[must_use]
    pub const fn predecessor(&self) -> &EpochIdentity {
        &self.from
    }

    /// The successor identity.
    #[must_use]
    pub const fn successor(&self) -> &EpochIdentity {
        &self.to
    }

    /// The published compatibility verdict.
    #[must_use]
    pub const fn compatibility(&self) -> Compatibility {
        self.compatibility
    }
}

/// Why an advance is not a valid advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochAdvanceError {
    /// The successor identity equals the predecessor. An epoch advance always produces a
    /// new identity (plan §4.6).
    NotAnAdvance,
}

impl fmt::Display for EpochAdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("epoch advance does not change the epoch identity")
    }
}

impl core::error::Error for EpochAdvanceError {}

// --- the epoch set carried by a result ------------------------------------------------

/// What a result pins one epoch to.
///
/// [`Unpinned`](Self::Unpinned) is a *named* absence, not a missing field. An
/// unsupported or empty task still names every epoch; the ones it cannot pin read
/// `Unpinned`, mirroring the assurance envelope's rule that "every dimension names a
/// producer or reads `Unsupported`" (RFC 0026, result envelope) and the INV-007 omission
/// manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochBinding<'a> {
    /// The identity this result is pinned to.
    Pinned(&'a EpochIdentity),
    /// This result pins no identity for the epoch, and says so.
    Unpinned,
}

impl<'a> EpochBinding<'a> {
    /// The pinned token, or [`None`] when unpinned.
    #[must_use]
    pub fn as_str(self) -> Option<&'a str> {
        match self {
            Self::Pinned(identity) => Some(identity.as_str()),
            Self::Unpinned => None,
        }
    }

    /// Whether this binding names an identity.
    #[must_use]
    pub const fn is_pinned(self) -> bool {
        matches!(self, Self::Pinned(_))
    }
}

/// One named entry of an [`EpochSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochEntry<'a> {
    /// Which epoch this entry names.
    pub kind: EpochKind,
    /// What the result pins it to.
    pub binding: EpochBinding<'a>,
}

/// The `epochs` object of a result envelope: the epochs a result is pinned to.
///
/// > | `epochs` | object | semantic/engine/proof epochs the result is pinned to |
/// >
/// > — RFC 0026, "Result envelope"
///
/// Every constructor path yields a set that names all six kinds. [`entries`](Self::entries)
/// always returns six entries in [`EpochKind::ALL`] order, so a result cannot quietly
/// drop an epoch it does not know — the PR-1 exit condition that an unsupported empty
/// task "returns a valid machine result naming every epoch"
/// (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpochSet {
    protocol: Option<ProtocolEpoch>,
    protocol_identity: Option<EpochIdentity>,
    semantic: Option<SemanticEpoch>,
    intent: Option<IntentEpoch>,
    evidence: Option<EvidenceEpoch>,
    proof: Option<ProofEpoch>,
    corpus: Option<CorpusEpoch>,
}

impl EpochSet {
    /// A set that pins nothing: every epoch is named and every one reads
    /// [`EpochBinding::Unpinned`].
    ///
    /// This is what an unsupported or empty task reports. It carries no success flag,
    /// because pinning nothing is not a success.
    #[must_use]
    pub fn unpinned() -> Self {
        Self::default()
    }

    /// Pin the protocol epoch.
    #[must_use]
    pub fn with_protocol(mut self, epoch: ProtocolEpoch) -> Self {
        self.protocol_identity = Some(epoch.identity());
        self.protocol = Some(epoch);
        self
    }

    /// Pin the semantic epoch.
    #[must_use]
    pub fn with_semantic(mut self, epoch: SemanticEpoch) -> Self {
        self.semantic = Some(epoch);
        self
    }

    /// Pin the intent epoch.
    #[must_use]
    pub fn with_intent(mut self, epoch: IntentEpoch) -> Self {
        self.intent = Some(epoch);
        self
    }

    /// Pin the evidence epoch.
    #[must_use]
    pub fn with_evidence(mut self, epoch: EvidenceEpoch) -> Self {
        self.evidence = Some(epoch);
        self
    }

    /// Pin the proof epoch.
    #[must_use]
    pub fn with_proof(mut self, epoch: ProofEpoch) -> Self {
        self.proof = Some(epoch);
        self
    }

    /// Pin the corpus epoch.
    #[must_use]
    pub fn with_corpus(mut self, epoch: CorpusEpoch) -> Self {
        self.corpus = Some(epoch);
        self
    }

    /// The pinned protocol epoch, if any.
    #[must_use]
    pub const fn protocol(&self) -> Option<ProtocolEpoch> {
        self.protocol
    }

    /// The pinned semantic epoch, if any.
    #[must_use]
    pub const fn semantic(&self) -> Option<&SemanticEpoch> {
        self.semantic.as_ref()
    }

    /// The pinned intent epoch, if any.
    #[must_use]
    pub const fn intent(&self) -> Option<&IntentEpoch> {
        self.intent.as_ref()
    }

    /// The pinned evidence epoch, if any.
    #[must_use]
    pub const fn evidence(&self) -> Option<&EvidenceEpoch> {
        self.evidence.as_ref()
    }

    /// The pinned proof epoch, if any.
    #[must_use]
    pub const fn proof(&self) -> Option<&ProofEpoch> {
        self.proof.as_ref()
    }

    /// The pinned corpus epoch, if any.
    #[must_use]
    pub const fn corpus(&self) -> Option<&CorpusEpoch> {
        self.corpus.as_ref()
    }

    /// The binding for one epoch kind.
    #[must_use]
    pub fn binding(&self, kind: EpochKind) -> EpochBinding<'_> {
        let identity = match kind {
            EpochKind::Protocol => self.protocol_identity.as_ref(),
            EpochKind::Semantic => self.semantic.as_ref().map(SemanticEpoch::identity),
            EpochKind::Intent => self.intent.as_ref().map(IntentEpoch::identity),
            EpochKind::Evidence => self.evidence.as_ref().map(EvidenceEpoch::identity),
            EpochKind::Proof => self.proof.as_ref().map(ProofEpoch::identity),
            EpochKind::Corpus => self.corpus.as_ref().map(CorpusEpoch::identity),
        };
        identity.map_or(EpochBinding::Unpinned, EpochBinding::Pinned)
    }

    /// Every epoch, named, in [`EpochKind::ALL`] order.
    ///
    /// The array length is six by construction: a result cannot report five epochs.
    #[must_use]
    pub fn entries(&self) -> [EpochEntry<'_>; 6] {
        EpochKind::ALL.map(|kind| EpochEntry {
            kind,
            binding: self.binding(kind),
        })
    }

    /// The epochs this set does not pin, in [`EpochKind::ALL`] order.
    ///
    /// This is the omission list a machine result reports (INV-007) rather than a
    /// success flag.
    #[must_use]
    pub fn unpinned_kinds(&self) -> Vec<EpochKind> {
        EpochKind::ALL
            .into_iter()
            .filter(|kind| !self.binding(*kind).is_pinned())
            .collect()
    }

    /// Check `current` against the epochs this set pins, for a continuation resume.
    ///
    /// > Resume rejects mismatched snapshots or epochs.
    /// >
    /// > — plan §9.6; continuations "MUST be rejected with `ContinuationEpochMismatch` on
    /// > mismatch (never silently re-run)" (RFC 0026)
    ///
    /// `self` is the continuation's pinned set. An epoch the continuation pins must be
    /// pinned identically in `current`; an epoch it leaves unpinned constrains nothing,
    /// because the continuation resumes "only under their pinned epoch" (plan §4.6) and
    /// declared nothing to resume under. An epoch pinned by the continuation but absent
    /// from `current` is a mismatch, not a permission to decode best-effort
    /// (docs/09 T13).
    ///
    /// Returns the first disagreeing kind in [`EpochKind::ALL`] order, or [`None`] when
    /// the resume is admissible.
    #[must_use]
    pub fn first_mismatch(&self, current: &Self) -> Option<EpochKind> {
        EpochKind::ALL.into_iter().find(|kind| {
            match (self.binding(*kind), current.binding(*kind)) {
                (EpochBinding::Unpinned, _) => false,
                (EpochBinding::Pinned(pinned), EpochBinding::Pinned(offered)) => pinned != offered,
                (EpochBinding::Pinned(_), EpochBinding::Unpinned) => true,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(token: &str) -> EpochIdentity {
        EpochIdentity::new(token).expect("test token is a canonical identity")
    }

    // --- PR-1 exit seed: every epoch is named ----------------------------------------

    #[test]
    fn epoch_kind_names_every_epoch_pr_1_lists() {
        // START_HERE_IMPLEMENTATION.md, PR 1: "protocol, semantic, intent, evidence,
        // proof, and corpus epochs".
        let names: Vec<&str> = EpochKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(
            names,
            [
                "protocol", "semantic", "intent", "evidence", "proof", "corpus"
            ]
        );
    }

    #[test]
    fn epoch_kinds_are_distinct() {
        for (index, kind) in EpochKind::ALL.iter().enumerate() {
            for other in &EpochKind::ALL[index + 1..] {
                assert_ne!(kind, other);
                assert_ne!(kind.as_str(), other.as_str());
            }
        }
    }

    #[test]
    fn an_unpinned_set_still_names_every_epoch() {
        // PR 1 exit: an unsupported empty task returns a machine result naming every
        // epoch — and no success flag.
        let set = EpochSet::unpinned();
        let entries = set.entries();

        assert_eq!(entries.len(), EpochKind::ALL.len());
        for (entry, kind) in entries.iter().zip(EpochKind::ALL) {
            assert_eq!(entry.kind, kind);
            assert_eq!(entry.binding, EpochBinding::Unpinned);
        }
        assert_eq!(set.unpinned_kinds(), EpochKind::ALL.to_vec());
    }

    #[test]
    fn a_partially_pinned_set_still_names_every_epoch() {
        let set = EpochSet::unpinned()
            .with_protocol(ProtocolEpoch::new(3, 0))
            .with_semantic(SemanticEpoch::new("cml/0.2").expect("valid"));

        let entries = set.entries();
        assert_eq!(entries.len(), 6);
        assert_eq!(entries[0].binding, EpochBinding::Pinned(&identity("3.0")));
        assert_eq!(
            entries[1].binding,
            EpochBinding::Pinned(&identity("cml/0.2"))
        );
        for entry in &entries[2..] {
            assert_eq!(entry.binding, EpochBinding::Unpinned);
        }
        assert_eq!(
            set.unpinned_kinds(),
            vec![
                EpochKind::Intent,
                EpochKind::Evidence,
                EpochKind::Proof,
                EpochKind::Corpus
            ]
        );
    }

    #[test]
    fn a_fully_pinned_set_omits_nothing() {
        let set = EpochSet::unpinned()
            .with_protocol(ProtocolEpoch::new(3, 0))
            .with_semantic(SemanticEpoch::new("cml/0.2").expect("valid"))
            .with_intent(IntentEpoch::new("intent/0.1").expect("valid"))
            .with_evidence(EvidenceEpoch::new("continuum-evidence-graph/1").expect("valid"))
            .with_proof(ProofEpoch::new("leanprover/lean4:v4.32.1").expect("valid"))
            .with_corpus(CorpusEpoch::new("tla-examples@91c22ea").expect("valid"));

        assert!(set.unpinned_kinds().is_empty());
        assert!(set.entries().iter().all(|entry| entry.binding.is_pinned()));
        assert_eq!(set.protocol(), Some(ProtocolEpoch::new(3, 0)));
        assert_eq!(
            set.proof().map(ProofEpoch::as_str),
            Some("leanprover/lean4:v4.32.1")
        );
    }

    // --- epoch identity ---------------------------------------------------------------

    #[test]
    fn identities_from_the_dossier_are_accepted() {
        // RFC 0019 corpus port manifest, docs/23 Lean pin, RFC 0019 native semantics.
        for token in [
            "cml/0.2",
            "tla-examples@91c22ea",
            "leanprover/lean4:v4.32.1",
            "continuum-proof-receipt/v1",
        ] {
            assert_eq!(EpochIdentity::new(token).expect("valid").as_str(), token);
        }
    }

    #[test]
    fn an_empty_identity_is_rejected() {
        assert_eq!(EpochIdentity::new(""), Err(EpochIdentityError::Empty));
    }

    #[test]
    fn a_non_canonical_identity_is_rejected() {
        // Whitespace, control characters, and non-ASCII admit several spellings of one
        // identity, which a content identity may not do (ADR-0013).
        assert_eq!(
            EpochIdentity::new("cml 0.2"),
            Err(EpochIdentityError::NonCanonical {
                index: 3,
                character: ' '
            })
        );
        assert!(matches!(
            EpochIdentity::new("cml/0.2\n"),
            Err(EpochIdentityError::NonCanonical { .. })
        ));
        assert!(matches!(
            EpochIdentity::new("cml/0.2\u{2010}"),
            Err(EpochIdentityError::NonCanonical { .. })
        ));
    }

    #[test]
    fn identity_equality_is_exact() {
        assert_eq!(identity("cml/0.2"), identity("cml/0.2"));
        assert_ne!(identity("cml/0.2"), identity("CML/0.2"));
        assert_ne!(identity("cml/0.2"), identity("cml/0.20"));
    }

    // --- the six are not conflated ----------------------------------------------------

    #[test]
    fn each_epoch_type_carries_its_own_kind() {
        assert_eq!(SemanticEpoch::KIND, EpochKind::Semantic);
        assert_eq!(IntentEpoch::KIND, EpochKind::Intent);
        assert_eq!(EvidenceEpoch::KIND, EpochKind::Evidence);
        assert_eq!(ProofEpoch::KIND, EpochKind::Proof);
        assert_eq!(CorpusEpoch::KIND, EpochKind::Corpus);
        assert_eq!(ProtocolEpoch::new(3, 0).identity().as_str(), "3.0");
    }

    #[test]
    fn the_same_token_under_two_kinds_stays_two_epochs() {
        // RFC 0026: the six "MUST NOT be conflated". The types do not unify even when
        // their tokens coincide, and the set files them separately.
        let semantic = SemanticEpoch::new("v1").expect("valid");
        let proof = ProofEpoch::new("v1").expect("valid");
        assert_eq!(semantic.identity(), proof.identity());
        assert_ne!(SemanticEpoch::KIND, ProofEpoch::KIND);

        let set = EpochSet::unpinned().with_semantic(semantic);
        assert!(set.binding(EpochKind::Semantic).is_pinned());
        assert!(!set.binding(EpochKind::Proof).is_pinned());
    }

    #[test]
    fn epochs_render_as_their_token() {
        assert_eq!(
            ProofEpoch::new("leanprover/lean4:v4.32.1")
                .expect("valid")
                .to_string(),
            "leanprover/lean4:v4.32.1"
        );
        assert_eq!(
            CorpusEpoch::new("tla-examples@91c22ea")
                .expect("valid")
                .to_string(),
            "tla-examples@91c22ea"
        );
    }

    // --- protocol epoch ---------------------------------------------------------------

    #[test]
    fn protocol_epochs_order_by_major_then_minor() {
        assert!(ProtocolEpoch::new(2, 9) < ProtocolEpoch::new(3, 0));
        assert!(ProtocolEpoch::new(3, 0) < ProtocolEpoch::new(3, 1));
        assert_eq!(ProtocolEpoch::new(3, 1), ProtocolEpoch::new(3, 1));
    }

    #[test]
    fn protocol_epochs_round_trip_their_wire_spelling() {
        // RFC 0026: `protocol_version` (e.g. "3.0").
        let epoch: ProtocolEpoch = "3.0".parse().expect("valid");
        assert_eq!(epoch, ProtocolEpoch::new(3, 0));
        assert_eq!(epoch.to_string(), "3.0");
        assert_eq!(
            "12.34".parse::<ProtocolEpoch>().expect("valid"),
            ProtocolEpoch::new(12, 34)
        );
    }

    #[test]
    fn non_canonical_protocol_spellings_are_rejected() {
        for text in ["", "3", "3.", ".0", "3.0.1", "03.1", "3.01", "v3.0", "3.-1"] {
            assert_eq!(
                text.parse::<ProtocolEpoch>(),
                Err(ProtocolEpochError::Malformed),
                "{text:?} should not parse"
            );
        }
    }

    #[test]
    fn negotiation_selects_the_highest_common_version() {
        let supported = [
            ProtocolEpoch::new(2, 4),
            ProtocolEpoch::new(3, 0),
            ProtocolEpoch::new(3, 1),
        ];
        let offered = ProtocolRange::new(ProtocolEpoch::new(2, 0), ProtocolEpoch::new(3, 0))
            .expect("ordered range");
        assert_eq!(
            ProtocolEpoch::negotiate(&supported, offered),
            Some(ProtocolEpoch::new(3, 0))
        );
    }

    #[test]
    fn negotiation_fails_closed_when_there_is_no_common_version() {
        // RFC 0026: reject with `ProtocolVersionUnsupported` — never fall back outside
        // the client's range.
        let supported = [ProtocolEpoch::new(3, 0), ProtocolEpoch::new(3, 1)];
        let offered = ProtocolRange::new(ProtocolEpoch::new(1, 0), ProtocolEpoch::new(2, 9))
            .expect("ordered range");
        assert_eq!(ProtocolEpoch::negotiate(&supported, offered), None);
    }

    #[test]
    fn an_inverted_protocol_range_is_not_a_range() {
        assert!(ProtocolRange::new(ProtocolEpoch::new(3, 1), ProtocolEpoch::new(3, 0)).is_none());
        let range = ProtocolRange::new(ProtocolEpoch::new(3, 0), ProtocolEpoch::new(3, 1))
            .expect("ordered range");
        assert_eq!(range.low(), ProtocolEpoch::new(3, 0));
        assert_eq!(range.high(), ProtocolEpoch::new(3, 1));
        assert!(range.contains(ProtocolEpoch::new(3, 1)));
        assert!(!range.contains(ProtocolEpoch::new(2, 9)));
    }

    #[test]
    fn the_service_window_covers_majors_n_and_n_minus_one() {
        let window = ProtocolWindow::new(3);
        assert_eq!(window.current_major(), 3);
        assert!(window.serves(ProtocolEpoch::new(3, 7)));
        assert!(window.serves(ProtocolEpoch::new(2, 0)));
        assert!(!window.serves(ProtocolEpoch::new(1, 9)));
        assert!(!window.serves(ProtocolEpoch::new(4, 0)));
    }

    #[test]
    fn the_first_major_has_no_predecessor_to_serve() {
        let window = ProtocolWindow::new(0);
        assert!(window.serves(ProtocolEpoch::new(0, 1)));
        assert!(!window.serves(ProtocolEpoch::new(1, 0)));
    }

    // --- advancing --------------------------------------------------------------------

    #[test]
    fn an_advance_records_kind_predecessor_successor_and_compatibility() {
        let from = SemanticEpoch::new("cml/0.2").expect("valid");
        let to = SemanticEpoch::new("cml/0.3").expect("valid");
        let advance = from
            .advance_to(&to, Compatibility::Revalidate)
            .expect("a real advance");

        assert_eq!(advance.kind(), EpochKind::Semantic);
        assert_eq!(advance.predecessor(), from.identity());
        assert_eq!(advance.successor(), to.identity());
        assert_eq!(advance.compatibility(), Compatibility::Revalidate);
        // Plan §4.6: the predecessor is untouched; artifacts pinned to it stay valid.
        assert_eq!(from.as_str(), "cml/0.2");
    }

    #[test]
    fn an_advance_to_the_same_identity_is_rejected() {
        let epoch = CorpusEpoch::new("tla-examples@91c22ea").expect("valid");
        assert_eq!(
            epoch.advance_to(&epoch.clone(), Compatibility::Preserved),
            Err(EpochAdvanceError::NotAnAdvance)
        );
    }

    #[test]
    fn every_compatibility_verdict_is_expressible() {
        let from = EvidenceEpoch::new("continuum-evidence-graph/1").expect("valid");
        let to = EvidenceEpoch::new("continuum-evidence-graph/2").expect("valid");
        for verdict in [
            Compatibility::Preserved,
            Compatibility::Revalidate,
            Compatibility::Incompatible,
        ] {
            assert_eq!(
                from.advance_to(&to, verdict)
                    .expect("a real advance")
                    .compatibility(),
                verdict
            );
        }
    }

    #[test]
    fn a_protocol_epoch_advance_is_recorded_the_same_way() {
        let advance = ProtocolEpoch::new(3, 0)
            .advance_to(ProtocolEpoch::new(4, 0), Compatibility::Incompatible)
            .expect("a real advance");
        assert_eq!(advance.kind(), EpochKind::Protocol);
        assert_eq!(advance.predecessor().as_str(), "3.0");
        assert_eq!(advance.successor().as_str(), "4.0");
    }

    // --- continuation resume ----------------------------------------------------------

    #[test]
    fn a_resume_under_the_pinned_epochs_is_admissible() {
        let pinned = EpochSet::unpinned()
            .with_semantic(SemanticEpoch::new("cml/0.2").expect("valid"))
            .with_proof(ProofEpoch::new("leanprover/lean4:v4.32.1").expect("valid"));
        let current = pinned
            .clone()
            .with_corpus(CorpusEpoch::new("tla-examples@91c22ea").expect("valid"));

        assert_eq!(pinned.first_mismatch(&current), None);
    }

    #[test]
    fn a_resume_under_a_changed_epoch_is_rejected() {
        // plan §9.6 / RFC 0026 `ContinuationEpochMismatch`: never silently re-run.
        let pinned =
            EpochSet::unpinned().with_semantic(SemanticEpoch::new("cml/0.2").expect("valid"));
        let current =
            EpochSet::unpinned().with_semantic(SemanticEpoch::new("cml/0.3").expect("valid"));

        assert_eq!(pinned.first_mismatch(&current), Some(EpochKind::Semantic));
    }

    #[test]
    fn a_pinned_epoch_the_daemon_no_longer_holds_is_rejected() {
        // docs/09 T13: unknown breaking epoch is a typed rejection, not best-effort.
        let pinned = EpochSet::unpinned()
            .with_proof(ProofEpoch::new("leanprover/lean4:v4.32.1").expect("valid"));
        assert_eq!(
            pinned.first_mismatch(&EpochSet::unpinned()),
            Some(EpochKind::Proof)
        );
    }

    #[test]
    fn an_epoch_the_continuation_never_pinned_constrains_nothing() {
        let pinned = EpochSet::unpinned();
        let current = EpochSet::unpinned()
            .with_corpus(CorpusEpoch::new("tla-examples@91c22ea").expect("valid"));
        assert_eq!(pinned.first_mismatch(&current), None);
    }

    #[test]
    fn the_first_mismatch_is_reported_in_pr_1_order() {
        let pinned = EpochSet::unpinned()
            .with_intent(IntentEpoch::new("intent/0.1").expect("valid"))
            .with_proof(ProofEpoch::new("lean/1").expect("valid"));
        let current = EpochSet::unpinned()
            .with_intent(IntentEpoch::new("intent/0.2").expect("valid"))
            .with_proof(ProofEpoch::new("lean/2").expect("valid"));

        assert_eq!(pinned.first_mismatch(&current), Some(EpochKind::Intent));
    }

    #[test]
    fn error_types_render_and_implement_error() {
        fn assert_error<E: core::error::Error>(error: &E) -> String {
            error.to_string()
        }
        assert!(!assert_error(&EpochIdentityError::Empty).is_empty());
        assert!(!assert_error(&ProtocolEpochError::Malformed).is_empty());
        assert!(!assert_error(&EpochAdvanceError::NotAnAdvance).is_empty());
        assert_eq!(EpochKind::Corpus.to_string(), "corpus");
    }
}
