//! The finite closure certificate, emitted as wire bytes (PR 8, IMPL-05).
//!
//! # What this module produces
//!
//! One byte string: the `CONTCERT` finite-closure certificate for a **closed**
//! exploration of a declared [`Model`]. It is the artifact that carries this engine's
//! answer across plan §20's serialization boundary —
//!
//! > a serialization boundary between every engine and the kernel — a certificate is
//! > checked from its wire form, never from shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! — and it is the whole of what the trusted checking base ever sees of this crate.
//! Nothing else here crosses: `continuum-kernel-core` takes `&[u8]` and has no
//! constructor for a decoded certificate outside its own decoder
//! (`crates/continuum-kernel-core/src/wire.rs:13-18`), so these bytes are the only
//! statement an engine can make to a checker.
//!
//! # Why the format is transcribed rather than imported
//!
//! This crate does **not** depend on `continuum-kernel-core`, and the certificate
//! below is written from the *documented* grammar rather than from the decoder's
//! types. `tools/check_crate_boundaries.py` forbids the edge in the other direction
//! (RULE `certificate-checker-not-search`) and takes no position on this one; the
//! reason not to take it anyway is docs/03 §8:
//!
//! > Independent paths should differ in: language/runtime; state representation;
//! > traversal; arithmetic; parser; certificate decoding; solver.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §8, "Diversity against common-mode
//! >   bugs"
//!
//! A producer and a checker that share a codec share its bugs invisibly. The kernel's
//! own test producer is written under the same rule and says so — "deliberately
//! written against the *documented* wire grammar rather than against the decoder's
//! internals" (`crates/continuum-kernel-core/src/fixture.rs:5-9`) — and this module is
//! the third independent writing of that grammar, checked against the second by
//! `tests/certificate_kernel_differential.rs`.
//!
//! # The grammar, as this module writes it
//!
//! ```text
//! certificate  := header envelope body
//!
//! header       := magic:8 wire_epoch:u16 kind:u16
//! magic        := "CONTCERT"                         -- MAGIC
//! wire_epoch   := 2                                  -- WIRE_EPOCH
//! kind         := 1 finite-closure                   -- FINITE_CLOSURE_KIND
//!
//! envelope     := model_digest:token
//!                 semantic_epoch:token
//!                 property_digest:token
//!                 scope_digest:token
//!                 assumptions_digest:token
//!                 producer:token
//!                 schema_epoch:u16
//!                 domain_pack_count:u16 token*
//!
//! body         := model_len:u32 model                -- Model::identity, byte for byte
//!                 property
//!                 state_count:u32 state*
//!                 row * state_count
//! property     := 1                                  -- state-domain
//!               | 2 predicate:token                  -- invariant
//! state        := i64 * variable_count
//! row          := transition_count:u32 transition*
//! transition   := action:u16 target:u32              -- table index
//! ```
//!
//! — the wire-epoch-2 grammar in `crates/continuum-kernel-core/src/wire.rs`'s module
//! documentation (bn-35y4f). Integers are big-endian and `i64` is two's complement.
//! Epoch 2 carries the model's canonical encoding instead of a separate domain,
//! initial-state list and action list, so the kernel re-derives all three, and every
//! successor, from the model. Three sequences are *strictly* ascending: domain-pack
//! digests, state vectors, and the transitions within one row.
//!
//! **Each arrives already ordered**, which is the point of the two modules underneath
//! this one:
//!
//! | Sequence | Where its order comes from |
//! |---|---|
//! | state vectors | [`Reachable::states`] — "certificate emission is therefore a copy, not a sort" (`bfs.rs:281-286`) |
//! | one successor row | [`Model::successors`], strictly ascending by `(action, target)`; a target's table index preserves that order because the table is ascending |
//! | domain-pack digests | the caller's, and the one sequence this module has to check |
//!
//! [`Ident`] is byte-ordered for exactly this reason, and refuses names the kernel's
//! token decoder would refuse ([`crate::ident`]). So emission sorts nothing, and no
//! ordering argument in this file rests on a comparison written in this file.
//!
//! # Only a closed exploration can be emitted
//!
//! [`emit_finite_closure`] takes a [`ClosedSet`], whose only constructor is
//! [`ClosedSet::of`] over an [`Exploration`], and which is `None` on the
//! [`Exploration::Exhausted`] arm because [`Exploration::closed`] is:
//!
//! > only [`Exploration::Complete`] carries a closed set, and only a closed set can be
//! > certified. [`Exploration::closed`] is the seam that says so in the type rather
//! > than in prose.
//! >
//! > — `crates/continuum-engine-reference/src/bfs.rs:78-81` (bn-3p8u)
//!
//! The field is private and there is no other way to build one, so "a certificate from
//! a truncated exploration" is not a mistake this API can make. That matters because
//! the kernel would catch it only sometimes: a bounded run's explored set is closed
//! *by accident* whenever the bound tripped on a row that discovered nothing new, and
//! a checker cannot tell an accident from a proof.
//!
//! # What is carried, and what is re-derived
//!
//! The three obligations `Init ⊆ S`, `Post(S) ⊆ S`, `S ⊆ P` (RFC 0005, "Closed
//! reachable set") are *the kernel's* to re-derive from these bytes, and it does —
//! over the certificate's own carried relation, never over a summary
//! (`crates/continuum-kernel-core/src/check.rs:92-147`). This module does not check
//! them and does not claim them:
//!
//! - `Init ⊆ S` holds because [`explore`](crate::bfs::explore) seeds the visited set
//!   with [`Model::initial_states`] before it walks;
//! - `Post(S) ⊆ S` holds because the [`Exploration::Complete`] arm *is* the statement
//!   that the queue emptied with every discovered state expanded (`bfs.rs:443-447`);
//! - `S ⊆ P` holds because every state written passed [`Model::successors`]' domain
//!   check on the way in, and the declared domain written beside the table is the same
//!   [`Model::variables`] that check consulted.
//!
//! Re-checking any of them here would be this crate marking its own homework. The
//! independent statement is the kernel's `Verdict`, and it is asserted against these
//! bytes in `tests/certificate_kernel_differential.rs`.
//!
//! One thing *is* re-derived: the successor rows. [`Reachable`] carries the reachable
//! set and its depths, not the transition relation, so each row is recomputed by
//! [`Model::successors`] as it is written. That is a second evaluation of a pure
//! function of the model — the reference path is written for obvious correctness, not
//! speed (`docs/01 §7.1`) — and it has a side benefit: a state that is not a state of
//! *this* model fails that call, so passing a reachable set explored from a different
//! model is [`EmissionError::Evaluation`] rather than a certificate about nothing.
//!
//! # The envelope is the caller's, and that is deliberate
//!
//! > A certificate is valid only relative to: model hash, CIR semantics version,
//! > property hash, domain-pack profile hashes, bounds, assumptions, engine version,
//! > certificate schema version.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Claim envelope"
//!
//! Six of those eight are digests and epochs of artifacts this crate cannot see. It
//! declares no content hasher (ADR-0013 puts canonical identity in `continuum-value`,
//! which this crate deliberately does not depend on — see [`crate::ident`]), it is
//! handed a [`Model`] rather than the CML source that elaborated to it, and it has no
//! idea which domain packs were in force. So [`ClaimEnvelope`] is a struct of borrowed
//! strings the caller supplies, modelled on the trusted-input seam the kernel's own
//! receipt uses for the same reason —
//!
//! > every field of a proof receipt that a self-contained certificate checker
//! > structurally cannot derive […] carried, labelled, and named as trusted, never
//! > invented
//! >
//! > — `crates/continuum-kernel-core/src/receipt.rs:38-42`
//!
//! — and nothing on it is defaulted, guessed, or synthesized. What this module *does*
//! refuse is a malformed one: every field is shape-checked against the kernel's token
//! grammar before a byte is written, so a caller cannot produce an artifact that would
//! be rejected on shape at the far end ([`EnvelopeError`]).
//!
//! The two fields that are *not* the caller's are the two the format fixes:
//! `schema_epoch` must equal the header's [`WIRE_EPOCH`] or the decoder rejects the
//! pair outright (`crates/continuum-kernel-core/src/wire.rs:419-424`), and
//! `property_class` is [`PROPERTY_CLASS_STATE_DOMAIN`] because the property this
//! certificate carries *is* the declared state domain it writes. A settable field
//! whose only other values name a rejection is not a choice.
//!
//! # Determinism (INV-005)
//!
//! Emission is a pure function of `(model, closed set, envelope)`. It reads no clock,
//! draws no randomness, allocates no identity, and iterates only over slices that were
//! ordered before it saw them, so the same three inputs produce byte-identical output —
//! asserted in `tests/certificate_wire.rs`, and the reason a certificate can be
//! content-addressed by a layer above this one.

