//! The certificate wire form: serialized bytes in, inert data out (PR 9, IMPL-04).
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
//! Every public entry point of this crate takes `&[u8]`. There is no constructor for
//! a decoded certificate outside this module, so no engine can hand the checker a
//! structure it built: the only way in is through bytes this decoder accepted. That
//! is the mechanical half of INV-004 ("no self-certification"); the other half —
//! that the checker cannot even link an engine — is
//! `tools/check_crate_boundaries.py`.
//!
//! # Why this crate decodes for itself
//!
//! > Independent paths should differ in: language/runtime; state representation;
//! > traversal; arithmetic; parser; certificate decoding; solver.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §8, "Diversity against common-mode
//! >   bugs"
//!
//! The decoder below shares no code with `continuum-value`'s canonical encoding, and
//! this crate depends on nothing — no workspace crate and no external crate. A codec
//! bug shared by producer and checker is invisible to both; RFC 0005's "Independence"
//! section asks for exactly this separation, and it costs one small reader.
//!
//! # The contract
//!
//! The dossier fixes the *contents* of a closed-finite-state-space certificate
//! (docs/03 §6.1) and its claim envelope (RFC 0005, "Claim envelope"), and
//! `notes/plan/schemas/proof-receipt.schema.json` references the artifact only as
//! `{"kind": "closed-set", "hash", "path"}` — the byte layout is left open. What
//! follows is therefore this crate's own contract, derived field by field from those
//! sections, and versioned by [`WIRE_EPOCH`] so that a later layout is a new epoch
//! rather than a silent reinterpretation (plan §4.6; ADR-0018).
//!
//! ```text
//! certificate  := header envelope body
//!
//! header       := magic:8 wire_epoch:u16 kind:u16
//! magic        := "CONTCERT"
//! wire_epoch   := 1 | 2                              -- LEGACY_WIRE_EPOCH, WIRE_EPOCH
//! kind         := 1 finite-closure | 2 state-type    -- CertificateKind::code
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
//! domain       := variable_count:u16 variable*
//! variable     := name:token lo:i64 hi:i64
//!
//! table        := state_count:u32 state*
//! state        := i64 * variable_count
//!
//! -- wire epoch 1 (model-free; decoded for the ADR-0018 two-epoch window)
//! body(2)      := domain table
//! body(1)      := domain table
//!                 property_class:u16
//!                 initial_count:u32 state*
//!                 action_count:u16 token*
//!                 row * state_count
//! row          := transition_count:u32 transition*
//! transition   := action:u16 state
//!
//! -- wire epoch 2 (model-bound; bn-35y4f, RFC 0005 correction 1)
//! body(1)      := model_len:u32 model                -- continuum-model/1, see crate::model
//!                 property
//!                 table                              -- arity = the model's variable count
//!                 row2 * state_count
//! property     := 1                                  -- state-domain
//!               | 2 predicate:token                  -- invariant: a predicate of the model
//! row2         := transition_count:u32 transition2*
//! transition2  := action:u16 target:u32              -- indices, each range-checked
//! ```
//!
//! Epoch 2 defines only the finite-closure family; kind 2 at epoch 2 is
//! [`Feature::CertificateKind`]. Its state domain, initial states and action names
//! are the carried model's, not separate sections, so they cannot disagree with it.
//! Its transitions name their targets by table index: the closure obligation no
//! longer needs a carried vector to be located, because the kernel re-derives every
//! successor from the model and locates *that* in the table (`crate::check`).
//!
//! Integers are big-endian; `i64` is two's complement. Outside the epoch-2 model
//! section nothing nests. Inside it, expressions recurse, and both the decoder and
//! the evaluator bound that recursion at [`MAX_EXPRESSION_DEPTH`] frames, so neither
//! can overflow a stack (RFC 0005 "cycle/recursion bounds").
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
//! Five sequences — domain-pack digests, variable names, state vectors, initial
//! state vectors, action names, and the transitions within one successor row — must
//! be *strictly* ascending in byte/lexicographic order. Strictness is doing three
//! jobs at once: it forbids duplicates, it makes the encoding of a given certificate
//! unique, and it makes a state's index in the table a total function of the table's
//! contents, which is what lets [`StateTable::position`] be a binary search rather
//! than a scan. Trailing bytes are rejected for the same reason. Every count has a
//! declared maximum, and the byte string itself has [`MAX_CERTIFICATE_BYTES`]. The
//! epoch-2 model section's outcome and predicate counts are bounded by its bytes
//! ([`MAX_MODEL_BYTES`]) and its node arena by [`MAX_EXPRESSION_NODES`]: no count
//! reserves memory before its bytes are read.
//!
//! # Malformed input is data, not an event
//!
//! docs/12 §11 lists "malformed artifact panic in kernel" as a release-blocker
//! class. Accordingly this module contains no indexing, no slicing, no `unwrap`, no
//! `expect`, and no wire-derived `Vec::with_capacity` (a declared count is never
//! allowed to reserve memory before its bytes have been read). Every read goes
//! through [`Reader`] and returns a typed [`Rejection`]; the crate-level lints in
//! `lib.rs` make a regression a compile error rather than a review question.

