//! The SAT certificate wire form: serialized bytes in, inert data out (PR 9).
//!
//! # Why a wire form at all
//!
//! > the `continuum-kernel-*` crates form the trusted checking base: no shared
//! > optimized evaluator code with any engine, no async, no unsafe, no plugins or
//! > dynamic loading, a <15,000 non-test-line covenant (docs/03), and a
//! > serialization boundary between every engine and the kernel — a certificate is
//! > checked from its wire form, never from shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! Every public entry point of this crate takes `&[u8]`. There is no constructor for a
//! decoded certificate outside this module, so no solver adapter can hand the checker
//! a structure it built: the only way in is through bytes this decoder accepted.
//!
//! # Why this crate decodes for itself
//!
//! > Independent paths should differ in: language/runtime; state representation;
//! > traversal; arithmetic; parser; certificate decoding; solver.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §8, "Diversity against common-mode
//! >   bugs"
//!
//! The reader below is deliberately *not* `continuum-kernel-core`'s reader, even
//! though the two are shaped alike and read the same eight envelope fields. plan §20's
//! crate table has four kernel crates and no shared one; a decoder bug in a common
//! crate would be a common-mode bug across the whole trusted base, which is the exact
//! failure docs/03 §8 is written against.
//!
//! # The contract
//!
//! The dossier fixes the certificate *family* — RFC 0005's "Kernel layering" assigns
//! this crate the LRAT checker, docs/03 §7 makes a checked LRAT proof the
//! `certificate checked` row of the UNSAT trust table, and
//! `notes/plan/schemas/proof-receipt.schema.json` fixes the receipt spelling `lrat` —
//! but no byte layout. What follows is therefore this crate's own contract, derived
//! from the LRAT literature the dossier cites (docs/13 [S49], the DRAT-trim/LRAT line
//! of work) and versioned by [`WIRE_EPOCH`] so that a later layout is a new epoch
//! rather than a silent reinterpretation (plan §4.6; ADR-0018).
//!
//! ```text
//! certificate  := header envelope body
//!
//! header       := magic:8 wire_epoch:u16 kind:u16
//! magic        := "CONTSATC"
//! wire_epoch   := 1                                  -- WIRE_EPOCH
//! kind         := 1 lrat                             -- CertificateKind::code
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
//! body(1)      := formula proof
//! formula      := variable_count:u32 clause_count:u32 clause*
//! clause       := literal_count:u32 literal*
//! literal      := i32                                -- nonzero, |l| <= variable_count
//! proof        := step_count:u32 step*
//! step         := step_kind:u16 payload
//! step_kind    := 1 add | 2 delete                   -- other codes are Unsupported
//! add          := id:u32 clause hint_count:u32 hint*
//! hint         := u32                                -- an antecedent clause id
//! delete       := id_count:u32 id*
//! ```
//!
//! Integers are big-endian; `i32` is two's complement. Nothing nests, so the format
//! has no recursion for RFC 0005's "cycle/recursion bounds" to bound and the decoder
//! cannot overflow a stack.
//!
//! # Clause identifiers
//!
//! The `i`-th input clause has identifier `i`, counting from one. An addition step
//! declares its own identifier, which must exceed every identifier already in use.
//! That is LRAT's own convention, and it does three things at once: it makes an
//! identifier's meaning independent of how far decoding has got, it forbids a producer
//! from reusing an identifier to change what an earlier antecedent referred to, and it
//! makes the clause store a strictly ascending table that [`crate::check`] can binary
//! search instead of indexing a producer-supplied map it would have to trust.
//!
//! # Canonicity
//!
//! > size limits; streaming validation; deterministic resource accounting;
//! > cycle/recursion bounds; hash-agility policy; rejection of duplicate/ambiguous
//! > encodings.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Resource
//! >   bounds"
//!
//! Three sequences are *sets* and must be strictly ascending: the domain-pack digests,
//! the literals of a clause, and the identifiers of a deletion step. Literals are
//! ordered by variable, and within a variable the negative literal precedes the
//! positive one, so `[1, 2, -3]` is canonical and `[-3, 1, 2]` is not. Strictness
//! forbids duplicates and makes each encoding unique.
//!
//! Three sequences are *sequences*, whose index carries meaning, and are therefore not
//! sorted: the clauses of the formula (their positions are their identifiers), the
//! proof steps (their order is the derivation), and the antecedents of a chain (their
//! order is the propagation order). Permuting one of those produces a different
//! artifact, not a second encoding of the same one.
//!
//! Trailing bytes are rejected. Every count has a declared maximum, running totals are
//! enforced across clauses and chains, and the byte string itself has
//! [`MAX_CERTIFICATE_BYTES`] (docs/16 PO-KER-002).
//!
//! # Malformed input is data, not an event
//!
//! docs/12 §11 lists "malformed artifact panic in kernel" as a release-blocker class
//! and docs/16 PO-KER-001 states decoder totality as a proof obligation. Accordingly
//! this module contains no indexing, no slicing, no `unwrap`, no `expect`, and no
//! wire-derived `Vec::with_capacity` — a declared count never reserves memory before
//! its bytes have been read. Every read goes through [`Reader`] and returns a typed
//! [`Rejection`].

