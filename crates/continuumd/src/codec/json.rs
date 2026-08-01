//! `canonical_json`: the value model, the one writer, and the strict reader.
//!
//! # This is the project's canonical-JSON discipline, not a second one
//!
//! RFC 0037's **ID5** is where the rules live, and
//! `crates/continuum-intent/src/canonical_json.rs` is their reference implementation:
//!
//! > **ID5.** Encoding MUST be byte-deterministic across platforms and releases within a
//! > schema version: JSON with object keys sorted ascending by Unicode code point, no
//! > insignificant whitespace, UTF-8 output, integers in shortest decimal form without
//! > sign or leading zeros, no floating-point values, and explicit encoding of empty
//! > sets […].
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Canonical identity"
//!
//! `rule encoding.canonical_form` adopts those rules for the wire, and this module
//! implements them for the wire. It is a separate implementation from the intent crate's
//! and that is deliberate, on that module's own terms: it says of itself that it "is not a
//! general-purpose JSON library. It exists to give the Intent Contract one byte spelling",
//! and its value model is `i64`-integer-only, which cannot represent this protocol's `U64`
//! at all. Reusing it would mean widening a type that another artifact class's identity is
//! computed over — the one change ADR-0013 is most insistent about not making casually.
//!
//! What is shared is the *rule*, and the rules are cited here one by one so that the two
//! implementations can be read against each other:
//!
//! | ID5 clause | How it is met here |
//! |---|---|
//! | keys sorted ascending by Unicode code point | [`Json::Object`] is a [`BTreeMap`], and Rust's `str` ordering is byte-lexicographic over UTF-8, which is order-isomorphic to code-point order. There is no other key order to present — and the *reader* rejects an out-of-order document ([`JsonError::KeyOrder`]) rather than re-sorting it, because silently re-sorting would accept a second byte spelling of a document that already has one. |
//! | no insignificant whitespace | [`Json::write_canonical`] emits none. |
//! | UTF-8 output | the only string sink is a `Vec<u8>` fed from `&str`. |
//! | shortest decimal, no sign, no leading zeros | [`Json::Integer`] is a `u64` rendered by `Display`; the reader rejects a leading zero and a sign. |
//! | no floating-point | there is no float variant, and the reader rejects a fraction or an exponent rather than rounding it. |
//!
//! # Two rules this module adds, both from the IDL rather than from ID5
//!
//! - **`U64` has one spelling per magnitude.** The IDL says a `U64` "encodes as a number
//!   only when it is exactly representable; otherwise as a decimal string", and RFC 0026's
//!   malformed-input list makes a number beyond that range `MalformedRequest`. So
//!   [`MAX_EXACT_INTEGER`] bounds [`Json::Integer`] at parse time, and the *type mapping*
//!   in [`super`] — not the writer — decides which of the two spellings a given value
//!   takes. A writer that chose per value would give one number two spellings depending on
//!   where it was written from.
//! - **Depth is bounded before recursion.** [`MAX_DEPTH`] matches
//!   `continuum_value::value::MAX_DEPTH` and the intent crate's, for the reason both give:
//!   a bound checked before recursing turns an adversarial document into a typed error
//!   instead of a stack overflow.
//!
//! # What is *not* checked here, stated rather than implied
//!
//! ADR-0013 requires identity-bearing strings to be NFC, and nothing in this workspace
//! normalizes or classifies Unicode — `continuum-workspace` deliberately treats the NFC
//! and NFD spellings of one path as two distinct byte strings rather than folding them.
//! This module therefore does not check NFC, and the honest statement of what that costs
//! is narrow: every identity-bearing string this protocol declares a `@pattern` for is
//! ASCII-only (`EpochIdentity` is `^[!-~]+$`, handles are a prefix plus
//! `[A-Za-z0-9_-]+`, `OperationName` is `^[a-z]+\.[a-z_]+$`, `ActorId`, `RequestId` and
//! `AuditCorrelationId` likewise), and every ASCII string is already its own NFC. The
//! strings that could differ are the three the IDL constrains with no pattern —
//! `Commitment`, `PageToken`, and `FileComponent.path` — and for those the obligation is
//! unenforced here and recorded as RFC 0026 F17 rather than claimed.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