use crate::model::Model;
use crate::verdict::{
    CertificateKind, Feature, Field, PropertyClass, Rejection, Resource, TokenFault,
};

/// The eight bytes every certificate begins with.
pub const MAGIC: [u8; 8] = *b"CONTCERT";

/// The newest wire epoch this build implements, and the one producers write: the
/// model-bound epoch (bn-35y4f).
///
/// A certificate declaring an epoch other than this one or [`LEGACY_WIRE_EPOCH`] is
/// [`Feature::WireEpoch`], never a rejection: the artifact may be valid under a
/// contract this build does not know.
pub const WIRE_EPOCH: u16 = 2;

/// The model-free wire epoch, still decoded in the ADR-0018 two-epoch window.
///
/// A claim checked from it keeps `certificate-model-correspondence` among its
/// trusted components, so the assurance difference is visible in every verdict.
pub const LEGACY_WIRE_EPOCH: u16 = 1;

/// The largest model section a wire-epoch-2 certificate may carry (16 MiB).
///
/// A larger model is [`Feature::ResourceBound`]: a checker limit, not a fault.
pub const MAX_MODEL_BYTES: u32 = 1 << 24;

/// The deepest expression the carried model may nest, counting a leaf at the root as
/// depth 1. The model layer's own bound (`continuum-model-core` `MAX_EXPR_DEPTH`).
pub const MAX_EXPRESSION_DEPTH: usize = 32;

/// The most expression nodes the carried model may hold in total.
pub const MAX_EXPRESSION_NODES: u32 = 1 << 20;

/// The most evaluation work one wire-epoch-2 check may perform, in the units of
/// `crate::check`'s precharge: one per expression node, one per state component
/// copied or compared, and a binary search per successor lookup.
///
/// The work is computed from the decoded sizes and charged *before* the first
/// evaluation; a certificate over the bound is [`Feature::ResourceBound`].
///
/// Measured (release build, bn-35y4f): a certificate just under the bound checks in
/// about 15 s; the durable register's claim-A certificate checks in about 0.22 s.
pub const MAX_EVALUATION_WORK: u64 = 1 << 33;

/// The largest certificate byte string the kernel will look at (64 MiB).
pub const MAX_CERTIFICATE_BYTES: usize = 1 << 26;

/// The longest admissible token (digest, variable name, action name).
pub const MAX_TOKEN_BYTES: usize = 128;

/// The largest number of domain-pack profile digests an envelope may carry.
pub const MAX_DOMAIN_PACKS: u16 = 64;

/// The largest number of state variables a declared state domain may have.
pub const MAX_VARIABLES: u16 = 64;

/// The largest number of states a certificate's canonical table may carry.
pub const MAX_STATES: u32 = 1 << 20;

/// The largest number of distinct actions a finite-closure certificate may declare.
pub const MAX_ACTIONS: u16 = 4096;

