//! The closed verdict vocabulary of the trusted checking base (INV-008, PR 9).
//!
//! # Why this is not a `bool`
//!
//! > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity,
//! > and incomplete proof search are distinct outcomes.
//! >
//! > — `notes/plan/plan.md` §2, INV-008 "Typed inconclusiveness"
//!
//! > No CLI may print a bare "verified" without exposing the assurance class and
//! > trusted components.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`,
//! >   "Proof-producing solver policy"
//!
//! A `bool` return outruns its evidence in three separate ways: it cannot say *what*
//! was established, it cannot distinguish "this artifact is invalid" from "this
//! checker does not implement that certificate family", and it invites a caller to
//! render `true` as a claim the kernel never made. [`Verdict`] is therefore a closed
//! three-way vocabulary, and its success arm carries a [`CheckedClaim`] naming the
//! obligations that were discharged, the envelope they are relative to, and the
//! components the result still trusts.
//!
//! # The three arms
//!
//! - [`Verdict::Verified`] — every obligation of the certificate family was
//!   re-derived from the certificate's own bytes.
//! - [`Verdict::Rejected`] — the bytes are not a valid certificate of the family
//!   they claim. This is a statement about the *artifact*, never about the model.
//! - [`Verdict::Unsupported`] — the bytes name a wire epoch, certificate family, or
//!   property class this checker does not implement. Rejecting these would report a
//!   possibly valid certificate as invalid, which is exactly the ambiguity INV-008
//!   forbids; ADR-0017's treatment of unchecked solver evidence is the same move.
//!
//! There is deliberately no `Unknown`, no `Warning`, and no `Verified` variant that
//! can be constructed without a [`CheckedClaim`].

use crate::wire::Envelope;

/// The outcome of checking one certificate from its wire form.
///
/// Closed by construction: a caller matching on this enum has enumerated every
/// outcome the kernel can produce (INV-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every obligation of the certificate's family was re-derived from its bytes.
    ///
    /// The payload states what was checked, and against which envelope; see
    /// [`CheckedClaim`].
    Verified(CheckedClaim),
    /// The bytes are not a well-formed, internally consistent certificate of the
    /// family they claim to belong to.
    Rejected(Rejection),
    /// The bytes are well-formed enough to name a feature this checker does not
    /// implement. Not a claim that the certificate is invalid.
    Unsupported(Feature),
}

impl Verdict {
    /// Whether this verdict is [`Verdict::Verified`].
    ///
    /// Provided for assertions and for rendering, never as a substitute for
    /// matching: a caller that collapses [`Verdict::Rejected`] and
    /// [`Verdict::Unsupported`] into one `false` has thrown away the distinction
    /// INV-008 exists to preserve.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    /// The claim established by a [`Verdict::Verified`], or `None`.
    #[must_use]
    pub const fn claim(&self) -> Option<&CheckedClaim> {
        match self {
            Self::Verified(claim) => Some(claim),
            Self::Rejected(_) | Self::Unsupported(_) => None,
        }
    }
}

/// The certificate families `continuum-kernel-core` implements.
///
/// RFC 0005 "Certificate families" names seven; the counterexample witness,
/// partial-order closure, refinement, and liveness families belong to later PRs and
/// to `continuum-kernel-temporal`, and the solver-proof families to
/// `continuum-kernel-sat` / `continuum-kernel-smt`. A wire `kind` code outside this
/// enum is [`Feature::CertificateKind`], not a rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertificateKind {
    /// RFC 0005 "Closed reachable set" / docs/03 §6.1 "Closed finite state space":
    /// `Init ⊆ S`, `Post(S) ⊆ S`, `S ⊆ P`.
    FiniteClosure,
    /// The state-typing half of docs/23's "finite closure safety for DieHard
    /// `TypeOK`" milestone: every state in the certificate's table lies in the
    /// declared state domain (docs/16 PO-MOD-003).
    StateType,
}

