//! Exact finite values, their canonical encoding, and their total order (PR 2).
//!
//! # What this module is for
//!
//! > Implement:
//! >
//! > - exact finite values from Revision 2;
//! > - canonical encoding and total order;
//! > - content identity per ADR-0013 […]
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 2
//!
//! This module delivers the first two bullets: the value domain, the canonical
//! encoding, and the order. Computing a content *address* over
//! [`Value::encode`] bytes, publishing an artifact atomically, and the
//! collision-injection tests are the remaining PR 2 slices and deliberately do
//! not appear here — see "Seams left open on purpose" below.
//!
//! Everything else in Continuum that has to agree on what a state *is* agrees
//! here. Plan §20's dependency rules put the consequence bluntly:
//!
//! > in certified lanes, content identity is exact canonical identity; hash
//! > collisions are resolved by canonical comparison; 256-bit-hash identity is
//! > permitted only in explicitly labeled non-certified modes (ADR-0013).
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! and `notes/plan/docs/33_REVISION_3_ARCHITECTURE.md` lists the "canonical
//! value decoder" first among the components that belong inside the smallest
//! trust base. [`Value::decode`] is therefore written as a trust-base
//! component: it rejects every non-canonical spelling of a value rather than
//! repairing it, it bounds its own recursion, and it never allocates on a
//! length an attacker declared.
//!
//! # The value domain
//!
//! The domain is Revision 2's value system, narrowed to the values that are
//! *exact and finite*:
//!
//! > The core supports:
//! >
//! > - booleans, bounded and mathematical integers/naturals;
//! > - uninterpreted finite atoms/model values;
//! > - finite sets and symbolic set predicates;
//! > - tuples, records, variants, options, sequences;
//! > - total finite functions and maps;
//! > - relations and graphs; […]
//! >
//! > — `notes/plan/archive/plan-revision-2.md` §5.3, "Value system"
//!
//! Prose alone does not fix a domain (INV-003, "Schemas decide, prose does
//! not"), and one schema already enumerates the kinds normatively:
//! `notes/plan/schemas/cir.schema.json` `$defs.value` admits the bare JSON
//! scalars `null`, `boolean`, `integer`, `string` and the tagged kinds
//!
//! ```text
//! nat  int  bitvec  bytes  tuple  record  variant
//! set  map  seq  multiset  symbol  opaque
//! ```
//!
//! [`ValueKind`] is exactly that list — thirteen tagged kinds plus the three
//! bare scalars that the tagged list does not repeat — and
//! [`ValueKind::as_str`] returns the schema's own spelling. RFC 0001 requires
//! the correspondence to hold in the other direction too: a CIR label's
//! "payloads MUST use the Continuum value algebra, not arbitrary serde blobs".
//!
//! Three Revision 2 bullets are absent, and each absence is a decision:
//!
//! - **Options** are a [`ValueKind::Variant`]; the schema's kind list has no
//!   separate `option`, and giving `none` two spellings would give one value
//!   two canonical encodings.
//! - **Relations and graphs** are sets of tuples. They are a modelling idiom
//!   over [`ValueKind::Set`], not a constructor.
//! - **Symbolic set predicates, bounded quantification, comprehensions, and
//!   higher-order operators** are not values at all. They are expressions in
//!   the `Symbolic`/`Theorem` fragments (ADR-0025) and belong to the model
//!   language, not to the exact finite value algebra.
//!
//! # No floating point
//!
//! > Floating-point literals are excluded from v1 so the canonical encoding
//! > stays byte-deterministic.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Node set"
//!
//! RFC 0001 leaves the door ajar ("canonical NaNs if floats are enabled"), but
//! nothing in Revision 3 enables them, and a float carries a partial order —
//! which would make [`Value`]'s [`Ord`] a lie. There is no float constructor,
//! and adding one is a semantic-epoch event (plan §4.6), not an addition.
//!
//! # Canonical encoding (CVNF-1)
//!
//! `CVNF-1` — Continuum Value Normal Form, version 1 — is this module's name
//! for the byte encoding, deliberately parallel to RFC 0037's `CPNF-1` for
//! property ASTs. Its identifier is [`ENCODING_ID`]. RFC 0001 fixes what
//! canonicalization has to cover:
//!
//! > Canonicalization includes sorted maps/sets, normalized integer
//! > representation, canonical NaNs if floats are enabled, normalized paths,
//! > and stable symbol interning.
//! >
//! > — `notes/plan/rfcs/0001-causal-intermediate-representation.md`,
//! >   "Serialization"
//!
//! Two primitives carry the whole encoding.
//!
//! - `nat(n)` — one length byte `k`, then the `k`-byte big-endian magnitude of
//!   `n` with no leading zero byte. `k = 0` encodes zero. The length byte comes
//!   first, so `nat` is *order preserving*: `a < b` if and only if
//!   `nat(a) < nat(b)` lexicographically.
//! - `int(n)` — `0x01 ++ nat(n)` when `n >= 0`; `0x00 ++ !nat(-n)` when
//!   `n < 0`, where `!` is the bytewise complement. Negatives sort before
//!   non-negatives on the sign byte, and complementing reverses the magnitude
//!   order so that a larger magnitude encodes to smaller bytes. `int` is order
//!   preserving over all of ℤ.
//!
//! `nat` also carries every length and multiplicity, so a collection is not
//! limited to 255 elements: the *length byte* bounds the magnitude at 255
//! bytes, which is 2040 bits of headroom above today's `u128` carrier.
//!
//! Each value is a one-byte kind tag ([`ValueKind::tag`], assigned in
//! declaration order) followed by a self-delimiting payload:
//!
//! ```text
//! 0x01 null       (no payload)
//! 0x02 boolean    0x00 | 0x01
//! 0x03 nat        nat(n)
//! 0x04 int        int(n)
//! 0x05 bitvec     nat(width) ++ nat(bits)
//! 0x06 bytes      nat(len) ++ len bytes
//! 0x07 string     nat(len) ++ len UTF-8 bytes
//! 0x08 symbol     nat(len) ++ len name bytes
//! 0x09 tuple      nat(n) ++ n values
//! 0x0a seq        nat(n) ++ n values
//! 0x0b set        nat(n) ++ n values, strictly ascending
//! 0x0c multiset   nat(n) ++ n (value, nat(count>=1)), values strictly ascending
//! 0x0d record     nat(n) ++ n (name, value), names strictly ascending
//! 0x0e variant    name ++ value
//! 0x0f map        nat(n) ++ n (value, value), keys strictly ascending
//! 0x10 opaque     name(domain) ++ nat(len) ++ len bytes
//! ```
//!
//! `0x00` is never a kind tag, so a stray zero byte cannot begin a valid
//! encoding.
//!
//! The encoding is **prefix-free**: every payload declares its own extent, so
//! no encoding is a proper prefix of another. That is what makes the order
//! below decomposable.
//!
//! ## The encoding is primary; the order is derived
//!
//! ADR-0013 does not order values — it makes the canonical encoding the
//! identity:
//!
//! > Canonical structural encodings define identity. Hashes index and
//! > partition; collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`
//!
//! Where the dossier does need an order over values, it takes it from the
//! encoding rather than inventing one:
//!
//! > The `operands` of `and` and `or` MUST be sorted ascending by the byte
//! > ordering of their own N8 encodings […]
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, CPNF-1 rule N4
//!
//! So [`Ord`] for [`Value`] is *defined* as lexicographic order on CVNF-1
//! encodings, and [`Value::cmp`] is the structural computation of that order —
//! it never allocates a byte. `value_order_is_lexicographic_order_on_encodings`
//! pins the two together on every pair of the sample domain; if a future edit
//! changes one and not the other, that test fails rather than the identity
//! silently drifting.
//!
//! One consequence is worth stating plainly, because it surprises people:
//! sequences, strings, and byte strings compare **shortlex** — length first,
//! contents second — so `"z" < "aa"`. That is not the lexicographic order Rust
//! gives `str`, and it is not an accident. A length-prefixed encoding orders by
//! length first, and having [`Ord`] agree with the bytes matters more than
//! matching `str`'s convention: an ordering that disagrees with the encoding is
//! an ordering that can put a `BTreeSet` in a state its own serialization
//! rejects.
//!
//! # Why [`Value`] has no [`Hash`] implementation
//!
//! Deliberate. A [`Hash`] impl is precisely the affordance that lets a caller
//! index states by fingerprint and never compare canonically — the failure
//! ADR-0013 exists to prevent ("A system promising stronger assurance should
//! not depend on collision improbability"). The 256-bit content address that
//! explicitly labeled non-certified lanes may use is computed *from*
//! [`Value::encode`] bytes by PR 2's content-identity slice, over the canonical
//! encoding, never from a `std` hasher over the in-memory layout. Not
//! implementing [`Hash`] also makes `HashMap`-ordered iteration over values
//! unrepresentable, which is the mechanical half of docs/19 §7.
//!
//! # Determinism obligations this module has to meet
//!
//! > Test each retained scenario over: worker counts 1, 2, 8, 32; debug/release;
//! > supported OS/architectures; different hash seeds where internal structures
//! > allow […] Semantic artifacts must be identical or differences must be
//! > explicitly non-semantic and normalized away.
//! >
//! > — `notes/plan/docs/19_TEST_STRATEGY.md` §7, "Determinism matrix"
//!
//! Nothing here reads the clock, the environment, an allocator address, or a
//! hash seed; every ordered container is a `BTreeMap`/`BTreeSet` keyed by the
//! order above; integers are fixed-width with an explicit endianness. The
//! encoding of a value is therefore a function of the value alone, and
//! `golden_vectors_are_pinned` freezes the byte strings so a refactor cannot
//! change an identity without a failing test. Changing a golden vector is an
//! epoch event (plan §4.6), never a fix.
//!
//! # Seams left open on purpose
//!
//! - **Content addressing is not here.** [`Value::encode`] produces the bytes a
//!   digest is taken over; choosing the digest, labeling certified versus
//!   non-certified identity modes, and resolving injected collisions by
//!   canonical comparison are PR 2's ADR-0013 slice.
//! - **Atomic publication and authorization are not here** (INV-017, plan §4.4);
//!   they are PR 2's CAS slice and need a store, which a leaf crate has no
//!   business owning.
//! - **Sorts and types are not here.** [`Value`] is untyped on purpose: it
//!   carries values, not the `Finite[T]`/`Canonical[T]` capabilities of
//!   RFC 0003. Well-sortedness — including the [`Value::Nat`] versus
//!   [`Value::Int`] hazard described on [`Value::Nat`] — is the elaborator's
//!   obligation, discharged before values are compared.
//! - **Definedness is not here.** RFC 0003 requires division by zero, sequence
//!   indexing, and non-enumerable choice to produce proof obligations or
//!   rejection; those are evaluation outcomes, and this module has no
//!   evaluator.
//! - **Interning is not here.** RFC 0001's "stable symbol interning" is a
//!   representation optimization for the CIR; [`Name`] stores its bytes, and an
//!   interning table that changed identity would defeat the point.