use core::fmt;

use crate::bfs::{Exploration, MAX_STATES, MAX_TRANSITIONS, Reachable};
use crate::ident::{Ident, IdentError};
use crate::model::{EvaluationError, MAX_ACTIONS, MAX_VARIABLES, Model, State, Step};

/// The eight bytes every certificate begins with.
///
/// `crates/continuum-kernel-core/src/wire.rs:110`.
pub const MAGIC: [u8; 8] = *b"CONTCERT";

/// The wire epoch this module writes: the model-bound epoch 2 (bn-35y4f).
///
/// `crates/continuum-kernel-core/src/wire.rs` `WIRE_EPOCH`. A certificate declaring any other
/// epoch is a *feature* the checker does not implement rather than a rejection
/// (`.../wire.rs:914-919`), which is why this is a constant and not a parameter: an
/// engine that could write an epoch of its choosing could write one no checker knows.
pub const WIRE_EPOCH: u16 = 2;

/// The `kind` code of the finite-closure family.
///
/// `CertificateKind::FiniteClosure` (`crates/continuum-kernel-core/src/verdict.rs:89-113`).
/// The other RFC 0005 families — state typing, LRAT, Alethe, SCC/ranking,
/// counterexample and refinement witnesses — are not this module's; see
/// `crates/continuum-kernel-core/src/lib.rs:47-62`.
pub const FINITE_CLOSURE_KIND: u16 = 1;