/// The largest number of transitions a finite-closure certificate may carry in
/// total, across every successor row.
pub const MAX_TRANSITIONS: u64 = 1 << 22;

// ---------------------------------------------------------------------------
// reader
// ---------------------------------------------------------------------------

/// A bounds-checked, forward-only reader over certificate bytes.
///
/// Every accessor names the [`Field`] it is reading, so a truncation reports where
/// the bytes ran out instead of unwinding. The reader never seeks backwards, which
/// is what makes "streaming validation" (RFC 0005, "Resource bounds") a property of
/// the format rather than an aspiration.
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

    /// Read one big-endian two's-complement `i64`.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than eight bytes remain.
    pub fn i64(&mut self, field: Field) -> Result<i64, Rejection> {
        let bytes = self.take(field, 8)?;
        let mut buf = [0_u8; 8];
        buf.copy_from_slice(bytes);
        Ok(i64::from_be_bytes(buf))
    }

    /// Read one length-prefixed token and validate its shape.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] on a short read, or [`Rejection::MalformedToken`]
    /// when the token is empty, too long, or contains a byte outside printable
    /// ASCII.
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
        Token::from_bytes(field, bytes)
    }

    /// Read one big-endian `u64` (the model section's count width).
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than eight bytes remain.
    pub(crate) fn u64(&mut self, field: Field) -> Result<u64, Rejection> {
        let bytes = self.take(field, 8)?;
        let mut buf = [0_u8; 8];
        buf.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(buf))
    }

    /// How many bytes have been consumed.
    pub(crate) const fn offset(&self) -> usize {
        self.offset
    }

    /// Read a count and check it against the format's declared range.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] on a short read, or [`Rejection::CountOutOfRange`]
    /// when the declared count is outside `min..=max`.
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

/// A non-empty printable-ASCII byte string: a digest, an epoch, or a name.
///
/// Ordered by bytes, which is the order the canonicity rules are stated in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(Vec<u8>);

impl Token {
    /// Validate `bytes` as a token's contents: printable ASCII only. The length rules
    /// are the caller's, because the certificate and the model section prefix a
    /// token with lengths of different widths.
    pub(crate) fn from_bytes(field: Field, bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.is_empty() {
            return Err(Rejection::MalformedToken {
                field,
                fault: TokenFault::Empty,
            });
        }
        if bytes.len() > MAX_TOKEN_BYTES {
            return Err(Rejection::MalformedToken {
                field,
                fault: TokenFault::TooLong { found: bytes.len() },
            });
        }
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
        Ok(Self(bytes.to_vec()))
    }

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
/// > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Claim
/// >   envelope"
///
/// The eight lines above are these eight fields, in that order. "Checks this
/// envelope" is bounded by what a self-contained artifact makes checkable: the
/// kernel checks that every field is present and well-shaped, that the digest list
/// is canonical, and that [`Self::schema_epoch`] agrees with the header. Whether
/// `model_digest` is a digest *of the caller's model* is not a property of these
/// bytes; it is bound by the receipt (RFC 0024, ADR-0035) and reported by
/// `CheckedClaim::trusted_components`.
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

    /// Identity of the engine build that produced the certificate.
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
// state domain and state table
// ---------------------------------------------------------------------------

/// One declared state variable and the inclusive integer range it may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    name: Token,
    lo: i64,
    hi: i64,
}

impl Variable {
    /// A declared variable. Crate-internal: only a decoder builds one.
    pub(crate) const fn new(name: Token, lo: i64, hi: i64) -> Self {
        Self { name, lo, hi }
    }

    /// The variable's name.
    #[must_use]
    pub const fn name(&self) -> &Token {
        &self.name
    }

    /// The smallest value the declared domain admits.
    #[must_use]
    pub const fn lo(&self) -> i64 {
        self.lo
    }

    /// The largest value the declared domain admits.
    #[must_use]
    pub const fn hi(&self) -> i64 {
        self.hi
    }

    /// Whether `value` lies in the declared inclusive range.
    #[must_use]
    pub const fn admits(&self, value: i64) -> bool {
        self.lo <= value && value <= self.hi
    }
}

