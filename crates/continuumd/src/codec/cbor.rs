//! `canonical_cbor`: the value model, the one writer, and the strict reader.
//!
//! # This is the IDL's CBOR, not the CBOR specification's
//!
//! RFC 0026 fixes two encodings and requires them to carry "identical canonical field
//! order". `rule encoding.canonical_form` says what that order is, and says outright that
//! it is *not* the one the CBOR specification (STD 94) fixes for its own deterministic
//! encoding:
//!
//! > A canonical-CBOR encoder therefore orders map keys by field name in code-point order
//! > and MUST NOT use the length-first map ordering the CBOR specification's own
//! > deterministic-encoding section fixes: this protocol fixes one order for both
//! > encodings, and the IDL is the source of it.
//! >
//! > — `rule encoding.canonical_form`
//!
//! So this module implements a *subset* of CBOR that is deterministic in that
//! specification's sense everywhere the two agree, and follows the IDL where they do not.
//! The whole divergence is one row, and it is stated here rather than left to be
//! discovered:
//!
//! | Question | CBOR core deterministic encoding | This protocol | Who wins |
//! |---|---|---|---|
//! | map key order | bytewise lexicographic over the *encoded* key, which for text keys sorts shorter keys first whatever they contain (`"b"` before `"aa"`) | ascending Unicode code-point order of the key itself (`"aa"` before `"b"`) | **the IDL** |
//! | definite vs indefinite lengths | definite only | definite only | agree |
//! | integer head | shortest form that carries the value | shortest form | agree |
//! | string/array/map length head | shortest form | shortest form | agree |
//! | floating point | shortest form, canonical NaN | **absent** — the protocol declares no float, so a float of any width is malformed | agree, vacuously |
//! | tags | permitted, deterministically encoded | **absent** — no tag is load-bearing in this protocol, so a tag is malformed | narrower here |
//! | major type 1 (negative) | permitted | **absent** — the protocol declares no signed integer | narrower here |
//! | simple values other than 20/21/22 | permitted | **absent** — `undefined` in particular is not a named absence and `null` already is one | narrower here |
//!
//! Every "narrower here" row is a rejection rather than an omission: a reader that
//! *ignored* a tag would accept a second byte spelling of a message that already has one,
//! which is the property `rule encoding.canonical_form` exists to establish.
//!
//! # Why the subset is *derived* rather than invented, and why the IDL needed no edit
//!
//! The rule's first sentence is the whole specification of the subset:
//!
//! > Both encodings are canonical: one message has exactly one byte spelling under the
//! > negotiated encoding, and re-encoding a decoded message MUST reproduce it byte for
//! > byte.
//!
//! CBOR gives a writer more freedom than JSON does, and *every* one of those freedoms is a
//! second byte spelling of a message that already has one. So each rule below is a
//! consequence of that sentence rather than a decision this module took, and
//! `the_canonical_subset_is_derived_from_one_spelling_not_chosen` in
//! `tests/codec_canonical_cbor.rs` proves each derivation the only way it can be proved:
//! by exhibiting the rejected spelling *and* the accepted one and showing they denote the
//! same message.
//!
//! | Rule | The second spelling it excludes |
//! |---|---|
//! | definite lengths only | `9f 01 ff` and `81 01` are both the array `[1]` |
//! | shortest head | `18 01` and `01` are both the integer `1` |
//! | no tags | `c0 61 61` and `61 61` both denote the text `"a"` to a reader that ignores tags |
//! | code-point key order | `{"b":…,"aa":…}` written either way is one message |
//! | no duplicate keys | a map with one key twice has two meanings, not one spelling |
//! | text keys only | a struct's keys are field names, and the IDL's only map is `map<String, T>`; an integer key denotes no declared field |
//! | no `undefined`, no other simple value | the protocol declares no such value, so it denotes no message at all |
//! | no negative integers, no floats | the protocol declares neither type (`rule encoding.canonical_form`, "no floating-point value appears anywhere in this protocol") |
//!
//! Two of those rows are also what the CBOR specification's own deterministic encoding
//! requires, and the coincidence is not an argument: the derivation stands on the IDL's
//! sentence, which is why the *disagreeing* row (key order) comes out the IDL's way
//! without any special pleading.
//!
//! # The leaf spellings the IDL fixes for CBOR, and where they differ from JSON
//!
//! Two of the nine scalars are spelled differently in the two encodings, and both
//! differences are the IDL's, quoted from §3:
//!
//! - **`Bytes`** — "JSON encodes it base64url without padding; CBOR uses a byte string".
//!   So a `Bytes` field is major type 2 here and a text string there, and a *text* string
//!   offered to a `Bytes` field in CBOR is rejected rather than base64-decoded: that would
//!   be a second spelling.
//! - **`U64`** — "JSON encodes it as a number only when it is exactly representable;
//!   otherwise as a decimal string. CBOR uses an unsigned integer". CBOR carries the whole
//!   `u64` range in major type 0, so there is one spelling per value and no boundary at
//!   `2^53 − 1` at all. A decimal *string* offered to a `U64` field in CBOR is rejected
//!   for the same reason.
//!
//! Both differences live in [`Document`](super::Document)'s implementation for this type
//! rather than in the type layer, which is what lets one `ProtocolValue` implementation
//! serve both encodings: the *declaration* says `Bytes`, and each encoding spells it.
//!
//! `Bool` is the third scalar the IDL mentions CBOR for — "CBOR major type 7 simple values
//! 20/21" — and that is what [`Cbor::Bool`] writes.
//!
//! # Reader rejections, against their `canonical_json` twins
//!
//! `rule encoding.canonical_form` binds both encodings equally, so every rejection the
//! JSON reader makes has a twin here unless the construct does not exist in CBOR. The
//! table is in [`CborError`].
//!
//! # Allocation is bounded by the input, not by the input's claims
//!
//! A CBOR head declares a length before its content, which JSON never does, so a reader
//! that trusted the declaration would let nine bytes ask for a `2^64`-element allocation.
//! Nothing here reserves capacity from a declared length: strings are sliced out of the
//! input before they are copied, and arrays and maps grow as their items are read. A
//! declared length larger than the remaining input therefore costs one bounds check and
//! becomes [`CborError::Truncated`].

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

