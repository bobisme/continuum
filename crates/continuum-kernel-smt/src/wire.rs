//! The SMT certificate wire form: serialized bytes in, inert data out (PR 9).
//!
//! # Why a wire form at all
//!
//! > the `continuum-kernel-*` crates form the trusted checking base: … a
//! > serialization boundary between every engine and the kernel — a certificate is
//! > checked from its wire form, never from shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! Every public entry point of this crate takes `&[u8]`. There is no constructor for a
//! decoded certificate outside this module, so no solver adapter can hand the checker a
//! structure it built.
//!
//! # Why this crate decodes for itself
//!
//! docs/03 §8 lists "certificate decoding" among the axes independent paths must differ
//! along, and plan §20's crate table has four kernel crates and no shared one. The
//! reader below is therefore this crate's own, not `continuum-kernel-sat`'s, even
//! though the two formats rhyme.
//!
//! # The contract
//!
//! The dossier fixes the certificate *family* — RFC 0005's "Kernel layering" assigns
//! this crate the "Alethe checker subset", docs/03 §6.2 states the theory-lemma trust
//! rule, and `notes/plan/schemas/proof-receipt.schema.json` fixes the receipt spelling
//! `smt-proof` — but no byte layout, and it does not fix which Alethe rules a first
//! slice must implement. What follows is therefore this crate's own contract, derived
//! from docs/03 §6.2's sentence "backend proofs can be Alethe/LRAT/solver-specific
//! **plus checked theory lemmas**": the resolution skeleton is the part that is
//! checkable without a decision procedure, and the theory lemmas are exactly the
//! remainder. It is versioned by [`WIRE_EPOCH`] so that a later layout — one carrying
//! Alethe rule applications rather than opaque lemmas — is a new epoch rather than a
//! silent reinterpretation (plan §4.6; ADR-0018).
//!
//! ```text
//! certificate  := header envelope body
//!
//! header       := magic:8 wire_epoch:u16 kind:u16
//! magic        := "CONTSMTC"
//! wire_epoch   := 1                                  -- WIRE_EPOCH
//! kind         := 1 smt-proof                        -- CertificateKind::code
//!
//! envelope     := model_digest:token                 -- RFC 0005 "model hash"
//!                 semantic_epoch:token               -- "CIR semantics version"
//!                 property_digest:token              -- "property hash"
//!                 scope_digest:token                 -- "bounds"
//!                 assumptions_digest:token           -- "assumptions"
//!                 producer:token                     -- "engine version"
//!                 schema_epoch:u16                   -- "certificate schema version"
//!                 domain_pack_count:u16 token*       -- "domain-pack profile hashes"
//!
//! body(1)      := atoms assertions lemmas proof
//! atoms        := atom_count:u32 token*              -- strictly ascending
//! assertions   := assertion_count:u32 clause*        -- the Boolean skeleton
//! lemmas       := lemma_count:u32 lemma*
//! lemma        := theory:token clause
//! clause       := literal_count:u32 literal*
//! literal      := i32                                -- nonzero, |l| <= atom_count
//! proof        := step_count:u32 step*
//! step         := step_kind:u16 payload
//! step_kind    := 1 resolve | 2 delete               -- other codes are Unsupported
//! resolve      := id:u32 clause hint_count:u32 hint*
//! hint         := u32                                -- an antecedent clause id
//! delete       := id_count:u32 id*
//! ```
//!
//! Integers are big-endian; `i32` is two's complement. Nothing nests, so the format has
//! no recursion for RFC 0005's "cycle/recursion bounds" to bound.
//!
//! # Atoms are opaque
//!
//! An atom is a printable-ASCII token and nothing else. The kernel never parses it,
//! never compares two atoms for theory-equivalence, and never infers a relation between
//! them: `x-lt-y` and `y-gt-x` are two atoms, and a certificate that needs them to be
//! the same must say so with a theory lemma. That is what makes the propositional half
//! checkable without a decision procedure, and it is why the atom table's
//! correspondence to the model is a trusted component rather than a checked one.
//!
//! # Clause identifiers
//!
//! Positional for the inputs, explicit and increasing for the derived clauses:
//!
//! ```text
//! 1 ..= assertion_count                       the asserted skeleton clauses
//! assertion_count+1 ..= +lemma_count          the theory lemmas
//! then, per resolution step, its declared id  strictly greater than every id so far
//! ```
//!
//! The identifier of an antecedent is therefore what says whether the refutation leaned
//! on a theory — [`crate::check`] classifies each antecedent by identifier range and
//! reports the theories it reached for.
//!
//! # Canonicity
//!
//! Sets are strictly ascending: the domain-pack digests, the atom table, the literals
//! of a clause (by atom, negative polarity first), and the identifiers of a deletion
//! step. Sequences whose index carries meaning are not sorted: the assertions and
//! lemmas (their positions are their identifiers), the proof steps (their order is the
//! derivation), and the antecedents of a chain (their order is the propagation order).
//!
//! Trailing bytes are rejected, every count has a declared maximum, running totals are
//! enforced across clauses and chains, and the byte string itself has
//! [`MAX_CERTIFICATE_BYTES`] (RFC 0005 "Resource bounds"; docs/16 PO-KER-002).
//!
//! # Malformed input is data, not an event
//!
//! No indexing, no slicing, no `unwrap`, no `expect`, and no wire-derived
//! `Vec::with_capacity`. Every read goes through [`Reader`] and returns a typed
//! [`Rejection`] (docs/12 §11; docs/16 PO-KER-001).