/// The declared state domain: an ordered, named tuple of integer ranges.
///
/// This is docs/03 §6.1's "invariant evaluation data or values sufficient to
/// recompute it" in the one shape the Phase A corpus needs — the DieHard port's
/// `invariant TypeOK { big in 0..5 && small in 0..3 }`
/// (`notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`) is exactly two
/// variables with two ranges. Variable names are strictly ascending, which fixes the
/// meaning of a state vector's positions without a separate index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDomain {
    variables: Vec<Variable>,
}

impl StateDomain {
    /// The declared variables, in canonical (strictly ascending name) order.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// How many components every state vector has.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.variables.len()
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Rejection> {
        let count = reader.counted_u16(Field::VariableCount, 1, MAX_VARIABLES)?;
        let mut variables: Vec<Variable> = Vec::new();
        for index in 0..count {
            let name = reader.token(Field::VariableName)?;
            if let Some(previous) = variables.last()
                && previous.name >= name
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::VariableName,
                    index: u32::from(index),
                });
            }
            let lo = reader.i64(Field::VariableRange)?;
            let hi = reader.i64(Field::VariableRange)?;
            if lo > hi {
                return Err(Rejection::InvertedVariableRange { variable: index });
            }
            variables.push(Variable { name, lo, hi });
        }
        Ok(Self { variables })
    }
}

/// The canonical state table: strictly ascending state vectors of fixed arity.
///
/// docs/03 §6.1 asks for a "canonical state table"; canonical here means the table
/// *is* its own index. A state's identity is its position, duplicates are
/// unrepresentable, and membership — the operation the closure obligation runs at
/// every transition — is a binary search over the vectors themselves rather than a
/// lookup into a producer-supplied map the kernel would have to trust.
///
/// Stored flat: `arity` consecutive `i64`s per state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTable {
    arity: usize,
    flat: Vec<i64>,
    len: u32,
}

impl StateTable {
    /// How many states the table carries.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.len
    }

    /// Whether the table is empty. Never true for a decoded certificate: the format
    /// requires at least one state.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many components each state vector has.
    #[must_use]
    pub const fn arity(&self) -> usize {
        self.arity
    }

    /// The state vector at `index`, or `None` when the index is out of range.
    #[must_use]
    pub fn state(&self, index: u32) -> Option<&[i64]> {
        let start = (index as usize).checked_mul(self.arity)?;
        let end = start.checked_add(self.arity)?;
        self.flat.get(start..end)
    }

    /// The position of `state` in the table, or `None` when it is absent.
    ///
    /// A binary search over a strictly ascending table; the ordering is a decode-time
    /// invariant, so this is a total function of the table's bytes.
    #[must_use]
    pub fn position(&self, state: &[i64]) -> Option<u32> {
        if state.len() != self.arity {
            return None;
        }
        let mut low: u32 = 0;
        let mut high: u32 = self.len;
        while low < high {
            let mid = low.wrapping_add(high.wrapping_sub(low).wrapping_div(2));
            let candidate = self.state(mid)?;
            match candidate.cmp(state) {
                core::cmp::Ordering::Less => low = mid.saturating_add(1),
                core::cmp::Ordering::Greater => high = mid,
                core::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    fn decode(reader: &mut Reader<'_>, arity: usize) -> Result<Self, Rejection> {
        let count = reader.counted_u32(Field::StateCount, 1, MAX_STATES)?;
        let mut flat: Vec<i64> = Vec::new();
        let mut previous_start: Option<usize> = None;
        for index in 0..count {
            let start = flat.len();
            for _ in 0..arity {
                flat.push(reader.i64(Field::StateVector)?);
            }
            if let Some(previous) = previous_start {
                let ordered = match (flat.get(previous..start), flat.get(start..)) {
                    (Some(before), Some(here)) => before < here,
                    _ => false,
                };
                if !ordered {
                    return Err(Rejection::NotStrictlyAscending {
                        field: Field::StateVector,
                        index,
                    });
                }
            }
            previous_start = Some(start);
        }
        Ok(Self {
            arity,
            flat,
            len: count,
        })
    }
}

// ---------------------------------------------------------------------------
// certificate bodies
// ---------------------------------------------------------------------------

/// One labelled transition of a finite-closure certificate.
///
/// The target is carried as a *state vector*, not as an index into the state table.
/// That choice is the whole point of the closure obligation: an index is necessarily
/// in range and would make `Post(S) ⊆ S` vacuous, whereas a vector must be located
/// in the table and a certificate that stopped exploring early cannot hide it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    action: u16,
    target: Vec<i64>,
}

impl Transition {
    /// Index of this transition's action in the certificate's action table.
    #[must_use]
    pub const fn action(&self) -> u16 {
        self.action
    }

    /// The successor state vector.
    #[must_use]
    pub fn target(&self) -> &[i64] {
        &self.target
    }
}

/// The decoded body of a finite-closure certificate (RFC 0005 "Closed reachable
/// set"; docs/03 §6.1).
///
/// Self-contained by construction: it carries the state domain that is its own
/// safety property, the canonical state table, the initial states, the action names,
/// and one successor row per table state. A row with no transitions is the "disabled
/// witness" of docs/03 §6.1 — an explicitly declared deadlock rather than a silent
/// omission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteClosureBody {
    domain: StateDomain,
    table: StateTable,
    property_class: u16,
    initial_states: Vec<Vec<i64>>,
    actions: Vec<Token>,
    rows: Vec<Vec<Transition>>,
}

