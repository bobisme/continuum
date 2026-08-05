//! The closed verdict vocabulary of the SAT checking base (INV-008, PR 9).
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
//! Nowhere is the distinction sharper than for solver evidence. ADR-0017 splits an
//! `unsat` answer into `CHECKED_CERTIFICATE` and `TRUSTED_SOLVER`, and docs/03 §7
//! makes "proof unavailable" a *different row* from "proof checked". A checker that
//! answered `false` to an unimplemented proof step would collapse "this refutation is
//! wrong" into "this build cannot read that step", which is the failure INV-008 names.
//! [`Verdict`] is therefore a closed three-way vocabulary, and its success arm carries
//! a [`CheckedClaim`] naming the obligations that were discharged, the envelope they
//! are relative to, and the components the result still trusts.
//!
//! # The three arms
//!
//! - [`Verdict::Verified`] — every step of the refutation was re-derived from the
//!   certificate's own clauses by unit propagation.
//! - [`Verdict::Rejected`] — the bytes are not a valid certificate of the family they
//!   claim. This is a statement about the *artifact*, never about the formula's
//!   satisfiability: a rejected refutation does not make a formula satisfiable.
//! - [`Verdict::Unsupported`] — the bytes name a wire epoch, certificate family, or
//!   proof-step kind this checker does not implement.
//!
//! There is deliberately no `Unknown`, no `Warning`, and no `Verified` variant that
//! can be constructed without a [`CheckedClaim`].

use crate::wire::Envelope;

/// The outcome of checking one SAT certificate from its wire form.
///
/// Closed by construction: a caller matching on this enum has enumerated every
/// outcome the kernel can produce (INV-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every proof step was re-derived from the certificate's own clauses.
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

/// The certificate families `continuum-kernel-sat` implements.
///
/// RFC 0005's "Kernel layering" gives this crate one line — "`continuum-kernel-sat`:
/// LRAT checker" — and `notes/plan/schemas/proof-receipt.schema.json` fixes its
/// receipt spelling as `lrat`. A wire `kind` code outside this enum is
/// [`Feature::CertificateKind`], not a rejection: `veripb` (pseudo-Boolean) is a
/// sibling entry in the same receipt enum and belongs to a later slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertificateKind {
    /// A clausal refutation: CNF plus reverse-unit-propagation steps ending in the
    /// empty clause (RFC 0005 "Inductive invariant", "SAT subproofs MAY use LRAT";
    /// docs/03 §7's `UNSAT / Alethe/LRAT proof checked` row).
    Lrat,
}

impl CertificateKind {
    /// Every family this crate implements, in wire-code order.
    pub const ALL: [Self; 1] = [Self::Lrat];

    /// The `kind` code this family occupies in the wire header.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Lrat => 1,
        }
    }

    /// The family a wire `kind` code names, or `None` if this crate does not
    /// implement it.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::Lrat),
            _ => None,
        }
    }

    /// The stable lower-case token naming this family in receipts and diagnostics.
    ///
    /// `lrat` is the spelling `notes/plan/schemas/proof-receipt.schema.json` fixes in
    /// its `certificate.kind` enum.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lrat => "lrat",
        }
    }
}

/// What a [`Verdict::Verified`] actually asserts.
///
/// docs/03 §2 makes a claim a record of its subject, its scope, its assumptions and
/// its *trusted* components rather than a level. This is the kernel-side fragment of
/// that record: the family checked, the envelope the claim is relative to, and the
/// sizes that were traversed. The receipt that binds it to a model, a toolchain and a
/// build digest is PR 9's receipt slice (INV-014); this type deliberately carries
/// nothing it did not itself re-derive.
///
/// The claim is precisely: *the CNF formula carried by these bytes has no satisfying
/// assignment*. It is not "the model is safe" and not "the property holds"; those
/// require the encoding correspondence named in [`Self::trusted_components`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClaim {
    kind: CertificateKind,
    envelope: Envelope,
    variables: u32,
    input_clauses: u32,
    derived_clauses: u32,
    deleted_clauses: u32,
    propagations: u64,
}

impl CheckedClaim {
    /// Build a claim record. Crate-internal: only a completed check may mint one.
    pub(crate) const fn new(
        kind: CertificateKind,
        envelope: Envelope,
        variables: u32,
        input_clauses: u32,
        derived_clauses: u32,
        deleted_clauses: u32,
        propagations: u64,
    ) -> Self {
        Self {
            kind,
            envelope,
            variables,
            input_clauses,
            derived_clauses,
            deleted_clauses,
            propagations,
        }
    }