use crate::verdict::{CertificateKind, Feature, Field, Rejection, TokenFault};

/// The eight bytes every SAT certificate begins with.
///
/// Distinct from every sibling kernel crate's magic on purpose: a closure certificate
/// handed to the SAT checker is [`Rejection::BadMagic`] at byte zero, not a confusing
/// failure deep inside a body decoder.
pub const MAGIC: [u8; 8] = *b"CONTSATC";

/// The wire epoch this build implements.
///
/// A certificate declaring any other epoch is [`Feature::WireEpoch`], never a
/// rejection: the artifact may be valid under a contract this build does not know.
pub const WIRE_EPOCH: u16 = 1;

/// The largest certificate byte string the kernel will look at (64 MiB).
pub const MAX_CERTIFICATE_BYTES: usize = 1 << 26;

/// The longest admissible token (digest, producer identity).
pub const MAX_TOKEN_BYTES: usize = 128;

/// The largest number of domain-pack profile digests an envelope may carry.
pub const MAX_DOMAIN_PACKS: u16 = 64;

/// The largest declared variable count.
pub const MAX_VARIABLES: u32 = 1 << 20;

/// The largest number of input clauses a formula may carry.
pub const MAX_CLAUSES: u32 = 1 << 20;

/// The largest number of literals in one clause.
pub const MAX_CLAUSE_LITERALS: u32 = 1 << 16;

/// The largest number of literals a certificate may carry in total, across the formula
/// and every derived clause.
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
///
/// Every accessor names the [`Field`] it is reading, so a truncation reports where the
/// bytes ran out instead of unwinding. The reader never seeks backwards, which is what
/// makes "streaming validation" (RFC 0005, "Resource bounds") a property of the format
/// rather than an aspiration.
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

/// A non-empty printable-ASCII byte string: a digest, an epoch, or an identity.
///
/// Ordered by bytes, which is the order the canonicity rules are stated in.
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

/// The claim envelope a certificate is valid *relative to* (RFC 0005).
///
/// > A certificate is valid only relative to: model hash, CIR semantics version,
/// > property hash, domain-pack profile hashes, bounds, assumptions, engine version,
/// > certificate schema version. The kernel checks this envelope before content.
/// >
/// > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Claim envelope"
///
/// The eight lines above are these eight fields, in that order. "Checks this envelope"
/// is bounded by what a self-contained artifact makes checkable: the kernel checks
/// that every field is present and well-shaped, that the digest list is canonical, and
/// that [`Self::schema_epoch`] agrees with the header. Whether `model_digest` is a
/// digest *of the caller's model* is not a property of these bytes; it is bound by the
/// receipt (RFC 0024, ADR-0035) and reported by
/// [`crate::CheckedClaim::trusted_components`].
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
// clauses
// ---------------------------------------------------------------------------

/// The canonical order on literals: by variable, negative polarity first.
///
/// Total, and independent of the two's-complement encoding — sorting literals as raw
/// `i32` would interleave the variables (`-3 < 1 < 2`) and make a clause's canonical
/// form unreadable next to its DIMACS spelling.
#[must_use]
const fn literal_key(literal: i32) -> (u32, bool) {
    (literal.unsigned_abs(), literal > 0)
}