use crate::verdict::{CertificateKind, Feature, Field, Rejection, TokenFault};

/// The eight bytes every SMT certificate begins with.
pub const MAGIC: [u8; 8] = *b"CONTSMTC";

/// The wire epoch this build implements.
pub const WIRE_EPOCH: u16 = 1;

/// The largest certificate byte string the kernel will look at (64 MiB).
pub const MAX_CERTIFICATE_BYTES: usize = 1 << 26;

/// The longest admissible token (digest, atom, theory name).
pub const MAX_TOKEN_BYTES: usize = 128;

/// The largest number of domain-pack profile digests an envelope may carry.
pub const MAX_DOMAIN_PACKS: u16 = 64;

/// The largest number of theory atoms.
pub const MAX_ATOMS: u32 = 1 << 20;

/// The largest number of asserted skeleton clauses.
pub const MAX_ASSERTIONS: u32 = 1 << 20;

/// The largest number of theory lemmas.
pub const MAX_LEMMAS: u32 = 1 << 20;

/// The largest number of literals in one clause.
pub const MAX_CLAUSE_LITERALS: u32 = 1 << 16;

/// The largest number of literals a certificate may carry in total.
pub const MAX_TOTAL_LITERALS: u64 = 1 << 22;

/// The largest number of proof steps.
pub const MAX_STEPS: u32 = 1 << 20;

/// The largest number of antecedents in one propagation chain.
pub const MAX_HINTS: u32 = 1 << 16;

/// The largest number of antecedents a certificate may carry in total.
pub const MAX_TOTAL_HINTS: u64 = 1 << 22;

/// The largest number of identifiers one deletion step may name.
pub const MAX_DELETIONS: u32 = 1 << 16;

/// The largest admissible clause identifier.
pub const MAX_CLAUSE_ID: u32 = 1 << 30;

// ---------------------------------------------------------------------------
// reader
// ---------------------------------------------------------------------------