impl FiniteClosureBody {
    /// The declared state domain.
    #[must_use]
    pub const fn domain(&self) -> &StateDomain {
        &self.domain
    }

    /// The canonical state table.
    #[must_use]
    pub const fn table(&self) -> &StateTable {
        &self.table
    }

    /// The declared property-class code, as written on the wire.
    #[must_use]
    pub const fn property_class_code(&self) -> u16 {
        self.property_class
    }

    /// The declared initial state vectors, strictly ascending.
    #[must_use]
    pub fn initial_states(&self) -> &[Vec<i64>] {
        &self.initial_states
    }

    /// The declared action names, strictly ascending.
    #[must_use]
    pub fn actions(&self) -> &[Token] {
        &self.actions
    }

    /// The successor rows, one per state table entry, in table order.
    #[must_use]
    pub fn rows(&self) -> &[Vec<Transition>] {
        &self.rows
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Rejection> {
        let domain = StateDomain::decode(reader)?;
        let arity = domain.arity();
        let table = StateTable::decode(reader, arity)?;
        let property_class = reader.u16(Field::PropertyClassCode)?;

        let initial_count = reader.counted_u32(Field::InitialCount, 0, MAX_STATES)?;
        if initial_count == 0 {
            return Err(Rejection::NoInitialStates);
        }
        let mut initial_states: Vec<Vec<i64>> = Vec::new();
        for index in 0..initial_count {
            let mut state: Vec<i64> = Vec::new();
            for _ in 0..arity {
                state.push(reader.i64(Field::InitialState)?);
            }
            if let Some(previous) = initial_states.last()
                && *previous >= state
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::InitialState,
                    index,
                });
            }
            initial_states.push(state);
        }

        let action_count = reader.counted_u16(Field::ActionCount, 1, MAX_ACTIONS)?;
        let mut actions: Vec<Token> = Vec::new();
        for index in 0..action_count {
            let name = reader.token(Field::ActionName)?;
            if let Some(previous) = actions.last()
                && *previous >= name
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ActionName,
                    index: u32::from(index),
                });
            }
            actions.push(name);
        }

        let mut rows: Vec<Vec<Transition>> = Vec::new();
        let mut total: u64 = 0;
        for _ in 0..table.len() {
            let count = reader.counted_u32(Field::TransitionCount, 0, MAX_STATES)?;
            total = total.saturating_add(u64::from(count));
            if total > MAX_TRANSITIONS {
                return Err(Rejection::CountOutOfRange {
                    field: Field::TransitionCount,
                    found: total,
                    min: 0,
                    max: MAX_TRANSITIONS,
                });
            }
            let mut row: Vec<Transition> = Vec::new();
            for index in 0..count {
                let action = reader.u16(Field::TransitionAction)?;
                let mut target: Vec<i64> = Vec::new();
                for _ in 0..arity {
                    target.push(reader.i64(Field::TransitionTarget)?);
                }
                let transition = Transition { action, target };
                if let Some(previous) = row.last()
                    && (previous.action, previous.target.as_slice())
                        >= (transition.action, transition.target.as_slice())
                {
                    return Err(Rejection::NotStrictlyAscending {
                        field: Field::TransitionAction,
                        index,
                    });
                }
                row.push(transition);
            }
            rows.push(row);
        }

        Ok(Self {
            domain,
            table,
            property_class,
            initial_states,
            actions,
            rows,
        })
    }
}

