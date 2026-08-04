//! The generated [`Value`] domain the `continuum-value` property suites draw from
//! (`bn-221j`).
//!
//! # The counterexample text *is* the canonical encoding
//!
//! [`ValueCase`] renders as a single hex atom holding [`Value::encode`] bytes, and parses
//! by [`Value::decode`]. That is deliberate and it is what makes retention exact:
//!
//! - the pinned string in a suite is the CVNF-1 identity of the counterexample, not a
//!   `Debug` rendering that a formatting change could alter;
//! - reading it back goes through the *strict* decoder, so a retained case that is not a
//!   canonical encoding fails to parse rather than being repaired into something adjacent;
//! - a retained case therefore cannot silently become a different input, which is the one
//!   way a "retained counterexample" could be theatre.
//!
//! # What the generator draws
//!
//! All sixteen [`ValueKind`]s, with an alphabet chosen so the interesting boundaries are
//! *reachable* rather than merely possible: `nat`'s length-byte steps (`255`/`256`,
//! `65_535`/`65_536`, `u64::MAX`, `u128::MAX`), `int`'s sign and complement boundaries
//! (`0`, `-1`, `-255`/`-256`, `i128::MIN`, `i128::MAX`), bitvector widths on both sides of
//! a byte and at `MAX_BITVEC_WIDTH`, text with multi-byte code points and a code point
//! whose UTF-8 sorts differently from its scalar value, and byte strings of equal length
//! but different content.
//!
//! Nothing is drawn that a constructor would refuse: sets, multisets, maps, and records are
//! de-duplicated before construction, and the depth budget is spent going down so
//! [`MAX_DEPTH`] is never exceeded. A generator that produced values the crate would reject
//! would be testing its own error handling.
//!
//! # Size, and why shrinking terminates
//!
//! [`Domain::size`] is the encoded byte length. Every candidate [`Values::shrink`] proposes
//! is either a strict sub-encoding (a child in place of its container, a container with one
//! element removed) or a scalar with a shorter payload, so accepted candidates strictly
//! decrease and `Value::Null` — one byte — is the global floor.

#![allow(dead_code)]

use continuum_value::value::{BitVec, MAX_BITVEC_WIDTH, MAX_DEPTH, Name, Value};

use crate::property::{Case, Domain, Prng, Sexp};

// --- the case -------------------------------------------------------------------------

/// A [`Value`], as a property-harness case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCase(pub Value);

impl ValueCase {
    /// The value.
    #[must_use]
    pub const fn get(&self) -> &Value {
        &self.0
    }

    /// The canonical encoding.
    #[must_use]
    pub fn encoded(&self) -> Vec<u8> {
        self.0.encode()
    }
}

impl Case for ValueCase {
    fn to_sexp(&self) -> Sexp {
        Sexp::atom(self.0.encode())
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        Value::decode(sexp.as_atom()?).ok().map(Self)
    }
}

// --- the alphabets --------------------------------------------------------------------

/// Naturals at every `nat` length-byte boundary the encoding has.
const NATS: [u128; 14] = [
    0,
    1,
    2,
    127,
    128,
    254,
    255,
    256,
    65_535,
    65_536,
    16_777_215,
    16_777_216,
    u64::MAX as u128,
    u128::MAX,
];

/// Integers at the sign byte and at the complement's magnitude boundaries.
const INTS: [i128; 14] = [
    0,
    1,
    -1,
    2,
    -2,
    127,
    -128,
    255,
    -255,
    256,
    -256,
    65_536,
    i128::MAX,
    i128::MIN,
];

/// Bitvector widths on both sides of a byte, a word, and the declared maximum.
const WIDTHS: [u32; 8] = [1, 2, 7, 8, 63, 64, 127, MAX_BITVEC_WIDTH];

/// Names: the shortest legal one, an ordering pair where shortlex and byte-lexicographic
/// disagree (`z` before `aa`), and a name that is a proper prefix of another.
const NAMES: [&str; 8] = ["a", "b", "z", "aa", "ab", "a.b", "~", "!"];

/// Text drawn to exercise UTF-8 rather than ASCII: a combining pair whose two spellings are
/// two values (no normalization is applied, and none should be), a code point above the
/// BMP, and a code point whose UTF-8 bytes order differently from a shorter string's.
const TEXTS: [&str; 8] = [
    "",
    "a",
    "z",
    "aa",
    "caf\u{e9}",
    "cafe\u{301}",
    "\u{1f600}",
    "\u{ff}",
];