use core::cmp::Ordering;
use core::fmt;
use core::num::NonZeroU64;
use std::collections::{BTreeMap, BTreeSet};

/// Identifier of the canonical encoding this module implements.
///
/// Named parallel to RFC 0037's `CPNF-1`. It belongs in the semantic epoch
/// (plan §4.6) once one is published for the value algebra; until then it is
/// the string a golden vector or an artifact header quotes so that "which
/// encoding" is never a guess.
pub const ENCODING_ID: &str = "cvnf-1";

/// The deepest value the encoding admits, counting scalars as depth 1.
///
/// The decoder walks untrusted bytes recursively, and docs/19 §6 requires
/// fuzzing of "canonical encodings" to include "malicious cyclic/oversized
/// artifacts". A declared-nesting bound is what turns a hostile artifact into a
/// typed [`DecodeError::DepthExceeded`] instead of a stack overflow.
///
/// The public constructors enforce the same bound, so the round-trip law holds
/// without qualification: every value built through them encodes to bytes this
/// module's decoder accepts.
///
/// The number is measured, not guessed: on the pinned toolchain a `dev`-profile
/// decode — the profile with the largest stack frames — still succeeded at 256
/// levels on a default 2 MiB thread stack and failed somewhere below 512, so the
/// bound is set a factor of four below the observed floor. Model values nest
/// through records, tuples and sets, not through recursion; anything that would
/// approach this bound is a list, and a list is a [`Value::Seq`].
pub const MAX_DEPTH: usize = 64;

/// The largest bit width [`Value::BitVec`] carries.
///
/// The dossier calls for "bounded bitvectors" (RFC 0003, "Type system") without
/// fixing a width. `u128` is the widest exact integer carrier Rust's standard
/// library offers, and widening past it is a carrier change, not an encoding
/// change: `nat` already admits 2040-bit magnitudes.
pub const MAX_BITVEC_WIDTH: u32 = 128;

const SIGN_NEGATIVE: u8 = 0x00;
const SIGN_NON_NEGATIVE: u8 = 0x01;

// --- names -----------------------------------------------------------------

/// A canonical identifier: a symbol, a record field, a variant tag, or an
/// opaque domain.
///
/// Construction restricts a name to non-empty printable ASCII, for the reason
/// [`crate::epoch::EpochIdentity`] restricts an epoch token: under ADR-0013 an
/// identity has exactly one canonical spelling, and Unicode that normalizes
/// several ways would let two spellings denote one name — and therefore two
/// encodings denote one value. Model values, field names, and constructor tags
/// are drawn from source identifiers, so the restriction costs nothing that
/// Revision 2's "uninterpreted finite atoms/model values" needs.
///
/// Ordering is shortlex over the name's bytes, matching the encoding (see the
/// module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    /// Build a name from its canonical spelling.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::EmptyName`] for the empty string and
    /// [`ValueError::NonCanonicalName`] for the first character outside
    /// printable ASCII.
    pub fn new(name: &str) -> Result<Self, ValueError> {
        if name.is_empty() {
            return Err(ValueError::EmptyName);
        }
        if let Some((index, character)) = name.char_indices().find(|(_, c)| !c.is_ascii_graphic()) {
            return Err(ValueError::NonCanonicalName { index, character });
        }
        Ok(Self(name.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_blobs(self.0.as_bytes(), other.0.as_bytes())
    }
}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// --- bit vectors -----------------------------------------------------------

/// A bounded bitvector: a width in `1..=`[`MAX_BITVEC_WIDTH`] and the bits
/// below it.
///
/// Width is part of the value. A 4-bit `0b0001` and an 8-bit `0b0000_0001` are
/// different values with different encodings, exactly as they are different
/// SMT-LIB sorts; nothing here silently widens or truncates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BitVec {
    width: u32,
    bits: u128,
}

impl BitVec {
    /// Build a bitvector.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::BitVecWidth`] when `width` is `0` or above
    /// [`MAX_BITVEC_WIDTH`], and [`ValueError::BitVecOutOfRange`] when `bits`
    /// has a bit set at or above `width` — that spelling is a second encoding
    /// of a value the width cannot hold, so it is rejected rather than masked.
    pub fn new(width: u32, bits: u128) -> Result<Self, ValueError> {
        if width == 0 || width > MAX_BITVEC_WIDTH {
            return Err(ValueError::BitVecWidth { width });
        }
        if width < MAX_BITVEC_WIDTH && bits >= (1u128 << width) {
            return Err(ValueError::BitVecOutOfRange { width, bits });
        }
        Ok(Self { width, bits })
    }

    /// The declared width in bits.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// The bits, zero-extended into a `u128`.
    #[must_use]
    pub const fn bits(self) -> u128 {
        self.bits
    }
}

impl fmt::Display for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.bits, self.width)
    }
}

// --- opaque values ---------------------------------------------------------

/// A value from an external domain that declares its own canonical form.
///
/// > opaque external domains with declared equality/canonicalization
/// >
/// > — `notes/plan/rfcs/0003-continuum-model-language.md`, "Type system"
///
/// The bytes are the domain's *declared canonical form*; this module compares
/// and orders them but cannot check them, and says so rather than pretending.
/// A domain that hands over two byte strings for one value has broken its own
/// declaration, and the failure surfaces as two identities — which is the
/// visible, diagnosable failure, not a silent merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opaque {
    domain: Name,
    bytes: Vec<u8>,
}

impl Opaque {
    /// Build an opaque value from its domain and that domain's canonical bytes.
    #[must_use]
    pub fn new(domain: Name, bytes: Vec<u8>) -> Self {
        Self { domain, bytes }
    }

    /// The declaring domain.
    #[must_use]
    pub const fn domain(&self) -> &Name {
        &self.domain
    }

    /// The domain's canonical bytes for this value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Ord for Opaque {
    fn cmp(&self, other: &Self) -> Ordering {
        self.domain
            .cmp(&other.domain)
            .then_with(|| compare_blobs(&self.bytes, &other.bytes))
    }
}

impl PartialOrd for Opaque {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// --- kinds -----------------------------------------------------------------

/// The kind of an exact finite value.
///
/// The variants are `notes/plan/schemas/cir.schema.json` `$defs.value`: the
/// thirteen tagged `kind`s, plus the three bare JSON scalars (`null`,
/// `boolean`, `string`) that the tagged list does not repeat. The schema's bare
/// `integer` is [`ValueKind::Int`]; it is one kind with one spelling, not two.
///
/// Declaration order *is* [`ValueKind::tag`] order, and therefore the order in
/// which values of different kinds compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueKind {
    /// The absent scalar (`null`).
    Null,
    /// A boolean.
    Bool,
    /// A natural number.
    Nat,
    /// A mathematical integer.
    Int,
    /// A bounded bitvector.
    BitVec,
    /// An uninterpreted byte string.
    Bytes,
    /// A Unicode text string (`string`).
    Text,
    /// An uninterpreted finite atom (a model value).
    Symbol,
    /// A fixed-arity heterogeneous tuple.
    Tuple,
    /// A finite sequence.
    Seq,
    /// A finite set.
    Set,
    /// A finite multiset.
    Multiset,
    /// A record with named fields.
    Record,
    /// A tagged union alternative.
    Variant,
    /// A total finite function.
    Map,
    /// A value from an external domain.
    Opaque,
}

impl ValueKind {
    /// Every kind, in tag order.
    pub const ALL: [Self; 16] = [
        Self::Null,
        Self::Bool,
        Self::Nat,
        Self::Int,
        Self::BitVec,
        Self::Bytes,
        Self::Text,
        Self::Symbol,
        Self::Tuple,
        Self::Seq,
        Self::Set,
        Self::Multiset,
        Self::Record,
        Self::Variant,
        Self::Map,
        Self::Opaque,
    ];

    /// The schema's spelling of this kind (`cir.schema.json` `$defs.value`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "boolean",
            Self::Nat => "nat",
            Self::Int => "int",
            Self::BitVec => "bitvec",
            Self::Bytes => "bytes",
            Self::Text => "string",
            Self::Symbol => "symbol",
            Self::Tuple => "tuple",
            Self::Seq => "seq",
            Self::Set => "set",
            Self::Multiset => "multiset",
            Self::Record => "record",
            Self::Variant => "variant",
            Self::Map => "map",
            Self::Opaque => "opaque",
        }
    }

    /// The one-byte tag this kind carries in CVNF-1.
    ///
    /// Tags start at `0x01`; `0x00` is reserved so that a zero byte can never
    /// open a valid encoding.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Null => 0x01,
            Self::Bool => 0x02,
            Self::Nat => 0x03,
            Self::Int => 0x04,
            Self::BitVec => 0x05,
            Self::Bytes => 0x06,
            Self::Text => 0x07,
            Self::Symbol => 0x08,
            Self::Tuple => 0x09,
            Self::Seq => 0x0a,
            Self::Set => 0x0b,
            Self::Multiset => 0x0c,
            Self::Record => 0x0d,
            Self::Variant => 0x0e,
            Self::Map => 0x0f,
            Self::Opaque => 0x10,
        }
    }

    /// The kind a tag byte names, or `None` when no kind carries it.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(Self::Null),
            0x02 => Some(Self::Bool),
            0x03 => Some(Self::Nat),
            0x04 => Some(Self::Int),
            0x05 => Some(Self::BitVec),
            0x06 => Some(Self::Bytes),
            0x07 => Some(Self::Text),
            0x08 => Some(Self::Symbol),
            0x09 => Some(Self::Tuple),
            0x0a => Some(Self::Seq),
            0x0b => Some(Self::Set),
            0x0c => Some(Self::Multiset),
            0x0d => Some(Self::Record),
            0x0e => Some(Self::Variant),
            0x0f => Some(Self::Map),
            0x10 => Some(Self::Opaque),
            _ => None,
        }
    }
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- the value algebra -----------------------------------------------------