/// How deep a canonical document may nest.
///
/// Matches `continuum_value::value::MAX_DEPTH` and
/// `continuum_intent::canonical_json::MAX_DEPTH`. No message this protocol declares comes
/// close: the deepest is a `ResultEnvelope` carrying a `payload` carrying a list of
/// structs, which is five.
pub const MAX_DEPTH: usize = 64;

/// The largest integer a JSON number carries exactly, `2^53 − 1`.
///
/// > `U64` […] JSON encodes it as a number only when it is exactly representable;
/// > otherwise as a decimal string.
/// >
/// > — IDL §3
///
/// A reader that accepted `9007199254740993` as a number would be accepting a value some
/// conforming readers round; RFC 0026's malformed-input list makes that
/// `MalformedRequest`, and [`Json::parse`] is where it becomes one.
pub const MAX_EXACT_INTEGER: u64 = (1 << 53) - 1;

/// A JSON document restricted to what `rule encoding.canonical_form` admits.
///
/// The restrictions are the point: no float variant, because the protocol declares no
/// floating-point type; unsigned integers only, because it declares no signed one; and an
/// object that is a [`BTreeMap`], so it has exactly one key order and that order is the
/// canonical one. Structural equality and canonical-byte equality therefore agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    /// The `null` literal: a *named* absence (INV-007), never an omission.
    Null,
    /// A `true`/`false` literal.
    Bool(bool),
    /// An unsigned integer, at most [`MAX_EXACT_INTEGER`].
    Integer(u64),
    /// A string.
    String(String),
    /// An array. Order is significant and is preserved verbatim.
    Array(Vec<Json>),
    /// An object, in canonical key order by construction.
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// Build an object from `(key, value)` pairs, rejecting a repeated key.
    ///
    /// # Errors
    ///
    /// [`JsonError::DuplicateKey`] when two pairs share a key: a document with two values
    /// for one key has two meanings, which is what a canonical form exists to prevent.
    pub fn object(fields: impl IntoIterator<Item = (String, Self)>) -> Result<Self, JsonError> {
        let mut map = BTreeMap::new();
        for (key, value) in fields {
            match map.entry(key) {
                Entry::Occupied(occupied) => {
                    return Err(JsonError::DuplicateKey {
                        key: occupied.key().clone(),
                    });
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(value);
                }
            }
        }
        Ok(Self::Object(map))
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
            Self::Null => out.extend_from_slice(b"null"),
            Self::Bool(true) => out.extend_from_slice(b"true"),
            Self::Bool(false) => out.extend_from_slice(b"false"),
            Self::Integer(value) => out.extend_from_slice(value.to_string().as_bytes()),
            Self::String(text) => write_string(text, out),
            Self::Array(items) => {
                out.push(b'[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    item.write_canonical(out);
                }
                out.push(b']');
            }
            Self::Object(fields) => {
                out.push(b'{');
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    write_string(key, out);
                    out.push(b':');
                    value.write_canonical(out);
                }
                out.push(b'}');
            }
        }
    }

    /// Parse a canonical document, rejecting everything the canonical form excludes.
    ///
    /// The reader is *liberal about escapes and strict about everything else*, which is
    /// the same split `continuum_intent::canonical_json` takes and for the same reason:
    /// JSON has several legal spellings of one string, the writer emits exactly one of
    /// them, and parse-then-write is therefore a normalizing operation. That is what makes
    /// "re-encoding a decoded message reproduces it byte for byte" a testable claim.
    ///
    /// # Errors
    ///
    /// [`JsonError`] for a duplicate key, trailing bytes, excessive depth, a
    /// floating-point number, a signed or leading-zero integer, an integer beyond
    /// [`MAX_EXACT_INTEGER`], a malformed escape, or invalid UTF-8.
    pub fn parse(bytes: &[u8]) -> Result<Self, JsonError> {
        let text = core::str::from_utf8(bytes).map_err(|_| JsonError::NotUtf8)?;
        let mut parser = Parser {
            chars: text.chars().collect(),
            at: 0,
        };
        let value = parser.value(0)?;
        if parser.at != parser.chars.len() {
            return Err(JsonError::TrailingBytes);
        }
        Ok(value)
    }

    /// The object's fields, when this is an object.
    #[must_use]
    pub const fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(fields) => Some(fields),
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

    /// The string, when this is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// Whether this is the `null` literal.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The name of this value's kind, for a typed mismatch report.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

