//! The temporal certificate wire form: serialized bytes in, inert data out (PR 9).
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
//! decoded certificate outside this module, so `continuum-engine-liveness` cannot hand
//! the checker a graph it built: the only way in is through bytes this decoder accepted.
//!
//! # The contract
//!
//! The dossier fixes the certificate *contents* — docs/03 §6.5 lists, for the graph
//! shape, "SCC decomposition; reachable-component witnesses; fairness acceptance
//! labels; absence of accepting SCCs", and for the ranking shape "well-founded domain;
//! decrease obligations; fairness-to-progress linkage; safety side conditions" — and
//! `notes/plan/schemas/proof-receipt.schema.json` fixes the receipt spellings `ranking`
//! and `fair-scc-exclusion`. It fixes no byte layout. What follows is therefore this
//! crate's own contract, derived field by field from those lists and versioned by
//! [`WIRE_EPOCH`] (plan §4.6; ADR-0018).
//!
//! ```text
//! certificate  := header envelope body
//!
//! header       := magic:8 wire_epoch:u16 kind:u16
//! magic        := "CONTTMPC"
//! wire_epoch   := 1                                  -- WIRE_EPOCH
//! kind         := 1 ranking | 2 fair-scc-exclusion   -- CertificateKind::code
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
//! body         := domain table property_class:u16 goal initial actions fairness
//!                 rows tail
//! domain       := variable_count:u16 variable*
//! variable     := name:token lo:i64 hi:i64
//! table        := state_count:u32 state*
//! state        := i64 * variable_count
//! goal         := goal_count:u32 state*
//! initial      := initial_count:u32 state*
//! actions      := action_count:u16 token*
//! fairness     := fairness_class:u16 fair_count:u16 action_index:u16*
//! rows         := row * state_count
//! row          := transition_count:u32 transition*
//! transition   := action:u16 state
//! tail(1)      := rank:u64 * state_count             -- ranking only
//! tail(2)      :=                                    -- fair-scc-exclusion
//! ```
//!
//! Integers are big-endian; `i64` is two's complement. Nothing nests, so the format has
//! no recursion for RFC 0005's "cycle/recursion bounds" to bound and the decoder cannot
//! overflow a stack. The *graph* the body describes does have cycles — that is the
//! point — and [`crate::check`] walks it with an explicit heap stack for the same
//! reason.
//!
//! # Vectors, not indices
//!
//! Every state that is not a table entry — an initial state, a goal state, a
//! transition target — is carried as a state *vector* and must be located in the table
//! by the checker. An index would necessarily be in range, which would make closure
//! vacuous and let a producer point a transition anywhere. This is
//! `continuum-kernel-core`'s choice for its finite-closure certificate, made again here
//! for the same reason and re-implemented rather than shared (docs/03 §8).
//!
//! # An action absent from a row is disabled, not undeclared
//!
//! A row lists exactly the transitions enabled at its state. An action that appears in
//! the action table but not in a given row is *disabled* at that state, and that is a
//! load-bearing fact rather than an omission: weak fairness is discharged either by
//! taking an action or by disabling it somewhere on the cycle, so a producer that
//! omitted a genuinely enabled action would be claiming a fairness obligation is
//! discharged when it is not. The action table is therefore the alphabet, and each row
//! is total with respect to it.
//!
//! # Canonicity
//!
//! > … rejection of duplicate/ambiguous encodings.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Resource bounds"
//!
//! Seven sequences must be strictly ascending: the domain-pack digests, the variable
//! names, the state vectors, the goal vectors, the initial vectors, the action names,
//! the fair-action indices, and the transitions within one successor row (ordered by
//! action, then target vector). Strictness forbids duplicates, makes each encoding
//! unique, and makes a state's index a total function of the table's contents — which
//! is what lets [`StateTable::position`] be a binary search rather than a scan.
//!
//! The successor rows themselves are a *sequence*: row `i` is the successor row of
//! table state `i`, so their order is determined and not free.
//!
//! Trailing bytes are rejected. Every count has a declared maximum and the byte string
//! itself has [`MAX_CERTIFICATE_BYTES`] (docs/16 PO-KER-002).
//!
//! # Malformed input is data, not an event
//!
//! No indexing, no slicing, no `unwrap`, no `expect`, and no wire-derived
//! `Vec::with_capacity`. Every read goes through [`Reader`] and returns a typed
//! [`Rejection`] (docs/12 §11; docs/16 PO-KER-001).

use crate::verdict::{CertificateKind, Feature, Field, Rejection, TokenFault};

/// The eight bytes every temporal certificate begins with.
pub const MAGIC: [u8; 8] = *b"CONTTMPC";

/// The wire epoch this build implements.
pub const WIRE_EPOCH: u16 = 1;

/// The largest certificate byte string the kernel will look at (64 MiB).
pub const MAX_CERTIFICATE_BYTES: usize = 1 << 26;

/// The longest admissible token (digest, variable name, action name).
pub const MAX_TOKEN_BYTES: usize = 128;

/// The largest number of domain-pack profile digests an envelope may carry.
pub const MAX_DOMAIN_PACKS: u16 = 64;

