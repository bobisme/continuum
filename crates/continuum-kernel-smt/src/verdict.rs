//! The closed verdict vocabulary of the SMT checking base (INV-008, PR 9).
//!
//! # Why this is not a `bool`, and why "verified" is not one thing
//!
//! > No CLI may print a bare "verified" without exposing the assurance class and
//! > trusted components.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`,
//! >   "Proof-producing solver policy"
//!
//! For SMT this is not a stylistic rule; it is the whole design. Two certificates can
//! both be [`Verdict::Verified`] and mean very different things: one whose empty clause
//! follows from the assertions alone has nothing left to trust, and one that leaned on
//! a linear-arithmetic lemma has an entire decision procedure outside the checked
//! region. [`CheckedClaim`] therefore carries an [`AssuranceClass`] and the list of
//! theories the proof actually used, and a renderer that drops them is misreporting the
//! claim rather than abbreviating it.
//!
//! # The three arms
//!
//! - [`Verdict::Verified`] — every resolution step was re-derived from the
//!   certificate's own clauses. Read [`CheckedClaim::assurance_class`] before calling
//!   it proved anything.
//! - [`Verdict::Rejected`] — the bytes are not a valid certificate of the family they
//!   claim. A statement about the *artifact*, never about the formula.
//! - [`Verdict::Unsupported`] — the bytes name a wire epoch, family, or proof-step kind
//!   this checker does not implement (INV-008).

use crate::wire::{Envelope, Token};

/// The outcome of checking one SMT certificate from its wire form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every resolution step was re-derived from the certificate's own clauses.
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
    /// Never a substitute for matching, and never a substitute for reading
    /// [`CheckedClaim::assurance_class`]: a caller that renders this `bool` as
    /// "proved" has thrown away the distinction ADR-0017 exists to preserve.
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

/// The certificate families `continuum-kernel-smt` implements.
///
/// `smt-proof` is the spelling `notes/plan/schemas/proof-receipt.schema.json` fixes in
/// its `certificate.kind` enum. A wire `kind` code outside this enum is
/// [`Feature::CertificateKind`], not a rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertificateKind {
    /// A propositional skeleton over theory atoms, a set of theory lemmas, and a
    /// resolution refutation of their conjunction (RFC 0005 "Kernel layering",
    /// "Alethe checker subset"; docs/03 §6.2).
    SmtProof,
}

impl CertificateKind {
    /// Every family this crate implements, in wire-code order.
    pub const ALL: [Self; 1] = [Self::SmtProof];

    /// The `kind` code this family occupies in the wire header.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::SmtProof => 1,
        }
    }

    /// The family a wire `kind` code names, or `None`.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::SmtProof),
            _ => None,
        }
    }

    /// The stable lower-case token naming this family in receipts and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SmtProof => "smt-proof",
        }
    }
}

/// How much of a verified claim rests on evidence this kernel actually replayed.
///
/// ```text
/// UNSAT:
///   Alethe/LRAT proof checked         → certificate checked
///   proof unavailable                 → solver-trusted
/// ```
///
/// — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §7, "Solver trust classes"
///
/// docs/03 §6.2 puts a proof that leans on unchecked theory lemmas on the second line:
/// "Until theory-proof support is mature, the result is `TRUSTED_SOLVER`, not
/// `CHECKED_CERTIFICATE`." ADR-0017 is the decision record. The two spellings below are
/// those two tokens, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssuranceClass {
    /// Every clause the refutation used is either an asserted skeleton clause or a
    /// clause this kernel derived. Nothing is trusted beyond the encoding itself.
    CheckedCertificate,
    /// The refutation used at least one theory lemma, whose validity this kernel
    /// cannot check. The named theories are trusted components.
    TrustedSolver,
}

impl AssuranceClass {
    /// The docs/03 §7 spelling of this class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckedCertificate => "CHECKED_CERTIFICATE",
            Self::TrustedSolver => "TRUSTED_SOLVER",
        }
    }
}

/// What a [`Verdict::Verified`] actually asserts.
///
/// The claim is precisely: *the carried propositional skeleton, conjoined with the
/// carried theory lemmas, has no propositional model*. Whether that entails the
/// caller's goal depends on two things the kernel cannot see — that the skeleton is the
/// abstraction of the asserted formulas, and that each used lemma is valid in the
/// theory it names — and both are named by [`Self::trusted_components`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClaim {
    kind: CertificateKind,
    envelope: Envelope,
    atoms: u32,
    assertions: u32,
    lemmas_carried: u32,
    lemmas_used: u32,
    derived_clauses: u32,
    deleted_clauses: u32,
    propagations: u64,
    theories: Vec<Token>,
}