/// A bounds-checked, forward-only reader over certificate bytes.
#[derive(Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Start reading at the beginning of `bytes`.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// How many bytes have not been consumed.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    /// Whether every byte has been consumed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Take exactly `len` bytes, or report the truncation.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than `len` bytes remain.
    pub fn take(&mut self, field: Field, len: usize) -> Result<&'a [u8], Rejection> {
        let end = self.offset.checked_add(len).ok_or(Rejection::Truncated {
            field,
            offset: self.offset,
            needed: len,
            available: self.remaining(),
        })?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(Rejection::Truncated {
                field,
                offset: self.offset,
                needed: len,
                available: self.remaining(),
            })?;
        self.offset = end;
        Ok(slice)
    }

    /// Read one big-endian `u16`.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than two bytes remain.
    pub fn u16(&mut self, field: Field) -> Result<u16, Rejection> {
        let bytes = self.take(field, 2)?;
        let mut buf = [0_u8; 2];
        buf.copy_from_slice(bytes);
        Ok(u16::from_be_bytes(buf))
    }

    /// Read one big-endian `u32`.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than four bytes remain.
    pub fn u32(&mut self, field: Field) -> Result<u32, Rejection> {
        let bytes = self.take(field, 4)?;
        let mut buf = [0_u8; 4];
        buf.copy_from_slice(bytes);
        Ok(u32::from_be_bytes(buf))
    }

    /// Read one big-endian two's-complement `i32`.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than four bytes remain.
    pub fn i32(&mut self, field: Field) -> Result<i32, Rejection> {
        let bytes = self.take(field, 4)?;
        let mut buf = [0_u8; 4];
        buf.copy_from_slice(bytes);
        Ok(i32::from_be_bytes(buf))
    }

    /// Read one length-prefixed token and validate its shape.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] on a short read, or [`Rejection::MalformedToken`] when
    /// the token is empty, too long, or contains a byte outside printable ASCII.
    pub fn token(&mut self, field: Field) -> Result<Token, Rejection> {
        let len = usize::from(self.u16(field)?);
        if len == 0 {
            return Err(Rejection::MalformedToken {
                field,
                fault: TokenFault::Empty,
            });
        }
        if len > MAX_TOKEN_BYTES {
            return Err(Rejection::MalformedToken {
                field,
                fault: TokenFault::TooLong { found: len },
            });
        }
        let bytes = self.take(field, len)?;
        for (offset, byte) in bytes.iter().enumerate() {
            if !byte.is_ascii_graphic() {
                return Err(Rejection::MalformedToken {
                    field,
                    fault: TokenFault::NonPrintable {
                        offset,
                        byte: *byte,
                    },
                });
            }
        }
        Ok(Token(bytes.to_vec()))
    }

    /// Read a count and check it against the format's declared range.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] on a short read, or [`Rejection::CountOutOfRange`] when
    /// the declared count is outside `min..=max`.
    fn counted_u16(&mut self, field: Field, min: u16, max: u16) -> Result<u16, Rejection> {
        let found = self.u16(field)?;
        if found < min || found > max {
            return Err(Rejection::CountOutOfRange {
                field,
                found: u64::from(found),
                min: u64::from(min),
                max: u64::from(max),
            });
        }
        Ok(found)
    }

    /// As [`Self::counted_u16`], for a 32-bit count.
    fn counted_u32(&mut self, field: Field, min: u32, max: u32) -> Result<u32, Rejection> {
        let found = self.u32(field)?;
        if found < min || found > max {
            return Err(Rejection::CountOutOfRange {
                field,
                found: u64::from(found),
                min: u64::from(min),
                max: u64::from(max),
            });
        }
        Ok(found)
    }
}

// ---------------------------------------------------------------------------
// tokens and the envelope
// ---------------------------------------------------------------------------

/// A non-empty printable-ASCII byte string: a digest, an atom, or a theory name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(Vec<u8>);

impl Token {
    /// The token's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The token as `&str`.
    ///
    /// Total: the decoder admits only printable ASCII, so the bytes are UTF-8 by
    /// construction. The fallback exists so that no code path can panic even if that
    /// invariant were ever weakened.
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("")
    }
}

/// The claim envelope a certificate is valid *relative to* (RFC 0005 "Claim
/// envelope"): model hash, CIR semantics version, property hash, domain-pack profile
/// hashes, bounds, assumptions, engine version, certificate schema version.
///
/// The kernel checks that every field is present and well-shaped, that the digest list
/// is canonical, and that [`Self::schema_epoch`] agrees with the header. Whether
/// `model_digest` is a digest of the caller's model is bound by the receipt (RFC 0024,
/// ADR-0035), not by these bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    model_digest: Token,
    semantic_epoch: Token,
    property_digest: Token,
    scope_digest: Token,
    assumptions_digest: Token,
    producer: Token,
    schema_epoch: u16,
    domain_pack_digests: Vec<Token>,
}

impl Envelope {
    /// Digest of the model the certificate speaks about.
    #[must_use]
    pub const fn model_digest(&self) -> &Token {
        &self.model_digest
    }

    /// The semantic epoch under which the certificate's claims are meaningful.
    #[must_use]
    pub const fn semantic_epoch(&self) -> &Token {
        &self.semantic_epoch
    }

    /// Digest of the property the certificate establishes.
    #[must_use]
    pub const fn property_digest(&self) -> &Token {
        &self.property_digest
    }