/// One clause: a set of literals, strictly ascending in the canonical literal order.
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
    ///
    /// Only possible across variables that appear twice, which the canonical order
    /// admits (`-1` immediately precedes `1`). Such a clause is a tautology; see
    /// [`Rejection::VacuousDerivation`].
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
        variables: u32,
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
            if literal.unsigned_abs() > variables {
                return Err(Rejection::LiteralOutOfRange {
                    clause: id,
                    position,
                    literal,
                    variables,
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

/// The CNF formula whose unsatisfiability the certificate refutes.
///
/// Clause identifiers are positional: the `i`-th clause has identifier `i`, counting
/// from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formula {
    variables: u32,
    clauses: Vec<Clause>,
}

impl Formula {
    /// The declared number of propositional variables.
    ///
    /// An upper bound the decoder enforces on every literal of every clause, including
    /// the clauses derived by the proof.
    #[must_use]
    pub const fn variables(&self) -> u32 {
        self.variables
    }

    /// The input clauses, in identifier order.
    #[must_use]
    pub fn clauses(&self) -> &[Clause] {
        &self.clauses
    }

    fn decode(reader: &mut Reader<'_>, total_literals: &mut u64) -> Result<Self, Rejection> {
        let variables = reader.counted_u32(Field::VariableCount, 1, MAX_VARIABLES)?;
        let count = reader.counted_u32(Field::ClauseCount, 1, MAX_CLAUSES)?;
        let mut clauses: Vec<Clause> = Vec::new();
        for index in 0..count {
            let id = index.saturating_add(1);
            clauses.push(Clause::decode(reader, id, variables, total_literals)?);
        }
        Ok(Self { variables, clauses })
    }
}

// ---------------------------------------------------------------------------
// proof steps
// ---------------------------------------------------------------------------

/// A clause addition justified by a propagation chain (LRAT's RUP step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addition {
    id: u32,
    clause: Clause,
    hints: Vec<u32>,
}

impl Addition {
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
    ///
    /// Not a set: the order is the derivation, so it is neither sorted nor deduplicated
    /// by the format.
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
    Add(Addition),
    /// Delete clauses that later steps will not need.
    Delete(Deletion),
}

/// The decoded body of an LRAT certificate: a formula and its refutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LratBody {
    formula: Formula,
    steps: Vec<Step>,
}

impl LratBody {
    /// The formula the certificate claims is unsatisfiable.
    #[must_use]
    pub const fn formula(&self) -> &Formula {
        &self.formula
    }

    /// The refutation, in derivation order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeFailure> {
        let mut total_literals: u64 = 0;
        let formula = Formula::decode(reader, &mut total_literals)?;
        let variables = formula.variables();
        let mut largest_id = u32::try_from(formula.clauses().len()).unwrap_or(u32::MAX);

        let count = reader.counted_u32(Field::StepCount, 1, MAX_STEPS)?;
        let mut steps: Vec<Step> = Vec::new();
        let mut total_hints: u64 = 0;
        for index in 0..count {
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
                    let clause = Clause::decode(reader, id, variables, &mut total_literals)?;
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
                    steps.push(Step::Add(Addition { id, clause, hints }));
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
        Ok(Self { formula, steps })
    }
}

/// A decoded certificate: the envelope plus one family body.
///
/// Inert data. It carries no behaviour, no reference to a producer, and no way to be
/// built except by [`decode`], which is what makes "checked from wire form, never from
/// shared memory" a property of the type system rather than a convention.
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
            Body::Lrat(_) => CertificateKind::Lrat,
        }
    }
}

/// The family-specific half of a decoded certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A clausal refutation.
    Lrat(LratBody),
}

/// Decode a certificate from its wire form.
///
/// The whole input must be one certificate: leftover bytes are
/// [`Rejection::TrailingBytes`], because a tolerated suffix is a second encoding of the
/// same claim (RFC 0005, "Resource bounds").
///
/// # Errors
///
/// [`DecodeFailure::Rejected`] when the bytes are not a valid certificate;
/// [`DecodeFailure::Unsupported`] when they are well-formed enough to name a wire
/// epoch, family, or proof-step kind this build does not implement. The two are kept
/// apart all the way down because collapsing them is exactly the ambiguity INV-008
/// forbids; [`crate::check_certificate`] folds them into the three-arm
/// [`crate::Verdict`].
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
        CertificateKind::Lrat => Body::Lrat(LratBody::decode(&mut reader)?),
    };

    if !reader.is_empty() {
        return Err(DecodeFailure::Rejected(Rejection::TrailingBytes {
            extra: reader.remaining(),
        }));
    }
    Ok(Certificate { envelope, body })
}