/// The one string spelling: quote and reverse solidus escaped, the five short escapes,
/// a lowercase-hex `u`-escape for every other C0 control, raw UTF-8 for everything else.
fn write_string(text: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for character in text.chars() {
        match character {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{8}' => out.extend_from_slice(b"\\b"),
            '\u{9}' => out.extend_from_slice(b"\\t"),
            '\u{a}' => out.extend_from_slice(b"\\n"),
            '\u{c}' => out.extend_from_slice(b"\\f"),
            '\u{d}' => out.extend_from_slice(b"\\r"),
            control if (control as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u{:04x}", control as u32).as_bytes());
            }
            other => {
                let mut buffer = [0_u8; 4];
                out.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out.push(b'"');
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek();
        if character.is_some() {
            self.at += 1;
        }
        character
    }

    fn expect(&mut self, character: char) -> Result<(), JsonError> {
        if self.bump() == Some(character) {
            Ok(())
        } else {
            Err(JsonError::Unexpected { at: self.at })
        }
    }

    fn literal(&mut self, text: &str) -> Result<(), JsonError> {
        for character in text.chars() {
            self.expect(character)?;
        }
        Ok(())
    }

    /// A canonical document has no insignificant whitespace, so the parser accepts none.
    /// Rejecting it is what keeps `parse` from admitting a pretty-printed spelling of a
    /// document whose canonical form is the compact one.
    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(JsonError::TooDeep);
        }
        match self.peek().ok_or(JsonError::Truncated)? {
            'n' => {
                self.literal("null")?;
                Ok(Json::Null)
            }
            't' => {
                self.literal("true")?;
                Ok(Json::Bool(true))
            }
            'f' => {
                self.literal("false")?;
                Ok(Json::Bool(false))
            }
            '"' => self.string().map(Json::String),
            '[' => self.array(depth),
            '{' => self.object(depth),
            digit if digit.is_ascii_digit() => self.integer(),
            '-' => Err(JsonError::Signed { at: self.at }),
            _ => Err(JsonError::Unexpected { at: self.at }),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.expect('[')?;
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            match self.bump() {
                Some(',') => {}
                Some(']') => return Ok(Json::Array(items)),
                _ => return Err(JsonError::Unexpected { at: self.at }),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.expect('{')?;
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        if self.peek() == Some('}') {
            self.at += 1;
            return Ok(Json::Object(fields));
        }
        let mut previous: Option<String> = None;
        loop {
            let key = self.string()?;
            // Canonical key order is checked on the way *in*, not restored on the way out.
            // A reader that silently re-sorted would accept a second byte spelling of a
            // document that already has one, which is exactly what `rule
            // encoding.canonical_form` exists to forbid — and it would make
            // `parse(b).to_canonical_bytes() == b` false for inputs it accepted.
            if previous.as_ref().is_some_and(|last| *last >= key) {
                return Err(JsonError::KeyOrder { at: self.at });
            }
            previous = Some(key.clone());
            self.expect(':')?;
            let value = self.value(depth + 1)?;
            match fields.entry(key) {
                Entry::Occupied(occupied) => {
                    return Err(JsonError::DuplicateKey {
                        key: occupied.key().clone(),
                    });
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(value);
                }
            }
            match self.bump() {
                Some(',') => {}
                Some('}') => return Ok(Json::Object(fields)),
                _ => return Err(JsonError::Unexpected { at: self.at }),
            }
        }
    }

    fn integer(&mut self) -> Result<Json, JsonError> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let digits: String = self.chars[start..self.at].iter().collect();
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(JsonError::LeadingZero { at: start });
        }
        if matches!(self.peek(), Some('.' | 'e' | 'E')) {
            return Err(JsonError::FloatingPoint { at: self.at });
        }
        let value: u64 = digits
            .parse()
            .map_err(|_| JsonError::IntegerRange { at: start })?;
        if value > MAX_EXACT_INTEGER {
            return Err(JsonError::IntegerRange { at: start });
        }
        Ok(Json::Integer(value))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect('"')?;
        let mut text = String::new();
        loop {
            match self.bump().ok_or(JsonError::Truncated)? {
                '"' => return Ok(text),
                '\\' => text.push(self.escape()?),
                control if (control as u32) < 0x20 => {
                    return Err(JsonError::RawControl { at: self.at });
                }
                other => text.push(other),
            }
        }
    }

    fn escape(&mut self) -> Result<char, JsonError> {
        match self.bump().ok_or(JsonError::Truncated)? {
            '"' => Ok('"'),
            '\\' => Ok('\\'),
            '/' => Ok('/'),
            'b' => Ok('\u{8}'),
            'f' => Ok('\u{c}'),
            'n' => Ok('\u{a}'),
            'r' => Ok('\u{d}'),
            't' => Ok('\u{9}'),
            'u' => self.unicode_escape(),
            _ => Err(JsonError::BadEscape { at: self.at }),
        }
    }

    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let first = self.hex4()?;
        // A surrogate pair is two escapes; a lone surrogate is not a character and is
        // rejected rather than replaced, because a replacement character is a second
        // spelling of a string that never had one.
        if (0xD800..0xDC00).contains(&first) {
            self.expect('\\')?;
            self.expect('u')?;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err(JsonError::BadEscape { at: self.at });
            }
            let combined = 0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(combined).ok_or(JsonError::BadEscape { at: self.at });
        }
        char::from_u32(first).ok_or(JsonError::BadEscape { at: self.at })
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let digit = self.bump().ok_or(JsonError::Truncated)?;
            let nibble = digit
                .to_digit(16)
                .ok_or(JsonError::BadEscape { at: self.at })?;
            value = value * 16 + nibble;
        }
        Ok(value)
    }
}