    /// The certificate family that was checked.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        self.kind
    }

    /// The claim envelope the certificate carried (RFC 0005 "Claim envelope").
    ///
    /// The kernel checked this envelope's *shape* and its internal agreement with the
    /// wire header. Deciding whether `model_digest` is the model the caller meant is
    /// the caller's obligation — see [`Self::trusted_components`].
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// The largest variable index any carried clause actually mentions.
    ///
    /// Not the declared `variable_count`: the declared count is an upper bound the
    /// decoder enforces on every literal, while this is what the refutation was
    /// actually run over.
    #[must_use]
    pub const fn variables(&self) -> u32 {
        self.variables
    }

    /// How many clauses the carried formula has.
    #[must_use]
    pub const fn input_clauses(&self) -> u32 {
        self.input_clauses
    }

    /// How many clauses the refutation derived, the empty clause included.
    #[must_use]
    pub const fn derived_clauses(&self) -> u32 {
        self.derived_clauses
    }

    /// How many clauses the refutation deleted.
    #[must_use]
    pub const fn deleted_clauses(&self) -> u32 {
        self.deleted_clauses
    }

    /// How many unit propagations the checker performed re-deriving the refutation.
    ///
    /// RFC 0005's "Resource bounds" asks for "deterministic resource accounting"; this
    /// is the accounting for the one unbounded-looking loop in the crate, and it is a
    /// function of the bytes alone.
    #[must_use]
    pub const fn propagations(&self) -> u64 {
        self.propagations
    }

    /// The components this verdict still trusts, in the sense of docs/03 §2's
    /// `trusted: Vec<TrustedComponent>`.
    ///
    /// The kernel re-derives every propagation *inside* the certificate. Two things it
    /// structurally cannot re-derive, because a self-contained artifact does not
    /// contain them, are named here so no rendering of a verified claim can quietly
    /// omit them:
    ///
    /// - `formula-model-correspondence` — that the carried CNF is the propositional
    ///   encoding of the model and property named by the envelope digests. Checking a
    ///   refutation says nothing about *what was encoded*; RFC 0005 puts the envelope
    ///   hashes in the certificate for exactly this reason, and RFC 0024's receipt is
    ///   where the binding is recorded. RFC 0020's proof-producing transformation
    ///   chain (`-- SAT solve --> LRAT`) is what discharges it end to end.
    /// - `envelope-digest-binding` — that the envelope's digests were computed over
    ///   the artifacts they name. The kernel checks their shape, not their contents;
    ///   ADR-0013 canonical identity is `continuum-value`'s obligation.
    ///
    /// The solver is deliberately *absent* from this list. That is the whole point of
    /// checking the certificate: ADR-0017's `TRUSTED_SOLVER` becomes
    /// `CHECKED_CERTIFICATE` exactly when the proof is replayed here.
    #[must_use]
    pub const fn trusted_components(&self) -> &'static [&'static str] {
        &["envelope-digest-binding", "formula-model-correspondence"]
    }
}

/// A named position in the wire form, so a rejection says *where*.
///
/// The vocabulary is closed and mirrors the grammar documented on [`crate::wire`];
/// adding a field to the wire form adds a variant here.
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
    /// Envelope: the producing solver adapter's identity (RFC 0005 "engine version").
    Producer,
    /// Envelope: the certificate schema epoch (RFC 0005 "certificate schema version").
    SchemaEpoch,
    /// Envelope: the number of domain-pack profile digests.
    DomainPackCount,
    /// Envelope: one domain-pack profile digest.
    DomainPack,
    /// Formula: the declared number of propositional variables.
    VariableCount,
    /// Formula: the number of input clauses.
    ClauseCount,
    /// Any clause: the number of literals it carries.
    LiteralCount,
    /// Any clause: one literal.
    Literal,
    /// Proof: the number of steps.
    StepCount,
    /// Proof: one step's kind code.
    StepKind,
    /// Proof: an addition step's clause identifier.
    StepId,
    /// Proof: the number of antecedents in one addition step's chain.
    HintCount,
    /// Proof: one antecedent clause identifier.
    Hint,
    /// Proof: the number of identifiers in one deletion step.
    DeletionCount,
    /// Proof: one deleted clause identifier.
    Deletion,
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
            Self::ClauseCount => "clause-count",
            Self::LiteralCount => "literal-count",
            Self::Literal => "literal",
            Self::StepCount => "step-count",
            Self::StepKind => "step-kind",
            Self::StepId => "step-id",
            Self::HintCount => "hint-count",
            Self::Hint => "hint",
            Self::DeletionCount => "deletion-count",
            Self::Deletion => "deletion",
        }
    }
}