/// An exact finite value.
///
/// Every invariant that canonicity depends on is carried by the representation
/// rather than by a checked flag: sets are `BTreeSet`s, maps and records and
/// multisets are `BTreeMap`s keyed by the order below, multiplicities are
/// [`NonZeroU64`], names are [`Name`]s, and bitvector widths live inside
/// [`BitVec`]. Constructing a variant directly is therefore canonical by
/// construction; the public constructors add only the [`MAX_DEPTH`] bound and
/// the duplicate-input rejections that a `BTreeMap` would otherwise silently
/// resolve.
///
/// The bound is the one thing the representation cannot carry, so it is stated
/// rather than assumed: [`Value::encode`], [`Value::depth`], [`Value::cmp`] and
/// `Drop` all recurse over the structure, and a value assembled by direct
/// variant construction past [`MAX_DEPTH`] is outside what this module promises.
/// Build through the constructors and the question never arises.
///
/// [`Ord`] is lexicographic order on [`Value::encode`] bytes, computed
/// structurally. [`Hash`] is deliberately not implemented — see the module
/// documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// The absent scalar.
    Null,
    /// A boolean.
    Bool(bool),
    /// A natural number.
    ///
    /// # `Nat` and `Int` denote different values
    ///
    /// `Nat(3)` and `Int(3)` are distinct values with distinct encodings, as
    /// `nat` and `int` are distinct kinds in `cir.schema.json`. Continuum's
    /// model language is typed (RFC 0003), so a state component has one
    /// declared sort and an elaborated model produces one of the two spellings,
    /// never both; the pair is a hazard only for a producer that emits values
    /// without a sort, and such a producer is already outside the fragment.
    /// This module will not coerce between them: silently promoting `Nat(3)`
    /// to `Int(3)` would make identity depend on the coercion direction, which
    /// is exactly the ambiguity ADR-0013 exists to remove.
    Nat(u128),
    /// A mathematical integer.
    ///
    /// The carrier is `i128`. The dossier's integers are mathematical, and the
    /// `int` encoding is already carrier-independent — `Int(3)` encodes to the
    /// same bytes whatever width holds it, and `nat` admits 2040-bit magnitudes
    /// — so widening the carrier later changes no existing identity and
    /// invalidates no golden vector. Until then, [`DecodeError::IntegerTooWide`]
    /// reports an integer this carrier cannot hold as *unsupported*, never as
    /// malformed.
    Int(i128),
    /// A bounded bitvector.
    BitVec(BitVec),
    /// An uninterpreted byte string.
    Bytes(Vec<u8>),
    /// A Unicode text string.
    ///
    /// Rust's `String` is already exact: valid UTF-8, no unpaired surrogates,
    /// one byte sequence per value. No Unicode normalization is applied, and
    /// none should be — two distinct code-point sequences are two distinct
    /// values, which is what "exact" means. (Identifiers are a different
    /// matter; see [`Name`].)
    Text(String),
    /// An uninterpreted finite atom — Revision 2's model value.
    Symbol(Name),
    /// A fixed-arity heterogeneous tuple.
    Tuple(Vec<Value>),
    /// A finite sequence.
    Seq(Vec<Value>),
    /// A finite set.
    Set(BTreeSet<Value>),
    /// A finite multiset, as element/multiplicity entries.
    Multiset(BTreeMap<Value, NonZeroU64>),
    /// A record with named fields.
    Record(BTreeMap<Name, Value>),
    /// A tagged union alternative. `Option` is this kind, not a separate one.
    Variant {
        /// The constructor tag.
        tag: Name,
        /// The alternative's payload.
        payload: Box<Value>,
    },
    /// A total finite function, given extensionally.
    Map(BTreeMap<Value, Value>),
    /// A value from an external domain.
    Opaque(Opaque),
}

impl Value {
    // --- scalar constructors ------------------------------------------------

    /// A natural number.
    #[must_use]
    pub const fn nat(value: u128) -> Self {
        Self::Nat(value)
    }

    /// A mathematical integer.
    ///
    /// Negative naturals are unrepresentable rather than rejected: the carrier
    /// of [`Value::Nat`] is `u128`, so there is no `Nat(-1)` to refuse.
    #[must_use]
    pub const fn int(value: i128) -> Self {
        Self::Int(value)
    }

    /// A bounded bitvector.
    ///
    /// # Errors
    ///
    /// Propagates [`BitVec::new`].
    pub fn bitvec(width: u32, bits: u128) -> Result<Self, ValueError> {
        BitVec::new(width, bits).map(Self::BitVec)
    }

    /// An uninterpreted byte string.
    #[must_use]
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(bytes.into())
    }

    /// A Unicode text string.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// An uninterpreted finite atom.
    #[must_use]
    pub const fn symbol(name: Name) -> Self {
        Self::Symbol(name)
    }

    // --- composite constructors ---------------------------------------------

    /// A tuple, in the given order.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DepthExceeded`] when the result would nest deeper
    /// than [`MAX_DEPTH`].
    pub fn tuple(items: impl IntoIterator<Item = Self>) -> Result<Self, ValueError> {
        let items: Vec<Self> = items.into_iter().collect();
        check_depth(depth_of_slice(&items))?;
        Ok(Self::Tuple(items))
    }

    /// A sequence, in the given order.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DepthExceeded`] when the result would nest deeper
    /// than [`MAX_DEPTH`].
    pub fn seq(items: impl IntoIterator<Item = Self>) -> Result<Self, ValueError> {
        let items: Vec<Self> = items.into_iter().collect();
        check_depth(depth_of_slice(&items))?;
        Ok(Self::Seq(items))
    }

    /// A finite set.
    ///
    /// Repeated elements collapse. That is not a canonicalization convenience
    /// but set theory — `{1, 1}` *is* `{1}` — so accepting the repetition
    /// cannot admit two encodings of one value, and rejecting it would make a
    /// legal union fail.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DepthExceeded`] when the result would nest deeper
    /// than [`MAX_DEPTH`].
    pub fn set(elements: impl IntoIterator<Item = Self>) -> Result<Self, ValueError> {
        let elements: BTreeSet<Self> = elements.into_iter().collect();
        check_depth(depth_of_iter(elements.iter()))?;
        Ok(Self::Set(elements))
    }

    /// A finite multiset, from element/multiplicity entries.
    ///
    /// # Errors
    ///
    /// - [`ValueError::ZeroMultiplicity`] — a zero count is a second spelling
    ///   of absence, so it is rejected rather than dropped.
    /// - [`ValueError::DuplicateElement`] — one element listed twice is an
    ///   ambiguous description, not an arithmetic instruction; the caller says
    ///   which multiplicity it meant.
    /// - [`ValueError::DepthExceeded`].
    pub fn multiset(entries: impl IntoIterator<Item = (Self, u64)>) -> Result<Self, ValueError> {
        let mut multiset: BTreeMap<Self, NonZeroU64> = BTreeMap::new();
        for (element, multiplicity) in entries {
            let Some(multiplicity) = NonZeroU64::new(multiplicity) else {
                return Err(ValueError::ZeroMultiplicity {
                    element: Box::new(element),
                });
            };
            if multiset.contains_key(&element) {
                return Err(ValueError::DuplicateElement {
                    element: Box::new(element),
                });
            }
            multiset.insert(element, multiplicity);
        }
        check_depth(depth_of_iter(multiset.keys()))?;
        Ok(Self::Multiset(multiset))
    }

    /// A record.
    ///
    /// # Errors
    ///
    /// - [`ValueError::DuplicateField`] — a repeated field name would let
    ///   insertion order decide the value, which is the ambient nondeterminism
    ///   INV-005 forbids.
    /// - [`ValueError::DepthExceeded`].
    pub fn record(fields: impl IntoIterator<Item = (Name, Self)>) -> Result<Self, ValueError> {
        let mut record: BTreeMap<Name, Self> = BTreeMap::new();
        for (name, value) in fields {
            if record.contains_key(&name) {
                return Err(ValueError::DuplicateField { name });
            }
            record.insert(name, value);
        }
        check_depth(depth_of_iter(record.values()))?;
        Ok(Self::Record(record))
    }

    /// A tagged union alternative.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DepthExceeded`] when the result would nest deeper
    /// than [`MAX_DEPTH`].
    pub fn variant(tag: Name, payload: Self) -> Result<Self, ValueError> {
        check_depth(payload.depth().saturating_add(1))?;
        Ok(Self::Variant {
            tag,
            payload: Box::new(payload),
        })
    }

    /// A total finite function, given extensionally.
    ///
    /// # Errors
    ///
    /// - [`ValueError::DuplicateKey`] — a repeated key would make the function
    ///   depend on insertion order.
    /// - [`ValueError::DepthExceeded`].
    pub fn map(entries: impl IntoIterator<Item = (Self, Self)>) -> Result<Self, ValueError> {
        let mut map: BTreeMap<Self, Self> = BTreeMap::new();
        for (key, value) in entries {
            if map.contains_key(&key) {
                return Err(ValueError::DuplicateKey { key: Box::new(key) });
            }
            map.insert(key, value);
        }
        let depth = map
            .iter()
            .map(|(key, value)| key.depth().max(value.depth()))
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        check_depth(depth)?;
        Ok(Self::Map(map))
    }

    /// A value from an external domain, in that domain's canonical bytes.
    #[must_use]
    pub fn opaque(domain: Name, bytes: impl Into<Vec<u8>>) -> Self {
        Self::Opaque(Opaque::new(domain, bytes.into()))
    }

    // --- queries -------------------------------------------------------------

    /// This value's kind.
    #[must_use]
    pub const fn kind(&self) -> ValueKind {
        match self {
            Self::Null => ValueKind::Null,
            Self::Bool(_) => ValueKind::Bool,
            Self::Nat(_) => ValueKind::Nat,
            Self::Int(_) => ValueKind::Int,
            Self::BitVec(_) => ValueKind::BitVec,
            Self::Bytes(_) => ValueKind::Bytes,
            Self::Text(_) => ValueKind::Text,
            Self::Symbol(_) => ValueKind::Symbol,
            Self::Tuple(_) => ValueKind::Tuple,
            Self::Seq(_) => ValueKind::Seq,
            Self::Set(_) => ValueKind::Set,
            Self::Multiset(_) => ValueKind::Multiset,
            Self::Record(_) => ValueKind::Record,
            Self::Variant { .. } => ValueKind::Variant,
            Self::Map(_) => ValueKind::Map,
            Self::Opaque(_) => ValueKind::Opaque,
        }
    }

    /// How deeply this value nests; a scalar has depth 1.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::Null
            | Self::Bool(_)
            | Self::Nat(_)
            | Self::Int(_)
            | Self::BitVec(_)
            | Self::Bytes(_)
            | Self::Text(_)
            | Self::Symbol(_)
            | Self::Opaque(_) => 1,
            Self::Tuple(items) | Self::Seq(items) => depth_of_slice(items),
            Self::Set(elements) => depth_of_iter(elements.iter()),
            Self::Multiset(entries) => depth_of_iter(entries.keys()),
            Self::Record(fields) => depth_of_iter(fields.values()),
            Self::Variant { payload, .. } => payload.depth().saturating_add(1),
            Self::Map(entries) => entries
                .iter()
                .map(|(key, value)| key.depth().max(value.depth()))
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        }
    }

    // --- canonical encoding --------------------------------------------------

    /// This value's canonical CVNF-1 encoding.
    ///
    /// The bytes are a function of the value alone: no clock, no environment,
    /// no allocator address, no hash seed, no platform-dependent width or
    /// endianness. This is the byte string a content address is taken over
    /// (ADR-0013) and the byte string [`Ord`] compares.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out);
        out
    }

    /// Append this value's canonical encoding to `out`.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        out.push(self.kind().tag());
        match self {
            Self::Null => {}
            Self::Bool(value) => out.push(u8::from(*value)),
            Self::Nat(value) => push_nat(out, *value),
            Self::Int(value) => push_int(out, *value),
            Self::BitVec(value) => {
                push_nat(out, u128::from(value.width()));
                push_nat(out, value.bits());
            }
            Self::Bytes(bytes) => push_blob(out, bytes),
            Self::Text(text) => push_blob(out, text.as_bytes()),
            Self::Symbol(name) => push_blob(out, name.as_str().as_bytes()),
            Self::Tuple(items) | Self::Seq(items) => {
                push_len(out, items.len());
                for item in items {
                    item.encode_into(out);
                }
            }
            Self::Set(elements) => {
                push_len(out, elements.len());
                for element in elements {
                    element.encode_into(out);
                }
            }
            Self::Multiset(entries) => {
                push_len(out, entries.len());
                for (element, multiplicity) in entries {
                    element.encode_into(out);
                    push_nat(out, u128::from(multiplicity.get()));
                }
            }
            Self::Record(fields) => {
                push_len(out, fields.len());
                for (name, value) in fields {
                    push_blob(out, name.as_str().as_bytes());
                    value.encode_into(out);
                }
            }
            Self::Variant { tag, payload } => {
                push_blob(out, tag.as_str().as_bytes());
                payload.encode_into(out);
            }
            Self::Map(entries) => {
                push_len(out, entries.len());
                for (key, value) in entries {
                    key.encode_into(out);
                    value.encode_into(out);
                }
            }
            Self::Opaque(opaque) => {
                push_blob(out, opaque.domain().as_str().as_bytes());
                push_blob(out, opaque.bytes());
            }
        }
    }

    /// Decode a canonical CVNF-1 encoding.
    ///
    /// Strict by construction: every non-canonical spelling of a value is an
    /// error, never a repair. A leading zero in an integer magnitude, a
    /// negative zero, an out-of-order or repeated set element, map key or
    /// record field, a zero multiplicity, a bitvector with bits above its
    /// width, a non-ASCII name, invalid UTF-8, an unknown tag, and trailing
    /// bytes are all rejected. Two byte strings therefore never decode to one
    /// value, which is what makes the encoding usable as an identity.
    ///
    /// # Errors
    ///
    /// Returns the [`DecodeError`] naming the first violation and its byte
    /// offset.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut cursor = Cursor::new(bytes);
        let value = cursor.value(1)?;
        if cursor.offset < bytes.len() {
            return Err(DecodeError::TrailingBytes {
                offset: cursor.offset,
            });
        }
        Ok(value)
    }
}