impl CertificateKind {
    /// Every family this crate implements, in wire-code order.
    pub const ALL: [Self; 2] = [Self::FiniteClosure, Self::StateType];

    /// The `kind` code this family occupies in the wire header.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::FiniteClosure => 1,
            Self::StateType => 2,
        }
    }

    /// The family a wire `kind` code names, or `None` if this crate does not
    /// implement it.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::FiniteClosure),
            2 => Some(Self::StateType),
            _ => None,
        }
    }

    /// The stable lower-case token naming this family in receipts and diagnostics.
    ///
    /// `finite-closure` is the token `notes/plan/notes/START_HERE_IMPLEMENTATION.md`
    /// PR 9 uses; `closed-set` is the *receipt* spelling fixed by
    /// `notes/plan/schemas/proof-receipt.schema.json` and is applied by the receipt
    /// layer, not here.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FiniteClosure => "finite-closure",
            Self::StateType => "state-type",
        }
    }
}

/// The property classes the finite-closure checker can evaluate for itself.
///
/// docs/03 §6.1 requires a closed-finite-state-space certificate to carry
/// "invariant evaluation data or values sufficient to recompute it". This crate
/// implements the one class the Phase A corpus needs — a conjunction of inclusive
/// integer ranges over the declared state variables, which is exactly the shape of
/// the DieHard `TypeOK` invariant (`notes/plan/corpus/tla-examples/ports/TV-009`,
/// `invariant TypeOK { big in 0..5 && small in 0..3 }`).
///
/// Richer property languages need the propositional/first-order expression evaluator
/// RFC 0005 places in `continuum-kernel-core`; until it lands, a certificate that
/// names another class is [`Feature::PropertyClass`] rather than a rejection, so no
/// caller can read "unimplemented" as "false".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyClass {
    /// The safety property is the declared state domain itself: every state variable
    /// lies within its inclusive range.
    StateDomain,
}

impl PropertyClass {
    /// Every property class this crate evaluates, in wire-code order.
    pub const ALL: [Self; 1] = [Self::StateDomain];

    /// The `property_class` code this class occupies in the wire body.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::StateDomain => 1,
        }
    }

    /// The class a wire code names, or `None` if this crate cannot evaluate it.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::StateDomain),
            _ => None,
        }
    }

    /// The stable lower-case token naming this class in receipts and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StateDomain => "state-domain",
        }
    }
}

/// What a [`Verdict::Verified`] actually asserts.
///
/// docs/03 §2 makes a claim a record of its subject, its scope, its assumptions and
/// its *trusted* components rather than a level. This is the kernel-side fragment of
/// that record: the family checked, the envelope the claim is relative to, and the
/// sizes that were traversed. The receipt that binds it to a model, a toolchain and
/// a build digest is PR 9's receipt slice (INV-014); this type deliberately carries
/// nothing it did not itself re-derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClaim {
    kind: CertificateKind,
    property: PropertyClass,
    envelope: Envelope,
    states: u32,
    initial_states: u32,
    transitions: u64,
}

impl CheckedClaim {
    /// Build a claim record. Crate-internal: only a completed check may mint one.
    pub(crate) const fn new(
        kind: CertificateKind,
        property: PropertyClass,
        envelope: Envelope,
        states: u32,
        initial_states: u32,
        transitions: u64,
    ) -> Self {
        Self {
            kind,
            property,
            envelope,
            states,
            initial_states,
            transitions,
        }
    }