/// Byte strings including two of equal length and different content, which is what
/// separates shortlex from a length comparison.
const BLOBS: [&[u8]; 6] = [b"", b"\x00", b"\x01", b"\xff", b"\x00\x00", b"\x00\x01"];

// --- the domain -----------------------------------------------------------------------

/// Generated [`Value`]s, bounded by a depth and a width.
#[derive(Debug, Clone, Copy)]
pub struct Values {
    /// The deepest generated value, counting scalars as depth 1. Never above
    /// [`MAX_DEPTH`].
    pub max_depth: usize,
    /// The most children any one container gets.
    pub max_width: usize,
}

impl Values {
    /// A domain bounded by `max_depth` and `max_width`.
    ///
    /// `max_depth` is clamped to [`MAX_DEPTH`]: the generator's job is to produce values the
    /// crate admits, so the bound it declares is the bound it honours.
    #[must_use]
    pub const fn new(max_depth: usize, max_width: usize) -> Self {
        Self {
            max_depth: if max_depth > MAX_DEPTH {
                MAX_DEPTH
            } else if max_depth == 0 {
                1
            } else {
                max_depth
            },
            max_width,
        }
    }

    /// Shallow and narrow: nesting four deep with at most three children per container.
    ///
    /// Small on purpose. A property suite's job is to cover the *shape space*, and a
    /// counterexample the reader can hold in their head is worth more than a deep one; the
    /// boundary cases that need the real limits are constructed directly, at the limit,
    /// rather than hoped for from a generator.
    #[must_use]
    pub const fn small() -> Self {
        Self::new(4, 3)
    }

    /// A scalar.
    fn scalar(&self, rng: &mut Prng) -> Value {
        match rng.below(9) {
            0 => Value::Null,
            1 => Value::Bool(rng.flip()),
            2 => Value::nat(*rng.pick(&NATS)),
            3 => Value::int(*rng.pick(&INTS)),
            4 => {
                let width = *rng.pick(&WIDTHS);
                let mask = if width >= MAX_BITVEC_WIDTH {
                    u128::MAX
                } else {
                    (1u128 << width) - 1
                };
                let bits = (u128::from(rng.next_u64()) | (u128::from(rng.next_u64()) << 64)) & mask;
                Value::BitVec(BitVec::new(width, bits).expect("the bits were masked to the width"))
            }
            5 => Value::bytes(rng.pick(&BLOBS).to_vec()),
            6 => Value::text(*rng.pick(&TEXTS)),
            7 => Value::Symbol(name(rng.pick(&NAMES))),
            _ => Value::opaque(name(rng.pick(&NAMES)), rng.pick(&BLOBS).to_vec()),
        }
    }

    /// A value nesting at most `budget` deep.
    fn value(&self, rng: &mut Prng, budget: usize) -> Value {
        if budget <= 1 {
            return self.scalar(rng);
        }
        // Scalars stay likely at every level: a corpus that is all containers never reaches
        // the scalar boundaries the alphabets above exist for.
        match rng.below(14) {
            0..=6 => self.scalar(rng),
            7 => Value::tuple(self.children(rng, budget)).expect("bounded by the budget"),
            8 => Value::seq(self.children(rng, budget)).expect("bounded by the budget"),
            9 => Value::set(self.children(rng, budget)).expect("bounded by the budget"),
            10 => {
                let entries: Vec<(Value, u64)> = dedup(self.children(rng, budget))
                    .into_iter()
                    .map(|element| (element, 1 + rng.next_u64() % 4))
                    .collect();
                Value::multiset(entries).expect("de-duplicated, non-zero, bounded")
            }
            11 => {
                let mut fields: Vec<(Name, Value)> = Vec::new();
                let mut used: Vec<Name> = Vec::new();
                for child in self.children(rng, budget) {
                    let field = name(rng.pick(&NAMES));
                    if used.contains(&field) {
                        continue;
                    }
                    used.push(field.clone());
                    fields.push((field, child));
                }
                Value::record(fields).expect("de-duplicated and bounded")
            }
            12 => Value::variant(
                name(rng.pick(&NAMES)),
                self.value(rng, budget.saturating_sub(1)),
            )
            .expect("bounded by the budget"),
            _ => {
                let keys = dedup(self.children(rng, budget));
                let entries: Vec<(Value, Value)> = keys
                    .into_iter()
                    .map(|key| {
                        let value = self.value(rng, budget.saturating_sub(1));
                        (key, value)
                    })
                    .collect();
                Value::map(entries).expect("de-duplicated and bounded")
            }
        }
    }