    /// Digest of the declared bounds/scope the claim is made within.
    #[must_use]
    pub const fn scope_digest(&self) -> &Token {
        &self.scope_digest
    }

    /// Digest of the assumptions the claim is made under.
    #[must_use]
    pub const fn assumptions_digest(&self) -> &Token {
        &self.assumptions_digest
    }

    /// Identity of the solver adapter build that produced the certificate.
    #[must_use]
    pub const fn producer(&self) -> &Token {
        &self.producer
    }

    /// The certificate schema epoch the producer wrote against.
    #[must_use]
    pub const fn schema_epoch(&self) -> u16 {
        self.schema_epoch
    }

    /// Domain-pack profile digests, strictly ascending.
    #[must_use]
    pub fn domain_pack_digests(&self) -> &[Token] {
        &self.domain_pack_digests
    }

    fn decode(reader: &mut Reader<'_>, header_epoch: u16) -> Result<Self, Rejection> {
        let model_digest = reader.token(Field::ModelDigest)?;
        let semantic_epoch = reader.token(Field::SemanticEpoch)?;
        let property_digest = reader.token(Field::PropertyDigest)?;
        let scope_digest = reader.token(Field::ScopeDigest)?;
        let assumptions_digest = reader.token(Field::AssumptionsDigest)?;
        let producer = reader.token(Field::Producer)?;
        let schema_epoch = reader.u16(Field::SchemaEpoch)?;
        if schema_epoch != header_epoch {
            return Err(Rejection::SchemaEpochMismatch {
                declared: schema_epoch,
                header: header_epoch,
            });
        }
        let count = reader.counted_u16(Field::DomainPackCount, 0, MAX_DOMAIN_PACKS)?;
        let mut domain_pack_digests: Vec<Token> = Vec::new();
        for index in 0..count {
            let digest = reader.token(Field::DomainPack)?;
            if let Some(previous) = domain_pack_digests.last()
                && *previous >= digest
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::DomainPack,
                    index: u32::from(index),
                });
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
            schema_epoch,
            domain_pack_digests,
        })
    }
}

// ---------------------------------------------------------------------------
// clauses and lemmas
// ---------------------------------------------------------------------------

/// The canonical order on literals: by atom, negative polarity first.
#[must_use]
const fn literal_key(literal: i32) -> (u32, bool) {
    (literal.unsigned_abs(), literal > 0)
}

/// One clause over theory atoms: a set of literals, strictly ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    literals: Vec<i32>,
}

impl Clause {
    /// The clause's literals, in canonical order.
    #[must_use]
    pub fn literals(&self) -> &[i32] {
        &self.literals
    }

    /// How many literals the clause has. Zero is the empty clause, `⊥`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.literals.len()
    }

    /// Whether this is the empty clause.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    /// Whether the clause contains a complementary literal pair.
    #[must_use]
    pub fn is_tautology(&self) -> bool {
        let mut previous: Option<i32> = None;
        for literal in &self.literals {
            if let Some(before) = previous
                && before.unsigned_abs() == literal.unsigned_abs()
            {
                return true;
            }
            previous = Some(*literal);
        }
        false
    }

    fn decode(
        reader: &mut Reader<'_>,
        id: u32,
        atoms: u32,
        total_literals: &mut u64,
    ) -> Result<Self, Rejection> {
        let count = reader.counted_u32(Field::LiteralCount, 0, MAX_CLAUSE_LITERALS)?;
        *total_literals = total_literals.saturating_add(u64::from(count));
        if *total_literals > MAX_TOTAL_LITERALS {
            return Err(Rejection::CountOutOfRange {
                field: Field::LiteralCount,
                found: *total_literals,
                min: 0,
                max: MAX_TOTAL_LITERALS,
            });
        }
        let mut literals: Vec<i32> = Vec::new();
        for position in 0..count {
            let literal = reader.i32(Field::Literal)?;
            if literal == 0 {
                return Err(Rejection::ZeroLiteral {
                    clause: id,
                    position,
                });
            }
            if literal.unsigned_abs() > atoms {
                return Err(Rejection::AtomOutOfRange {
                    clause: id,
                    position,
                    literal,
                    atoms,
                });
            }
            if let Some(previous) = literals.last()
                && literal_key(*previous) >= literal_key(literal)
            {
                return Err(Rejection::LiteralsNotAscending {
                    clause: id,
                    position,
                });
            }
            literals.push(literal);
        }
        Ok(Self { literals })
    }
}