/// Why a document is not canonical JSON.
///
/// Every variant is a *rejection*, never a repair: a canonical form that repaired input
/// would have two spellings of one document, which is what it exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The document ended mid-value.
    Truncated,
    /// Bytes follow the document.
    TrailingBytes,
    /// Nesting exceeds [`MAX_DEPTH`].
    TooDeep,
    /// An object names one key twice.
    DuplicateKey {
        /// The repeated key.
        key: String,
    },
    /// An object's keys are not in ascending code-point order, so the document is a
    /// second byte spelling of one the canonical form already has.
    KeyOrder {
        /// Character offset.
        at: usize,
    },
    /// A character no production admits here.
    Unexpected {
        /// Character offset.
        at: usize,
    },
    /// A number carries a fraction or an exponent; the protocol declares no float.
    FloatingPoint {
        /// Character offset.
        at: usize,
    },
    /// A number carries a sign; the protocol declares no signed integer.
    Signed {
        /// Character offset.
        at: usize,
    },
    /// A number carries a leading zero, which is a second spelling of one value.
    LeadingZero {
        /// Character offset.
        at: usize,
    },
    /// A number is beyond [`MAX_EXACT_INTEGER`], where a `U64` must be a decimal string.
    IntegerRange {
        /// Character offset.
        at: usize,
    },
    /// A string carries an unescaped C0 control.
    RawControl {
        /// Character offset.
        at: usize,
    },
    /// A backslash escape no JSON production admits.
    BadEscape {
        /// Character offset.
        at: usize,
    },
}