    /// The certificate family that was checked.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        self.kind
    }

    /// The property class that was evaluated at every state.
    #[must_use]
    pub const fn property(&self) -> PropertyClass {
        self.property
    }

    /// The claim envelope the certificate carried (RFC 0005 "Claim envelope").
    ///
    /// The kernel checked this envelope's *shape* and its internal agreement with
    /// the wire header. Deciding whether `model_digest` is the model the caller
    /// meant is the caller's obligation — see [`Self::trusted_components`].
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// How many distinct states the certificate's canonical table carried.
    #[must_use]
    pub const fn states(&self) -> u32 {
        self.states
    }

    /// How many initial states were located in that table.
    #[must_use]
    pub const fn initial_states(&self) -> u32 {
        self.initial_states
    }

    /// How many labelled transitions were traversed for the closure obligation.
    ///
    /// Zero for [`CertificateKind::StateType`], which carries no transition
    /// relation.
    #[must_use]
    pub const fn transitions(&self) -> u64 {
        self.transitions
    }

    /// The components this verdict still trusts, in the sense of docs/03 §2's
    /// `trusted: Vec<TrustedComponent>`.
    ///
    /// The kernel re-derives every obligation *inside* the certificate. Two things
    /// it structurally cannot re-derive, because a self-contained artifact does not
    /// contain them, are named here so no rendering of a verified claim can quietly
    /// omit them:
    ///
    /// - `certificate-model-correspondence` — that the carried transition relation
    ///   is the transition relation of the model named by `model_digest`. Binding
    ///   those is the receipt/producer obligation (RFC 0024; ADR-0035), and it is
    ///   precisely why RFC 0005 puts the envelope hashes in the certificate.
    /// - `envelope-digest-binding` — that the envelope's digests were computed over
    ///   the artifacts they name. The kernel checks their shape, not their contents;
    ///   ADR-0013 canonical identity is `continuum-value`'s obligation.
    #[must_use]
    pub const fn trusted_components(&self) -> &'static [&'static str] {
        &[
            "certificate-model-correspondence",
            "envelope-digest-binding",
        ]
    }
}

/// A named position in the wire form, so a rejection says *where*.
///
/// The vocabulary is closed and mirrors the grammar documented on
/// [`crate::wire`]; adding a field to the wire form adds a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The certificate byte string as a whole.
    Certificate,
    /// The eight-byte format magic.
    Magic,
    /// The header's wire-epoch field.
    WireEpoch,
    /// The header's certificate-family field.
    Kind,
    /// Envelope: the model digest (RFC 0005 "model hash").
    ModelDigest,
    /// Envelope: the semantic epoch (RFC 0005 "CIR semantics version").
    SemanticEpoch,
    /// Envelope: the property digest (RFC 0005 "property hash").
    PropertyDigest,
    /// Envelope: the scope digest (RFC 0005 "bounds").
    ScopeDigest,
    /// Envelope: the assumptions digest (RFC 0005 "assumptions").
    AssumptionsDigest,
    /// Envelope: the producing engine's identity (RFC 0005 "engine version").
    Producer,
    /// Envelope: the certificate schema epoch (RFC 0005 "certificate schema
    /// version").
    SchemaEpoch,
    /// Envelope: the number of domain-pack profile digests.
    DomainPackCount,
    /// Envelope: one domain-pack profile digest.
    DomainPack,
    /// State domain: the number of declared state variables.
    VariableCount,
    /// State domain: one variable's name.
    VariableName,
    /// State domain: one variable's inclusive range.
    VariableRange,
    /// State table: the number of states.
    StateCount,
    /// State table: one state vector.
    StateVector,
    /// Finite closure: the property-class code.
    PropertyClassCode,
    /// Finite closure: the number of initial states.
    InitialCount,
    /// Finite closure: one initial state vector.
    InitialState,
    /// Finite closure: the number of declared actions.
    ActionCount,
    /// Finite closure: one action name.
    ActionName,
    /// Finite closure: the number of transitions in one successor row.
    TransitionCount,
    /// Finite closure: one transition's action index.
    TransitionAction,
    /// Finite closure: one transition's target state vector.
    TransitionTarget,
}