/// The `property_class` code of "the safety property is the declared state domain".
///
/// `PropertyClass::StateDomain` in `crates/continuum-kernel-core/src/verdict.rs`: the
/// property is the model's declared state domain.
pub const PROPERTY_CLASS_STATE_DOMAIN: u16 = 1;

/// The `property_class` code of "the safety property is one named predicate of the
/// model" (`PropertyClass::Invariant`), followed on the wire by the predicate's name.
pub const PROPERTY_CLASS_INVARIANT: u16 = 2;

/// The largest model section the kernel will decode (16 MiB).
///
/// `MAX_MODEL_BYTES` from `crates/continuum-kernel-core/src/wire.rs`.
pub const MAX_MODEL_BYTES: usize = 1 << 24;

/// The largest number of domain-pack profile digests an envelope may carry.
///
/// `MAX_DOMAIN_PACKS` from `crates/continuum-kernel-core/src/wire.rs:125`.
pub const MAX_DOMAIN_PACKS: u16 = 64;

/// The largest certificate byte string the kernel will look at (64 MiB).
///
/// `MAX_CERTIFICATE_BYTES` from `crates/continuum-kernel-core/src/wire.rs:119`. It is
/// checked *after* encoding rather than predicted before it: the exact length is a
/// fact about the bytes, and a prediction that disagreed with them would be a second
/// implementation of the format.
pub const MAX_CERTIFICATE_BYTES: usize = 1 << 26;

// ---------------------------------------------------------------------------
// where an emission failed
// ---------------------------------------------------------------------------

/// A named position in the wire form, so a failure says *where*.
///
/// The vocabulary mirrors `continuum-kernel-core`'s `Field`
/// (`crates/continuum-kernel-core/src/verdict.rs:293-298`) at the positions this
/// module can fail at. Two names for one position would be two spellings of one fact,
/// so the spellings are kept identical where both sides have the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// `envelope.model_digest`.
    ModelDigest,
    /// `envelope.semantic_epoch`.
    SemanticEpoch,
    /// `envelope.property_digest`.
    PropertyDigest,
    /// `envelope.scope_digest`.
    ScopeDigest,
    /// `envelope.assumptions_digest`.
    AssumptionsDigest,
    /// `envelope.producer`.
    Producer,
    /// One domain-pack profile digest.
    DomainPack,
    /// `body.variable_count`.
    VariableCount,
    /// `body.state_count`.
    StateCount,
    /// `body.initial_count`.
    InitialCount,
    /// `body.action_count`.
    ActionCount,
    /// One successor row's `transition_count`.
    TransitionCount,
    /// The running total of transitions across every row.
    TransitionTotal,
    /// A transition's action index.
    TransitionAction,
    /// The model section's length.
    ModelLength,
}