/// The largest number of state variables a declared state domain may have.
pub const MAX_VARIABLES: u16 = 64;

/// The largest number of states a certificate's canonical table may carry.
///
/// Smaller than `continuum-kernel-core`'s bound, and deliberately: the component search
/// keeps several per-state arrays and one adjacency list live at once, so this is the
/// bound at which the checker's own working set stays proportional to something a
/// reviewer can reason about (RFC 0005 "deterministic resource accounting").
pub const MAX_STATES: u32 = 1 << 16;

/// The largest number of distinct actions a certificate may declare.
pub const MAX_ACTIONS: u16 = 4096;

/// The largest number of transitions a certificate may carry in total, across every
/// successor row.
pub const MAX_TRANSITIONS: u64 = 1 << 20;

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

    /// Read one big-endian `u64`.
    ///
    /// # Errors
    ///
    /// [`Rejection::Truncated`] when fewer than eight bytes remain.
    pub fn u64(&mut self, field: Field) -> Result<u64, Rejection> {
        let bytes = self.take(field, 8)?;
        let mut buf = [0_u8; 8];
        buf.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(buf))
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

    /// Read one state vector of `arity` components.
    fn state(&mut self, field: Field, arity: usize) -> Result<Vec<i64>, Rejection> {
        let mut state: Vec<i64> = Vec::new();
        for _ in 0..arity {
            state.push(self.i64(field)?);
        }
        Ok(state)
    }

    /// Read a strictly ascending sequence of state vectors.
    fn state_set(
        &mut self,
        count_field: Field,
        state_field: Field,
        arity: usize,
        min: u32,
    ) -> Result<Vec<Vec<i64>>, Rejection> {
        let count = self.counted_u32(count_field, min, MAX_STATES)?;
        let mut states: Vec<Vec<i64>> = Vec::new();
        for index in 0..count {
            let state = self.state(state_field, arity)?;
            if let Some(previous) = states.last()
                && *previous >= state
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: state_field,
                    index,
                });
            }
            states.push(state);
        }
        Ok(states)
    }
}

// ---------------------------------------------------------------------------
// tokens and the envelope
// ---------------------------------------------------------------------------

/// A non-empty printable-ASCII byte string: a digest, an epoch, or a name.
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
    /// construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("")
    }
}

/// The claim envelope a certificate is valid *relative to* (RFC 0005 "Claim
/// envelope"): model hash, CIR semantics version, property hash, domain-pack profile
/// hashes, bounds, assumptions, engine version, certificate schema version.
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
    ///
    /// For a fair-cycle-exclusion certificate this is where the model's fairness
    /// assumptions live: the declared fair-action set is checked against the action
    /// table, but that it *is* the model's assumption set is the producer's obligation
    /// (docs/16 PO-LIV-001).
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
/// The table *is* its own index. A state's identity is its position, duplicates are
/// unrepresentable, and membership — the operation the closure obligation runs at every
/// transition — is a binary search over the vectors themselves rather than a lookup
/// into a producer-supplied map the kernel would have to trust.
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

    /// Whether the table is empty. Never true for a decoded certificate.
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
// the body
// ---------------------------------------------------------------------------

/// One labelled transition.
///
/// The target is carried as a *state vector*, not as an index into the state table:
/// an index is necessarily in range and would make closure vacuous.
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

/// The decoded body of a temporal certificate.
///
/// One shape for both families: they differ only in whether a ranking follows, and in
/// which obligations [`crate::check`] then runs over the same carried graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalBody {
    domain: StateDomain,
    table: StateTable,
    property_class: u16,
    goal_states: Vec<Vec<i64>>,
    initial_states: Vec<Vec<i64>>,
    actions: Vec<Token>,
    fairness_class: u16,
    fair_actions: Vec<u16>,
    rows: Vec<Vec<Transition>>,
    ranks: Vec<u64>,
}

impl TemporalBody {
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

    /// The goal set, as strictly ascending state vectors.
    #[must_use]
    pub fn goal_states(&self) -> &[Vec<i64>] {
        &self.goal_states
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

    /// The declared fairness-class code, as written on the wire.
    #[must_use]
    pub const fn fairness_class_code(&self) -> u16 {
        self.fairness_class
    }

    /// The action indices declared weakly fair, strictly ascending.
    #[must_use]
    pub fn fair_actions(&self) -> &[u16] {
        &self.fair_actions
    }

    /// The successor rows, one per state table entry, in table order.
    #[must_use]
    pub fn rows(&self) -> &[Vec<Transition>] {
        &self.rows
    }

    /// The ranking, one entry per state table entry — empty for a family that carries
    /// none.
    #[must_use]
    pub fn ranks(&self) -> &[u64] {
        &self.ranks
    }

    fn decode(reader: &mut Reader<'_>, kind: CertificateKind) -> Result<Self, Rejection> {
        let domain = StateDomain::decode(reader)?;
        let arity = domain.arity();
        let table = StateTable::decode(reader, arity)?;
        let property_class = reader.u16(Field::PropertyClassCode)?;

        let goal_states = reader.state_set(Field::GoalCount, Field::GoalState, arity, 0)?;
        let initial_states =
            reader.state_set(Field::InitialCount, Field::InitialState, arity, 0)?;
        if initial_states.is_empty() {
            return Err(Rejection::NoInitialStates);
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

        let fairness_class = reader.u16(Field::FairnessClassCode)?;
        let fair_count = reader.counted_u16(Field::FairActionCount, 0, MAX_ACTIONS)?;
        let mut fair_actions: Vec<u16> = Vec::new();
        for index in 0..fair_count {
            let action = reader.u16(Field::FairAction)?;
            if let Some(previous) = fair_actions.last()
                && *previous >= action
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::FairAction,
                    index: u32::from(index),
                });
            }
            fair_actions.push(action);
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
                let target = reader.state(Field::TransitionTarget, arity)?;
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

        let mut ranks: Vec<u64> = Vec::new();
        if matches!(kind, CertificateKind::Ranking) {
            for _ in 0..table.len() {
                ranks.push(reader.u64(Field::Rank)?);
            }
        }

        Ok(Self {
            domain,
            table,
            property_class,
            goal_states,
            initial_states,
            actions,
            fairness_class,
            fair_actions,
            rows,
            ranks,
        })
    }
}