    /// Between zero and `max_width` children, each one level shallower.
    ///
    /// Linear in the width by construction: one `push` per child, no doubling anywhere.
    fn children(&self, rng: &mut Prng, budget: usize) -> Vec<Value> {
        let width = rng.between(0, self.max_width);
        let mut out = Vec::with_capacity(width);
        for _ in 0..width {
            out.push(self.value(rng, budget - 1));
        }
        out
    }
}

impl Domain for Values {
    type Item = ValueCase;

    fn generate(&self, rng: &mut Prng) -> ValueCase {
        ValueCase(self.value(rng, self.max_depth))
    }

    fn shrink(&self, item: &ValueCase) -> Vec<ValueCase> {
        shrink_value(&item.0).into_iter().map(ValueCase).collect()
    }

    fn size(&self, item: &ValueCase) -> usize {
        item.0.encode().len()
    }
}

// --- shrinking ------------------------------------------------------------------------

/// Candidate smaller values, most aggressive first.
///
/// The order is the strategy: `Null` first, so any value that refutes a law for structural
/// reasons collapses in one step; then each child on its own, so a container narrows to the
/// part that matters; then the container minus one element; then payload reductions for the
/// scalars.
fn shrink_value(value: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if !matches!(value, Value::Null) {
        out.push(Value::Null);
    }
    match value {
        Value::Null => {}
        Value::Bool(true) => out.push(Value::Bool(false)),
        Value::Bool(false) => {}
        Value::Nat(n) => {
            if *n != 0 {
                out.push(Value::nat(0));
                out.push(Value::nat(n / 2));
                out.push(Value::nat(n - 1));
            }
        }
        Value::Int(n) => {
            if *n != 0 {
                out.push(Value::int(0));
                out.push(Value::int(n / 2));
                if *n > i128::MIN {
                    out.push(Value::int(-n));
                }
            }
        }
        Value::BitVec(bits) => {
            if bits.bits() != 0 {
                if let Ok(zeroed) = BitVec::new(bits.width(), 0) {
                    out.push(Value::BitVec(zeroed));
                }
            }
            if bits.width() > 1 {
                if let Ok(narrow) = BitVec::new(1, 0) {
                    out.push(Value::BitVec(narrow));
                }
            }
        }
        Value::Bytes(bytes) => {
            if !bytes.is_empty() {
                out.push(Value::bytes(Vec::new()));
                out.push(Value::bytes(bytes[..bytes.len() / 2].to_vec()));
                out.push(Value::bytes(bytes[..bytes.len() - 1].to_vec()));
            }
        }
        Value::Text(text) => {
            if !text.is_empty() {
                out.push(Value::text(""));
                let count = text.chars().count();
                out.push(Value::text(
                    text.chars().take(count / 2).collect::<String>(),
                ));
                out.push(Value::text(
                    text.chars().take(count - 1).collect::<String>(),
                ));
            }
        }
        Value::Symbol(symbol) => {
            for shorter in shorter_names(symbol) {
                out.push(Value::Symbol(shorter));
            }
        }
        Value::Opaque(opaque) => {
            if !opaque.bytes().is_empty() {
                out.push(Value::opaque(opaque.domain().clone(), Vec::new()));
            }
            for shorter in shorter_names(opaque.domain()) {
                out.push(Value::opaque(shorter, opaque.bytes().to_vec()));
            }
        }
        Value::Tuple(items) | Value::Seq(items) => {
            out.extend(items.iter().cloned());
            let rebuild = |items: Vec<Value>| {
                if matches!(value, Value::Tuple(_)) {
                    Value::tuple(items)
                } else {
                    Value::seq(items)
                }
            };
            for index in 0..items.len() {
                let mut kept = items.clone();
                kept.remove(index);
                if let Ok(built) = rebuild(kept) {
                    out.push(built);
                }
            }
        }
        Value::Set(elements) => {
            out.extend(elements.iter().cloned());
            for skipped in 0..elements.len() {
                let kept: Vec<Value> = elements
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != skipped)
                    .map(|(_, element)| element.clone())
                    .collect();
                if let Ok(built) = Value::set(kept) {
                    out.push(built);
                }
            }
        }
        Value::Multiset(entries) => {
            out.extend(entries.keys().cloned());
            for skipped in 0..entries.len() {
                let kept: Vec<(Value, u64)> = entries
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != skipped)
                    .map(|(_, (element, count))| (element.clone(), count.get()))
                    .collect();
                if let Ok(built) = Value::multiset(kept) {
                    out.push(built);
                }
            }
            // A multiplicity above one is a payload to reduce as well as a structure.
            let flattened: Vec<(Value, u64)> =
                entries.keys().map(|element| (element.clone(), 1)).collect();
            if let Ok(built) = Value::multiset(flattened) {
                out.push(built);
            }
        }
        Value::Record(fields) => {
            out.extend(fields.values().cloned());
            for skipped in 0..fields.len() {
                let kept: Vec<(Name, Value)> = fields
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != skipped)
                    .map(|(_, (field, child))| (field.clone(), child.clone()))
                    .collect();
                if let Ok(built) = Value::record(kept) {
                    out.push(built);
                }
            }
        }
        Value::Variant { tag, payload } => {
            out.push(payload.as_ref().clone());
            for shorter in shorter_names(tag) {
                if let Ok(built) = Value::variant(shorter, payload.as_ref().clone()) {
                    out.push(built);
                }
            }
        }
        Value::Map(entries) => {
            out.extend(entries.keys().cloned());
            out.extend(entries.values().cloned());
            for skipped in 0..entries.len() {
                let kept: Vec<(Value, Value)> = entries
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != skipped)
                    .map(|(_, (key, child))| (key.clone(), child.clone()))
                    .collect();
                if let Ok(built) = Value::map(kept) {
                    out.push(built);
                }
            }
        }
    }
    out
}

