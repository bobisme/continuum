//! **Serialization stability** — the property suite for the two canonical wire encodings
//! (`bn-221j`).
//!
//! # The law
//!
//! > A golden vector is a byte sequence, not a shape: the JSON and CBOR vectors for one
//! > exchange MUST decode to the same values, and re-encoding either MUST reproduce it byte
//! > for byte. A vector that only round-trips through a permissive parser is not a golden
//! > vector.
//! >
//! > — RFC 0026, "Golden wire vectors"
//!
//! `tests/codec_canonical_form.rs` and `tests/codec_canonical_cbor.rs` already pin that for
//! five named exchanges, as literal bytes. This file is the other half of the same claim:
//! the same three rules, over a *generated* document space rather than five fixtures, which
//! is what turns "these five vectors are stable" into "the encoder is stable".
//!
//! Four properties, and the last two are the ones a fixture suite structurally cannot make:
//!
//! 1. **purity** — `to_canonical_bytes` is a function of the document. Two encodings of one
//!    document, and of two separately built equal documents, are the same bytes.
//! 2. **fixpoint** — `from_canonical_bytes(to_canonical_bytes(d)) == d`, and re-encoding
//!    reproduces the bytes exactly. Together with purity this is "one document, one
//!    spelling".
//! 3. **order independence** — the bytes do not depend on the order an object's members
//!    were *presented* in. This is the property that makes a canonical form canonical, and
//!    it is invisible to a fixture, because a fixture presents its members once.
//! 4. **cross-encoding agreement** — a schema-guided reader recovers the same values from
//!    the JSON and the CBOR encoding of one document. IDL §3 gives the two encodings two
//!    spellings for exactly two things (`Bytes` and a large `U64`); everything else must
//!    agree structurally, and those two must agree *as values*.
//!
//! # Why the suite writes its own encoder, and what that is for
//!
//! [`write_json`] is a second JSON writer, defect-parameterized. With [`Defect::None`] it is
//! asserted — over the whole generated corpus — to agree byte for byte with the real
//! `Document` encoder ([`positive_the_suites_own_writer_agrees_with_the_real_encoder`]).
//! That agreement is what makes the seeded violations below *minimal perturbations* rather
//! than strawmen: each one is the same writer with a single rule changed, and the suite's
//! own laws, unmodified, catch it. Nothing in `src/` is touched.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | encoding is a pure function of the document, in both encodings | [`positive_encoding_is_a_pure_function_of_the_document`] |
//! | `decode(encode(d)) == d` and re-encoding is byte-identical | [`positive_a_canonical_encoding_is_a_fixpoint`] |
//! | member presentation order does not reach the bytes | [`positive_member_order_does_not_reach_the_bytes`] |
//! | JSON and CBOR carry the same values to a schema-guided reader | [`positive_the_two_encodings_carry_the_same_values`] |
//! | distinct documents have distinct bytes, in both encodings | [`positive_distinct_documents_have_distinct_bytes`] |
//! | the suite's own writer agrees with the real encoder | [`positive_the_suites_own_writer_agrees_with_the_real_encoder`] |
//! | the depth bound and the `U64` spelling boundary hold | [`boundary_the_depth_bound_and_the_u64_spelling_boundary_hold`] |
//! | **the suite can fail**: an object writer that keeps insertion order | [`falsification_an_insertion_ordered_object_writer_is_caught`] |
//! | **the suite can fail**: a `U64` written as a number beyond the exact range | [`falsification_a_u64_written_beyond_the_exact_range_is_caught`] |
//! | both shrunk counterexamples are retained and replay on their own | [`falsification_the_retained_counterexamples_replay_deterministically`] |

#[path = "../../continuum-value/tests/support/property.rs"]
mod property;

use std::collections::BTreeMap;

use continuumd::codec::Document;
use continuumd::codec::cbor::Cbor;
use continuumd::codec::json::{Json, MAX_DEPTH, MAX_EXACT_INTEGER};

use property::{
    Case, Domain, NearPairs, Pair, Pairs, Plan, Prng, Sexp, check, expect_replay_refutes,
};

/// Cases per law.
const CASES: usize = 512;

// --- the document shape --------------------------------------------------------------------

/// Object member names, chosen so canonical (code-point) order is *not* the order they are
/// written in here: `A` sorts before `a`, and `aa` before `b`.
const KEYS: [&str; 5] = ["b", "a", "z", "aa", "A"];

/// Strings that exercise the writer's one escape spelling: the two characters JSON must
/// escape, a C0 control that takes a `u`-escape, a solidus that must *not* be escaped, and
/// a code point above the BMP.
const TEXTS: [&str; 7] = ["", "a", "\"", "\\", "\u{7}", "/", "\u{1f600}"];