impl Field {
    /// The stable lower-case token naming this position.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelDigest => "model-digest",
            Self::SemanticEpoch => "semantic-epoch",
            Self::PropertyDigest => "property-digest",
            Self::ScopeDigest => "scope-digest",
            Self::AssumptionsDigest => "assumptions-digest",
            Self::Producer => "producer",
            Self::DomainPack => "domain-pack",
            Self::VariableCount => "variable-count",
            Self::StateCount => "state-count",
            Self::InitialCount => "initial-count",
            Self::ActionCount => "action-count",
            Self::TransitionCount => "transition-count",
            Self::TransitionTotal => "transition-total",
            Self::TransitionAction => "transition-action",
            Self::ModelLength => "model-length",
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// the claim envelope
// ---------------------------------------------------------------------------

/// The claim envelope a certificate is valid *relative to*, as the caller states it.
///
/// Borrowed strings rather than owned tokens, and every field required rather than
/// defaulted: this is a trusted-input inventory, and its whole discipline is that
/// nothing on it is invented by the crate that writes the certificate. See the module
/// documentation for why each field has to come from outside, and
/// `crates/continuum-kernel-core/src/receipt.rs:38-70` for the same seam on the
/// checking side.
///
/// Every field is checked against the wire form's token grammar — non-empty printable
/// ASCII, at most [`crate::ident::MAX_IDENT_BYTES`] bytes — by
/// [`ClaimEnvelope::validate`], which
/// [`emit_finite_closure`] runs before it writes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimEnvelope<'a> {
    /// RFC 0005 "model hash" — digest of the model the certificate speaks about.
    pub model_digest: &'a str,
    /// RFC 0005 "CIR semantics version" — the semantic epoch the claim is meaningful
    /// under.
    pub semantic_epoch: &'a str,
    /// RFC 0005 "property hash" — digest of the property being established. This
    /// module writes [`PROPERTY_CLASS_STATE_DOMAIN`]; naming *which* state-domain
    /// property that is, canonically, is the caller's.
    pub property_digest: &'a str,
    /// RFC 0005 "bounds" — digest of the declared scope the claim is made within.
    pub scope_digest: &'a str,
    /// RFC 0005 "assumptions" — digest of the assumptions the claim is made under.
    pub assumptions_digest: &'a str,
    /// RFC 0005 "engine version" — identity of the engine build that produced the
    /// certificate. [`PRODUCER`] is this crate's honest spelling of its own name and
    /// version; a caller that knows a build digest should say so instead.
    pub producer: &'a str,
    /// RFC 0005 "domain-pack profile hashes", strictly ascending and at most
    /// [`MAX_DOMAIN_PACKS`] of them. Empty is admissible: the wire form's
    /// `domain_pack_count` has a minimum of zero
    /// (`crates/continuum-kernel-core/src/wire.rs:425`).
    pub domain_pack_digests: &'a [&'a str],
}

/// This crate's name and version, as an engine-version token.
///
/// The one envelope field this crate can state about itself without being told, and it
/// is offered as a constant rather than defaulted into the envelope: a field that
/// filled itself in would make the seam's "nothing here is invented" rule true of
/// seven fields instead of eight. `env!` reads the manifest at compile time, so this
/// is a constant of the build and not an ambient read (INV-005).
pub const PRODUCER: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

impl ClaimEnvelope<'_> {
    /// Check every field against the wire form's token grammar and canonicity rules.
    ///
    /// # Errors
    ///
    /// Every arm of [`EnvelopeError`].
    pub fn validate(&self) -> Result<(), EnvelopeError> {
        Validated::new(self).map(|_| ())
    }
}

/// The envelope as tokens, once every field has been checked.
///
/// Private, and the only thing [`emit_finite_closure`] will write an envelope from:
/// holding the validated form in its own type is what stops a later edit writing the
/// caller's `&str` directly and skipping the check.
#[derive(Debug)]
struct Validated {
    model_digest: Ident,
    semantic_epoch: Ident,
    property_digest: Ident,
    scope_digest: Ident,
    assumptions_digest: Ident,
    producer: Ident,
    domain_pack_digests: Vec<Ident>,
}

impl Validated {
    /// Fields are checked in the order the decoder reads them
    /// (`crates/continuum-kernel-core/src/wire.rs:411-438`), so a caller who gets two
    /// fields wrong is told about the same one every time.
    fn new(envelope: &ClaimEnvelope<'_>) -> Result<Self, EnvelopeError> {
        let token = |field: Field, value: &str| {
            Ident::new(value).map_err(|source| EnvelopeError::MalformedToken { field, source })
        };
        let model_digest = token(Field::ModelDigest, envelope.model_digest)?;
        let semantic_epoch = token(Field::SemanticEpoch, envelope.semantic_epoch)?;
        let property_digest = token(Field::PropertyDigest, envelope.property_digest)?;
        let scope_digest = token(Field::ScopeDigest, envelope.scope_digest)?;
        let assumptions_digest = token(Field::AssumptionsDigest, envelope.assumptions_digest)?;
        let producer = token(Field::Producer, envelope.producer)?;

        let count = envelope.domain_pack_digests.len();
        if count > MAX_DOMAIN_PACKS as usize {
            return Err(EnvelopeError::TooManyDomainPacks {
                found: count,
                max: MAX_DOMAIN_PACKS,
            });
        }
        let mut domain_pack_digests: Vec<Ident> = Vec::new();
        for (index, digest) in envelope.domain_pack_digests.iter().enumerate() {
            let digest = Ident::new(digest)
                .map_err(|source| EnvelopeError::MalformedDomainPack { index, source })?;
            if let Some(previous) = domain_pack_digests.last()
                && *previous >= digest
            {
                return Err(EnvelopeError::DomainPacksNotAscending { index });
            }
            domain_pack_digests.push(digest);
        }

        Ok(Self {
            model_digest,
            semantic_epoch,
            property_digest,
            scope_digest,
            assumptions_digest,
            producer,
            domain_pack_digests,
        })
    }
}