/// The decoded body of a state-type certificate.
///
/// The typing half of docs/23's "finite closure safety for DieHard `TypeOK`"
/// milestone and of docs/16 PO-MOD-003 ("every enabled action produces a state in
/// the declared state domain"), carrying no transition relation: it claims only that
/// every state in its table is well-typed. Composed with a finite-closure
/// certificate over the same table, that is the `TypeOK`-for-all-reachable-states
/// claim of RFC 0012's first accepted milestone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTypeBody {
    domain: StateDomain,
    table: StateTable,
}

impl StateTypeBody {
    /// The declared state domain.
    #[must_use]
    pub const fn domain(&self) -> &StateDomain {
        &self.domain
    }

    /// The canonical state table.
    #[must_use]
    pub const fn table(&self) -> &StateTable {
        &self.table
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Rejection> {
        let domain = StateDomain::decode(reader)?;
        let arity = domain.arity();
        let table = StateTable::decode(reader, arity)?;
        Ok(Self { domain, table })
    }
}

/// The decoded body of a wire-epoch-2 finite-closure certificate: the model it is
/// about, the property, the canonical state table, and one successor row per table
/// state with targets named by table index.
///
/// Every index was range-checked at decode: an action index against the model's
/// actions and a target index against the table. Each row is strictly ascending by
/// `(action, target)`, which, because the table is strictly ascending, is the order
/// of `(action, target vector)` a wire-epoch-1 row is written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelClosureBody {
    model: Model,
    property: PropertyClass,
    predicate: Option<usize>,
    table: StateTable,
    entries: Vec<(u16, u32)>,
    ends: Vec<u32>,
}

impl ModelClosureBody {
    /// The canonical state table.
    #[must_use]
    pub const fn table(&self) -> &StateTable {
        &self.table
    }

    /// The property class the certificate declares.
    #[must_use]
    pub const fn property(&self) -> PropertyClass {
        self.property
    }

    /// The carried model's canonical encoding (`continuum-model/1`), byte for byte.
    ///
    /// This is the model's identity (ADR-0013). A caller binds the certificate to
    /// its own model by comparing these bytes with that model's identity.
    #[must_use]
    pub fn model_identity(&self) -> &[u8] {
        self.model.identity()
    }

    /// How many transitions the rows carry in total.
    #[must_use]
    pub fn transition_count(&self) -> u64 {
        u64::try_from(self.entries.len()).unwrap_or(u64::MAX)
    }

    /// The invariant's predicate name, for [`PropertyClass::Invariant`].
    #[must_use]
    pub fn invariant(&self) -> Option<&Token> {
        self.predicate
            .and_then(|index| self.model.predicate_name(index))
    }

    pub(crate) const fn model(&self) -> &Model {
        &self.model
    }

    pub(crate) const fn predicate(&self) -> Option<usize> {
        self.predicate
    }