/// Byte strings, which JSON spells as base64url and CBOR as a byte string — one of IDL §3's
/// two declared disagreements.
const BLOBS: [&[u8]; 5] = [b"", b"\x00", b"\xff", b"\x00\x01", b"\xfb\xff\xfe"];

/// Unsigned integers on both sides of the exactly-representable boundary — IDL §3's other
/// declared disagreement, and the one place a JSON writer has two spellings to choose
/// between.
const UNSIGNED: [u64; 6] = [
    0,
    1,
    MAX_EXACT_INTEGER - 1,
    MAX_EXACT_INTEGER,
    MAX_EXACT_INTEGER + 1,
    u64::MAX,
];

/// A document, independent of the encoding that will carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shape {
    Null,
    Bool(bool),
    Unsigned(u64),
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<Shape>),
    /// Members **in presentation order**, which is what makes order independence testable:
    /// a `BTreeMap` here would make the property true by construction and prove nothing.
    Map(Vec<(String, Shape)>),
}

impl Shape {
    /// Drop repeated member names, keeping the first — the only normalization, because a
    /// document with one name twice has two meanings and is not what this suite is about.
    fn normalized(self) -> Self {
        match self {
            Self::Array(items) => Self::Array(items.into_iter().map(Self::normalized).collect()),
            Self::Map(entries) => {
                let mut seen: Vec<String> = Vec::with_capacity(entries.len());
                let mut kept = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key.clone());
                    kept.push((key, value.normalized()));
                }
                Self::Map(kept)
            }
            other => other,
        }
    }

    /// The same document with every object's members in canonical order — what a reader
    /// recovers, whatever order the writer was handed.
    fn sorted(&self) -> Self {
        match self {
            Self::Array(items) => Self::Array(items.iter().map(Self::sorted).collect()),
            Self::Map(entries) => {
                let mut sorted: Vec<(String, Self)> = entries
                    .iter()
                    .map(|(key, value)| (key.clone(), value.sorted()))
                    .collect();
                sorted.sort_by(|left, right| left.0.cmp(&right.0));
                Self::Map(sorted)
            }
            other => other.clone(),
        }
    }

    /// The same document with every object's members rotated by `step`.
    fn rotated(&self, step: usize) -> Self {
        match self {
            Self::Array(items) => {
                Self::Array(items.iter().map(|item| item.rotated(step)).collect())
            }
            Self::Map(entries) => {
                let mut rotated: Vec<(String, Self)> = entries
                    .iter()
                    .map(|(key, value)| (key.clone(), value.rotated(step)))
                    .collect();
                let width = rotated.len();
                if width > 0 {
                    rotated.rotate_left(step % width);
                }
                Self::Map(rotated)
            }
            other => other.clone(),
        }
    }

    /// How deeply this document nests, counting a scalar as depth 1.
    fn depth(&self) -> usize {
        match self {
            Self::Array(items) => 1 + items.iter().map(Self::depth).max().unwrap_or(0),
            Self::Map(entries) => {
                1 + entries
                    .iter()
                    .map(|(_, value)| value.depth())
                    .max()
                    .unwrap_or(0)
            }
            _ => 1,
        }
    }

    /// This document in the encoding `D`.
    fn build<D: Document>(&self) -> D {
        match self {
            Self::Null => D::from_null(),
            Self::Bool(value) => D::from_bool(*value),
            Self::Unsigned(value) => D::from_unsigned(*value),
            Self::Text(text) => D::from_text(text),
            Self::Bytes(bytes) => D::from_byte_string(bytes),
            Self::Array(items) => D::from_items(items.iter().map(Self::build).collect()),
            Self::Map(entries) => {
                let mut map: BTreeMap<String, D> = BTreeMap::new();
                for (key, value) in entries {
                    map.entry(key.clone()).or_insert_with(|| value.build());
                }
                D::from_entries(map)
            }
        }
    }
}