/// Why an envelope is not one the kernel's decoder would accept.
///
/// Every arm is a shape the decoder rejects, caught here instead: an engine that wrote
/// the bytes anyway would be publishing an artifact it already knows is invalid, and
/// the rejection would arrive at the far end with no producer left to name the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// A single-token field is not a token: empty, too long, or not printable ASCII.
    MalformedToken {
        /// Which envelope field.
        field: Field,
        /// What the token grammar reported.
        source: IdentError,
    },
    /// A domain-pack profile digest is not a token.
    MalformedDomainPack {
        /// Its position in [`ClaimEnvelope::domain_pack_digests`].
        index: usize,
        /// What the token grammar reported.
        source: IdentError,
    },
    /// More domain-pack digests than [`MAX_DOMAIN_PACKS`].
    TooManyDomainPacks {
        /// How many were offered.
        found: usize,
        /// [`MAX_DOMAIN_PACKS`].
        max: u16,
    },
    /// The domain-pack digests are not strictly ascending, which for the wire form is
    /// a duplicate or an ambiguous encoding rather than a cosmetic complaint
    /// (`crates/continuum-kernel-core/src/wire.rs:88-95`).
    DomainPacksNotAscending {
        /// The first position that is not greater than its predecessor.
        index: usize,
    },
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToken { field, source } => {
                write!(f, "envelope field `{field}`: {source}")
            }
            Self::MalformedDomainPack { index, source } => {
                write!(f, "domain-pack digest {index}: {source}")
            }
            Self::TooManyDomainPacks { found, max } => {
                write!(f, "{found} domain-pack digests; the wire form admits {max}")
            }
            Self::DomainPacksNotAscending { index } => write!(
                f,
                "domain-pack digest {index} does not follow its predecessor; the \
                 sequence must be strictly ascending"
            ),
        }
    }
}

impl core::error::Error for EnvelopeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::MalformedToken { source, .. } | Self::MalformedDomainPack { source, .. } => {
                Some(source)
            }
            Self::TooManyDomainPacks { .. } | Self::DomainPacksNotAscending { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// the closed-set seam
// ---------------------------------------------------------------------------

/// A reachable set that is *closed*: the only thing a finite-closure certificate may
/// be built from.
///
/// The field is private and [`ClosedSet::of`] is the only constructor, so this type
/// cannot be obtained from an [`Exploration::Exhausted`] — the arm
/// [`Exploration::closed`] answers `None` for. That is the type-level half of a rule
/// prose cannot hold on its own:
///
/// > only [`Exploration::Complete`] carries a closed set, and only a closed set can be
/// > certified.
/// >
/// > — `crates/continuum-engine-reference/src/bfs.rs:78-80`
///
/// A [`Reachable`] alone would not do: [`Exploration::reachable`] hands one out for
/// *either* arm, deliberately, because a violation found in a partial set is a real
/// violation. Certification is the case where the difference matters, so the seam is
/// re-stated here in a type the emission signature can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedSet<'a> {
    reachable: &'a Reachable,
}

impl<'a> ClosedSet<'a> {
    /// The closed reachable set of `exploration`, or `None` when a bound tripped.
    #[must_use]
    pub const fn of(exploration: &'a Exploration) -> Option<Self> {
        match exploration.closed() {
            Some(reachable) => Some(Self { reachable }),
            None => None,
        }
    }

    /// The reachable set, in canonical order with its depth column.
    #[must_use]
    pub const fn reachable(self) -> &'a Reachable {
        self.reachable
    }
}

// ---------------------------------------------------------------------------
// emission
// ---------------------------------------------------------------------------