/// A clause the certificate asserts is valid in a named theory.
///
/// The kernel treats the theory name as a label and the clause as an axiom. Both travel
/// into [`crate::CheckedClaim::trusted_components`] when the refutation uses the lemma;
/// neither is checked. docs/03 §6.2's "plus checked theory lemmas" is the slice after
/// this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lemma {
    theory: Token,
    clause: Clause,
}

impl Lemma {
    /// The theory the producer claims makes this clause valid.
    #[must_use]
    pub const fn theory(&self) -> &Token {
        &self.theory
    }

    /// The lemma clause.
    #[must_use]
    pub const fn clause(&self) -> &Clause {
        &self.clause
    }
}

// ---------------------------------------------------------------------------
// proof steps
// ---------------------------------------------------------------------------

/// A clause derivation justified by a propagation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    id: u32,
    clause: Clause,
    hints: Vec<u32>,
}

impl Resolution {
    /// The identifier the derived clause takes.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// The derived clause.
    #[must_use]
    pub const fn clause(&self) -> &Clause {
        &self.clause
    }

    /// The antecedent chain, in propagation order.
    #[must_use]
    pub fn hints(&self) -> &[u32] {
        &self.hints
    }
}

/// A clause deletion: identifiers to remove from the active set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deletion {
    ids: Vec<u32>,
}

impl Deletion {
    /// The identifiers to delete, strictly ascending.
    #[must_use]
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
}

/// One step of the refutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Derive a clause, justified by a propagation chain.
    Resolve(Resolution),
    /// Delete clauses that later steps will not need.
    Delete(Deletion),
}

/// The decoded body of an SMT certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtProofBody {
    atoms: Vec<Token>,
    assertions: Vec<Clause>,
    lemmas: Vec<Lemma>,
    steps: Vec<Step>,
}

impl SmtProofBody {
    /// The theory atoms, strictly ascending. Opaque to the kernel.
    #[must_use]
    pub fn atoms(&self) -> &[Token] {
        &self.atoms
    }

    /// The propositional skeleton of the asserted formulas, in identifier order.
    #[must_use]
    pub fn assertions(&self) -> &[Clause] {
        &self.assertions
    }

    /// The theory lemmas, in identifier order.
    #[must_use]
    pub fn lemmas(&self) -> &[Lemma] {
        &self.lemmas
    }

    /// The refutation, in derivation order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeFailure> {
        let atom_count = reader.counted_u32(Field::AtomCount, 1, MAX_ATOMS)?;
        let mut atoms: Vec<Token> = Vec::new();
        for index in 0..atom_count {
            let atom = reader.token(Field::Atom)?;
            if let Some(previous) = atoms.last()
                && *previous >= atom
            {
                return Err(DecodeFailure::Rejected(Rejection::NotStrictlyAscending {
                    field: Field::Atom,
                    index,
                }));
            }
            atoms.push(atom);
        }

        let mut total_literals: u64 = 0;
        let assertion_count = reader.counted_u32(Field::AssertionCount, 1, MAX_ASSERTIONS)?;
        let mut assertions: Vec<Clause> = Vec::new();
        for index in 0..assertion_count {
            let id = index.saturating_add(1);
            assertions.push(Clause::decode(reader, id, atom_count, &mut total_literals)?);
        }

        let lemma_count = reader.counted_u32(Field::LemmaCount, 0, MAX_LEMMAS)?;
        let mut lemmas: Vec<Lemma> = Vec::new();
        for index in 0..lemma_count {
            let id = assertion_count.saturating_add(index).saturating_add(1);
            let theory = reader.token(Field::LemmaTheory)?;
            let clause = Clause::decode(reader, id, atom_count, &mut total_literals)?;
            if clause.is_tautology() {
                return Err(DecodeFailure::Rejected(Rejection::VacuousLemma {
                    lemma: index,
                }));
            }
            lemmas.push(Lemma { theory, clause });
        }