/// Read a document back through the `Document` accessors, guided by the shape a schema
/// would have declared.
///
/// This is what a typed reader does, and it is the only honest way to compare the two
/// encodings: JSON spells a `Bytes` as a base64url string and a large `U64` as a decimal
/// string, so a reader that did not know which field it was reading could not recover
/// either. IDL §3 says those are the two disagreements; this function is that statement,
/// executable.
fn read<D: Document>(document: &D, expected: &Shape) -> Result<Shape, String> {
    match expected {
        Shape::Null => {
            if document.is_null() {
                Ok(Shape::Null)
            } else {
                Err(format!("expected null, found {}", document.kind()))
            }
        }
        Shape::Bool(_) => document
            .as_bool()
            .map(Shape::Bool)
            .ok_or_else(|| format!("expected a boolean, found {}", document.kind())),
        Shape::Unsigned(_) => document
            .as_u64("u64")
            .map(Shape::Unsigned)
            .map_err(|error| format!("expected a u64: {error}")),
        Shape::Text(_) => document
            .as_text()
            .map(|text| Shape::Text(text.to_owned()))
            .ok_or_else(|| format!("expected a string, found {}", document.kind())),
        Shape::Bytes(_) => document
            .as_byte_string("bytes")
            .map(Shape::Bytes)
            .map_err(|error| format!("expected a byte string: {error}")),
        Shape::Array(items) => {
            let read_items = document
                .as_items()
                .ok_or_else(|| format!("expected an array, found {}", document.kind()))?;
            if read_items.len() != items.len() {
                return Err(format!(
                    "expected {} items, found {}",
                    items.len(),
                    read_items.len()
                ));
            }
            let mut out = Vec::with_capacity(items.len());
            for (item, expected_item) in read_items.iter().zip(items) {
                out.push(read(item, expected_item)?);
            }
            Ok(Shape::Array(out))
        }
        Shape::Map(entries) => {
            let read_entries = document
                .as_entries()
                .ok_or_else(|| format!("expected a map, found {}", document.kind()))?;
            if read_entries.len() != entries.len() {
                return Err(format!(
                    "expected {} members, found {}",
                    entries.len(),
                    read_entries.len()
                ));
            }
            let mut out = Vec::with_capacity(entries.len());
            for (key, expected_value) in entries {
                let value = read_entries
                    .get(key)
                    .ok_or_else(|| format!("the member {key} is missing"))?;
                out.push((key.clone(), read(value, expected_value)?));
            }
            Ok(Shape::Map(out))
        }
    }
}

// --- the case ------------------------------------------------------------------------------

/// Shape tags in the counterexample text: one ASCII letter each.
const TAG_NULL: u8 = b'n';
const TAG_BOOL: u8 = b'b';
const TAG_UNSIGNED: u8 = b'u';
const TAG_TEXT: u8 = b't';
const TAG_BYTES: u8 = b'y';
const TAG_ARRAY: u8 = b'a';
const TAG_MAP: u8 = b'm';

/// A document, as a property-harness case.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShapeCase(Shape);

impl Case for ShapeCase {
    fn to_sexp(&self) -> Sexp {
        fn render(shape: &Shape) -> Sexp {
            match shape {
                Shape::Null => Sexp::list([Sexp::atom(vec![TAG_NULL])]),
                Shape::Bool(value) => Sexp::list([
                    Sexp::atom(vec![TAG_BOOL]),
                    Sexp::atom(vec![u8::from(*value)]),
                ]),
                Shape::Unsigned(value) => Sexp::list([
                    Sexp::atom(vec![TAG_UNSIGNED]),
                    Sexp::number(u128::from(*value)),
                ]),
                Shape::Text(text) => Sexp::list([Sexp::atom(vec![TAG_TEXT]), Sexp::text(text)]),
                Shape::Bytes(bytes) => {
                    Sexp::list([Sexp::atom(vec![TAG_BYTES]), Sexp::atom(bytes.clone())])
                }
                Shape::Array(items) => {
                    let mut out = vec![Sexp::atom(vec![TAG_ARRAY])];
                    out.extend(items.iter().map(render));
                    Sexp::list(out)
                }
                Shape::Map(entries) => {
                    let mut out = vec![Sexp::atom(vec![TAG_MAP])];
                    out.extend(
                        entries
                            .iter()
                            .map(|(key, value)| Sexp::list([Sexp::text(key), render(value)])),
                    );
                    Sexp::list(out)
                }
            }
        }
        render(&self.0)
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        fn parse(sexp: &Sexp) -> Option<Shape> {
            let items = sexp.as_list()?;
            let [tag, rest @ ..] = items else {
                return None;
            };
            match *tag.as_atom()? {
                [TAG_NULL] if rest.is_empty() => Some(Shape::Null),
                [TAG_BOOL] => Some(Shape::Bool(rest.first()?.as_atom()? == [1u8])),
                [TAG_UNSIGNED] => Some(Shape::Unsigned(
                    u64::try_from(rest.first()?.as_number()?).ok()?,
                )),
                [TAG_TEXT] => Some(Shape::Text(rest.first()?.as_text()?.to_owned())),
                [TAG_BYTES] => Some(Shape::Bytes(rest.first()?.as_atom()?.to_vec())),
                [TAG_ARRAY] => {
                    let mut out = Vec::with_capacity(rest.len());
                    for item in rest {
                        out.push(parse(item)?);
                    }
                    Some(Shape::Array(out))
                }
                [TAG_MAP] => {
                    let mut out = Vec::with_capacity(rest.len());
                    for entry in rest {
                        let [key, value] = entry.as_tuple::<2>()?;
                        out.push((key.as_text()?.to_owned(), parse(value)?));
                    }
                    Some(Shape::Map(out))
                }
                _ => None,
            }
        }
        parse(sexp).map(|shape| Self(shape.normalized()))
    }
}