impl CheckedClaim {
    /// Build a claim record. Crate-internal: only a completed check may mint one.
    #[allow(
        clippy::too_many_arguments,
        reason = "a claim is a record of everything the check re-derived; collapsing \
                  the fields into a struct literal here would only move the list"
    )]
    pub(crate) const fn new(
        kind: CertificateKind,
        envelope: Envelope,
        atoms: u32,
        assertions: u32,
        lemmas_carried: u32,
        lemmas_used: u32,
        derived_clauses: u32,
        deleted_clauses: u32,
        propagations: u64,
        theories: Vec<Token>,
    ) -> Self {
        Self {
            kind,
            envelope,
            atoms,
            assertions,
            lemmas_carried,
            lemmas_used,
            derived_clauses,
            deleted_clauses,
            propagations,
            theories,
        }
    }

    /// The certificate family that was checked.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        self.kind
    }

    /// The claim envelope the certificate carried (RFC 0005 "Claim envelope").
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// How many theory atoms the certificate declared.
    #[must_use]
    pub const fn atoms(&self) -> u32 {
        self.atoms
    }

    /// How many skeleton clauses the assertion set contributed.
    #[must_use]
    pub const fn assertions(&self) -> u32 {
        self.assertions
    }

    /// How many theory lemmas the certificate carries.
    #[must_use]
    pub const fn lemmas_carried(&self) -> u32 {
        self.lemmas_carried
    }

    /// How many theory lemmas the refutation actually named as antecedents.
    ///
    /// A carried but unused lemma inflates the artifact, not the trust set.
    #[must_use]
    pub const fn lemmas_used(&self) -> u32 {
        self.lemmas_used
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

    /// How many unit propagations the checker performed replaying the refutation.
    ///
    /// RFC 0005's "deterministic resource accounting", as a function of the bytes.
    #[must_use]
    pub const fn propagations(&self) -> u64 {
        self.propagations
    }

    /// The theories whose lemmas the refutation used, deduplicated and ascending.
    #[must_use]
    pub fn theories(&self) -> &[Token] {
        &self.theories
    }

    /// docs/03 §7's assurance class for this claim.
    ///
    /// [`AssuranceClass::CheckedCertificate`] exactly when the refutation used no
    /// theory lemma: a propositionally unsatisfiable skeleton is unsatisfiable under
    /// every theory, so nothing about the theories needs to be believed.
    #[must_use]
    pub fn assurance_class(&self) -> AssuranceClass {
        if self.theories.is_empty() {
            AssuranceClass::CheckedCertificate
        } else {
            AssuranceClass::TrustedSolver
        }
    }

    /// The components this verdict still trusts, in the sense of docs/03 §2's
    /// `trusted: Vec<TrustedComponent>`.
    ///
    /// Two are structural and always present:
    ///
    /// - `skeleton-model-correspondence` — that the carried atom table and skeleton
    ///   clauses are the propositional abstraction of the formulas named by the
    ///   envelope digests. The kernel treats atoms as opaque tokens and could not
    ///   check this if it wanted to; RFC 0024's receipt is where the binding is
    ///   recorded.
    /// - `envelope-digest-binding` — that the envelope's digests were computed over
    ///   the artifacts they name (ADR-0013 canonical identity is `continuum-value`'s
    ///   obligation).
    ///
    /// Then one `theory-lemma:<theory>` entry per theory whose lemmas the refutation
    /// used. This is a deliberate over-approximation: a lemma is counted as used when
    /// any accepted step names it as an antecedent, even if the clause that step
    /// derived turns out not to feed the empty clause. Naming a theory that was not
    /// strictly needed is safe; omitting one that was is not.
    #[must_use]
    pub fn trusted_components(&self) -> Vec<String> {
        let mut components = vec![
            "envelope-digest-binding".to_owned(),
            "skeleton-model-correspondence".to_owned(),
        ];
        for theory in &self.theories {
            let mut entry = String::from("theory-lemma:");
            entry.push_str(theory.as_str());
            components.push(entry);
        }
        components
    }
}

/// A named position in the wire form, so a rejection says *where*.
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
    /// The number of declared theory atoms.
    AtomCount,
    /// One theory atom.
    Atom,
    /// The number of asserted skeleton clauses.
    AssertionCount,
    /// The number of theory lemmas.
    LemmaCount,
    /// One theory lemma's theory name.
    LemmaTheory,
    /// Any clause: the number of literals it carries.
    LiteralCount,
    /// Any clause: one literal.
    Literal,
    /// Proof: the number of steps.
    StepCount,
    /// Proof: one step's kind code.
    StepKind,
    /// Proof: a resolution step's clause identifier.
    StepId,
    /// Proof: the number of antecedents in one step's chain.
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
            Self::AtomCount => "atom-count",
            Self::Atom => "atom",
            Self::AssertionCount => "assertion-count",
            Self::LemmaCount => "lemma-count",
            Self::LemmaTheory => "lemma-theory",
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
/// Tokens here are the envelope digests, the theory atoms, and the theory names.
/// Restricting them to non-empty printable ASCII is the same discipline
/// `continuum-value`'s `Name` applies for the same reason (ADR-0013): an identity with
/// two spellings is two identities.
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