/// Why no certificate was written.
///
/// No arm reports a *checking* failure: whether the certificate is true is the
/// kernel's question, asked of the bytes. These are the four ways the bytes could not
/// be produced at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionError {
    /// The claim envelope is not one the decoder would accept.
    Envelope(EnvelopeError),
    /// A table state's successor row could not be recomputed.
    ///
    /// Unreachable for a [`ClosedSet`] of the model that produced it — every state in
    /// it was expanded by [`explore`](crate::bfs::explore) already. It is reachable,
    /// and is the point of the arm, when a reachable set explored from *one* model is
    /// offered with *another*: the mismatched state fails
    /// [`Model::successors`]' domain check and is named here rather than written into
    /// a certificate about nothing.
    Evaluation {
        /// The state whose row failed.
        state: State,
        /// What the model layer reported. Boxed for the reason
        /// `ExplorationError::Evaluation` boxes its own (`bfs.rs:531-537`): a
        /// well-formed emission never produces this variant, and widening every
        /// successful emission's `Result` by the size of a defect report charges the
        /// success path for a failure it does not have.
        source: Box<EvaluationError>,
    },
    /// A count the wire form bounds is outside its range.
    ///
    /// The declared limits are `MAX_VARIABLES`, `MAX_STATES`, `MAX_ACTIONS` and
    /// `MAX_TRANSITIONS`, each already recorded on the declaration and exploration
    /// side (`model.rs:86-101`, `bfs.rs:118-155`) — so this is the boundary between
    /// "explorable" and "certifiable", reported rather than silently truncated.
    CountOutOfRange {
        /// Which position.
        field: Field,
        /// What this model and exploration would need.
        found: u64,
        /// The smallest admissible value.
        min: u64,
        /// The largest admissible value.
        max: u64,
    },
    /// The invariant names no predicate the model declares.
    UnknownPredicate {
        /// The name asked for.
        name: String,
    },
    /// A successor of a table state is not in the closed set.
    ///
    /// Unreachable for a [`ClosedSet`] of the model that produced it; reachable when a
    /// set explored from one model is offered with another.
    NotClosed {
        /// The state whose successor is missing.
        state: State,
    },
    /// The encoded certificate is larger than [`MAX_CERTIFICATE_BYTES`].
    Oversized {
        /// How many bytes were written.
        bytes: usize,
        /// [`MAX_CERTIFICATE_BYTES`].
        max: usize,
    },
}

impl From<EnvelopeError> for EmissionError {
    fn from(error: EnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

impl fmt::Display for EmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(source) => write!(f, "claim envelope: {source}"),
            Self::Evaluation { state, source } => {
                write!(f, "successor row of state {state}: {source}")
            }
            Self::CountOutOfRange {
                field,
                found,
                min,
                max,
            } => write!(
                f,
                "`{field}` is {found}; the wire form admits {min}..={max}"
            ),
            Self::UnknownPredicate { name } => {
                write!(f, "the model declares no predicate `{name}`")
            }
            Self::NotClosed { state } => {
                write!(f, "a successor of state {state} is not in the closed set")
            }
            Self::Oversized { bytes, max } => {
                write!(f, "certificate is {bytes} bytes; the limit is {max}")
            }
        }
    }
}

impl core::error::Error for EmissionError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Envelope(source) => Some(source),
            Self::Evaluation { source, .. } => Some(&**source),
            Self::CountOutOfRange { .. }
            | Self::UnknownPredicate { .. }
            | Self::NotClosed { .. }
            | Self::Oversized { .. } => None,
        }
    }
}

/// Emit the finite closure certificate for a closed exploration of `model`, whose
/// property is the model's declared state domain.
///
/// Pure: the returned bytes are a function of `model`, `closed` and `envelope`, and of
/// nothing else.
///
/// # Errors
///
/// Every arm of [`EmissionError`] except [`EmissionError::UnknownPredicate`]. Nothing
/// is written unless every one of them has been ruled out: the counts are checked and
/// every successor row is recomputed before the first byte is produced, so a failed
/// emission never leaves a partial artifact for a caller to mistake for a short one.
/// ([`EmissionError::NotClosed`] is found while the rows are written, into a local
/// buffer that is dropped on error, so it too returns no bytes.)
pub fn emit_finite_closure(
    model: &Model,
    closed: ClosedSet<'_>,
    envelope: &ClaimEnvelope<'_>,
) -> Result<Vec<u8>, EmissionError> {
    emit(model, closed, envelope, None)
}