/// Generated documents.
#[derive(Debug, Clone, Copy)]
struct Shapes {
    max_depth: usize,
    max_width: usize,
}

impl Shapes {
    const fn small() -> Self {
        Self {
            max_depth: 4,
            max_width: 3,
        }
    }

    fn scalar(&self, rng: &mut Prng) -> Shape {
        match rng.below(5) {
            0 => Shape::Null,
            1 => Shape::Bool(rng.flip()),
            2 => Shape::Unsigned(*rng.pick(&UNSIGNED)),
            3 => Shape::Text((*rng.pick(&TEXTS)).to_owned()),
            _ => Shape::Bytes(rng.pick(&BLOBS).to_vec()),
        }
    }

    fn shape(&self, rng: &mut Prng, budget: usize) -> Shape {
        if budget <= 1 {
            return self.scalar(rng);
        }
        match rng.below(6) {
            0..=3 => self.scalar(rng),
            4 => {
                let width = rng.between(0, self.max_width);
                let mut items = Vec::with_capacity(width);
                for _ in 0..width {
                    items.push(self.shape(rng, budget - 1));
                }
                Shape::Array(items)
            }
            _ => {
                let width = rng.between(0, self.max_width);
                let mut entries = Vec::with_capacity(width);
                for _ in 0..width {
                    entries.push(((*rng.pick(&KEYS)).to_owned(), self.shape(rng, budget - 1)));
                }
                Shape::Map(entries)
            }
        }
    }
}

impl Domain for Shapes {
    type Item = ShapeCase;

    fn generate(&self, rng: &mut Prng) -> ShapeCase {
        ShapeCase(self.shape(rng, self.max_depth).normalized())
    }

    fn shrink(&self, item: &ShapeCase) -> Vec<ShapeCase> {
        shrink_shape(&item.0).into_iter().map(ShapeCase).collect()
    }

    fn size(&self, item: &ShapeCase) -> usize {
        fn weigh(shape: &Shape) -> usize {
            match shape {
                Shape::Null => 1,
                Shape::Bool(_) => 2,
                Shape::Unsigned(value) => {
                    2 + usize::try_from(value.checked_ilog10().unwrap_or(0)).unwrap_or(0)
                }
                Shape::Text(text) => 2 + text.len(),
                Shape::Bytes(bytes) => 2 + bytes.len(),
                Shape::Array(items) => 2 + items.iter().map(weigh).sum::<usize>(),
                Shape::Map(entries) => {
                    2 + entries
                        .iter()
                        .map(|(key, value)| key.len() + weigh(value))
                        .sum::<usize>()
                }
            }
        }
        weigh(&item.0)
    }
}