/// Every reason the kernel refuses a byte string as an SMT certificate.
///
/// The vocabulary is closed. Each variant is a statement about the artifact — never
/// about the formula, the theories, or the solver that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The certificate is larger than [`crate::wire::MAX_CERTIFICATE_BYTES`]
    /// (RFC 0005 "Resource bounds"; docs/16 PO-KER-002).
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
    /// Bytes remain after the certificate body ends (RFC 0005 "rejection of
    /// duplicate/ambiguous encodings").
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
    SchemaEpochMismatch {
        /// The epoch declared in the envelope.
        declared: u16,
        /// The epoch declared in the header.
        header: u16,
    },
    /// A clause carries the literal `0`, which names no atom and no polarity.
    ZeroLiteral {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the offending literal within the clause.
        position: u32,
    },
    /// A clause names an atom outside the declared atom table.
    ///
    /// The atom table is the certificate's whole vocabulary: a literal that leaves it
    /// refers to something the artifact never introduced.
    AtomOutOfRange {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the offending literal within the clause.
        position: u32,
        /// The offending literal.
        literal: i32,
        /// The declared atom count it exceeds.
        atoms: u32,
    },
    /// A clause's literals are not strictly ascending in the canonical literal order.
    LiteralsNotAscending {
        /// Identifier of the clause.
        clause: u32,
        /// Position of the first literal that did not exceed its predecessor.
        position: u32,
    },
    /// A resolution step's clause identifier does not exceed every identifier already
    /// in use.
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
    ConflictBeforeEnd {
        /// Index of the step.
        step: u32,
        /// Position at which the conflict occurred.
        position: u32,
    },
    /// The chain ran to its end without falsifying any antecedent.
    NoConflict {
        /// Index of the step.
        step: u32,
    },
    /// A step derives a clause containing a complementary literal pair, so its
    /// obligation is discharged by nothing.
    VacuousDerivation {
        /// Index of the step.
        step: u32,
    },
    /// A theory lemma is a tautology of propositional logic.
    ///
    /// Such a lemma needs no theory, so labelling it with one would put a theory into
    /// the trusted set for no reason. It is refused rather than silently reclassified,
    /// because a producer that emits one is describing its proof incorrectly.
    VacuousLemma {
        /// Index of the lemma in the lemma list.
        lemma: u32,
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
    ProofContinuesAfterEmptyClause {
        /// Index of the step that derived the empty clause.
        step: u32,
    },
    /// The proof ended without deriving the empty clause.
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
            Self::AtomOutOfRange { .. } => "atom-out-of-range",
            Self::LiteralsNotAscending { .. } => "literals-not-ascending",
            Self::IdNotIncreasing { .. } => "id-not-increasing",
            Self::HintOutOfRange { .. } => "hint-out-of-range",
            Self::HintDeleted { .. } => "hint-deleted",
            Self::HintSatisfied { .. } => "hint-satisfied",
            Self::HintNotUnit { .. } => "hint-not-unit",
            Self::ConflictBeforeEnd { .. } => "conflict-before-end",
            Self::NoConflict { .. } => "no-conflict",
            Self::VacuousDerivation { .. } => "vacuous-derivation",
            Self::VacuousLemma { .. } => "vacuous-lemma",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// The certificate declares a wire epoch this build does not implement
    /// (plan §4.6, ADR-0018).
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
    /// Reserved for the Alethe rule vocabulary a theory-step checker will need. An
    /// Alethe proof carrying, say, a `la_generic` step is a valid proof; this build
    /// cannot replay it and says so rather than calling it invalid.
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
    }

    #[test]
    fn the_family_token_is_the_receipt_spelling() {
        // `notes/plan/schemas/proof-receipt.schema.json`, certificate.kind enum.
        assert_eq!(CertificateKind::SmtProof.as_str(), "smt-proof");
    }

    #[test]
    fn the_assurance_tokens_are_the_docs_03_spellings() {
        assert_eq!(
            AssuranceClass::CheckedCertificate.as_str(),
            "CHECKED_CERTIFICATE"
        );
        assert_eq!(AssuranceClass::TrustedSolver.as_str(), "TRUSTED_SOLVER");
        // Ordering matters for rendering: the weaker class must not sort as the
        // stronger one by accident.
        assert!(AssuranceClass::CheckedCertificate < AssuranceClass::TrustedSolver);
    }

    #[test]
    fn field_tokens_are_unique() {
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
            Field::AtomCount,
            Field::Atom,
            Field::AssertionCount,
            Field::LemmaCount,
            Field::LemmaTheory,
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

        let unsupported = Verdict::Unsupported(Feature::ProofStep { found: 4 });
        assert!(!unsupported.is_verified());
        assert!(unsupported.claim().is_none());
    }
}