/// Why [`decode`] did not produce a certificate.
///
/// Two arms, not one: "these bytes are not a certificate" and "these bytes are a
/// certificate of a contract I do not implement" are different facts, and a decoder
/// that returned only the first would let a caller report a valid artifact as invalid
/// (INV-008).
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
        let outcome = reader.u32(Field::ClauseCount);
        assert_eq!(
            outcome,
            Err(Rejection::Truncated {
                field: Field::ClauseCount,
                offset: 0,
                needed: 4,
                available: 1,
            })
        );
    }

    #[test]
    fn reader_take_is_bounds_checked_at_the_edge() {
        let mut reader = Reader::new(b"ab");
        assert_eq!(reader.take(Field::Magic, 2).ok(), Some(b"ab".as_slice()));
        assert!(reader.is_empty());
        assert!(reader.take(Field::Magic, 1).is_err());
        assert!(reader.take(Field::Magic, usize::MAX).is_err());
    }

    #[test]
    fn tokens_reject_empty_oversized_and_non_printable() {
        let mut empty = Encoder::new();
        empty.u16(0);
        assert!(matches!(
            Reader::new(empty.as_slice()).token(Field::Producer),
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
            Reader::new(long.as_slice()).token(Field::Producer),
            Err(Rejection::MalformedToken {
                fault: TokenFault::TooLong { .. },
                ..
            })
        ));

        let mut space = Encoder::new();
        space.u16(3);
        space.bytes(b"a b");
        assert!(matches!(
            Reader::new(space.as_slice()).token(Field::Producer),
            Err(Rejection::MalformedToken {
                fault: TokenFault::NonPrintable {
                    offset: 1,
                    byte: b' '
                },
                ..
            })
        ));
    }

    #[test]
    fn the_canonical_literal_order_is_by_variable_then_polarity() {
        assert!(literal_key(-1) < literal_key(1));
        assert!(literal_key(1) < literal_key(-2));
        assert!(literal_key(-2) < literal_key(2));
        // Raw i32 order would put -3 first; the canonical order does not.
        assert!(literal_key(1) < literal_key(-3));
    }

    #[test]
    fn a_tautological_clause_is_recognised() {
        let both = Clause {
            literals: vec![-1, 1, 2],
        };
        assert!(both.is_tautology());
        let neither = Clause {
            literals: vec![-1, 2, -3],
        };
        assert!(!neither.is_tautology());
    }

    #[test]
    fn the_green_certificate_decodes_with_its_envelope() {
        let bytes = Plan::three_variable_refutation().encode();
        let certificate = decode(&bytes).expect("the green refutation decodes");
        assert_eq!(certificate.kind(), CertificateKind::Lrat);
        assert_eq!(certificate.envelope().schema_epoch(), WIRE_EPOCH);
        assert_eq!(
            certificate.envelope().model_digest().as_str(),
            "blake3:three-variable-cnf"
        );
        // Irrefutable: the family enum has exactly one variant in this epoch.
        let Body::Lrat(body) = certificate.body();
        assert_eq!(body.formula().variables(), 3);
        assert_eq!(body.formula().clauses().len(), 8);
        assert_eq!(body.steps().len(), 8);
    }

    #[test]
    fn trailing_bytes_are_a_second_encoding_and_are_rejected() {
        let mut bytes = Plan::three_variable_refutation().encode();
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
        // Truncation is the most likely corruption of a stored artifact; no prefix
        // may decode, and none may panic.
        let bytes = Plan::three_variable_refutation().encode();
        for cut in 0..bytes.len() {
            let prefix = bytes.get(..cut).expect("cut is within the certificate");
            assert!(
                decode(prefix).is_err(),
                "prefix of length {cut} decoded as a certificate"
            );
        }
    }
}