/// A decoded certificate: the envelope, the family, and one body.
///
/// Inert data, with no constructor outside [`decode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    envelope: Envelope,
    kind: CertificateKind,
    body: TemporalBody,
}

impl Certificate {
    /// The claim envelope (RFC 0005).
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// The family this certificate belongs to.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        self.kind
    }

    /// The body.
    #[must_use]
    pub const fn body(&self) -> &TemporalBody {
        &self.body
    }
}

/// Decode a certificate from its wire form.
///
/// # Errors
///
/// [`DecodeFailure::Rejected`] when the bytes are not a valid certificate;
/// [`DecodeFailure::Unsupported`] when they name a wire epoch or family this build does
/// not implement (INV-008).
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
    let body = TemporalBody::decode(&mut reader, kind)?;

    if !reader.is_empty() {
        return Err(DecodeFailure::Rejected(Rejection::TrailingBytes {
            extra: reader.remaining(),
        }));
    }
    Ok(Certificate {
        envelope,
        kind,
        body,
    })
}

/// Why [`decode`] did not produce a certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeFailure {
    /// The bytes are not a well-formed, internally consistent certificate.
    Rejected(Rejection),
    /// The bytes name a wire epoch or family this build does not implement.
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
            reader.u32(Field::StateCount),
            Err(Rejection::Truncated {
                field: Field::StateCount,
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
            Reader::new(empty.as_slice()).token(Field::ActionName),
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
            Reader::new(long.as_slice()).token(Field::ActionName),
            Err(Rejection::MalformedToken {
                fault: TokenFault::TooLong { .. },
                ..
            })
        ));

        let mut space = Encoder::new();
        space.u16(3);
        space.bytes(b"a b");
        assert!(matches!(
            Reader::new(space.as_slice()).token(Field::ActionName),
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
        let bytes = Plan::drain_ranking().encode();
        let certificate = decode(&bytes).expect("the green ranking certificate decodes");
        let table = certificate.body().table();
        for index in 0..table.len() {
            let state = table.state(index).expect("in-range state is present");
            assert_eq!(table.position(state), Some(index));
        }
        assert_eq!(table.state(table.len()), None);
        assert_eq!(table.position(&[i64::MIN, i64::MIN]), None);
        assert_eq!(table.position(&[0]), None, "arity mismatch is not a member");
    }

    #[test]
    fn the_green_certificates_decode_with_their_envelopes() {
        let ranking = decode(&Plan::drain_ranking().encode()).expect("ranking decodes");
        assert_eq!(ranking.kind(), CertificateKind::Ranking);
        assert_eq!(ranking.envelope().schema_epoch(), WIRE_EPOCH);
        assert_eq!(ranking.body().table().len(), 9);
        assert_eq!(ranking.body().ranks().len(), 9);
        assert_eq!(ranking.body().goal_states().len(), 3);
        assert!(ranking.body().fair_actions().is_empty());

        let exclusion =
            decode(&Plan::fair_progress_exclusion().encode()).expect("exclusion decodes");
        assert_eq!(exclusion.kind(), CertificateKind::FairSccExclusion);
        assert_eq!(exclusion.body().table().len(), 2);
        assert!(
            exclusion.body().ranks().is_empty(),
            "an exclusion certificate carries no ranking"
        );
        assert_eq!(exclusion.body().fair_actions(), [0]);
        assert_eq!(exclusion.body().actions().len(), 3);
    }

    #[test]
    fn trailing_bytes_are_a_second_encoding_and_are_rejected() {
        let mut bytes = Plan::drain_ranking().encode();
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
        for bytes in [
            Plan::drain_ranking().encode(),
            Plan::fair_progress_exclusion().encode(),
        ] {
            for cut in 0..bytes.len() {
                let prefix = bytes.get(..cut).expect("cut is within the certificate");
                assert!(
                    decode(prefix).is_err(),
                    "prefix of length {cut} decoded as a certificate"
                );
            }
        }
    }
}