/// Shorter spellings of a name: the first character alone, and `a` — the shortest legal
/// name there is.
fn shorter_names(current: &Name) -> Vec<Name> {
    let text = current.as_str();
    let mut out = Vec::new();
    if text != "a" {
        out.push(name("a"));
    }
    if text.len() > 1 {
        if let Some(first) = text.chars().next() {
            if let Ok(short) = Name::new(&first.to_string()) {
                out.push(short);
            }
        }
    }
    out
}

// --- construction helpers -------------------------------------------------------------

/// A name from a spelling the alphabets above guarantee is legal.
///
/// # Panics
///
/// When the spelling is not printable ASCII, which would be a bug in an alphabet rather
/// than a case a property is about.
#[must_use]
pub fn name(text: &str) -> Name {
    Name::new(text).expect("the generator's alphabets are printable ASCII")
}

/// Values with duplicates removed, keeping the first of each.
fn dedup(values: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::with_capacity(values.len());
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

// --- linear adversarial constructions -------------------------------------------------

/// A value nesting exactly `depth` deep, built by counting to it.
///
/// One `Value::seq` per level, `depth - 1` times — linear in `depth`, never doubling. The
/// workspace has an OOM in its history from a test that grew its input by repeated
/// doubling; a bound is reached by counting to it.
///
/// # Panics
///
/// When `depth` is zero or above [`MAX_DEPTH`], which the crate would refuse to build.
#[must_use]
pub fn nested(depth: usize) -> Value {
    assert!(
        (1..=MAX_DEPTH).contains(&depth),
        "the encoding admits depths 1..={MAX_DEPTH}"
    );
    let mut value = Value::Null;
    for _ in 1..depth {
        value = Value::seq([value]).expect("each level stays inside the bound");
    }
    value
}

/// A set of exactly `width` distinct naturals, built by counting to it.
///
/// # Panics
///
/// Never for a `width` a `u128` can count to; the constructor's depth bound cannot be
/// reached by a set of scalars.
#[must_use]
pub fn wide_set(width: usize) -> Value {
    let mut elements = Vec::with_capacity(width);
    for index in 0..width {
        elements.push(Value::nat(u128::try_from(index).unwrap_or(0)));
    }
    Value::set(elements).expect("a set of scalars nests two deep")
}