    /// The carried successor row of table state `index`: `(action, target)` pairs,
    /// strictly ascending, every index range-checked at decode. `None` past the table.
    #[must_use]
    pub fn row(&self, index: u32) -> Option<&[(u16, u32)]> {
        let position = usize::try_from(index).ok()?;
        let end = usize::try_from(*self.ends.get(position)?).ok()?;
        let start = match position.checked_sub(1) {
            Some(previous) => usize::try_from(*self.ends.get(previous)?).ok()?,
            None => 0,
        };
        self.entries.get(start..end)
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeFailure> {
        let len = reader.u32(Field::ModelLength)?;
        let section = reader.take(
            Field::ModelLength,
            usize::try_from(len).unwrap_or(usize::MAX),
        )?;
        if len > MAX_MODEL_BYTES {
            return Err(DecodeFailure::Unsupported(Feature::ResourceBound {
                resource: Resource::ModelBytes,
                needed: u64::from(len),
                limit: u64::from(MAX_MODEL_BYTES),
            }));
        }
        let model = crate::model::decode(section)?;

        let class_code = reader.u16(Field::PropertyClassCode)?;
        let (property, predicate) = match PropertyClass::from_code(class_code) {
            Some(PropertyClass::StateDomain) => (PropertyClass::StateDomain, None),
            Some(PropertyClass::Invariant) => {
                let name = reader.token(Field::PropertyPredicate)?;
                let index = model
                    .predicate_index(&name)
                    .ok_or(Rejection::UnresolvedName {
                        field: Field::PropertyPredicate,
                    })?;
                (PropertyClass::Invariant, Some(index))
            }
            None => {
                return Err(DecodeFailure::Unsupported(Feature::PropertyClass {
                    found: class_code,
                }));
            }
        };

        let table = StateTable::decode(reader, model.variables().len())?;
        let action_count = model.action_count();
        let mut entries: Vec<(u16, u32)> = Vec::new();
        let mut ends: Vec<u32> = Vec::new();
        let mut total: u64 = 0;
        for state in 0..table.len() {
            let count = reader.counted_u32(Field::TransitionCount, 0, MAX_STATES)?;
            total = total.saturating_add(u64::from(count));
            if total > MAX_TRANSITIONS {
                return Err(Rejection::CountOutOfRange {
                    field: Field::TransitionCount,
                    found: total,
                    min: 0,
                    max: MAX_TRANSITIONS,
                }
                .into());
            }
            let mut previous: Option<(u16, u32)> = None;
            for entry in 0..count {
                let action = reader.u16(Field::TransitionAction)?;
                let target = reader.u32(Field::TransitionTarget)?;
                if usize::from(action) >= action_count {
                    return Err(Rejection::UnknownAction {
                        state,
                        entry,
                        action,
                    }
                    .into());
                }
                if target >= table.len() {
                    return Err(Rejection::TargetOutOfRange {
                        state,
                        entry,
                        target,
                    }
                    .into());
                }
                if previous.is_some_and(|before| before >= (action, target)) {
                    return Err(Rejection::NotStrictlyAscending {
                        field: Field::TransitionAction,
                        index: entry,
                    }
                    .into());
                }
                previous = Some((action, target));
                entries.push((action, target));
            }
            ends.push(u32::try_from(entries.len()).unwrap_or(u32::MAX));
        }

        Ok(Self {
            model,
            property,
            predicate,
            table,
            entries,
            ends,
        })
    }
}

/// A decoded certificate: the envelope plus one family body.
///
/// Inert data. It carries no behaviour, no reference to a producer, and no way to be
/// built except by [`decode`], which is what makes "checked from wire form, never
/// from shared memory" a property of the type system rather than a convention.
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
            Body::FiniteClosure(_) | Body::ModelClosure(_) => CertificateKind::FiniteClosure,
            Body::StateType(_) => CertificateKind::StateType,
        }
    }
}

/// The family-specific half of a decoded certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A wire-epoch-1 closed-reachable-set certificate, carrying its own relation.
    FiniteClosure(FiniteClosureBody),
    /// A wire-epoch-2 closed-reachable-set certificate, carrying its model.
    ModelClosure(ModelClosureBody),
    /// A state-typing certificate.
    StateType(StateTypeBody),
}