// --- the total order -------------------------------------------------------

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let kinds = self.kind().tag().cmp(&other.kind().tag());
        if kinds != Ordering::Equal {
            return kinds;
        }
        match (self, other) {
            (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Nat(left), Self::Nat(right)) => left.cmp(right),
            (Self::Int(left), Self::Int(right)) => left.cmp(right),
            (Self::BitVec(left), Self::BitVec(right)) => left.cmp(right),
            (Self::Bytes(left), Self::Bytes(right)) => compare_blobs(left, right),
            (Self::Text(left), Self::Text(right)) => {
                compare_blobs(left.as_bytes(), right.as_bytes())
            }
            (Self::Symbol(left), Self::Symbol(right)) => left.cmp(right),
            (Self::Tuple(left), Self::Tuple(right)) | (Self::Seq(left), Self::Seq(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().cmp(right.iter())),
            (Self::Set(left), Self::Set(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().cmp(right.iter())),
            (Self::Multiset(left), Self::Multiset(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().cmp(right.iter())),
            (Self::Record(left), Self::Record(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().cmp(right.iter())),
            (
                Self::Variant {
                    tag: left_tag,
                    payload: left_payload,
                },
                Self::Variant {
                    tag: right_tag,
                    payload: right_payload,
                },
            ) => left_tag
                .cmp(right_tag)
                .then_with(|| left_payload.cmp(right_payload)),
            (Self::Map(left), Self::Map(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().cmp(right.iter())),
            (Self::Opaque(left), Self::Opaque(right)) => left.cmp(right),
            // Unreachable: equal tags imply equal variants, and every pairing
            // is listed above. Returning `Equal` here would be a silent
            // identity merge, so the arm is a panic rather than a default.
            _ => unreachable!("equal kind tags imply the same variant"),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Shortlex over bytes: length first, contents second — the order a
/// length-prefixed encoding induces.
fn compare_blobs(left: &[u8], right: &[u8]) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

// --- depth helpers ---------------------------------------------------------

fn depth_of_slice(items: &[Value]) -> usize {
    depth_of_iter(items.iter())
}

fn depth_of_iter<'a>(items: impl Iterator<Item = &'a Value>) -> usize {
    items.map(Value::depth).max().unwrap_or(0).saturating_add(1)
}

fn check_depth(depth: usize) -> Result<(), ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::DepthExceeded {
            depth,
            max: MAX_DEPTH,
        });
    }
    Ok(())
}

// --- encoding primitives ---------------------------------------------------

fn push_nat(out: &mut Vec<u8>, value: u128) {
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(16);
    let magnitude = &bytes[start..];
    // A `u128` magnitude is at most 16 bytes, so the length byte cannot
    // truncate.
    out.push(u8::try_from(magnitude.len()).expect("a u128 magnitude is at most 16 bytes"));
    out.extend_from_slice(magnitude);
}

fn push_int(out: &mut Vec<u8>, value: i128) {
    if value >= 0 {
        out.push(SIGN_NON_NEGATIVE);
        push_nat(out, value.unsigned_abs());
    } else {
        out.push(SIGN_NEGATIVE);
        let start = out.len();
        push_nat(out, value.unsigned_abs());
        for byte in &mut out[start..] {
            *byte = !*byte;
        }
    }
}

fn push_len(out: &mut Vec<u8>, len: usize) {
    push_nat(
        out,
        u128::try_from(len).expect("a collection length fits in a u128"),
    );
}

fn push_blob(out: &mut Vec<u8>, bytes: &[u8]) {
    push_len(out, bytes.len());
    out.extend_from_slice(bytes);
}

// --- decoding --------------------------------------------------------------

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(DecodeError::UnexpectedEnd {
                offset: self.offset,
            })?;
        self.offset += 1;
        Ok(byte)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(DecodeError::UnexpectedEnd {
                offset: self.offset,
            })?;
        if end > self.bytes.len() {
            return Err(DecodeError::UnexpectedEnd {
                offset: self.offset,
            });
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    /// Read a `nat`, optionally complementing every byte first (the negative
    /// half of `int`).
    fn nat_body(&mut self, complement: bool) -> Result<u128, DecodeError> {
        let offset = self.offset;
        let raw_len = self.byte()?;
        let len = usize::from(if complement { !raw_len } else { raw_len });
        let magnitude = self.take(len)?;
        let first = magnitude
            .first()
            .map(|byte| if complement { !*byte } else { *byte });
        if first == Some(0) {
            return Err(DecodeError::NonMinimalInteger { offset });
        }
        if len > 16 {
            return Err(DecodeError::IntegerTooWide { offset, bytes: len });
        }
        let mut value: u128 = 0;
        for byte in magnitude {
            let byte = if complement { !*byte } else { *byte };
            value = (value << 8) | u128::from(byte);
        }
        Ok(value)
    }

    fn nat(&mut self) -> Result<u128, DecodeError> {
        self.nat_body(false)
    }

    fn int(&mut self) -> Result<i128, DecodeError> {
        let offset = self.offset;
        match self.byte()? {
            SIGN_NON_NEGATIVE => {
                let magnitude = self.nat()?;
                i128::try_from(magnitude)
                    .map_err(|_| DecodeError::IntegerTooWide { offset, bytes: 16 })
            }
            SIGN_NEGATIVE => {
                let magnitude = self.nat_body(true)?;
                if magnitude == 0 {
                    return Err(DecodeError::NegativeZero { offset });
                }
                if magnitude == 1u128 << 127 {
                    return Ok(i128::MIN);
                }
                let magnitude = i128::try_from(magnitude)
                    .map_err(|_| DecodeError::IntegerTooWide { offset, bytes: 16 })?;
                Ok(-magnitude)
            }
            byte => Err(DecodeError::InvalidSign { byte, offset }),
        }
    }

    fn len(&mut self) -> Result<usize, DecodeError> {
        let offset = self.offset;
        let len = self.nat()?;
        usize::try_from(len).map_err(|_| DecodeError::LengthTooLarge { offset })
    }

    fn blob(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.len()?;
        self.take(len)
    }

    fn text(&mut self) -> Result<String, DecodeError> {
        let offset = self.offset;
        let bytes = self.blob()?;
        core::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| DecodeError::InvalidUtf8 { offset })
    }

    fn name(&mut self) -> Result<Name, DecodeError> {
        let offset = self.offset;
        let text = self.text()?;
        Name::new(&text).map_err(|error| DecodeError::InvalidName { offset, error })
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::DepthExceeded {
                offset: self.offset,
                max: MAX_DEPTH,
            });
        }
        let offset = self.offset;
        let tag = self.byte()?;
        let kind = ValueKind::from_tag(tag).ok_or(DecodeError::UnknownTag { tag, offset })?;
        match kind {
            ValueKind::Null => Ok(Value::Null),
            ValueKind::Bool => {
                let offset = self.offset;
                match self.byte()? {
                    0x00 => Ok(Value::Bool(false)),
                    0x01 => Ok(Value::Bool(true)),
                    byte => Err(DecodeError::InvalidBool { byte, offset }),
                }
            }
            ValueKind::Nat => Ok(Value::Nat(self.nat()?)),
            ValueKind::Int => Ok(Value::Int(self.int()?)),
            ValueKind::BitVec => {
                let offset = self.offset;
                let width = self.nat()?;
                let bits = self.nat()?;
                let width = u32::try_from(width).map_err(|_| DecodeError::InvalidBitVec {
                    offset,
                    error: ValueError::BitVecWidth {
                        width: MAX_BITVEC_WIDTH.saturating_add(1),
                    },
                })?;
                BitVec::new(width, bits)
                    .map(Value::BitVec)
                    .map_err(|error| DecodeError::InvalidBitVec { offset, error })
            }
            ValueKind::Bytes => Ok(Value::Bytes(self.blob()?.to_vec())),
            ValueKind::Text => Ok(Value::Text(self.text()?)),
            ValueKind::Symbol => Ok(Value::Symbol(self.name()?)),
            ValueKind::Tuple => self.items(depth).map(Value::Tuple),
            ValueKind::Seq => self.items(depth).map(Value::Seq),
            ValueKind::Set => self.set(depth),
            ValueKind::Multiset => self.multiset(depth),
            ValueKind::Record => self.record(depth),
            ValueKind::Variant => {
                let tag = self.name()?;
                let payload = self.value(depth + 1)?;
                Ok(Value::Variant {
                    tag,
                    payload: Box::new(payload),
                })
            }
            ValueKind::Map => self.map(depth),
            ValueKind::Opaque => {
                let domain = self.name()?;
                let bytes = self.blob()?.to_vec();
                Ok(Value::opaque(domain, bytes))
            }
        }
    }

    // Each composite lives in its own frame. The recursive cycle is
    // `value` -> composite -> `value`, so keeping the per-kind locals out of
    // `value` is what keeps the stack cost of [`MAX_DEPTH`] levels small — the
    // decoder's recursion bound is only as honest as the frame it bounds.

    #[inline(never)]
    fn items(&mut self, depth: usize) -> Result<Vec<Value>, DecodeError> {
        let len = self.len()?;
        let mut items = Vec::new();
        for _ in 0..len {
            items.push(self.value(depth + 1)?);
        }
        Ok(items)
    }

    #[inline(never)]
    fn set(&mut self, depth: usize) -> Result<Value, DecodeError> {
        let len = self.len()?;
        let mut elements: BTreeSet<Value> = BTreeSet::new();
        for _ in 0..len {
            let offset = self.offset;
            let element = self.value(depth + 1)?;
            if elements.last().is_some_and(|previous| *previous >= element) {
                return Err(DecodeError::NotAscending { offset });
            }
            elements.insert(element);
        }
        Ok(Value::Set(elements))
    }

    #[inline(never)]
    fn multiset(&mut self, depth: usize) -> Result<Value, DecodeError> {
        let len = self.len()?;
        let mut entries: BTreeMap<Value, NonZeroU64> = BTreeMap::new();
        for _ in 0..len {
            let offset = self.offset;
            let element = self.value(depth + 1)?;
            if entries
                .last_key_value()
                .is_some_and(|(previous, _)| *previous >= element)
            {
                return Err(DecodeError::NotAscending { offset });
            }
            let count_offset = self.offset;
            let multiplicity = self.nat()?;
            let multiplicity = u64::try_from(multiplicity)
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or(DecodeError::InvalidMultiplicity {
                    offset: count_offset,
                })?;
            entries.insert(element, multiplicity);
        }
        Ok(Value::Multiset(entries))
    }

    #[inline(never)]
    fn record(&mut self, depth: usize) -> Result<Value, DecodeError> {
        let len = self.len()?;
        let mut fields: BTreeMap<Name, Value> = BTreeMap::new();
        for _ in 0..len {
            let offset = self.offset;
            let name = self.name()?;
            if fields
                .last_key_value()
                .is_some_and(|(previous, _)| *previous >= name)
            {
                return Err(DecodeError::NotAscending { offset });
            }
            let value = self.value(depth + 1)?;
            fields.insert(name, value);
        }
        Ok(Value::Record(fields))
    }

    #[inline(never)]
    fn map(&mut self, depth: usize) -> Result<Value, DecodeError> {
        let len = self.len()?;
        let mut entries: BTreeMap<Value, Value> = BTreeMap::new();
        for _ in 0..len {
            let offset = self.offset;
            let key = self.value(depth + 1)?;
            if entries
                .last_key_value()
                .is_some_and(|(previous, _)| *previous >= key)
            {
                return Err(DecodeError::NotAscending { offset });
            }
            let value = self.value(depth + 1)?;
            entries.insert(key, value);
        }
        Ok(Value::Map(entries))
    }
}