impl Field {
    /// The stable lower-case token naming this field in diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certificate => "certificate",
            Self::Magic => "magic",
            Self::WireEpoch => "wire-epoch",
            Self::Kind => "kind",
            Self::ModelDigest => "model-digest",
            Self::SemanticEpoch => "semantic-epoch",
            Self::PropertyDigest => "property-digest",
            Self::ScopeDigest => "scope-digest",
            Self::AssumptionsDigest => "assumptions-digest",
            Self::Producer => "producer",
            Self::SchemaEpoch => "schema-epoch",
            Self::DomainPackCount => "domain-pack-count",
            Self::DomainPack => "domain-pack",
            Self::VariableCount => "variable-count",
            Self::VariableName => "variable-name",
            Self::VariableRange => "variable-range",
            Self::StateCount => "state-count",
            Self::StateVector => "state-vector",
            Self::PropertyClassCode => "property-class",
            Self::InitialCount => "initial-count",
            Self::InitialState => "initial-state",
            Self::ActionCount => "action-count",
            Self::ActionName => "action-name",
            Self::TransitionCount => "transition-count",
            Self::TransitionAction => "transition-action",
            Self::TransitionTarget => "transition-target",
        }
    }
}

/// Why a length-prefixed token is not a legal token.
///
/// Tokens are the envelope digests and the variable/action names. Restricting them
/// to non-empty printable ASCII is the same discipline `continuum-value`'s `Name`
/// applies for the same reason (ADR-0013): an identity with two spellings is two
/// identities, and a certificate is compared by bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenFault {
    /// The token declared length zero.
    Empty,
    /// The token declared more bytes than [`crate::wire::MAX_TOKEN_BYTES`].
    TooLong {
        /// The declared length.
        found: usize,
    },
    /// The token contains a byte outside printable ASCII `0x21..=0x7E`.
    NonPrintable {
        /// Offset of the offending byte within the token.
        offset: usize,
        /// The offending byte.
        byte: u8,
    },
}

/// Every reason the kernel refuses a byte string as a certificate.
///
/// The vocabulary is closed. Each variant is a statement about the artifact — never
/// about the model, the property, or the engine that produced it. A malformed
/// artifact is a typed rejection here, which is what keeps "malformed artifact panic
/// in kernel" (docs/12 §11, release-blocker classes) out of the release path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The certificate is larger than [`crate::wire::MAX_CERTIFICATE_BYTES`].
    ///
    /// RFC 0005 "Resource bounds": certificate checking is itself an attack surface,
    /// so the format declares size limits before it decodes anything.
    Oversized {
        /// The length of the supplied byte string.
        found: usize,
    },
    /// The reader needed more bytes than the certificate contains.
    Truncated {
        /// The field being read when the bytes ran out.
        field: Field,
        /// Offset at which the read began.
        offset: usize,
        /// Number of bytes the field needed.
        needed: usize,
        /// Number of bytes actually remaining.
        available: usize,
    },
    /// The leading eight bytes are not [`crate::wire::MAGIC`].
    BadMagic,
    /// Bytes remain after the certificate body ends.
    ///
    /// A trailing-byte tolerance is a second encoding of the same certificate, and
    /// RFC 0005 "Resource bounds" requires "rejection of duplicate/ambiguous
    /// encodings".
    TrailingBytes {
        /// How many bytes followed the body.
        extra: usize,
    },
    /// A length-prefixed token is not a legal token.
    MalformedToken {
        /// Which token.
        field: Field,
        /// What is wrong with it.
        fault: TokenFault,
    },
    /// A declared count is outside the range the format admits.
    CountOutOfRange {
        /// Which count.
        field: Field,
        /// The declared value.
        found: u64,
        /// The smallest admissible value.
        min: u64,
        /// The largest admissible value.
        max: u64,
    },
    /// A sequence required to be strictly ascending is not.
    ///
    /// Canonical ordering is what makes the encoding unique: it forbids duplicates,
    /// fixes the identity of a state as its position in the table, and denies a
    /// producer any freedom that two certificates could differ by (docs/03 §6.1,
    /// "canonical state table").
    NotStrictlyAscending {
        /// Which sequence.
        field: Field,
        /// Index of the first element that did not exceed its predecessor.
        index: u32,
    },
    /// A declared state variable has `lo > hi`, so its range admits no value.
    InvertedVariableRange {
        /// Index of the variable in the declared state domain.
        variable: u16,
    },
    /// The envelope's schema epoch disagrees with the wire header's epoch.
    ///
    /// An internal inconsistency: the certificate names two different contracts for
    /// itself, and RFC 0005 has the kernel check the envelope before the content.
    SchemaEpochMismatch {
        /// The epoch declared in the envelope.
        declared: u16,
        /// The epoch declared in the header.
        header: u16,
    },
    /// A finite-closure certificate declared no initial states.
    ///
    /// docs/16 PO-MOD-001 ("Initial satisfiability") admits an empty `Init` only
    /// when emptiness is explicitly expected. The wire form has no way to express
    /// that expectation, so the kernel fails closed.
    NoInitialStates,
    /// A declared initial state does not appear in the certificate's state table.
    ///
    /// The `Init ⊆ S` obligation of RFC 0005 "Closed reachable set", and
    /// `ClosureCertificate.containsInit` in `lean/Continuum/Certificate.lean`.
    InitialStateNotInTable {
        /// Position of the initial state in the declared initial sequence.
        position: u32,
    },
    /// A transition names an action index that is not in the action table.
    UnknownAction {
        /// Index of the source state in the state table.
        state: u32,
        /// Index of the transition within that state's successor row.
        entry: u32,
        /// The out-of-range action index.
        action: u16,
    },
    /// A transition leaves the certificate's state table.
    ///
    /// The `Post(S) ⊆ S` obligation of RFC 0005 "Closed reachable set", and
    /// `ClosureCertificate.closed` in `lean/Continuum/Certificate.lean`. This is the
    /// rejection that makes a truncated exploration unusable as a proof.
    ClosureFailure {
        /// Index of the source state in the state table.
        state: u32,
        /// Index of the transition within that state's successor row.
        entry: u32,
    },
    /// A state in the table violates the certificate's own safety property.
    ///
    /// The `S ⊆ P` obligation of RFC 0005 "Closed reachable set", and
    /// `ClosureCertificate.safeOnMember` in `lean/Continuum/Certificate.lean`. For
    /// [`CertificateKind::StateType`] it is docs/16 PO-MOD-003 directly.
    PropertyViolated {
        /// Index of the state in the state table.
        state: u32,
        /// Index of the variable whose declared range the state leaves.
        variable: u16,
        /// The value the state carried for that variable.
        value: i64,
    },
}