/// How deep a canonical document may nest.
///
/// The same bound as [`super::json::MAX_DEPTH`], and for the same reason: a bound checked
/// before recursing turns an adversarial document into a typed error instead of a stack
/// overflow. One bound rather than two because `rule encoding.canonical_form` binds the
/// two encodings to one shape, and a message that is too deep in one is too deep in both.
pub const MAX_DEPTH: usize = super::json::MAX_DEPTH;

// The eight major types, named. Only six appear in a conforming message.
const MAJOR_UNSIGNED: u8 = 0;
const MAJOR_NEGATIVE: u8 = 1;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const MAJOR_TAG: u8 = 6;
const MAJOR_SIMPLE: u8 = 7;

// The three major-7 values this subset admits. `undefined` (23) is deliberately not one:
// `null` is the protocol's *named* absence (INV-007) and a second one would be a second
// spelling of it.
const SIMPLE_FALSE: u8 = 20;
const SIMPLE_TRUE: u8 = 21;
const SIMPLE_NULL: u8 = 22;

/// A CBOR document restricted to what `rule encoding.canonical_form` admits.
///
/// The restrictions mirror [`super::json::Json`]'s one for one — no float variant, no
/// signed integer, no tag, and a map that is a [`BTreeMap`] so it has exactly one key
/// order and that order is the canonical one — with the two leaves where the IDL spells
/// CBOR differently made explicit: [`Bytes`](Self::Bytes) is a byte string rather than a
/// base64url text, and [`Unsigned`](Self::Unsigned) carries the whole `u64` range rather
/// than stopping where a JSON number does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cbor {
    /// The `null` simple value: a *named* absence (INV-007), never an omission.
    Null,
    /// A `false`/`true` simple value.
    Bool(bool),
    /// An unsigned integer, major type 0, over the whole `u64` range.
    Unsigned(u64),
    /// A UTF-8 text string, major type 3.
    Text(String),
    /// A byte string, major type 2 — the IDL's CBOR spelling of `Bytes`.
    Bytes(Vec<u8>),
    /// An array, major type 4. Order is significant and is preserved verbatim.
    Array(Vec<Cbor>),
    /// A map, major type 5, in canonical key order by construction. Keys are text
    /// strings: the IDL declares `map<String, T>` and nothing else.
    Map(BTreeMap<String, Cbor>),
}