        let mut largest_id = assertion_count.saturating_add(lemma_count);
        let step_count = reader.counted_u32(Field::StepCount, 1, MAX_STEPS)?;
        let mut steps: Vec<Step> = Vec::new();
        let mut total_hints: u64 = 0;
        for index in 0..step_count {
            let kind = reader.u16(Field::StepKind)?;
            match kind {
                1 => {
                    let id = reader.u32(Field::StepId)?;
                    if id <= largest_id || id > MAX_CLAUSE_ID {
                        return Err(DecodeFailure::Rejected(Rejection::IdNotIncreasing {
                            step: index,
                            found: id,
                            previous: largest_id,
                        }));
                    }
                    largest_id = id;
                    let clause = Clause::decode(reader, id, atom_count, &mut total_literals)?;
                    let hint_count = reader.counted_u32(Field::HintCount, 0, MAX_HINTS)?;
                    total_hints = total_hints.saturating_add(u64::from(hint_count));
                    if total_hints > MAX_TOTAL_HINTS {
                        return Err(DecodeFailure::Rejected(Rejection::CountOutOfRange {
                            field: Field::HintCount,
                            found: total_hints,
                            min: 0,
                            max: MAX_TOTAL_HINTS,
                        }));
                    }
                    let mut hints: Vec<u32> = Vec::new();
                    for position in 0..hint_count {
                        let hint = reader.u32(Field::Hint)?;
                        if hint == 0 || hint > MAX_CLAUSE_ID {
                            return Err(DecodeFailure::Rejected(Rejection::HintOutOfRange {
                                step: index,
                                position,
                                id: hint,
                            }));
                        }
                        hints.push(hint);
                    }
                    steps.push(Step::Resolve(Resolution { id, clause, hints }));
                }
                2 => {
                    let id_count = reader.counted_u32(Field::DeletionCount, 1, MAX_DELETIONS)?;
                    let mut ids: Vec<u32> = Vec::new();
                    for position in 0..id_count {
                        let id = reader.u32(Field::Deletion)?;
                        if id == 0 || id > MAX_CLAUSE_ID {
                            return Err(DecodeFailure::Rejected(Rejection::DeletionOutOfRange {
                                step: index,
                                position,
                                id,
                            }));
                        }
                        if let Some(previous) = ids.last()
                            && *previous >= id
                        {
                            return Err(DecodeFailure::Rejected(Rejection::NotStrictlyAscending {
                                field: Field::Deletion,
                                index: position,
                            }));
                        }
                        ids.push(id);
                    }
                    steps.push(Step::Delete(Deletion { ids }));
                }
                other => {
                    return Err(DecodeFailure::Unsupported(Feature::ProofStep {
                        found: other,
                    }));
                }
            }
        }

        Ok(Self {
            atoms,
            assertions,
            lemmas,
            steps,
        })
    }
}

/// A decoded certificate: the envelope plus one family body.
///
/// Inert data, with no constructor outside [`decode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    envelope: Envelope,
    body: Body,
}

impl Certificate {
    /// The claim envelope (RFC 0005).
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// The family body.
    #[must_use]
    pub const fn body(&self) -> &Body {
        &self.body
    }

    /// The family this certificate belongs to.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        match self.body {
            Body::SmtProof(_) => CertificateKind::SmtProof,
        }
    }
}

/// The family-specific half of a decoded certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A skeleton-plus-lemmas refutation.
    SmtProof(SmtProofBody),
}

/// Decode a certificate from its wire form.
///
/// # Errors
///
/// [`DecodeFailure::Rejected`] when the bytes are not a valid certificate;
/// [`DecodeFailure::Unsupported`] when they name a wire epoch, family, or proof-step
/// kind this build does not implement (INV-008).
pub fn decode(bytes: &[u8]) -> Result<Certificate, DecodeFailure> {
    if bytes.len() > MAX_CERTIFICATE_BYTES {
        return Err(DecodeFailure::Rejected(Rejection::Oversized {
            found: bytes.len(),
        }));
    }
    let mut reader = Reader::new(bytes);

    let magic = reader.take(Field::Magic, MAGIC.len())?;
    if magic != MAGIC {
        return Err(DecodeFailure::Rejected(Rejection::BadMagic));
    }
    let epoch = reader.u16(Field::WireEpoch)?;
    if epoch != WIRE_EPOCH {
        return Err(DecodeFailure::Unsupported(Feature::WireEpoch {
            found: epoch,
        }));
    }
    let kind_code = reader.u16(Field::Kind)?;
    let Some(kind) = CertificateKind::from_code(kind_code) else {
        return Err(DecodeFailure::Unsupported(Feature::CertificateKind {
            found: kind_code,
        }));
    };

    let envelope = Envelope::decode(&mut reader, epoch)?;
    let body = match kind {
        CertificateKind::SmtProof => Body::SmtProof(SmtProofBody::decode(&mut reader)?),
    };

    if !reader.is_empty() {
        return Err(DecodeFailure::Rejected(Rejection::TrailingBytes {
            extra: reader.remaining(),
        }));
    }
    Ok(Certificate { envelope, body })
}