/// Decode a certificate from its wire form.
///
/// The whole input must be one certificate: leftover bytes are
/// [`Rejection::TrailingBytes`], because a tolerated suffix is a second encoding of
/// the same claim (RFC 0005, "Resource bounds").
///
/// # Errors
///
/// [`DecodeFailure::Rejected`] when the bytes are not a valid certificate;
/// [`DecodeFailure::Unsupported`] when they are well-formed enough to name a wire
/// epoch or family this build does not implement. The two are kept apart all the way
/// down because collapsing them is exactly the ambiguity INV-008 forbids;
/// [`crate::check_certificate`] folds them into the three-arm [`crate::Verdict`].
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
    if epoch != WIRE_EPOCH && epoch != LEGACY_WIRE_EPOCH {
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

    if epoch == WIRE_EPOCH && kind != CertificateKind::FiniteClosure {
        return Err(DecodeFailure::Unsupported(Feature::CertificateKind {
            found: kind_code,
        }));
    }

    let envelope = Envelope::decode(&mut reader, epoch)?;
    let body = match kind {
        CertificateKind::FiniteClosure if epoch == WIRE_EPOCH => {
            Body::ModelClosure(ModelClosureBody::decode(&mut reader)?)
        }
        CertificateKind::FiniteClosure => {
            Body::FiniteClosure(FiniteClosureBody::decode(&mut reader)?)
        }
        CertificateKind::StateType => Body::StateType(StateTypeBody::decode(&mut reader)?),
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
/// that returned only the first would let a caller report a valid artifact as
/// invalid (INV-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeFailure {
    /// The bytes are not a well-formed, internally consistent certificate.
    Rejected(Rejection),
    /// The bytes name a wire epoch or certificate family this build does not
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
    use crate::fixture::{Encoder, closure_bytes, type_bytes};

    #[test]
    fn reader_reports_truncation_instead_of_panicking() {
        let mut reader = Reader::new(&[0x00]);
        let outcome = reader.u32(Field::StateCount);
        assert_eq!(
            outcome,
            Err(Rejection::Truncated {
                field: Field::StateCount,
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
    fn state_table_position_is_a_total_binary_search() {
        let bytes = type_bytes();
        let certificate = decode(&bytes).expect("green state-type certificate decodes");
        let Body::StateType(body) = certificate.body() else {
            panic!("state-type certificate decoded as another family");
        };
        let table = body.table();
        for index in 0..table.len() {
            let state = table.state(index).expect("in-range state is present");
            assert_eq!(table.position(state), Some(index));
        }
        assert_eq!(table.state(table.len()), None);
        assert_eq!(table.position(&[i64::MIN, i64::MIN]), None);
        assert_eq!(table.position(&[0]), None, "arity mismatch is not a member");
    }

    #[test]
    fn green_certificates_decode_with_their_envelopes() {
        let bytes = closure_bytes();
        let certificate = decode(&bytes).expect("green finite-closure certificate decodes");
        assert_eq!(certificate.kind(), CertificateKind::FiniteClosure);
        assert_eq!(certificate.envelope().schema_epoch(), LEGACY_WIRE_EPOCH);
        assert_eq!(
            certificate.envelope().model_digest().as_str(),
            "blake3:diehard-model"
        );
        assert_eq!(certificate.envelope().domain_pack_digests().len(), 0);

        let Body::FiniteClosure(body) = certificate.body() else {
            panic!("finite-closure certificate decoded as another family");
        };
        assert_eq!(body.domain().arity(), 2);
        assert_eq!(body.table().len(), 16);
        assert_eq!(body.rows().len(), 16);
        assert_eq!(body.actions().len(), 6);
        assert_eq!(body.initial_states().len(), 1);
        assert_eq!(body.property_class_code(), 1);
    }

    #[test]
    fn trailing_bytes_are_a_second_encoding_and_are_rejected() {
        let mut bytes = type_bytes();
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
        let bytes = closure_bytes();
        for cut in 0..bytes.len() {
            let prefix = bytes.get(..cut).expect("cut is within the certificate");
            assert!(
                decode(prefix).is_err(),
                "prefix of length {cut} decoded as a certificate"
            );
        }
    }
}