impl Cbor {
    /// Build a map from `(key, value)` pairs, rejecting a repeated key.
    ///
    /// # Errors
    ///
    /// [`CborError::DuplicateKey`] when two pairs share a key: a document with two values
    /// for one key has two meanings, which is what a canonical form exists to prevent.
    /// The mirror of [`super::json::Json::object`].
    pub fn map(fields: impl IntoIterator<Item = (String, Self)>) -> Result<Self, CborError> {
        let mut map = BTreeMap::new();
        for (key, value) in fields {
            match map.entry(key) {
                Entry::Occupied(occupied) => {
                    return Err(CborError::DuplicateKey {
                        key: occupied.key().clone(),
                    });
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(value);
                }
            }
        }
        Ok(Self::Map(map))
    }

    /// The canonical encoding of this document.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_canonical(&mut out);
        out
    }

    /// Append the canonical encoding of this document to `out`.
    pub fn write_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::Null => out.push((MAJOR_SIMPLE << 5) | SIMPLE_NULL),
            Self::Bool(false) => out.push((MAJOR_SIMPLE << 5) | SIMPLE_FALSE),
            Self::Bool(true) => out.push((MAJOR_SIMPLE << 5) | SIMPLE_TRUE),
            Self::Unsigned(value) => write_head(MAJOR_UNSIGNED, *value, out),
            Self::Text(text) => {
                write_head(MAJOR_TEXT, length(text.len()), out);
                out.extend_from_slice(text.as_bytes());
            }
            Self::Bytes(bytes) => {
                write_head(MAJOR_BYTES, length(bytes.len()), out);
                out.extend_from_slice(bytes);
            }
            Self::Array(items) => {
                write_head(MAJOR_ARRAY, length(items.len()), out);
                for item in items {
                    item.write_canonical(out);
                }
            }
            Self::Map(fields) => {
                // `BTreeMap` iterates in ascending key order and `str` compares
                // byte-lexicographically over UTF-8, which is order-isomorphic to
                // code-point order. That is the IDL's order, written directly — there is
                // no second sort here and no length-first comparison anywhere.
                write_head(MAJOR_MAP, length(fields.len()), out);
                for (key, value) in fields {
                    write_head(MAJOR_TEXT, length(key.len()), out);
                    out.extend_from_slice(key.as_bytes());
                    value.write_canonical(out);
                }
            }
        }
    }

    /// Parse a canonical document, rejecting everything the canonical form excludes.
    ///
    /// Unlike the JSON reader this one is strict about *everything*: CBOR gives each value
    /// one spelling to begin with, so there is no escape layer to be liberal about, and
    /// every remaining freedom the format allows — an indefinite length, a longer-than-
    /// necessary head, a tag, a float — is a second spelling this reader refuses.
    ///
    /// # Errors
    ///
    /// [`CborError`] for a duplicate key, keys out of order, a non-text key, trailing
    /// bytes, excessive depth, an indefinite length, a non-shortest head, a negative
    /// integer, a float, a tag, a simple value other than `false`/`true`/`null`, a
    /// reserved additional-information value, invalid UTF-8, or truncation.
    pub fn parse(bytes: &[u8]) -> Result<Self, CborError> {
        let mut reader = Reader { bytes, at: 0 };
        let value = reader.value(0)?;
        if reader.at != bytes.len() {
            return Err(CborError::TrailingBytes);
        }
        Ok(value)
    }

    /// The map's entries, when this is a map.
    #[must_use]
    pub const fn as_map(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Map(fields) => Some(fields),
            _ => None,
        }
    }

    /// The array's items, when this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The text, when this is a text string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The bytes, when this is a byte string.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Whether this is the `null` simple value.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The name of this value's kind, for a typed mismatch report.
    ///
    /// The four kinds CBOR shares with JSON carry the *same* names, so a
    /// [`TypeMismatch`](super::CodecError::TypeMismatch) reads identically whichever
    /// encoding produced it; the two that exist only here are named for their major type.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Unsigned(_) => "integer",
            Self::Text(_) => "string",
            Self::Bytes(_) => "byte string",
            Self::Array(_) => "array",
            Self::Map(_) => "object",
        }
    }
}

/// A slice length as the head's argument.
///
/// Infallible on every target this workspace builds for: `usize` is at most 64 bits, so
/// the conversion cannot fail, and stating that here keeps the writer free of casts.
fn length(count: usize) -> u64 {
    u64::try_from(count).expect("a slice length fits in a u64 on every supported target")
}