/// Why [`decode`] did not produce a certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeFailure {
    /// The bytes are not a well-formed, internally consistent certificate.
    Rejected(Rejection),
    /// The bytes name a wire epoch, family, or step kind this build does not
    /// implement.
    Unsupported(Feature),
}

impl From<Rejection> for DecodeFailure {
    fn from(rejection: Rejection) -> Self {
        Self::Rejected(rejection)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures; the no-panic covenant \
                  is a property of the shipped decoder, not of its test harness"
    )]

    use super::*;
    use crate::fixture::{Encoder, Plan};

    #[test]
    fn reader_reports_truncation_instead_of_panicking() {
        let mut reader = Reader::new(&[0x00]);
        assert_eq!(
            reader.u32(Field::AtomCount),
            Err(Rejection::Truncated {
                field: Field::AtomCount,
                offset: 0,
                needed: 4,
                available: 1,
            })
        );
    }

    #[test]
    fn tokens_reject_empty_oversized_and_non_printable() {
        let mut empty = Encoder::new();
        empty.u16(0);
        assert!(matches!(
            Reader::new(empty.as_slice()).token(Field::Atom),
            Err(Rejection::MalformedToken {
                fault: TokenFault::Empty,
                ..
            })
        ));

        let mut long = Encoder::new();
        long.u16(
            u16::try_from(MAX_TOKEN_BYTES)
                .unwrap_or(u16::MAX)
                .saturating_add(1),
        );
        assert!(matches!(
            Reader::new(long.as_slice()).token(Field::Atom),
            Err(Rejection::MalformedToken {
                fault: TokenFault::TooLong { .. },
                ..
            })
        ));

        // An atom with a space in it is two spellings of one identity waiting to
        // happen (ADR-0013).
        let mut space = Encoder::new();
        space.u16(5);
        space.bytes(b"x < y");
        assert!(matches!(
            Reader::new(space.as_slice()).token(Field::Atom),
            Err(Rejection::MalformedToken {
                fault: TokenFault::NonPrintable { offset: 1, .. },
                ..
            })
        ));
    }

    #[test]
    fn the_green_certificate_decodes_with_its_envelope() {
        let bytes = Plan::ordering_and_congruence().encode();
        let certificate = decode(&bytes).expect("the green refutation decodes");
        assert_eq!(certificate.kind(), CertificateKind::SmtProof);
        assert_eq!(certificate.envelope().schema_epoch(), WIRE_EPOCH);
        // Irrefutable: the family enum has exactly one variant in this epoch.
        let Body::SmtProof(body) = certificate.body();
        assert_eq!(body.atoms().len(), 4);
        assert_eq!(body.assertions().len(), 3);
        assert_eq!(body.lemmas().len(), 2);
        assert_eq!(body.steps().len(), 3);
        assert_eq!(body.atoms().first().map(Token::as_str), Some("f-x-eq-f-y"));
        assert_eq!(
            body.lemmas().first().map(|lemma| lemma.theory().as_str()),
            Some("LIA")
        );
    }

    #[test]
    fn trailing_bytes_are_a_second_encoding_and_are_rejected() {
        let mut bytes = Plan::ordering_and_congruence().encode();
        bytes.push(0x00);
        assert_eq!(
            decode(&bytes),
            Err(DecodeFailure::Rejected(Rejection::TrailingBytes {
                extra: 1
            }))
        );
    }

    #[test]
    fn every_prefix_of_a_valid_certificate_is_rejected_not_accepted() {
        let bytes = Plan::ordering_and_congruence().encode();
        for cut in 0..bytes.len() {
            let prefix = bytes.get(..cut).expect("cut is within the certificate");
            assert!(
                decode(prefix).is_err(),
                "prefix of length {cut} decoded as a certificate"
            );
        }
    }
}