impl Rejection {
    /// The stable lower-case token naming this rejection reason.
    ///
    /// This crate owns its rejection vocabulary, so this crate spells it: a consumer
    /// that relays a rejection carries this token verbatim rather than transcribing
    /// the enum into a vocabulary of its own (the second-authority mistake bn-dtg61
    /// declined; RFC 0026 F19, paid at protocol 3.4 by bn-3jrtz). One token per
    /// variant, in the same diagnostic register as [`Field::as_str`]; the variant's
    /// numeric payload does not travel with it — those specifics are recoverable by
    /// re-running this kernel over the same bytes, which any holder of the artifact
    /// can do.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::Oversized { .. } => "oversized",
            Self::Truncated { .. } => "truncated",
            Self::BadMagic => "bad-magic",
            Self::TrailingBytes { .. } => "trailing-bytes",
            Self::MalformedToken { .. } => "malformed-token",
            Self::CountOutOfRange { .. } => "count-out-of-range",
            Self::NotStrictlyAscending { .. } => "not-strictly-ascending",
            Self::InvertedVariableRange { .. } => "inverted-variable-range",
            Self::SchemaEpochMismatch { .. } => "schema-epoch-mismatch",
            Self::NoInitialStates => "no-initial-states",
            Self::InitialStateNotInTable { .. } => "initial-state-not-in-table",
            Self::UnknownAction { .. } => "unknown-action",
            Self::ClosureFailure { .. } => "closure-failure",
            Self::PropertyViolated { .. } => "property-violated",
        }
    }

    /// The wire position this rejection names, when it names one.
    ///
    /// Exactly the variants that carry a [`Field`] member answer [`Some`]; the rest
    /// answer [`None`] rather than guessing a position the rejection never recorded.
    #[must_use]
    pub const fn field(&self) -> Option<Field> {
        match self {
            Self::Truncated { field, .. }
            | Self::MalformedToken { field, .. }
            | Self::CountOutOfRange { field, .. }
            | Self::NotStrictlyAscending { field, .. } => Some(*field),
            _ => None,
        }
    }
}