// --- errors ----------------------------------------------------------------

/// Why a value could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueError {
    /// A name was empty. A value has no anonymous field, tag, or domain.
    EmptyName,
    /// A name contained a character outside printable ASCII, so it has more
    /// than one possible spelling (ADR-0013 canonical identity).
    NonCanonicalName {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
    /// A bitvector width was zero or above [`MAX_BITVEC_WIDTH`].
    BitVecWidth {
        /// The rejected width.
        width: u32,
    },
    /// A bitvector carried a bit at or above its declared width.
    BitVecOutOfRange {
        /// The declared width.
        width: u32,
        /// The rejected bits.
        bits: u128,
    },
    /// A record listed one field name twice.
    DuplicateField {
        /// The repeated field name.
        name: Name,
    },
    /// A map listed one key twice.
    DuplicateKey {
        /// The repeated key.
        key: Box<Value>,
    },
    /// A multiset listed one element twice.
    DuplicateElement {
        /// The repeated element.
        element: Box<Value>,
    },
    /// A multiset entry declared a multiplicity of zero.
    ZeroMultiplicity {
        /// The element whose multiplicity was zero.
        element: Box<Value>,
    },
    /// The value would nest deeper than [`MAX_DEPTH`].
    DepthExceeded {
        /// The depth the value would have had.
        depth: usize,
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("name is empty"),
            Self::NonCanonicalName { index, character } => write!(
                f,
                "name has a non-canonical character {character:?} at byte {index}"
            ),
            Self::BitVecWidth { width } => write!(
                f,
                "bitvector width {width} is not in 1..={MAX_BITVEC_WIDTH}"
            ),
            Self::BitVecOutOfRange { width, bits } => {
                write!(f, "bitvector {bits} does not fit in {width} bits")
            }
            Self::DuplicateField { name } => write!(f, "record field {name} is listed twice"),
            Self::DuplicateKey { key } => write!(f, "map key {key:?} is listed twice"),
            Self::DuplicateElement { element } => {
                write!(f, "multiset element {element:?} is listed twice")
            }
            Self::ZeroMultiplicity { element } => write!(
                f,
                "multiset element {element:?} declares a multiplicity of zero"
            ),
            Self::DepthExceeded { depth, max } => {
                write!(f, "value nests {depth} deep, above the bound of {max}")
            }
        }
    }
}

impl core::error::Error for ValueError {}