/// Candidate smaller documents, most aggressive first.
///
/// `Null` first, then each child on its own, then the container minus one member, then a
/// member with its own reduction substituted in, then the payload reductions. The
/// substitution step is what lets a counterexample about an *object* also lose the noise
/// inside its members, rather than stopping at the first two-member object the generator
/// happened to draw.
///
/// The `U64` reductions are boundary-directed: `MAX_EXACT_INTEGER` and the magnitude one
/// above it are proposed before halving, because those two are where every rule about
/// spelling a `U64` changes, and halving alone walks past them without stopping.
fn shrink_shape(shape: &Shape) -> Vec<Shape> {
    /// How many reductions of one member are substituted back in. A cap, so the candidate
    /// list stays a bounded amount of work per shrink step.
    const PER_MEMBER: usize = 3;

    let mut out = Vec::new();
    if *shape != Shape::Null {
        out.push(Shape::Null);
    }
    match shape {
        Shape::Null | Shape::Bool(false) => {}
        Shape::Bool(true) => out.push(Shape::Bool(false)),
        Shape::Unsigned(0) => {}
        Shape::Unsigned(value) => {
            out.push(Shape::Unsigned(0));
            out.push(Shape::Unsigned(MAX_EXACT_INTEGER));
            out.push(Shape::Unsigned(MAX_EXACT_INTEGER + 1));
            out.push(Shape::Unsigned(value / 2));
            out.push(Shape::Unsigned(value - 1));
        }
        Shape::Text(text) if !text.is_empty() => out.push(Shape::Text(String::new())),
        Shape::Text(_) => {}
        Shape::Bytes(bytes) if !bytes.is_empty() => {
            out.push(Shape::Bytes(Vec::new()));
            out.push(Shape::Bytes(bytes[..bytes.len() - 1].to_vec()));
        }
        Shape::Bytes(_) => {}
        Shape::Array(items) => {
            out.extend(items.iter().cloned());
            for index in 0..items.len() {
                let mut kept = items.clone();
                kept.remove(index);
                out.push(Shape::Array(kept));
            }
            for index in 0..items.len() {
                for candidate in shrink_shape(&items[index]).into_iter().take(PER_MEMBER) {
                    let mut replaced = items.clone();
                    replaced[index] = candidate;
                    out.push(Shape::Array(replaced));
                }
            }
        }
        Shape::Map(entries) => {
            out.extend(entries.iter().map(|(_, value)| value.clone()));
            for index in 0..entries.len() {
                let mut kept = entries.clone();
                kept.remove(index);
                out.push(Shape::Map(kept));
            }
            for index in 0..entries.len() {
                for candidate in shrink_shape(&entries[index].1).into_iter().take(PER_MEMBER) {
                    let mut replaced = entries.clone();
                    replaced[index].1 = candidate;
                    out.push(Shape::Map(replaced));
                }
            }
            // Rename a member to a smaller key, which drives a two-member object towards
            // one canonical witness instead of whichever pair was drawn.
            for index in 0..entries.len() {
                for key in KEYS {
                    if key >= entries[index].0.as_str() {
                        continue;
                    }
                    let mut renamed = entries.clone();
                    renamed[index].0 = key.to_owned();
                    out.push(Shape::Map(renamed).normalized());
                }
            }
        }
    }
    out
}

// --- the suite's own writer ------------------------------------------------------------------

/// Which single rule this writer breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Defect {
    /// None: byte-identical to the real encoder, asserted over the whole corpus.
    None,
    /// Object members are emitted in presentation order rather than canonical order.
    InsertionOrder,
    /// Every `U64` is emitted as a JSON number, including magnitudes the IDL requires be
    /// spelled as a decimal string.
    NumberBeyondRange,
}

/// A canonical-JSON writer with one rule selectable.
///
/// With [`Defect::None`] this must agree with `<Json as Document>::to_canonical_bytes` on
/// every generated document — which is what makes the two defective settings *one-rule*
/// perturbations rather than unrelated code.
fn write_json(shape: &Shape, defect: Defect, out: &mut Vec<u8>) {
    match shape {
        Shape::Unsigned(value) if defect == Defect::NumberBeyondRange => {
            out.extend_from_slice(value.to_string().as_bytes());
        }
        Shape::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_json(item, defect, out);
            }
            out.push(b']');
        }
        Shape::Map(entries) => {
            let mut ordered: Vec<&(String, Shape)> = entries.iter().collect();
            if defect != Defect::InsertionOrder {
                ordered.sort_by(|left, right| left.0.cmp(&right.0));
            }
            out.push(b'{');
            for (index, (key, value)) in ordered.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(&Json::String(key.clone()).to_canonical_bytes());
                out.push(b':');
                write_json(value, defect, out);
            }
            out.push(b'}');
        }
        // Every scalar goes through the real encoder, so escapes, base64url, and the `U64`
        // spelling rule are the crate's and not this file's.
        other => out.extend_from_slice(&other.build::<Json>().to_canonical_bytes()),
    }
}

fn written(shape: &Shape, defect: Defect) -> Vec<u8> {
    let mut out = Vec::new();
    write_json(shape, defect, &mut out);
    out
}

// --- the laws ---------------------------------------------------------------------------------

fn encoding_is_pure(case: &ShapeCase) -> Result<(), String> {
    encoding_is_pure_in::<Json>(case).and_then(|()| encoding_is_pure_in::<Cbor>(case))
}

fn encoding_is_pure_in<D: Document>(case: &ShapeCase) -> Result<(), String> {
    let document: D = case.0.build();
    let again: D = case.0.build();
    if document != again {
        return Err(format!(
            "{:?}: two builds of one document are not equal",
            D::ENCODING
        ));
    }
    let bytes = document.to_canonical_bytes();
    if bytes != document.to_canonical_bytes() {
        return Err(format!(
            "{:?}: encoding twice gave two byte strings",
            D::ENCODING
        ));
    }
    if bytes != again.to_canonical_bytes() {
        return Err(format!(
            "{:?}: two separately built equal documents encode differently",
            D::ENCODING
        ));
    }
    Ok(())
}