/// Write a head in the shortest form that carries `value`.
///
/// This is the CBOR specification's "preferred serialization", and the two agree on it:
/// an argument below 24 rides in the initial byte, and otherwise the narrowest of the
/// one-, two-, four-, and eight-byte forms is used. A longer form is a second spelling of
/// one value, which is exactly the JSON reader's leading-zero rejection in another
/// alphabet.
fn write_head(major: u8, value: u64, out: &mut Vec<u8>) {
    let base = major << 5;
    if let Ok(byte) = u8::try_from(value) {
        if byte < 24 {
            out.push(base | byte);
        } else {
            out.push(base | 24);
            out.push(byte);
        }
        return;
    }
    if let Ok(short) = u16::try_from(value) {
        out.push(base | 25);
        out.extend_from_slice(&short.to_be_bytes());
        return;
    }
    if let Ok(word) = u32::try_from(value) {
        out.push(base | 26);
        out.extend_from_slice(&word.to_be_bytes());
        return;
    }
    out.push(base | 27);
    out.extend_from_slice(&value.to_be_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn peek(&self) -> Result<u8, CborError> {
        self.bytes.get(self.at).copied().ok_or(CborError::Truncated)
    }

    fn bump(&mut self) -> Result<u8, CborError> {
        let byte = self.peek()?;
        self.at += 1;
        Ok(byte)
    }

    /// The next `count` bytes, without allocating: a declared length longer than the
    /// input costs a bounds check and nothing else.
    fn take(&mut self, count: u64) -> Result<&'a [u8], CborError> {
        let count = usize::try_from(count).map_err(|_| CborError::Truncated)?;
        let end = self.at.checked_add(count).ok_or(CborError::Truncated)?;
        let slice = self.bytes.get(self.at..end).ok_or(CborError::Truncated)?;
        self.at = end;
        Ok(slice)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CborError> {
        let raw = self.take(length(N))?;
        <[u8; N]>::try_from(raw).map_err(|_| CborError::Truncated)
    }

    /// The major type and the argument of the next head, shortest form enforced.
    fn head(&mut self) -> Result<(u8, u64), CborError> {
        let at = self.at;
        let initial = self.bump()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        let value = match info {
            0..=23 => u64::from(info),
            24 => {
                let value = u64::from(self.bump()?);
                if value < 24 {
                    return Err(CborError::NonShortestHead { at });
                }
                value
            }
            25 => {
                let value = u64::from(u16::from_be_bytes(self.fixed::<2>()?));
                if value <= u64::from(u8::MAX) {
                    return Err(CborError::NonShortestHead { at });
                }
                value
            }
            26 => {
                let value = u64::from(u32::from_be_bytes(self.fixed::<4>()?));
                if value <= u64::from(u16::MAX) {
                    return Err(CborError::NonShortestHead { at });
                }
                value
            }
            27 => {
                let value = u64::from_be_bytes(self.fixed::<8>()?);
                if value <= u64::from(u32::MAX) {
                    return Err(CborError::NonShortestHead { at });
                }
                value
            }
            28..=30 => return Err(CborError::Reserved { at }),
            _ => return Err(CborError::IndefiniteLength { at }),
        };
        Ok((major, value))
    }

    fn value(&mut self, depth: usize) -> Result<Cbor, CborError> {
        if depth > MAX_DEPTH {
            return Err(CborError::TooDeep);
        }
        let at = self.at;
        if self.peek()? >> 5 == MAJOR_SIMPLE {
            return self.simple();
        }
        let (major, argument) = self.head()?;
        match major {
            MAJOR_UNSIGNED => Ok(Cbor::Unsigned(argument)),
            MAJOR_NEGATIVE => Err(CborError::NegativeInteger { at }),
            MAJOR_BYTES => Ok(Cbor::Bytes(self.take(argument)?.to_vec())),
            MAJOR_TEXT => Ok(Cbor::Text(self.text(argument, at)?)),
            MAJOR_ARRAY => self.array(argument, depth),
            MAJOR_MAP => self.map(argument, depth),
            // No tag is load-bearing in this protocol: a tagged value is a second
            // spelling of the value it tags.
            MAJOR_TAG => Err(CborError::Tag { at }),
            // Major type 7 is answered above, before the head is read, and a major type
            // is three bits, so nothing else exists. The arm is total rather than
            // `unreachable!` because a panic is not an answer a reader owes a caller.
            _ => Err(CborError::Reserved { at }),
        }
    }

    fn text(&mut self, argument: u64, at: usize) -> Result<String, CborError> {
        let raw = self.take(argument)?;
        core::str::from_utf8(raw)
            .map(ToOwned::to_owned)
            .map_err(|_| CborError::NotUtf8 { at })
    }

    fn array(&mut self, count: u64, depth: usize) -> Result<Cbor, CborError> {
        // No capacity is reserved from `count`: every item consumes at least one byte, so
        // an inflated count becomes `Truncated` after reading what is actually there.
        let mut items = Vec::new();
        let mut remaining = count;
        while remaining > 0 {
            remaining -= 1;
            items.push(self.value(depth + 1)?);
        }
        Ok(Cbor::Array(items))
    }

    fn map(&mut self, count: u64, depth: usize) -> Result<Cbor, CborError> {
        let mut fields: BTreeMap<String, Cbor> = BTreeMap::new();
        let mut previous: Option<String> = None;
        let mut remaining = count;
        while remaining > 0 {
            remaining -= 1;
            let at = self.at;
            // A key is a text string or the message is not one this protocol declares:
            // the IDL's only map type is `map<String, T>`, and every struct key is a
            // field name. Checked before the head is consumed so that an integer key is
            // named as such rather than decoded and then rejected.
            if self.peek()? >> 5 != MAJOR_TEXT {
                return Err(CborError::NonTextKey { at });
            }
            let (_, argument) = self.head()?;
            let key = self.text(argument, at)?;
            // Canonical key order is checked on the way *in*, not restored on the way
            // out, exactly as the JSON reader checks it: a reader that silently re-sorted
            // would accept a second byte spelling of a document that already has one.
            if previous.as_ref().is_some_and(|last| *last >= key) {
                return Err(CborError::KeyOrder { at });
            }
            previous = Some(key.clone());
            let value = self.value(depth + 1)?;
            match fields.entry(key) {
                Entry::Occupied(occupied) => {
                    return Err(CborError::DuplicateKey {
                        key: occupied.key().clone(),
                    });
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(value);
                }
            }
        }
        Ok(Cbor::Map(fields))
    }

    fn simple(&mut self) -> Result<Cbor, CborError> {
        let at = self.at;
        let initial = self.bump()?;
        match initial & 0x1F {
            SIMPLE_FALSE => Ok(Cbor::Bool(false)),
            SIMPLE_TRUE => Ok(Cbor::Bool(true)),
            SIMPLE_NULL => Ok(Cbor::Null),
            25..=27 => Err(CborError::FloatingPoint { at }),
            28..=30 => Err(CborError::Reserved { at }),
            31 => Err(CborError::IndefiniteLength { at }),
            _ => Err(CborError::SimpleValue { at }),
        }
    }
}

/// Why a document is not canonical CBOR.
///
/// Every variant is a *rejection*, never a repair, for the reason
/// [`JsonError`](super::json::JsonError) gives: a canonical form that repaired input
/// would have two spellings of one document.
///
/// # Against the `canonical_json` twins
///
/// `rule encoding.canonical_form` binds both encodings, so the two readers reject the
/// same *messages*; they differ only where a construct exists in one format and not the
/// other. Every row below is either a twin or a recorded reason there is none.
///
/// | `JsonError` | Twin here | Note |
/// |---|---|---|
/// | `NotUtf8` | [`NotUtf8`](Self::NotUtf8) | JSON rejects the whole document; CBOR rejects the text string, because only major type 3 is UTF-8 |
/// | `Truncated` | [`Truncated`](Self::Truncated) | |
/// | `TrailingBytes` | [`TrailingBytes`](Self::TrailingBytes) | |
/// | `TooDeep` | [`TooDeep`](Self::TooDeep) | one [`MAX_DEPTH`] for both |
/// | `DuplicateKey` | [`DuplicateKey`](Self::DuplicateKey) | |
/// | `KeyOrder` | [`KeyOrder`](Self::KeyOrder) | the IDL's order, not the CBOR specification's |
/// | `Unexpected` | [`Reserved`](Self::Reserved), [`SimpleValue`](Self::SimpleValue), [`Tag`](Self::Tag), [`NonTextKey`](Self::NonTextKey) | "a byte no production admits here" splits by major type in CBOR |
/// | `FloatingPoint` | [`FloatingPoint`](Self::FloatingPoint) | major 7 additional information 25, 26, 27 |
/// | `Signed` | [`NegativeInteger`](Self::NegativeInteger) | major type 1 |
/// | `LeadingZero` | [`NonShortestHead`](Self::NonShortestHead) | the same rule — one spelling per value — in the head rather than in decimal digits |
/// | `IntegerRange` | *none, and the reason is recorded* | JSON's bound at `2^53 − 1` exists because a JSON number beyond it is a value some conforming readers round. A CBOR integer is exact over the whole `u64` range, so there is no second spelling to exclude and no boundary to police |
/// | `RawControl` | *none, and the reason is recorded* | JSON has an escaped and an unescaped spelling of a control character and must pick one. A CBOR text string has no escape layer, so a control character has exactly one spelling already |
/// | `BadEscape` | *none, same reason* | there are no escapes to malform |
///
/// Three rejections have no JSON twin because the construct does not exist there:
/// [`IndefiniteLength`](Self::IndefiniteLength) (JSON's delimiters are its lengths),
/// [`Tag`](Self::Tag), and [`NonTextKey`](Self::NonTextKey) (a JSON key is a string by
/// grammar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborError {
    /// The document ended mid-value, or a declared length exceeds the input.
    Truncated,
    /// Bytes follow the document.
    TrailingBytes,
    /// Nesting exceeds [`MAX_DEPTH`].
    TooDeep,
    /// A map names one key twice.
    DuplicateKey {
        /// The repeated key.
        key: String,
    },
    /// A map's keys are not in ascending code-point order, so the document is a second
    /// byte spelling of one the canonical form already has.
    KeyOrder {
        /// Byte offset.
        at: usize,
    },
    /// A map key is not a text string; the IDL declares `map<String, T>` and nothing else.
    NonTextKey {
        /// Byte offset.
        at: usize,
    },
    /// An indefinite-length string, array, or map, or a stray break stop code. The
    /// canonical subset is definite-length only.
    IndefiniteLength {
        /// Byte offset.
        at: usize,
    },
    /// A head longer than the shortest one that carries its argument, which is a second
    /// spelling of one value.
    NonShortestHead {
        /// Byte offset.
        at: usize,
    },
    /// A negative integer; the protocol declares no signed integer.
    NegativeInteger {
        /// Byte offset.
        at: usize,
    },
    /// A half-, single-, or double-precision float; the protocol declares no float.
    FloatingPoint {
        /// Byte offset.
        at: usize,
    },
    /// A tagged value; no tag is load-bearing in this protocol.
    Tag {
        /// Byte offset.
        at: usize,
    },
    /// A simple value other than `false`, `true`, and `null` — `undefined` included.
    SimpleValue {
        /// Byte offset.
        at: usize,
    },
    /// An additional-information value the CBOR specification reserves (28, 29, 30).
    Reserved {
        /// Byte offset.
        at: usize,
    },
    /// A text string is not UTF-8.
    NotUtf8 {
        /// Byte offset of the string's head.
        at: usize,
    },
}