impl core::fmt::Display for JsonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // No offending byte is interpolated: the text came off the wire, and
        // `rule envelope.no_prose` keeps a typed failure's explanation stable and
        // non-interpolated (INV-016). The offset is a position, not content.
        match self {
            Self::NotUtf8 => f.write_str("the message is not UTF-8"),
            Self::Truncated => f.write_str("the document ends inside a value"),
            Self::TrailingBytes => f.write_str("bytes follow the document"),
            Self::TooDeep => f.write_str("the document nests deeper than the declared bound"),
            Self::DuplicateKey { .. } => f.write_str("an object names one key twice"),
            Self::KeyOrder { at } => {
                write!(f, "object keys are out of canonical order at offset {at}")
            }
            Self::Unexpected { at } => write!(f, "unexpected character at offset {at}"),
            Self::FloatingPoint { at } => {
                write!(f, "a floating-point number at offset {at}")
            }
            Self::Signed { at } => write!(f, "a signed number at offset {at}"),
            Self::LeadingZero { at } => write!(f, "a leading zero at offset {at}"),
            Self::IntegerRange { at } => {
                write!(f, "an integer beyond exact representation at offset {at}")
            }
            Self::RawControl { at } => write!(f, "an unescaped control at offset {at}"),
            Self::BadEscape { at } => write!(f, "a malformed escape at offset {at}"),
        }
    }
}

impl core::error::Error for JsonError {}

// --- base64url, for `Bytes` -------------------------------------------------------

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode `bytes` as base64url without padding, the IDL's JSON spelling of `Bytes`.
#[must_use]
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut buffer = [0_u8; 3];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let packed =
            (u32::from(buffer[0]) << 16) | (u32::from(buffer[1]) << 8) | u32::from(buffer[2]);
        let quantum = [
            ALPHABET[(packed >> 18) as usize & 0x3F],
            ALPHABET[(packed >> 12) as usize & 0x3F],
            ALPHABET[(packed >> 6) as usize & 0x3F],
            ALPHABET[packed as usize & 0x3F],
        ];
        // Without padding, a chunk of 1 byte spells 2 characters and one of 2 spells 3.
        let keep = chunk.len() + 1;
        for character in &quantum[..keep] {
            out.push(char::from(*character));
        }
    }
    out
}

/// Decode base64url without padding.
///
/// # Errors
///
/// [`Base64Error`] for a character outside the alphabet, a length no unpadded encoding
/// produces, or a final quantum whose unused low bits are non-zero — the last is what
/// keeps one byte string from having two spellings.
pub fn from_base64url(text: &str) -> Result<Vec<u8>, Base64Error> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for chunk in text.as_bytes().chunks(4) {
        if chunk.len() == 1 {
            return Err(Base64Error::Length);
        }
        let mut packed = 0_u32;
        for (index, character) in chunk.iter().enumerate() {
            let value = ALPHABET
                .iter()
                .position(|candidate| candidate == character)
                .ok_or(Base64Error::Alphabet)?;
            packed |= (value as u32) << (18 - 6 * index);
        }
        let produced = chunk.len() - 1;
        let bytes = [(packed >> 16) as u8, (packed >> 8) as u8, packed as u8];
        for byte in &bytes[produced..] {
            if *byte != 0 {
                return Err(Base64Error::NonCanonical);
            }
        }
        out.extend_from_slice(&bytes[..produced]);
    }
    Ok(out)
}

/// Why a string is not canonical base64url.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Error {
    /// A character outside the base64url alphabet — padding included, which unpadded
    /// base64url does not use.
    Alphabet,
    /// A length no unpadded encoding produces.
    Length,
    /// The final quantum's unused low bits are non-zero, so this is a second spelling of
    /// a byte string that already has one.
    NonCanonical,
}

impl core::fmt::Display for Base64Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Alphabet => "a character outside the base64url alphabet",
            Self::Length => "a length unpadded base64url does not produce",
            Self::NonCanonical => "a final quantum with non-zero unused bits",
        })
    }
}

impl core::error::Error for Base64Error {}