fn a_canonical_encoding_is_a_fixpoint(case: &ShapeCase) -> Result<(), String> {
    fixpoint_in::<Json>(&case.0.build::<Json>().to_canonical_bytes())
        .and_then(|()| fixpoint_in::<Cbor>(&case.0.build::<Cbor>().to_canonical_bytes()))
}

fn fixpoint_in<D: Document>(bytes: &[u8]) -> Result<(), String> {
    let decoded =
        D::from_canonical_bytes(bytes).map_err(|error| format!("{:?}: {error}", D::ENCODING))?;
    if decoded.to_canonical_bytes() != bytes {
        return Err(format!(
            "{:?}: re-encoding a decoded document changed the bytes",
            D::ENCODING
        ));
    }
    let again = D::from_canonical_bytes(&decoded.to_canonical_bytes())
        .map_err(|error| format!("{:?}: {error}", D::ENCODING))?;
    if again != decoded {
        return Err(format!("{:?}: decoding is not a fixpoint", D::ENCODING));
    }
    Ok(())
}

/// Member presentation order does not reach the bytes, over whichever writer is supplied.
fn order_independent_under(
    case: &ShapeCase,
    encode: impl Fn(&Shape) -> Vec<u8>,
) -> Result<(), String> {
    let expected = encode(&case.0);
    // Every rotation up to the widest object, plus the fully sorted presentation. Linear in
    // the width, deterministic, and enough to separate "order does not matter" from
    // "already-sorted input happens to work".
    for step in 0..=case.0.depth().max(1) * Shapes::small().max_width {
        if encode(&case.0.rotated(step)) != expected {
            return Err(format!(
                "rotating the members by {step} changed the bytes: member presentation \
                 order reached the encoding"
            ));
        }
    }
    if encode(&case.0.sorted()) != expected {
        return Err("sorting the members changed the bytes".to_owned());
    }
    Ok(())
}

fn member_order_does_not_reach_the_bytes(case: &ShapeCase) -> Result<(), String> {
    order_independent_under(case, |shape| shape.build::<Json>().to_canonical_bytes())?;
    order_independent_under(case, |shape| shape.build::<Cbor>().to_canonical_bytes())
}

fn the_two_encodings_carry_the_same_values(case: &ShapeCase) -> Result<(), String> {
    let expected = case.0.sorted();
    let from_json = read(&case.0.build::<Json>(), &expected)?;
    let from_cbor = read(&case.0.build::<Cbor>(), &expected)?;
    if from_json != from_cbor {
        return Err("the two encodings carried different values".to_owned());
    }
    if from_json != expected {
        return Err("a schema-guided read did not recover the document".to_owned());
    }
    // And through the bytes, which is the form RFC 0026's golden-vector rule is about.
    let json = Json::from_canonical_bytes(&case.0.build::<Json>().to_canonical_bytes())
        .map_err(|error| format!("canonical JSON did not decode: {error}"))?;
    let cbor = Cbor::from_canonical_bytes(&case.0.build::<Cbor>().to_canonical_bytes())
        .map_err(|error| format!("canonical CBOR did not decode: {error}"))?;
    if read(&json, &expected)? != read(&cbor, &expected)? {
        return Err("the two encodings' bytes carried different values".to_owned());
    }
    Ok(())
}

fn distinct_documents_have_distinct_bytes(pair: &Pair<ShapeCase>) -> Result<(), String> {
    injective_in::<Json>(pair).and_then(|()| injective_in::<Cbor>(pair))
}

fn injective_in<D: Document>(pair: &Pair<ShapeCase>) -> Result<(), String> {
    let left: D = pair.left.0.build();
    let right: D = pair.right.0.build();
    let same_bytes = left.to_canonical_bytes() == right.to_canonical_bytes();
    let same_document = left == right;
    if same_bytes == same_document {
        Ok(())
    } else if same_bytes {
        Err(format!(
            "{:?}: two different documents share one byte string",
            D::ENCODING
        ))
    } else {
        Err(format!(
            "{:?}: one document has two byte strings",
            D::ENCODING
        ))
    }
}

// --- the suite ----------------------------------------------------------------------------------

#[test]
fn positive_encoding_is_a_pure_function_of_the_document() {
    let plan = Plan::new("encoding is pure", 0x2021_0221_0005_0001, CASES);
    check(&plan, &Shapes::small(), encoding_is_pure).expect_held(&plan);
}