impl core::fmt::Display for CborError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // No offending byte is interpolated, for `rule envelope.no_prose` and INV-016 —
        // the same discipline `JsonError` keeps. The offset is a position, not content.
        match self {
            Self::Truncated => f.write_str("the document ends inside a value"),
            Self::TrailingBytes => f.write_str("bytes follow the document"),
            Self::TooDeep => f.write_str("the document nests deeper than the declared bound"),
            Self::DuplicateKey { .. } => f.write_str("a map names one key twice"),
            Self::KeyOrder { at } => {
                write!(f, "map keys are out of canonical order at offset {at}")
            }
            Self::NonTextKey { at } => write!(f, "a map key is not a text string at offset {at}"),
            Self::IndefiniteLength { at } => write!(f, "an indefinite length at offset {at}"),
            Self::NonShortestHead { at } => write!(f, "a non-shortest head at offset {at}"),
            Self::NegativeInteger { at } => write!(f, "a negative integer at offset {at}"),
            Self::FloatingPoint { at } => write!(f, "a floating-point number at offset {at}"),
            Self::Tag { at } => write!(f, "a tagged value at offset {at}"),
            Self::SimpleValue { at } => {
                write!(
                    f,
                    "a simple value other than false, true, or null at offset {at}"
                )
            }
            Self::Reserved { at } => write!(f, "a reserved additional value at offset {at}"),
            Self::NotUtf8 { at } => write!(f, "a text string that is not UTF-8 at offset {at}"),
        }
    }
}

impl core::error::Error for CborError {}