/// A feature the certificate names and this checker does not implement.
///
/// Distinct from [`Rejection`] on purpose: an unsupported certificate may be
/// perfectly valid, and reporting it as invalid would be a false negative dressed as
/// a verdict. INV-008 keeps "outside the declared fragments" apart from every other
/// non-success outcome; `continuum-value`'s `InconclusiveReason::Unsupported` is the
/// same distinction one layer up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// The certificate declares a wire epoch this build does not implement.
    ///
    /// A wire epoch is a content contract, not a version number to be tolerated: an
    /// epoch advance never mutates a published artifact (plan §4.6, ADR-0018), so a
    /// checker meets an unknown epoch by saying so.
    WireEpoch {
        /// The declared epoch.
        found: u16,
    },
    /// The certificate declares a family outside [`CertificateKind::ALL`].
    ///
    /// RFC 0005 names seven certificate families; this crate implements two, and the
    /// solver and temporal families belong to the sibling kernel crates.
    CertificateKind {
        /// The declared family code.
        found: u16,
    },
    /// The certificate declares a property class outside [`PropertyClass::ALL`].
    PropertyClass {
        /// The declared class code.
        found: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_kind_codes_round_trip() {
        for kind in CertificateKind::ALL {
            assert_eq!(CertificateKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(CertificateKind::from_code(0), None);
        assert_eq!(CertificateKind::from_code(3), None);
        assert_eq!(CertificateKind::from_code(u16::MAX), None);
    }

    #[test]
    fn property_class_codes_round_trip() {
        for class in PropertyClass::ALL {
            assert_eq!(PropertyClass::from_code(class.code()), Some(class));
        }
        assert_eq!(PropertyClass::from_code(0), None);
        assert_eq!(PropertyClass::from_code(2), None);
    }

    #[test]
    fn family_tokens_are_distinct_and_stable() {
        assert_eq!(CertificateKind::FiniteClosure.as_str(), "finite-closure");
        assert_eq!(CertificateKind::StateType.as_str(), "state-type");
        assert_eq!(PropertyClass::StateDomain.as_str(), "state-domain");
    }

    #[test]
    fn field_tokens_are_unique() {
        // A diagnostic that names two fields the same way names neither.
        let fields = [
            Field::Certificate,
            Field::Magic,
            Field::WireEpoch,
            Field::Kind,
            Field::ModelDigest,
            Field::SemanticEpoch,
            Field::PropertyDigest,
            Field::ScopeDigest,
            Field::AssumptionsDigest,
            Field::Producer,
            Field::SchemaEpoch,
            Field::DomainPackCount,
            Field::DomainPack,
            Field::VariableCount,
            Field::VariableName,
            Field::VariableRange,
            Field::StateCount,
            Field::StateVector,
            Field::PropertyClassCode,
            Field::InitialCount,
            Field::InitialState,
            Field::ActionCount,
            Field::ActionName,
            Field::TransitionCount,
            Field::TransitionAction,
            Field::TransitionTarget,
        ];
        let tokens: std::collections::BTreeSet<&str> = fields.iter().map(|f| f.as_str()).collect();
        assert_eq!(tokens.len(), fields.len());
    }

    #[test]
    fn a_non_verified_verdict_carries_no_claim() {
        let rejected = Verdict::Rejected(Rejection::BadMagic);
        assert!(!rejected.is_verified());
        assert!(rejected.claim().is_none());

        let unsupported = Verdict::Unsupported(Feature::WireEpoch { found: 2 });
        assert!(!unsupported.is_verified());
        assert!(unsupported.claim().is_none());
    }
}