/// Why a byte string is not a canonical value encoding.
///
/// Every variant carries the byte offset at which the violation was detected,
/// so a rejection is diagnosable without re-running the decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended inside a value.
    UnexpectedEnd {
        /// Offset at which more input was required.
        offset: usize,
    },
    /// A complete value was followed by more bytes. Concatenation is not a
    /// value, and accepting it would give one identity two encodings.
    TrailingBytes {
        /// Offset of the first surplus byte.
        offset: usize,
    },
    /// A byte in tag position names no kind.
    UnknownTag {
        /// The rejected byte.
        tag: u8,
        /// Offset of the tag.
        offset: usize,
    },
    /// A boolean payload was neither `0x00` nor `0x01`.
    InvalidBool {
        /// The rejected byte.
        byte: u8,
        /// Offset of the byte.
        offset: usize,
    },
    /// An integer sign byte was neither `0x00` nor `0x01`.
    InvalidSign {
        /// The rejected byte.
        byte: u8,
        /// Offset of the byte.
        offset: usize,
    },
    /// An integer magnitude carried a leading zero byte — a second encoding of
    /// a value that already has one.
    NonMinimalInteger {
        /// Offset of the length byte.
        offset: usize,
    },
    /// A negative integer had magnitude zero, which spells `0` twice.
    NegativeZero {
        /// Offset of the sign byte.
        offset: usize,
    },
    /// The integer is well formed but wider than this build's carrier.
    ///
    /// Not a malformed artifact: the encoding admits magnitudes far wider than
    /// `u128`, and reporting the difference honestly is what lets a later,
    /// wider carrier read these bytes without an epoch advance.
    IntegerTooWide {
        /// Offset of the length byte.
        offset: usize,
        /// Declared magnitude length in bytes.
        bytes: usize,
    },
    /// A declared length exceeds this platform's addressable range.
    LengthTooLarge {
        /// Offset of the length prefix.
        offset: usize,
    },
    /// A text payload was not valid UTF-8.
    InvalidUtf8 {
        /// Offset of the length prefix.
        offset: usize,
    },
    /// A name payload was not a canonical [`Name`].
    InvalidName {
        /// Offset of the length prefix.
        offset: usize,
        /// Why the name was rejected.
        error: ValueError,
    },
    /// A bitvector payload was not a canonical [`BitVec`].
    InvalidBitVec {
        /// Offset of the width prefix.
        offset: usize,
        /// Why the bitvector was rejected.
        error: ValueError,
    },
    /// A set element, multiset element, record field, or map key was not
    /// strictly greater than its predecessor.
    NotAscending {
        /// Offset of the offending entry.
        offset: usize,
    },
    /// A multiset multiplicity was zero or wider than `u64`.
    InvalidMultiplicity {
        /// Offset of the multiplicity.
        offset: usize,
    },
    /// The encoding nests deeper than [`MAX_DEPTH`].
    DepthExceeded {
        /// Offset at which the bound was crossed.
        offset: usize,
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd { offset } => {
                write!(f, "input ended inside a value at byte {offset}")
            }
            Self::TrailingBytes { offset } => {
                write!(f, "surplus bytes after a complete value at byte {offset}")
            }
            Self::UnknownTag { tag, offset } => {
                write!(f, "byte {tag:#04x} at {offset} names no value kind")
            }
            Self::InvalidBool { byte, offset } => {
                write!(
                    f,
                    "boolean byte {byte:#04x} at {offset} is not 0x00 or 0x01"
                )
            }
            Self::InvalidSign { byte, offset } => {
                write!(f, "sign byte {byte:#04x} at {offset} is not 0x00 or 0x01")
            }
            Self::NonMinimalInteger { offset } => {
                write!(f, "integer at {offset} has a leading zero byte")
            }
            Self::NegativeZero { offset } => {
                write!(f, "integer at {offset} is a negative zero")
            }
            Self::IntegerTooWide { offset, bytes } => write!(
                f,
                "integer at {offset} is {bytes} bytes wide, above this build's 16-byte carrier"
            ),
            Self::LengthTooLarge { offset } => {
                write!(f, "length at {offset} exceeds this platform's range")
            }
            Self::InvalidUtf8 { offset } => write!(f, "text at {offset} is not valid UTF-8"),
            Self::InvalidName { offset, error } => {
                write!(f, "name at {offset} is not canonical: {error}")
            }
            Self::InvalidBitVec { offset, error } => {
                write!(f, "bitvector at {offset} is not canonical: {error}")
            }
            Self::NotAscending { offset } => write!(
                f,
                "entry at {offset} is not strictly greater than its predecessor"
            ),
            Self::InvalidMultiplicity { offset } => {
                write!(f, "multiplicity at {offset} is zero or out of range")
            }
            Self::DepthExceeded { offset, max } => {
                write!(f, "encoding at {offset} nests deeper than {max}")
            }
        }
    }
}

impl core::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidName { error, .. } | Self::InvalidBitVec { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> Name {
        Name::new(text).expect("test name is canonical")
    }

    fn symbol(text: &str) -> Value {
        Value::symbol(name(text))
    }