/// Emit the finite closure certificate for a closed exploration of `model`, whose
/// property is the model's predicate `predicate`, established at every state of the
/// set.
///
/// This module does not evaluate the predicate: whether it holds is the kernel's
/// question, asked of the bytes. It only refuses a name the model does not declare.
///
/// # Errors
///
/// Every arm of [`EmissionError`].
pub fn emit_invariant_closure(
    model: &Model,
    closed: ClosedSet<'_>,
    envelope: &ClaimEnvelope<'_>,
    predicate: &str,
) -> Result<Vec<u8>, EmissionError> {
    emit(model, closed, envelope, Some(predicate))
}

fn emit(
    model: &Model,
    closed: ClosedSet<'_>,
    envelope: &ClaimEnvelope<'_>,
    predicate: Option<&str>,
) -> Result<Vec<u8>, EmissionError> {
    let envelope = Validated::new(envelope)?;

    let predicate = match predicate {
        Some(name) => {
            let declared = model
                .predicates()
                .iter()
                .find(|declared| declared.name().as_str() == name)
                .ok_or_else(|| EmissionError::UnknownPredicate {
                    name: name.to_owned(),
                })?;
            Some(declared.name().clone())
        }
        None => None,
    };

    let identity = model.identity();
    let identity = identity.as_bytes();
    if identity.len() > MAX_MODEL_BYTES {
        return Err(EmissionError::CountOutOfRange {
            field: Field::ModelLength,
            found: identity.len() as u64,
            min: 0,
            max: MAX_MODEL_BYTES as u64,
        });
    }
    let model_len = narrow_u32(identity.len());

    let variables = model.variables();
    let actions = model.actions();
    let states = closed.reachable().states();

    count_u16(Field::VariableCount, variables.len(), 1, MAX_VARIABLES)?;
    let action_count = count_u16(Field::ActionCount, actions.len(), 1, MAX_ACTIONS)?;
    let state_count = count_u32(Field::StateCount, states.len(), 1, MAX_STATES)?;
    // The count is a `u16` on the wire and was bounded by `MAX_DOMAIN_PACKS` when the
    // envelope was validated, so this conversion has no failure mode left to report.
    let domain_pack_count = narrow_u16(envelope.domain_pack_digests.len());

    // Every row first, and every count checked, before anything is written. The walk
    // guarantees each of these calls succeeds for the model it explored; recomputing
    // them is what makes that a fact about this emission rather than an assumption
    // carried from another module.
    let mut rows: Vec<(u32, Vec<Step>)> = Vec::with_capacity(states.len());
    let mut total: u64 = 0;
    for state in states {
        let row = model
            .successors(state)
            .map_err(|source| EmissionError::Evaluation {
                state: state.clone(),
                source: Box::new(source),
            })?;
        // One row's count is bounded by `MAX_STATES`, not by `MAX_TRANSITIONS`: that
        // is the decoder's own range for the field.
        let width = count_u32(Field::TransitionCount, row.len(), 0, MAX_STATES)?;
        total = total.saturating_add(u64::from(width));
        if total > MAX_TRANSITIONS {
            return Err(EmissionError::CountOutOfRange {
                field: Field::TransitionTotal,
                found: total,
                min: 0,
                max: MAX_TRANSITIONS,
            });
        }
        rows.push((width, row));
    }

    let mut out = Writer::new();
    out.bytes(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(FINITE_CLOSURE_KIND);

    out.token(&envelope.model_digest);
    out.token(&envelope.semantic_epoch);
    out.token(&envelope.property_digest);
    out.token(&envelope.scope_digest);
    out.token(&envelope.assumptions_digest);
    out.token(&envelope.producer);
    // The decoder rejects a `schema_epoch` that disagrees with the header, so the two
    // are one value.
    out.u16(WIRE_EPOCH);
    out.u16(domain_pack_count);
    for digest in &envelope.domain_pack_digests {
        out.token(digest);
    }

    out.u32(model_len);
    out.bytes(identity);

    match &predicate {
        Some(name) => {
            out.u16(PROPERTY_CLASS_INVARIANT);
            out.token(name);
        }
        None => out.u16(PROPERTY_CLASS_STATE_DOMAIN),
    }

    // A copy, not a sort: `Reachable::states` is already the canonical table
    // (`bfs.rs:281-286`), and every vector in it has the model's arity because every
    // one of them passed `Model::successors`' state check in the loop above.
    out.u32(state_count);
    for state in states {
        out.state(state);
    }

    for (index, (width, row)) in rows.iter().enumerate() {
        out.u32(*width);
        for step in row {
            out.u16(action_index(step, action_count)?);
            out.u32(target_index(states, step, index)?);
        }
    }

    let bytes = out.into_bytes();
    if bytes.len() > MAX_CERTIFICATE_BYTES {
        return Err(EmissionError::Oversized {
            bytes: bytes.len(),
            max: MAX_CERTIFICATE_BYTES,
        });
    }
    Ok(bytes)
}

/// A step's target as its index in the canonical table.
///
/// The table is strictly ascending, so the index is a binary search, and it preserves
/// the row's `(action, target)` order.
fn target_index(states: &[State], step: &Step, source: usize) -> Result<u32, EmissionError> {
    match states.binary_search(step.target()) {
        Ok(index) => Ok(narrow_u32(index)),
        Err(_) => Err(EmissionError::NotClosed {
            state: states
                .get(source)
                .cloned()
                .unwrap_or_else(|| step.target().clone()),
        }),
    }
}

/// A step's action index as the wire's `u16`.
///
/// `Model::successors` only ever labels a step with a position in `Model::actions`
/// (`model.rs:520-528`), and that sequence was just bounded by `MAX_ACTIONS`, so the
/// range check below cannot fail. It is here because the conversion has to be total
/// and because a checker that received an out-of-range index would answer
/// `Rejection::UnknownAction` (`crates/continuum-kernel-core/src/check.rs:125-131`) —
/// a defect worth naming on the producing side, where the action is still in hand.
fn action_index(step: &Step, action_count: u16) -> Result<u16, EmissionError> {
    let index = u16::try_from(step.action()).ok();
    match index {
        Some(index) if index < action_count => Ok(index),
        _ => Err(EmissionError::CountOutOfRange {
            field: Field::TransitionAction,
            found: step.action() as u64,
            min: 0,
            max: u64::from(action_count).saturating_sub(1),
        }),
    }
}

/// A `usize` count as the wire's `u16`, or the range it left.
fn count_u16(field: Field, found: usize, min: usize, max: usize) -> Result<u16, EmissionError> {
    if found < min || found > max {
        return Err(EmissionError::CountOutOfRange {
            field,
            found: found as u64,
            min: min as u64,
            max: max as u64,
        });
    }
    Ok(narrow_u16(found))
}

/// A `usize` count as the wire's `u32`, or the range it left.
fn count_u32(field: Field, found: usize, min: usize, max: usize) -> Result<u32, EmissionError> {
    if found < min || found > max {
        return Err(EmissionError::CountOutOfRange {
            field,
            found: found as u64,
            min: min as u64,
            max: max as u64,
        });
    }
    Ok(narrow_u32(found))
}

/// Saturating `usize -> u16`, applied only where a range check already ran.
///
/// The kernel's own narrowing helpers carry the same note and the same reason —
/// "it exists so that the conversion has no failure mode at all"
/// (`crates/continuum-kernel-core/src/check.rs:193-197`).
const fn narrow_u16(value: usize) -> u16 {
    if value > u16::MAX as usize {
        u16::MAX
    } else {
        value as u16
    }
}

/// Saturating `usize -> u32`. See [`narrow_u16`].
const fn narrow_u32(value: usize) -> u32 {
    if value > u32::MAX as usize {
        u32::MAX
    } else {
        value as u32
    }
}

// ---------------------------------------------------------------------------
// the byte writer
// ---------------------------------------------------------------------------

/// A sink for the wire form's primitives, and nothing else.
///
/// Five methods, no seeking, no back-patching: a length is written from a count that
/// was checked before the write began, never fixed up afterwards. Back-patching is how
/// a writer ends up with a declared count that disagrees with the data behind it,
/// which is precisely the class of malformed artifact the decoder's `declared_*`
/// adversarial cases are built to catch (`crates/continuum-kernel-core/src/fixture.rs:87-118`).
#[derive(Debug)]
struct Writer {
    out: Vec<u8>,
}

impl Writer {
    const fn new() -> Self {
        Self { out: Vec::new() }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    fn u16(&mut self, value: u16) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// A length-prefixed token.
    ///
    /// [`Ident::new`] refuses anything longer than [`crate::ident::MAX_IDENT_BYTES`],
    /// which is the wire form's `MAX_TOKEN_BYTES`
    /// (`crates/continuum-kernel-core/src/wire.rs:122`), so the length prefix cannot
    /// saturate; taking an [`Ident`] rather than a `&str` is what makes that a
    /// type-level fact rather than a caller's obligation.
    fn token(&mut self, name: &Ident) {
        let bytes = name.as_str().as_bytes();
        self.u16(narrow_u16(bytes.len()));
        self.bytes(bytes);
    }

    /// A state vector: one `i64` per declared variable, in canonical variable order.
    fn state(&mut self, state: &State) {
        for value in state.as_slice() {
            self.i64(*value);
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.out
    }
}