#[test]
fn positive_a_canonical_encoding_is_a_fixpoint() {
    let plan = Plan::new("encoding is a fixpoint", 0x2021_0221_0005_0002, CASES);
    check(&plan, &Shapes::small(), a_canonical_encoding_is_a_fixpoint).expect_held(&plan);
}

#[test]
fn positive_member_order_does_not_reach_the_bytes() {
    let plan = Plan::new("order independence", 0x2021_0221_0005_0003, CASES);
    check(
        &plan,
        &Shapes::small(),
        member_order_does_not_reach_the_bytes,
    )
    .expect_held(&plan);
}

#[test]
fn positive_the_two_encodings_carry_the_same_values() {
    let plan = Plan::new("cross-encoding agreement", 0x2021_0221_0005_0004, CASES);
    check(
        &plan,
        &Shapes::small(),
        the_two_encodings_carry_the_same_values,
    )
    .expect_held(&plan);
}

#[test]
fn positive_distinct_documents_have_distinct_bytes() {
    let plan = Plan::new(
        "encoding is injective, adjacent",
        0x2021_0221_0005_0005,
        CASES,
    );
    check(
        &plan,
        &NearPairs(Shapes::small()),
        distinct_documents_have_distinct_bytes,
    )
    .expect_held(&plan);

    let plan = Plan::new(
        "encoding is injective, independent",
        0x2021_0221_0005_0015,
        CASES,
    );
    check(
        &plan,
        &Pairs(Shapes::small()),
        distinct_documents_have_distinct_bytes,
    )
    .expect_held(&plan);
}

#[test]
fn positive_the_suites_own_writer_agrees_with_the_real_encoder() {
    // The premise of both seeded violations below: with no defect selected, this file's
    // writer is the crate's writer, byte for byte, on every drawn document.
    let plan = Plan::new(
        "the suite's writer is the real one",
        0x2021_0221_0005_0006,
        CASES,
    );
    check(&plan, &Shapes::small(), |case: &ShapeCase| {
        let mine = written(&case.0, Defect::None);
        let theirs = case.0.build::<Json>().to_canonical_bytes();
        if mine == theirs {
            Ok(())
        } else {
            Err(format!(
                "the suite's writer disagrees with the encoder: {} vs {}",
                String::from_utf8_lossy(&mine),
                String::from_utf8_lossy(&theirs)
            ))
        }
    })
    .expect_held(&plan);
}

#[test]
fn boundary_the_depth_bound_and_the_u64_spelling_boundary_hold() {
    // Depth: exactly at the bound, built by counting to it — one array per level, never by
    // doubling. Both readers enter at nesting zero and refuse a value nested past
    // `MAX_DEPTH`, so `MAX_DEPTH` enclosing arrays is the deepest admissible document.
    let mut deepest = Shape::Null;
    for _ in 0..MAX_DEPTH {
        deepest = Shape::Array(vec![deepest]);
    }
    assert_eq!(
        deepest.depth(),
        MAX_DEPTH + 1,
        "MAX_DEPTH arrays around a scalar"
    );
    let case = ShapeCase(deepest.clone());
    assert_eq!(encoding_is_pure(&case), Ok(()));
    assert_eq!(a_canonical_encoding_is_a_fixpoint(&case), Ok(()));
    assert_eq!(the_two_encodings_carry_the_same_values(&case), Ok(()));

    // One past the bound is a typed rejection from both readers, not a stack overflow.
    let past = Shape::Array(vec![deepest]);
    assert!(
        Json::from_canonical_bytes(&past.build::<Json>().to_canonical_bytes()).is_err(),
        "canonical JSON must bound its own recursion"
    );
    assert!(
        Cbor::from_canonical_bytes(&past.build::<Cbor>().to_canonical_bytes()).is_err(),
        "canonical CBOR must bound its own recursion"
    );

    // The `U64` spelling boundary: a number up to the exactly-representable maximum, a
    // decimal string above it, and one value on each side that a schema-guided reader
    // recovers identically from both encodings.
    for value in UNSIGNED {
        let json = Shape::Unsigned(value).build::<Json>();
        let expected = if value <= MAX_EXACT_INTEGER {
            Json::Integer(value)
        } else {
            Json::String(value.to_string())
        };
        assert_eq!(json, expected, "the U64 spelling rule at {value}");
        assert_eq!(
            the_two_encodings_carry_the_same_values(&ShapeCase(Shape::Unsigned(value))),
            Ok(()),
            "the two encodings must agree on {value}"
        );
    }
    // CBOR spells every magnitude as an unsigned integer — IDL §3's declared disagreement.
    assert_eq!(
        Shape::Unsigned(u64::MAX).build::<Cbor>(),
        Cbor::Unsigned(u64::MAX)
    );
}