    fn hex(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    /// A domain broad enough to exercise every kind, every ordering rule, and
    /// the boundaries of every primitive.
    ///
    /// The order laws below are checked *exhaustively* over this domain rather
    /// than sampled: for a total order, exhaustive on a small domain is a
    /// stronger statement than random on a large one.
    fn sample_values() -> Vec<Value> {
        let mut values = vec![
            Value::Null,
            Value::Bool(false),
            Value::Bool(true),
            Value::nat(0),
            Value::nat(1),
            Value::nat(255),
            Value::nat(256),
            Value::nat(u128::MAX),
            Value::int(i128::MIN),
            Value::int(-256),
            Value::int(-255),
            Value::int(-1),
            Value::int(0),
            Value::int(1),
            Value::int(255),
            Value::int(i128::MAX),
            Value::bitvec(1, 0).expect("canonical"),
            Value::bitvec(1, 1).expect("canonical"),
            Value::bitvec(8, 1).expect("canonical"),
            Value::bitvec(8, 255).expect("canonical"),
            Value::bitvec(128, u128::MAX).expect("canonical"),
            Value::bytes(Vec::new()),
            Value::bytes(vec![0x00]),
            Value::bytes(vec![0xff]),
            Value::bytes(vec![0x00, 0x00]),
            Value::text(""),
            Value::text("a"),
            Value::text("z"),
            Value::text("aa"),
            Value::text("héllo"),
            symbol("n1"),
            symbol("n2"),
            symbol("node"),
            Value::opaque(name("ieee754"), vec![]),
            Value::opaque(name("ieee754"), vec![0x01]),
            Value::opaque(name("uuid"), vec![0x01]),
        ];

        let scalars = values.clone();
        values.push(Value::tuple([]).expect("canonical"));
        values.push(Value::tuple([Value::Null]).expect("canonical"));
        values.push(Value::tuple([Value::Null, Value::Bool(true)]).expect("canonical"));
        values.push(Value::seq([]).expect("canonical"));
        values.push(Value::seq([Value::nat(1)]).expect("canonical"));
        values.push(Value::seq([Value::nat(1), Value::nat(2)]).expect("canonical"));
        values.push(Value::set([]).expect("canonical"));
        values.push(Value::set([Value::nat(1)]).expect("canonical"));
        values.push(Value::set([Value::nat(1), Value::nat(2)]).expect("canonical"));
        values.push(Value::set(scalars.iter().take(6).cloned()).expect("canonical"));
        values.push(Value::multiset([]).expect("canonical"));
        values.push(Value::multiset([(Value::nat(1), 1)]).expect("canonical"));
        values.push(Value::multiset([(Value::nat(1), 2)]).expect("canonical"));
        values.push(Value::multiset([(Value::nat(1), 1), (Value::nat(2), 3)]).expect("canonical"));
        values.push(Value::record([]).expect("canonical"));
        values.push(Value::record([(name("a"), Value::nat(1))]).expect("canonical"));
        values.push(
            Value::record([(name("a"), Value::nat(1)), (name("b"), Value::nat(2))])
                .expect("canonical"),
        );
        values.push(Value::variant(name("none"), Value::Null).expect("canonical"));
        values.push(Value::variant(name("some"), Value::nat(1)).expect("canonical"));
        values.push(Value::variant(name("some"), Value::nat(2)).expect("canonical"));
        values.push(Value::map([]).expect("canonical"));
        values.push(Value::map([(Value::nat(1), Value::Null)]).expect("canonical"));
        values.push(
            Value::map([(Value::nat(1), Value::Null), (Value::nat(2), Value::Null)])
                .expect("canonical"),
        );
        // Nesting: a set of records, a map keyed by tuples, a sequence of sets.
        values.push(
            Value::set([
                Value::record([(name("id"), Value::nat(1))]).expect("canonical"),
                Value::record([(name("id"), Value::nat(2))]).expect("canonical"),
            ])
            .expect("canonical"),
        );
        values.push(
            Value::map([(
                Value::tuple([symbol("n1"), Value::nat(0)]).expect("canonical"),
                Value::set([Value::nat(7)]).expect("canonical"),
            )])
            .expect("canonical"),
        );
        values.push(
            Value::seq([
                Value::set([]).expect("canonical"),
                Value::set([Value::Null]).expect("canonical"),
            ])
            .expect("canonical"),
        );
        values
    }

    // --- domain ----------------------------------------------------------------

    #[test]
    fn value_kinds_are_the_cir_schema_value_kinds() {
        // notes/plan/schemas/cir.schema.json, `$defs.value`: the thirteen
        // tagged kinds plus the bare JSON scalars null/boolean/string.
        let tagged: Vec<&str> = ValueKind::ALL
            .iter()
            .map(|kind| kind.as_str())
            .filter(|name| !matches!(*name, "null" | "boolean" | "string"))
            .collect();
        let mut schema_kinds = tagged.clone();
        schema_kinds.sort_unstable();
        let mut expected = vec![
            "nat", "int", "bitvec", "bytes", "tuple", "record", "variant", "set", "map", "seq",
            "multiset", "symbol", "opaque",
        ];
        expected.sort_unstable();
        assert_eq!(
            schema_kinds, expected,
            "the tagged kinds must be exactly cir.schema.json's `kind` enum"
        );
        assert_eq!(tagged.len(), 13);
        assert_eq!(ValueKind::ALL.len(), 16);
    }

    #[test]
    fn kind_tags_are_distinct_and_in_declaration_order() {
        let mut previous = 0u8;
        for kind in ValueKind::ALL {
            assert!(kind.tag() > previous, "{kind} tag is not ascending");
            previous = kind.tag();
            assert_eq!(ValueKind::from_tag(kind.tag()), Some(kind));
        }
        assert_eq!(ValueKind::from_tag(0x00), None, "0x00 is reserved");
        assert_eq!(ValueKind::from_tag(0x11), None);
    }

    #[test]
    fn every_kind_appears_in_the_sample_domain() {
        let kinds: BTreeSet<u8> = sample_values()
            .iter()
            .map(|value| value.kind().tag())
            .collect();
        assert_eq!(
            kinds.len(),
            ValueKind::ALL.len(),
            "the sample domain must exercise every kind"
        );
    }

    // --- order laws ------------------------------------------------------------

    #[test]
    fn order_is_reflexive_antisymmetric_and_total() {
        let values = sample_values();
        for left in &values {
            assert_eq!(left.cmp(left), Ordering::Equal, "reflexivity");
            for right in &values {
                let forward = left.cmp(right);
                let backward = right.cmp(left);
                // Totality: `cmp` is defined for every pair, and exactly one of
                // <, =, > holds — `Ordering` has no fourth case.
                assert_eq!(forward, backward.reverse(), "antisymmetry");
                assert_eq!(
                    forward == Ordering::Equal,
                    left == right,
                    "the order must agree with equality"
                );
            }
        }
    }

    #[test]
    fn order_is_transitive() {
        let values = sample_values();
        for a in &values {
            for b in &values {
                if a.cmp(b) != Ordering::Less {
                    continue;
                }
                for c in &values {
                    if b.cmp(c) == Ordering::Less {
                        assert_eq!(
                            a.cmp(c),
                            Ordering::Less,
                            "transitivity: {a:?} < {b:?} < {c:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn value_order_is_lexicographic_order_on_encodings() {
        // ADR-0013 makes the canonical encoding primary; RFC 0037 N4 orders by
        // the bytes of that encoding. `Ord` computes the same order
        // structurally, and this test is what keeps the two from drifting.
        let values = sample_values();
        let encodings: Vec<Vec<u8>> = values.iter().map(Value::encode).collect();
        for (left_index, left) in values.iter().enumerate() {
            for (right_index, right) in values.iter().enumerate() {
                assert_eq!(
                    left.cmp(right),
                    encodings[left_index].cmp(&encodings[right_index]),
                    "structural order disagrees with encoded order for {left:?} vs {right:?}"
                );
            }
        }
    }

    #[test]
    fn distinct_values_have_distinct_encodings() {
        // Injectivity of the canonical encoder — the property
        // docs/10_INTEROP_AND_MIGRATION.md exports as a proof obligation
        // ("canonical encoder injectivity for a type").
        let values = sample_values();
        let encodings: BTreeSet<Vec<u8>> = values.iter().map(Value::encode).collect();
        let distinct: BTreeSet<&Value> = values.iter().collect();
        assert_eq!(encodings.len(), distinct.len());
    }

    #[test]
    fn no_encoding_is_a_prefix_of_another() {
        // Prefix-freeness is what makes componentwise comparison equal to
        // lexicographic comparison of the concatenated encodings.
        let values = sample_values();
        let encodings: Vec<Vec<u8>> = values.iter().map(Value::encode).collect();
        for left in &encodings {
            for right in &encodings {
                if left.len() < right.len() {
                    assert_ne!(&right[..left.len()], left.as_slice(), "prefix-free");
                }
            }
        }
    }

    #[test]
    fn shortlex_is_the_documented_string_order() {
        // Deliberately not `str`'s lexicographic order; see the module docs.
        assert!(Value::text("z") < Value::text("aa"));
        assert!(Value::text("aa") < Value::text("ab"));
        assert!(Value::bytes(vec![0xff]) < Value::bytes(vec![0x00, 0x00]));
    }

    #[test]
    fn negative_integers_order_below_non_negative_ones() {
        let mut sorted = vec![
            Value::int(1),
            Value::int(i128::MIN),
            Value::int(-1),
            Value::int(0),
            Value::int(-256),
        ];
        sorted.sort();
        assert_eq!(
            sorted,
            vec![
                Value::int(i128::MIN),
                Value::int(-256),
                Value::int(-1),
                Value::int(0),
                Value::int(1),
            ]
        );
    }

    // --- round trips -----------------------------------------------------------

    #[test]
    fn every_value_round_trips() {
        for value in sample_values() {
            let encoded = value.encode();
            let decoded = Value::decode(&encoded).expect("canonical bytes decode");
            assert_eq!(decoded, value, "round trip changed the value");
            assert_eq!(
                decoded.encode(),
                encoded,
                "re-encoding a decoded value changed the bytes"
            );
        }
    }

    #[test]
    fn integer_boundaries_round_trip() {
        let boundaries = [
            i128::MIN,
            i128::MIN + 1,
            -(1i128 << 64),
            -256,
            -255,
            -1,
            0,
            1,
            255,
            256,
            1i128 << 64,
            i128::MAX,
        ];
        for value in boundaries {
            let encoded = Value::int(value).encode();
            assert_eq!(
                Value::decode(&encoded).expect("canonical"),
                Value::int(value)
            );
        }
        for value in [0u128, 1, 255, 256, u128::MAX] {
            let encoded = Value::nat(value).encode();
            assert_eq!(
                Value::decode(&encoded).expect("canonical"),
                Value::nat(value)
            );
        }
    }

    // --- determinism -----------------------------------------------------------

    #[test]
    fn construction_order_does_not_change_the_encoding() {
        // docs/19 §7: semantic artifacts must be identical across the
        // determinism matrix. Insertion order is the cheapest way to smuggle a
        // difference in, so it is the one pinned first.
        let forwards = Value::set([Value::nat(1), Value::nat(2), Value::nat(3)]).expect("ok");
        let backwards = Value::set([Value::nat(3), Value::nat(2), Value::nat(1)]).expect("ok");
        let repeated =
            Value::set([Value::nat(2), Value::nat(1), Value::nat(3), Value::nat(1)]).expect("ok");
        assert_eq!(forwards.encode(), backwards.encode());
        assert_eq!(forwards.encode(), repeated.encode());

        let record_forwards = Value::record([
            (name("alpha"), Value::nat(1)),
            (name("beta"), Value::nat(2)),
            (name("gamma"), Value::nat(3)),
        ])
        .expect("ok");
        let record_backwards = Value::record([
            (name("gamma"), Value::nat(3)),
            (name("beta"), Value::nat(2)),
            (name("alpha"), Value::nat(1)),
        ])
        .expect("ok");
        assert_eq!(record_forwards.encode(), record_backwards.encode());

        let map_forwards =
            Value::map([(symbol("n1"), Value::nat(1)), (symbol("n2"), Value::nat(2))]).expect("ok");
        let map_backwards =
            Value::map([(symbol("n2"), Value::nat(2)), (symbol("n1"), Value::nat(1))]).expect("ok");
        assert_eq!(map_forwards.encode(), map_backwards.encode());

        let bag_forwards = Value::multiset([(Value::nat(1), 2), (Value::nat(2), 1)]).expect("ok");
        let bag_backwards = Value::multiset([(Value::nat(2), 1), (Value::nat(1), 2)]).expect("ok");
        assert_eq!(bag_forwards.encode(), bag_backwards.encode());
    }

    #[test]
    fn encoding_is_a_function_of_the_value_alone() {
        let values = sample_values();
        for value in &values {
            let first = value.encode();
            let second = value.clone().encode();
            assert_eq!(first, second);
        }
    }

    // --- constructors reject non-canonical input --------------------------------

    #[test]
    fn names_must_be_non_empty_printable_ascii() {
        assert_eq!(Name::new(""), Err(ValueError::EmptyName));
        assert_eq!(
            Name::new("a b"),
            Err(ValueError::NonCanonicalName {
                index: 1,
                character: ' '
            })
        );
        assert_eq!(
            Name::new("é"),
            Err(ValueError::NonCanonicalName {
                index: 0,
                character: 'é'
            })
        );
        assert!(Name::new("node_1").is_ok());
    }

    #[test]
    fn bitvectors_reject_bad_widths_and_out_of_range_bits() {
        assert_eq!(
            Value::bitvec(0, 0),
            Err(ValueError::BitVecWidth { width: 0 })
        );
        assert_eq!(
            Value::bitvec(129, 0),
            Err(ValueError::BitVecWidth { width: 129 })
        );
        assert_eq!(
            Value::bitvec(4, 16),
            Err(ValueError::BitVecOutOfRange { width: 4, bits: 16 })
        );
        assert!(Value::bitvec(4, 15).is_ok());
        assert!(Value::bitvec(128, u128::MAX).is_ok());
    }

    #[test]
    fn records_reject_duplicate_fields() {
        assert_eq!(
            Value::record([(name("a"), Value::nat(1)), (name("a"), Value::nat(2))]),
            Err(ValueError::DuplicateField { name: name("a") })
        );
    }

    #[test]
    fn maps_reject_duplicate_keys() {
        assert_eq!(
            Value::map([
                (Value::nat(1), Value::Null),
                (Value::nat(1), Value::Bool(true))
            ]),
            Err(ValueError::DuplicateKey {
                key: Box::new(Value::nat(1))
            })
        );
    }

    #[test]
    fn multisets_reject_zero_and_duplicate_multiplicities() {
        assert_eq!(
            Value::multiset([(Value::nat(1), 0)]),
            Err(ValueError::ZeroMultiplicity {
                element: Box::new(Value::nat(1))
            })
        );
        assert_eq!(
            Value::multiset([(Value::nat(1), 1), (Value::nat(1), 2)]),
            Err(ValueError::DuplicateElement {
                element: Box::new(Value::nat(1))
            })
        );
    }

    #[test]
    fn sets_collapse_repeated_elements() {
        let set = Value::set([Value::nat(1), Value::nat(1)]).expect("ok");
        assert_eq!(set, Value::set([Value::nat(1)]).expect("ok"));
    }

    #[test]
    fn constructors_reject_values_deeper_than_the_bound() {
        let mut value = Value::Null;
        for _ in 1..MAX_DEPTH {
            value = Value::seq([value]).expect("within the bound");
        }
        assert_eq!(value.depth(), MAX_DEPTH);
        assert_eq!(
            Value::seq([value.clone()]),
            Err(ValueError::DepthExceeded {
                depth: MAX_DEPTH + 1,
                max: MAX_DEPTH
            })
        );
        // The bound is exactly the decoder's, so the deepest constructible
        // value still round-trips.
        let encoded = value.encode();
        assert_eq!(Value::decode(&encoded).expect("canonical"), value);
    }

    // --- the decoder rejects non-canonical bytes ---------------------------------

    #[test]
    fn decoder_rejects_trailing_bytes() {
        let mut bytes = Value::Null.encode();
        bytes.push(0x01);
        assert_eq!(
            Value::decode(&bytes),
            Err(DecodeError::TrailingBytes { offset: 1 })
        );
    }

    #[test]
    fn decoder_rejects_truncation() {
        let bytes = Value::nat(256).encode();
        for len in 0..bytes.len() {
            assert!(
                matches!(
                    Value::decode(&bytes[..len]),
                    Err(DecodeError::UnexpectedEnd { .. })
                ),
                "truncation to {len} bytes was accepted"
            );
        }
    }

    #[test]
    fn decoder_rejects_unknown_and_reserved_tags() {
        assert_eq!(
            Value::decode(&[0x00]),
            Err(DecodeError::UnknownTag {
                tag: 0x00,
                offset: 0
            })
        );
        assert_eq!(
            Value::decode(&[0xff]),
            Err(DecodeError::UnknownTag {
                tag: 0xff,
                offset: 0
            })
        );
    }

    #[test]
    fn decoder_rejects_non_minimal_integers() {
        // nat 1 spelled with a leading zero byte.
        assert_eq!(
            Value::decode(&[0x03, 0x02, 0x00, 0x01]),
            Err(DecodeError::NonMinimalInteger { offset: 1 })
        );
        // int 0 spelled as a negative zero.
        assert_eq!(
            Value::decode(&[0x04, SIGN_NEGATIVE, 0xff]),
            Err(DecodeError::NegativeZero { offset: 1 })
        );
    }

    #[test]
    fn decoder_rejects_invalid_scalar_payloads() {
        assert_eq!(
            Value::decode(&[0x02, 0x02]),
            Err(DecodeError::InvalidBool {
                byte: 0x02,
                offset: 1
            })
        );
        assert_eq!(
            Value::decode(&[0x04, 0x02, 0x00]),
            Err(DecodeError::InvalidSign {
                byte: 0x02,
                offset: 1
            })
        );
        // A 4-bit bitvector holding 16.
        assert_eq!(
            Value::decode(&[0x05, 0x01, 0x04, 0x01, 0x10]),
            Err(DecodeError::InvalidBitVec {
                offset: 1,
                error: ValueError::BitVecOutOfRange { width: 4, bits: 16 }
            })
        );
        // A symbol whose payload is not printable ASCII.
        assert_eq!(
            Value::decode(&[0x08, 0x01, 0x01, 0x20]),
            Err(DecodeError::InvalidName {
                offset: 1,
                error: ValueError::NonCanonicalName {
                    index: 0,
                    character: ' '
                }
            })
        );
        // Text that is not valid UTF-8.
        assert_eq!(
            Value::decode(&[0x07, 0x01, 0x01, 0xff]),
            Err(DecodeError::InvalidUtf8 { offset: 1 })
        );
    }

    #[test]
    fn decoder_rejects_unsorted_and_repeated_entries() {
        let two = Value::nat(2).encode();
        let one = Value::nat(1).encode();

        let mut set = vec![ValueKind::Set.tag(), 0x01, 0x02];
        set.extend_from_slice(&two);
        set.extend_from_slice(&one);
        assert!(matches!(
            Value::decode(&set),
            Err(DecodeError::NotAscending { .. })
        ));

        let mut repeated = vec![ValueKind::Set.tag(), 0x01, 0x02];
        repeated.extend_from_slice(&one);
        repeated.extend_from_slice(&one);
        assert!(matches!(
            Value::decode(&repeated),
            Err(DecodeError::NotAscending { .. })
        ));

        let mut map = vec![ValueKind::Map.tag(), 0x01, 0x02];
        map.extend_from_slice(&two);
        map.extend_from_slice(&Value::Null.encode());
        map.extend_from_slice(&one);
        map.extend_from_slice(&Value::Null.encode());
        assert!(matches!(
            Value::decode(&map),
            Err(DecodeError::NotAscending { .. })
        ));

        // Record fields out of order: "b" then "a".
        let mut record = vec![ValueKind::Record.tag(), 0x01, 0x02];
        record.extend_from_slice(&[0x01, 0x01, b'b']);
        record.extend_from_slice(&Value::Null.encode());
        record.extend_from_slice(&[0x01, 0x01, b'a']);
        record.extend_from_slice(&Value::Null.encode());
        assert!(matches!(
            Value::decode(&record),
            Err(DecodeError::NotAscending { .. })
        ));
    }

    #[test]
    fn decoder_rejects_zero_multiplicity() {
        let mut bytes = vec![ValueKind::Multiset.tag(), 0x01, 0x01];
        bytes.extend_from_slice(&Value::nat(1).encode());
        bytes.extend_from_slice(&[0x00]); // nat 0
        assert!(matches!(
            Value::decode(&bytes),
            Err(DecodeError::InvalidMultiplicity { .. })
        ));
    }

    #[test]
    fn decoder_bounds_its_own_recursion() {
        // An oversized artifact must be a typed error, not a stack overflow
        // (docs/19 §6, "malicious cyclic/oversized artifacts").
        let mut bytes = Vec::new();
        for _ in 0..=MAX_DEPTH {
            bytes.push(ValueKind::Seq.tag());
            bytes.extend_from_slice(&[0x01, 0x01]); // one element
        }
        bytes.push(ValueKind::Null.tag());
        assert!(matches!(
            Value::decode(&bytes),
            Err(DecodeError::DepthExceeded { .. })
        ));
    }

    #[test]
    fn decoder_does_not_trust_a_declared_length() {
        // A declared length of 2^64 with no payload must fail on input, not on
        // an allocation.
        let mut bytes = vec![ValueKind::Bytes.tag(), 0x09, 0x01];
        bytes.extend_from_slice(&[0x00; 8]);
        assert!(matches!(
            Value::decode(&bytes),
            Err(DecodeError::UnexpectedEnd { .. } | DecodeError::LengthTooLarge { .. })
        ));
    }

    #[test]
    fn decoder_reports_an_over_wide_integer_as_unsupported() {
        // 17 magnitude bytes: well formed, wider than this build's carrier.
        let mut bytes = vec![ValueKind::Nat.tag(), 0x11, 0x01];
        bytes.extend_from_slice(&[0x00; 16]);
        assert_eq!(
            Value::decode(&bytes),
            Err(DecodeError::IntegerTooWide {
                offset: 1,
                bytes: 17
            })
        );
    }

    // --- golden vectors ----------------------------------------------------------

    #[test]
    fn golden_vectors_are_pinned() {
        // Identity is the encoding (ADR-0013), so a change to any byte string
        // below changes the identity of every artifact containing that value.
        // That is a semantic-epoch event (plan §4.6, ADR-0018) with a published
        // per-artifact-class compatibility statement — never a refactor, and
        // never a test fixed up to match new output.
        //
        // RFC 0037 requires the same discipline of the sibling encoding:
        // "golden identity vectors are part of the acceptance suite".
        let vectors: Vec<(&str, Value, &str)> = vec![
            ("null", Value::Null, "01"),
            ("false", Value::Bool(false), "0200"),
            ("true", Value::Bool(true), "0201"),
            ("nat 0", Value::nat(0), "0300"),
            ("nat 1", Value::nat(1), "030101"),
            ("nat 255", Value::nat(255), "0301ff"),
            ("nat 256", Value::nat(256), "03020100"),
            (
                "nat u128::MAX",
                Value::nat(u128::MAX),
                "0310ffffffffffffffffffffffffffffffff",
            ),
            ("int 0", Value::int(0), "040100"),
            ("int 1", Value::int(1), "04010101"),
            ("int -1", Value::int(-1), "0400fefe"),
            ("int -255", Value::int(-255), "0400fe00"),
            ("int -256", Value::int(-256), "0400fdfeff"),
            (
                "int i128::MIN",
                Value::int(i128::MIN),
                "0400ef7fffffffffffffffffffffffffffffff",
            ),
            (
                "int i128::MAX",
                Value::int(i128::MAX),
                "0401107fffffffffffffffffffffffffffffff",
            ),
            (
                "bitvec 8#1",
                Value::bitvec(8, 1).expect("canonical"),
                "05 0108 0101",
            ),
            ("bytes []", Value::bytes(Vec::new()), "06 00"),
            (
                "bytes [00 ff]",
                Value::bytes(vec![0x00, 0xff]),
                "06 0102 00ff",
            ),
            ("text \"\"", Value::text(""), "07 00"),
            ("text \"ab\"", Value::text("ab"), "07 0102 6162"),
            ("symbol n1", symbol("n1"), "08 0102 6e31"),
            (
                "tuple (null, true)",
                Value::tuple([Value::Null, Value::Bool(true)]).expect("canonical"),
                "09 0102 01 0201",
            ),
            (
                "seq [1, 2]",
                Value::seq([Value::nat(1), Value::nat(2)]).expect("canonical"),
                "0a 0102 030101 030102",
            ),
            (
                // Built out of order: the encoding is the sorted one.
                "set {2, 1}",
                Value::set([Value::nat(2), Value::nat(1)]).expect("canonical"),
                "0b 0102 030101 030102",
            ),
            (
                "multiset {1: 2}",
                Value::multiset([(Value::nat(1), 2)]).expect("canonical"),
                "0c 0101 030101 0102",
            ),
            (
                "record {a: 1}",
                Value::record([(name("a"), Value::nat(1))]).expect("canonical"),
                "0d 0101 0101 61 030101",
            ),
            (
                "variant some(1)",
                Value::variant(name("some"), Value::nat(1)).expect("canonical"),
                "0e 0104 736f6d65 030101",
            ),
            (
                "map {1 |-> null}",
                Value::map([(Value::nat(1), Value::Null)]).expect("canonical"),
                "0f 0101 030101 01",
            ),
            (
                "opaque uuid[01]",
                Value::opaque(name("uuid"), vec![0x01]),
                "10 0104 75756964 0101 01",
            ),
        ];

        for (label, value, expected) in vectors {
            let expected: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
            assert_eq!(hex(&value.encode()), expected, "golden vector: {label}");
            assert_eq!(
                Value::decode(&value.encode()).expect("golden vector decodes"),
                value,
                "golden vector round trip: {label}"
            );
        }
    }

    #[test]
    fn encoding_id_is_stable() {
        assert_eq!(ENCODING_ID, "cvnf-1");
    }
}