/// Why a length-prefixed token is not a legal token.
///
/// Tokens are the envelope digests. Restricting them to non-empty printable ASCII is
/// the same discipline `continuum-value`'s `Name` applies for the same reason
/// (ADR-0013): an identity with two spellings is two identities, and a certificate is
/// compared by bytes.
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

/// Every reason the kernel refuses a byte string as a SAT certificate.
///
/// The vocabulary is closed. Each variant is a statement about the artifact — never
/// about the formula, the property, or the solver that produced it. A malformed
/// artifact is a typed rejection here, which is what keeps "malformed artifact panic
/// in kernel" (docs/12 §11) out of the release path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The certificate is larger than [`crate::wire::MAX_CERTIFICATE_BYTES`].
    ///
    /// RFC 0005 "Resource bounds": certificate checking is itself an attack surface,
    /// so the format declares size limits before it decodes anything (docs/16
    /// PO-KER-002).
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
    ///
    /// Each kernel crate has its own magic, so handing a closure certificate to the
    /// SAT checker is caught at byte zero rather than deep in a body decoder.
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
    NotStrictlyAscending {
        /// Which sequence.
        field: Field,
        /// Index of the first element that did not exceed its predecessor.
        index: u32,
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
    /// A clause carries the literal `0`, which names no variable and no polarity.
    ZeroLiteral {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the offending literal within the clause.
        position: u32,
    },
    /// A clause names a variable outside the declared variable count.
    LiteralOutOfRange {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the offending literal within the clause.
        position: u32,
        /// The offending literal.
        literal: i32,
        /// The declared variable count it exceeds.
        variables: u32,
    },
    /// A clause's literals are not strictly ascending in the canonical literal order.
    ///
    /// The order is by variable, then negative before positive (see [`crate::wire`]).
    /// Strictness forbids duplicate literals and makes a clause's encoding unique.
    LiteralsNotAscending {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the first literal that did not exceed its predecessor.
        position: u32,
    },
    /// An addition step's clause identifier does not exceed every identifier already
    /// in use.
    ///
    /// LRAT identifiers are assigned in increasing order; the rule makes an
    /// identifier's meaning independent of decoding order and makes the clause store a
    /// strictly ascending table that can be searched rather than scanned.
    IdNotIncreasing {
        /// Index of the offending step.
        step: u32,
        /// The declared identifier.
        found: u32,
        /// The largest identifier already in use.
        previous: u32,
    },
    /// An antecedent names a clause identifier that was never defined.
    HintOutOfRange {
        /// Index of the step.
        step: u32,
        /// Position of the antecedent within the chain.
        position: u32,
        /// The offending identifier.
        id: u32,
    },
    /// An antecedent names a clause that an earlier deletion step removed.
    HintDeleted {
        /// Index of the step.
        step: u32,
        /// Position of the antecedent within the chain.
        position: u32,
        /// The offending identifier.
        id: u32,
    },
    /// An antecedent is already satisfied by the assignment, so it cannot propagate.
    ///
    /// A propagation chain is a sequence of clauses each of which is *unit* under the
    /// running assignment. A satisfied clause proves nothing, and admitting one would
    /// let a producer pad a chain until it happened to end in a conflict.
    HintSatisfied {
        /// Index of the step.
        step: u32,
        /// Position of the antecedent within the chain.
        position: u32,
        /// The offending identifier.
        id: u32,
    },
    /// An antecedent has more than one unassigned literal, so it is not unit.
    HintNotUnit {
        /// Index of the step.
        step: u32,
        /// Position of the antecedent within the chain.
        position: u32,
        /// The offending identifier.
        id: u32,
        /// How many of its literals were unassigned.
        unassigned: u32,
    },
    /// The chain reached a conflict before its final antecedent.
    ///
    /// Every antecedent after the conflict is unreachable, so the chain has a second
    /// encoding with the tail removed — the ambiguity RFC 0005 "Resource bounds"
    /// forbids.
    ConflictBeforeEnd {
        /// Index of the step.
        step: u32,
        /// Position at which the conflict occurred.
        position: u32,
    },
    /// The chain ran to its end without falsifying any antecedent.
    ///
    /// The step's clause is therefore not reverse-unit-propagation derivable from the
    /// clauses it names, which is the whole obligation of an LRAT addition.
    NoConflict {
        /// Index of the step.
        step: u32,
    },
    /// An addition step derives a clause containing a complementary literal pair.
    ///
    /// Such a clause is a tautology: it is implied by anything, so its propagation
    /// obligation is discharged by nothing and the step carries no evidence. Refusing
    /// it keeps every accepted step a real derivation.
    VacuousDerivation {
        /// Index of the step.
        step: u32,
    },
    /// A deletion names a clause identifier that was never defined.
    DeletionOutOfRange {
        /// Index of the step.
        step: u32,
        /// Position within the deletion list.
        position: u32,
        /// The offending identifier.
        id: u32,
    },
    /// A deletion names a clause that an earlier deletion step already removed.
    DeletionRepeated {
        /// Index of the step.
        step: u32,
        /// Position within the deletion list.
        position: u32,
        /// The offending identifier.
        id: u32,
    },
    /// The refutation continues after the empty clause was derived.
    ///
    /// Everything after `⊥` is unreachable, so it is a second encoding of the same
    /// refutation.
    ProofContinuesAfterEmptyClause {
        /// Index of the step that derived the empty clause.
        step: u32,
    },
    /// The proof ended without deriving the empty clause.
    ///
    /// A clausal refutation that never reaches `⊥` refutes nothing, however many valid
    /// steps it contains.
    EmptyClauseNotDerived,
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
            Self::SchemaEpochMismatch { .. } => "schema-epoch-mismatch",
            Self::ZeroLiteral { .. } => "zero-literal",
            Self::LiteralOutOfRange { .. } => "literal-out-of-range",
            Self::LiteralsNotAscending { .. } => "literals-not-ascending",
            Self::IdNotIncreasing { .. } => "id-not-increasing",
            Self::HintOutOfRange { .. } => "hint-out-of-range",
            Self::HintDeleted { .. } => "hint-deleted",
            Self::HintSatisfied { .. } => "hint-satisfied",
            Self::HintNotUnit { .. } => "hint-not-unit",
            Self::ConflictBeforeEnd { .. } => "conflict-before-end",
            Self::NoConflict { .. } => "no-conflict",
            Self::VacuousDerivation { .. } => "vacuous-derivation",
            Self::DeletionOutOfRange { .. } => "deletion-out-of-range",
            Self::DeletionRepeated { .. } => "deletion-repeated",
            Self::ProofContinuesAfterEmptyClause { .. } => "proof-continues-after-empty-clause",
            Self::EmptyClauseNotDerived => "empty-clause-not-derived",
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
/// Distinct from [`Rejection`] on purpose: an unsupported certificate may be perfectly
/// valid, and reporting it as invalid would be a false negative dressed as a verdict.
/// INV-008 keeps "outside the declared fragments" apart from every other non-success
/// outcome; ADR-0017's treatment of unchecked solver evidence is the same move one
/// layer up.
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
    CertificateKind {
        /// The declared family code.
        found: u16,
    },
    /// The proof uses a step kind this build does not implement.
    ///
    /// Reserved for LRAT's RAT steps, whose obligation quantifies over every clause
    /// containing the negated pivot rather than over a named chain. A RAT proof is a
    /// valid proof; this build simply cannot replay it, and says so.
    ProofStep {
        /// The declared step-kind code.
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
        assert_eq!(CertificateKind::from_code(2), None);
        assert_eq!(CertificateKind::from_code(u16::MAX), None);
    }

    #[test]
    fn the_family_token_is_the_receipt_spelling() {
        // `notes/plan/schemas/proof-receipt.schema.json`, certificate.kind enum.
        assert_eq!(CertificateKind::Lrat.as_str(), "lrat");
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
            Field::ClauseCount,
            Field::LiteralCount,
            Field::Literal,
            Field::StepCount,
            Field::StepKind,
            Field::StepId,
            Field::HintCount,
            Field::Hint,
            Field::DeletionCount,
            Field::Deletion,
        ];
        let tokens: std::collections::BTreeSet<&str> = fields.iter().map(|f| f.as_str()).collect();
        assert_eq!(tokens.len(), fields.len());
    }

    #[test]
    fn a_non_verified_verdict_carries_no_claim() {
        let rejected = Verdict::Rejected(Rejection::BadMagic);
        assert!(!rejected.is_verified());
        assert!(rejected.claim().is_none());

        let unsupported = Verdict::Unsupported(Feature::ProofStep { found: 3 });
        assert!(!unsupported.is_verified());
        assert!(unsupported.claim().is_none());
    }
}