// --- anti-vacuity: the suite can fail --------------------------------------------------------

/// This suite's own order-independence law, over the insertion-ordered writer.
fn order_independent_under_an_insertion_ordered_writer(case: &ShapeCase) -> Result<(), String> {
    order_independent_under(case, |shape| written(shape, Defect::InsertionOrder))
}

/// This suite's own fixpoint law, over the writer that spells every `U64` as a number.
fn fixpoint_under_a_number_beyond_range(case: &ShapeCase) -> Result<(), String> {
    fixpoint_in::<Json>(&written(&case.0, Defect::NumberBeyondRange))
}

/// An object presenting two `null` members as `b` then `a` — the smallest document whose
/// member order is observable at all, in the one presentation order that is not already the
/// canonical one.
const RETAINED_INSERTION_ORDER: &str = "(#6d (#62 (#6e)) (#61 (#6e)))";

/// The smallest `U64` the IDL requires be spelled as a decimal string:
/// `MAX_EXACT_INTEGER + 1`, or 9007199254740992.
const RETAINED_NUMBER_BEYOND_RANGE: &str = "(#75 #39303037313939323534373430393932)";

#[test]
fn falsification_an_insertion_ordered_object_writer_is_caught() {
    let plan = Plan::new(
        "order independence, over a writer that keeps insertion order",
        0x2021_0221_0005_0003,
        CASES,
    );
    let refuted = check(
        &plan,
        &Shapes::small(),
        order_independent_under_an_insertion_ordered_writer,
    )
    .expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_INSERTION_ORDER);
    assert!(
        refuted
            .reason
            .contains("member presentation order reached the encoding"),
        "{}",
        refuted.reason
    );
    // The real encoder is order-independent on the very case that refutes the broken one,
    // and the broken writer is still *pure* and still a *fixpoint* — which is why order
    // independence is a law in its own right.
    assert_eq!(
        member_order_does_not_reach_the_bytes(&refuted.minimal),
        Ok(())
    );
    assert_eq!(encoding_is_pure(&refuted.minimal), Ok(()));

    let again = check(
        &plan,
        &Shapes::small(),
        order_independent_under_an_insertion_ordered_writer,
    )
    .expect_refuted(&plan);
    assert_eq!(again.minimal_repr, refuted.minimal_repr);
    assert_eq!(again.shrink_steps, refuted.shrink_steps);
}

#[test]
fn falsification_a_u64_written_beyond_the_exact_range_is_caught() {
    let plan = Plan::new(
        "fixpoint, over a writer that spells every U64 as a number",
        0x2021_0221_0005_0002,
        CASES,
    );
    let refuted = check(
        &plan,
        &Shapes::small(),
        fixpoint_under_a_number_beyond_range,
    )
    .expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_NUMBER_BEYOND_RANGE);
    assert_eq!(
        refuted.minimal.0,
        Shape::Unsigned(MAX_EXACT_INTEGER + 1),
        "the boundary is the first magnitude the IDL spells as a string"
    );
    // The real encoder round trips exactly this document.
    assert_eq!(a_canonical_encoding_is_a_fixpoint(&refuted.minimal), Ok(()));
    // And the defect is invisible one magnitude lower, which is what makes the boundary the
    // counterexample rather than an arbitrary large number.
    assert_eq!(
        fixpoint_under_a_number_beyond_range(&ShapeCase(Shape::Unsigned(MAX_EXACT_INTEGER))),
        Ok(())
    );
}

#[test]
fn falsification_the_retained_counterexamples_replay_deterministically() {
    let reason = expect_replay_refutes::<ShapeCase, _>(
        RETAINED_INSERTION_ORDER,
        order_independent_under_an_insertion_ordered_writer,
    );
    assert!(
        reason.contains("presentation order reached the encoding"),
        "{reason}"
    );

    let reason = expect_replay_refutes::<ShapeCase, _>(
        RETAINED_NUMBER_BEYOND_RANGE,
        fixpoint_under_a_number_beyond_range,
    );
    assert!(!reason.is_empty(), "the retained case must report a reason");

    for retained in [RETAINED_INSERTION_ORDER, RETAINED_NUMBER_BEYOND_RANGE] {
        let case = ShapeCase::from_repr(retained).expect("the retained case parses");
        assert_eq!(
            case.repr(),
            retained,
            "a retention cannot drift from its case"
        );
        assert_eq!(member_order_does_not_reach_the_bytes(&case), Ok(()));
        assert_eq!(a_canonical_encoding_is_a_fixpoint(&case), Ok(()));
    }
}
