//! The canonical JSON layer: RFC 0037's ID5/N8 byte spelling, and a strict reader
//! for it (PR-4 / IMPL-01).
//!
//! # Which layer owns which spelling
//!
//! Continuum has two canonical encodings, and they are *not* two spellings of one
//! thing. They belong to different artifact classes:
//!
//! - **CVNF-1** (`continuum_value::value`) is the canonical encoding of an exact
//!   finite *value* — a self-describing binary form with a total order. It is what
//!   `continuum_value::identity::ContentIdentity` is.
//! - **This module** is the canonical encoding of an Intent Contract *document*.
//!   RFC 0037 fixes it, and it fixes it as JSON:
//!
//!   > **ID5.** Encoding MUST be byte-deterministic across platforms and releases
//!   > within a schema version: JSON with object keys sorted ascending by Unicode
//!   > code point, no insignificant whitespace, UTF-8 output, integers in shortest
//!   > decimal form without sign or leading zeros, no floating-point values, and
//!   > explicit encoding of empty sets […]. This is the encoding N8 fixes for
//!   > property ASTs, applied to the whole document.
//!   >
//!   > — `notes/plan/rfcs/0037-intent-contract.md`, "Canonical identity"
//!
//! So the byte spelling of a property is JSON, because the RFC says so and because
//! `notes/plan/schemas/intent-contract.schema.json` — normative for artifact shape
//! under INV-003 — says the artifact is a JSON document. There is no binary record
//! framing here, and adding one would be the second spelling ADR-0013 exists to
//! prevent: the identity would then hash bytes that no schema validates.
//!
//! What is *shared* with `continuum-value` is the discipline, not the encoding:
//! identity is the canonical bytes themselves, a digest only indexes, and a
//! collision resolves by canonical comparison (ADR-0013). [`crate::property::PropertyIdentity`]
//! is that discipline over these bytes.
//!
//! # The determinism obligations, rule by rule
//!
//! ID5 is a list of five requirements. Each one is met by construction rather than
//! by convention:
//!
//! | ID5 clause | How it is met |
//! |---|---|
//! | object keys sorted ascending by Unicode code point | [`Json::Object`] is a [`BTreeMap`], and Rust's `str` ordering is byte-lexicographic over UTF-8, which is order-isomorphic to code-point order. A caller cannot present keys in another order because the container does not have one. |
//! | no insignificant whitespace | [`Json::write_canonical`] emits no whitespace at all. |
//! | UTF-8 output | The writer's only string sink is a `Vec<u8>` fed from `&str`. |
//! | integers in shortest decimal form without sign or leading zeros | [`Json::Integer`] is an `i64` rendered by Rust's `Display`, which is shortest-form with no `+` and no leading zeros. |
//! | no floating-point values | There is no float variant, and the reader rejects a fraction or exponent with [`JsonError::FloatingPoint`] rather than rounding it into an integer. |
//!
//! ## Escapes: the reader is liberal, the writer has one spelling
//!
//! JSON permits several spellings of one string: an escaped solidus for `/`, a
//! `u`-escape for an ASCII letter, upper- or lowercase hex digits. ID5 demands one
//! spelling of one document, so [`Json::parse`] accepts every legal escape and
//! [`Json::write_canonical`] emits exactly one form: the quote and the reverse
//! solidus escaped, the five short escapes (backspace, tab, line feed, form feed,
//! carriage return), a lowercase-hex `u`-escape for every other C0 control, and raw
//! UTF-8 for everything else. Parsing and re-writing is therefore a normalizing
//! operation, which is what makes `decode(encode(x)) == x` at the byte level a
//! testable claim rather than an aspiration.
//!
//! RFC 0037's node vocabulary keeps this narrow in practice: every `name`,
//! `operator`, and binder `variable` is an `identifier` matching
//! `^[A-Za-z_][A-Za-z0-9_.]*$`, so escapes only ever arise in a `literal` string
//! value, a claim `id`, or an observer reference.
//!
//! ## Why the reader is strict where the schema is strict
//!
//! `intent-contract.schema.json` sets `additionalProperties: false` on every object
//! in the property portion. A reader that silently ignored an unrecognized key
//! would accept documents the schema rejects, and RFC 0037 is explicit about the
//! direction to fail in:
//!
//! > A reader that encounters an unrecognized token in any of those vocabularies
//! > MUST fail closed […] Forward compatibility is achieved by rejecting, never by
//! > ignoring.
//! >
//! > — RFC 0037, "Versioning and revision"
//!
//! This module supplies the mechanism — duplicate keys, trailing bytes, and depth
//! are rejected here — and [`crate::property`] supplies the vocabulary check.
//!
//! # Seams left open on purpose
//!
//! - **This is not a general-purpose JSON library.** It exists to give the Intent
//!   Contract one byte spelling. It has no float support, no `serde` integration,
//!   and no streaming reader, because the contract has no floats, the workspace has
//!   no external dependencies, and a contract is small.
//! - **The sibling PR-4 field groups reuse it.** `assumptions`, `observers`,
//!   `bounds`, `fault_model`, `fairness`, `assurance`, `optimization`, and the
//!   policy table are all ID5-encoded objects. They build [`Json`] and call
//!   [`Json::to_canonical_bytes`]; none of them should grow a second writer.

use core::fmt;
use std::collections::BTreeMap;
use std::collections::btree_map::Iter as BTreeMapIter;

/// How deep a canonical JSON document may nest.
///
/// Matches `continuum_value::value::MAX_DEPTH`, for the same reason: a bound that a
/// decoder checks *before* it recurses turns an adversarial document into a typed
/// error instead of a stack overflow. A property AST that nests 64 deep is already
/// past anything a human states or a CML elaboration produces.
pub const MAX_DEPTH: usize = 64;

/// A JSON document restricted to what RFC 0037's canonical encoding admits.
///
/// The restrictions are the point:
///
/// - there is no float variant, because ID5 forbids floating-point values and
///   `term_literal` excludes them from v1 "so that the canonical encoding stays
///   byte-deterministic";
/// - an object is a [`BTreeMap`], so it has exactly one key order and that order is
///   ID5's;
/// - integers are `i64`, so the shortest-decimal rendering is Rust's and needs no
///   hand-written formatter.
///
/// [`PartialEq`] is structural, and because the container fixes key order,
/// structural equality and canonical-byte equality agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    /// The `null` literal.
    Null,
    /// A `true`/`false` literal.
    Bool(bool),
    /// An integer. ID5 admits no other number.
    Integer(i64),
    /// A string.
    String(String),
    /// An array. Order is significant and is preserved verbatim.
    Array(Vec<Json>),
    /// An object, in ID5 key order by construction.
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// Build an object from `(key, value)` pairs, rejecting a repeated key.
    ///
    /// # Errors
    ///
    /// [`JsonError::DuplicateKey`] when two pairs share a key. A silent
    /// last-writer-wins would let one document have two meanings, which is the
    /// failure ID5 exists to prevent.
    pub fn object(fields: impl IntoIterator<Item = (String, Self)>) -> Result<Self, JsonError> {
        let mut map = BTreeMap::new();
        for (key, value) in fields {
            if map.insert(key.clone(), value).is_some() {
                return Err(JsonError::DuplicateKey { at: 0, key });
            }
        }
        Ok(Self::Object(map))
    }

    /// The canonical ID5 encoding of this document.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_canonical(&mut out);
        out
    }

    /// Append the canonical ID5 encoding of this document to `out`.
    pub fn write_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::Null => out.extend_from_slice(b"null"),
            Self::Bool(true) => out.extend_from_slice(b"true"),
            Self::Bool(false) => out.extend_from_slice(b"false"),
            Self::Integer(value) => {
                // Rust's `i64` Display is shortest decimal with no `+` and no
                // leading zeros, and it renders `0` (never `-0`) for zero, which
                // is ID5's rule exactly.
                out.extend_from_slice(value.to_string().as_bytes());
            }
            Self::String(value) => write_string(value, out),
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

    /// Parse a JSON document.
    ///
    /// The input need not be canonical — any legal JSON spelling of an admissible
    /// document is accepted, and re-writing it with [`Json::write_canonical`]
    /// produces the canonical spelling. What is *not* accepted is a document
    /// outside ID5's vocabulary: a float, a duplicate key, trailing bytes, or a
    /// nesting past [`MAX_DEPTH`].
    ///
    /// # Errors
    ///
    /// [`JsonError`], always naming a byte offset so a rejection can be located
    /// rather than merely reported.
    pub fn parse(bytes: &[u8]) -> Result<Self, JsonError> {
        let mut parser = Parser { bytes, at: 0 };
        parser.skip_whitespace();
        let value = parser.value(0)?;
        parser.skip_whitespace();
        if parser.at < bytes.len() {
            return Err(JsonError::TrailingBytes { at: parser.at });
        }
        Ok(value)
    }

    /// The JSON type name, for error messages that say what was found.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// The object's fields, or `None` if this is not an object.
    #[must_use]
    pub const fn as_object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(fields) => Some(fields),
            _ => None,
        }
    }

    /// The array's items, or `None` if this is not an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The string's contents, or `None` if this is not a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// The boolean's value, or `None` if this is not a boolean.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The integer's value, or `None` if this is not an integer.
    #[must_use]
    pub const fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// Whether this is the `null` literal.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for Json {
    /// The canonical ID5 spelling.
    ///
    /// Safe as `Display` because the writer only ever emits UTF-8.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.to_canonical_bytes();
        match core::str::from_utf8(&bytes) {
            Ok(text) => f.write_str(text),
            Err(_) => Err(fmt::Error),
        }
    }
}

/// An ordered view over an object's fields, in ID5 key order.
///
/// Convenience for the field readers in [`crate::property`], which walk an object
/// exactly once and reject anything they did not expect.
pub type Fields<'a> = BTreeMapIter<'a, String, Json>;

/// Write one string in the single canonical escape spelling.
fn write_string(value: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for ch in value.chars() {
        match ch {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{8}' => out.extend_from_slice(b"\\b"),
            '\u{9}' => out.extend_from_slice(b"\\t"),
            '\u{a}' => out.extend_from_slice(b"\\n"),
            '\u{c}' => out.extend_from_slice(b"\\f"),
            '\u{d}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                out.extend_from_slice(b"\\u00");
                out.push(hex_digit((code >> 4) as u8));
                out.push(hex_digit((code & 0x0f) as u8));
            }
            c => {
                let mut buffer = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out.push(b'"');
}

const fn hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + (nibble - 10),
    }
}

// --- reader ------------------------------------------------------------------------------

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.bytes.get(self.at) {
            // JSON's four insignificant-whitespace bytes and nothing else. A
            // vertical tab or a form feed between tokens is not whitespace in JSON
            // and is rejected as an unexpected byte.
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Result<u8, JsonError> {
        self.bytes
            .get(self.at)
            .copied()
            .ok_or(JsonError::UnexpectedEnd { at: self.at })
    }

    fn expect(&mut self, literal: &[u8]) -> Result<(), JsonError> {
        let end = self.at + literal.len();
        match self.bytes.get(self.at..end) {
            Some(found) if found == literal => {
                self.at = end;
                Ok(())
            }
            Some(_) => Err(JsonError::Unexpected { at: self.at }),
            None => Err(JsonError::UnexpectedEnd {
                at: self.bytes.len(),
            }),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth >= MAX_DEPTH {
            return Err(JsonError::TooDeep { max: MAX_DEPTH });
        }
        match self.peek()? {
            b'n' => {
                self.expect(b"null")?;
                Ok(Json::Null)
            }
            b't' => {
                self.expect(b"true")?;
                Ok(Json::Bool(true))
            }
            b'f' => {
                self.expect(b"false")?;
                Ok(Json::Bool(false))
            }
            b'"' => self.string().map(Json::String),
            b'[' => self.array(depth),
            b'{' => self.object(depth),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(JsonError::Unexpected { at: self.at }),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek()? == b']' {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(JsonError::Unexpected { at: self.at }),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1; // '{'
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        self.skip_whitespace();
        if self.peek()? == b'}' {
            self.at += 1;
            return Ok(Json::Object(fields));
        }
        loop {
            self.skip_whitespace();
            let key_at = self.at;
            if self.peek()? != b'"' {
                return Err(JsonError::Unexpected { at: self.at });
            }
            let key = self.string()?;
            self.skip_whitespace();
            if self.peek()? != b':' {
                return Err(JsonError::Unexpected { at: self.at });
            }
            self.at += 1;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            if fields.insert(key.clone(), value).is_some() {
                return Err(JsonError::DuplicateKey { at: key_at, key });
            }
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                _ => return Err(JsonError::Unexpected { at: self.at }),
            }
        }
    }

    /// Read a number, rejecting everything ID5 excludes.
    ///
    /// The JSON number grammar already forbids `+` and leading zeros; a fraction or
    /// an exponent is a float and is rejected outright rather than rounded, and an
    /// integer past `i64` is rejected rather than saturated. Both would otherwise
    /// be a value silently replaced by a different value.
    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.at;
        if self.peek()? == b'-' {
            self.at += 1;
        }
        match self.peek()? {
            b'0' => {
                self.at += 1;
                // A digit after a leading zero is `01`, which JSON forbids.
                if matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                    return Err(JsonError::NumberSyntax { at: start });
                }
            }
            b'1'..=b'9' => {
                while matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return Err(JsonError::NumberSyntax { at: start }),
        }
        if matches!(self.bytes.get(self.at), Some(b'.' | b'e' | b'E')) {
            return Err(JsonError::FloatingPoint { at: start });
        }
        let text = core::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| JsonError::NumberSyntax { at: start })?;
        text.parse::<i64>()
            .map(Json::Integer)
            .map_err(|_| JsonError::IntegerOutOfRange { at: start })
    }

    fn string(&mut self) -> Result<String, JsonError> {
        let open = self.at;
        self.at += 1; // '"'
        let mut out = String::new();
        loop {
            let byte = match self.bytes.get(self.at) {
                Some(byte) => *byte,
                None => return Err(JsonError::UnterminatedString { at: open }),
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    self.escape(&mut out)?;
                }
                // A raw C0 control inside a string is forbidden by JSON; accepting
                // it would give one string two spellings.
                0x00..=0x1f => return Err(JsonError::ControlCharacter { at: self.at }),
                _ => {
                    let start = self.at;
                    while let Some(byte) = self.bytes.get(self.at) {
                        if matches!(byte, b'"' | b'\\') || *byte < 0x20 {
                            break;
                        }
                        self.at += 1;
                    }
                    let chunk = core::str::from_utf8(&self.bytes[start..self.at])
                        .map_err(|_| JsonError::NotUtf8 { at: start })?;
                    out.push_str(chunk);
                }
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<(), JsonError> {
        let at = self.at;
        let byte = self
            .bytes
            .get(self.at)
            .copied()
            .ok_or(JsonError::UnexpectedEnd { at })?;
        self.at += 1;
        let ch = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode_escape(at, out),
            _ => return Err(JsonError::StringEscape { at }),
        };
        out.push(ch);
        Ok(())
    }

    fn unicode_escape(&mut self, at: usize, out: &mut String) -> Result<(), JsonError> {
        let first = self.hex4(at)?;
        let code = if (0xd800..0xdc00).contains(&first) {
            // A high surrogate must be followed by `\u` and a low surrogate. A lone
            // surrogate has no Unicode scalar value and is rejected, not replaced
            // with U+FFFD: a substitution would give two distinct inputs one
            // encoding.
            if self.bytes.get(self.at) != Some(&b'\\') || self.bytes.get(self.at + 1) != Some(&b'u')
            {
                return Err(JsonError::LoneSurrogate { at });
            }
            self.at += 2;
            let second = self.hex4(at)?;
            if !(0xdc00..0xe000).contains(&second) {
                return Err(JsonError::LoneSurrogate { at });
            }
            0x1_0000 + ((first - 0xd800) << 10) + (second - 0xdc00)
        } else if (0xdc00..0xe000).contains(&first) {
            return Err(JsonError::LoneSurrogate { at });
        } else {
            first
        };
        let ch = char::from_u32(code).ok_or(JsonError::StringEscape { at })?;
        out.push(ch);
        Ok(())
    }

    fn hex4(&mut self, at: usize) -> Result<u32, JsonError> {
        let end = self.at + 4;
        let digits = self
            .bytes
            .get(self.at..end)
            .ok_or(JsonError::UnexpectedEnd {
                at: self.bytes.len(),
            })?;
        let mut code = 0u32;
        for digit in digits {
            let nibble = match digit {
                b'0'..=b'9' => u32::from(digit - b'0'),
                b'a'..=b'f' => u32::from(digit - b'a') + 10,
                b'A'..=b'F' => u32::from(digit - b'A') + 10,
                _ => return Err(JsonError::StringEscape { at }),
            };
            code = (code << 4) | nibble;
        }
        self.at = end;
        Ok(code)
    }
}

/// Why a byte string is not an admissible canonical JSON document.
///
/// Every variant carries a byte offset. A rejection that cannot be located is a
/// rejection nobody can act on, and RFC 0037 rejects rather than best-effort
/// decodes, so the offset is the whole diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    /// The input ended inside a value.
    UnexpectedEnd {
        /// Byte offset the read stopped at.
        at: usize,
    },
    /// A byte that cannot start or continue a value at this position.
    Unexpected {
        /// Byte offset of the offending byte.
        at: usize,
    },
    /// Bytes followed the top-level value.
    ///
    /// A second document appended to the first is two documents, not one, and one
    /// artifact has one encoding.
    TrailingBytes {
        /// Byte offset of the first trailing byte.
        at: usize,
    },
    /// One object declared the same key twice.
    DuplicateKey {
        /// Byte offset of the repeated key.
        at: usize,
        /// The repeated key.
        key: String,
    },
    /// A number carried a fraction or an exponent.
    ///
    /// v1 excludes floating-point values "so the canonical encoding stays
    /// byte-deterministic" (RFC 0037, "Node set"), so this is a rejection rather
    /// than a rounding.
    FloatingPoint {
        /// Byte offset of the number.
        at: usize,
    },
    /// An integer does not fit in an `i64`.
    IntegerOutOfRange {
        /// Byte offset of the number.
        at: usize,
    },
    /// A number is not spelled by JSON's grammar.
    NumberSyntax {
        /// Byte offset of the number.
        at: usize,
    },
    /// A string was never closed.
    UnterminatedString {
        /// Byte offset of the opening quote.
        at: usize,
    },
    /// A raw C0 control byte appeared inside a string.
    ControlCharacter {
        /// Byte offset of the control byte.
        at: usize,
    },
    /// A backslash escape this grammar does not define.
    StringEscape {
        /// Byte offset of the escape.
        at: usize,
    },
    /// A `\u` escape named half a surrogate pair.
    LoneSurrogate {
        /// Byte offset of the escape.
        at: usize,
    },
    /// A string's bytes are not UTF-8.
    NotUtf8 {
        /// Byte offset of the offending run.
        at: usize,
    },
    /// The document nests past [`MAX_DEPTH`].
    TooDeep {
        /// The bound, [`MAX_DEPTH`].
        max: usize,
    },
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd { at } => write!(f, "canonical JSON ends at byte {at}"),
            Self::Unexpected { at } => write!(f, "unexpected byte at {at}"),
            Self::TrailingBytes { at } => {
                write!(f, "canonical JSON has trailing bytes from {at}")
            }
            Self::DuplicateKey { at, key } => write!(
                f,
                "object at byte {at} declares the key {key:?} twice; one document has one meaning"
            ),
            Self::FloatingPoint { at } => write!(
                f,
                "number at byte {at} is floating-point; the v1 property AST admits integers only \
                 so the canonical encoding stays byte-deterministic (RFC 0037)"
            ),
            Self::IntegerOutOfRange { at } => {
                write!(f, "integer at byte {at} does not fit in an i64")
            }
            Self::NumberSyntax { at } => write!(f, "malformed number at byte {at}"),
            Self::UnterminatedString { at } => {
                write!(f, "string opened at byte {at} is never closed")
            }
            Self::ControlCharacter { at } => {
                write!(f, "raw control byte inside a string at byte {at}")
            }
            Self::StringEscape { at } => write!(f, "undefined string escape at byte {at}"),
            Self::LoneSurrogate { at } => {
                write!(f, "escape at byte {at} names half a surrogate pair")
            }
            Self::NotUtf8 { at } => write!(f, "string bytes from {at} are not UTF-8"),
            Self::TooDeep { max } => write!(f, "canonical JSON nests past the {max} bound"),
        }
    }
}

impl core::error::Error for JsonError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Json, JsonError> {
        Json::parse(text.as_bytes())
    }

    fn canonical(text: &str) -> String {
        String::from_utf8(parse(text).expect("parses").to_canonical_bytes()).expect("utf-8")
    }

    // --- ID5, clause by clause ---------------------------------------------------

    #[test]
    fn object_keys_are_written_in_code_point_order_whatever_order_they_arrived_in() {
        assert_eq!(
            canonical(r#"{"b":1,"a":2,"A":3}"#),
            r#"{"A":3,"a":2,"b":1}"#
        );
        // Code-point order, not locale or case-insensitive order: `Z` < `a`.
        assert_eq!(canonical(r#"{"a":1,"Z":2}"#), r#"{"Z":2,"a":1}"#);
        // Beyond ASCII, UTF-8 byte order and code-point order agree.
        assert_eq!(
            canonical("{\"\u{7f}\":1,\"\u{80}\":2}"),
            "{\"\u{7f}\":1,\"\u{80}\":2}"
        );
    }

    #[test]
    fn insignificant_whitespace_does_not_survive() {
        assert_eq!(
            canonical("  {\n\t\"a\" : [ 1 , 2 ]\r\n}  "),
            r#"{"a":[1,2]}"#
        );
    }

    #[test]
    fn integers_are_shortest_decimal_with_no_sign_and_no_leading_zeros() {
        assert_eq!(canonical("0"), "0");
        assert_eq!(canonical("-0"), "0");
        assert_eq!(canonical("-17"), "-17");
        assert_eq!(canonical("9223372036854775807"), "9223372036854775807");
        assert_eq!(parse("01"), Err(JsonError::NumberSyntax { at: 0 }));
        assert_eq!(parse("+1"), Err(JsonError::Unexpected { at: 0 }));
        assert_eq!(
            parse("9223372036854775808"),
            Err(JsonError::IntegerOutOfRange { at: 0 })
        );
    }

    #[test]
    fn a_float_is_rejected_rather_than_rounded_into_an_integer() {
        assert_eq!(parse("1.0"), Err(JsonError::FloatingPoint { at: 0 }));
        assert_eq!(parse("1e3"), Err(JsonError::FloatingPoint { at: 0 }));
        assert_eq!(parse("-2.5"), Err(JsonError::FloatingPoint { at: 0 }));
    }

    #[test]
    fn every_escape_spelling_of_one_string_reaches_one_encoding() {
        // An escaped solidus, a `u`-escape for an ASCII letter, and uppercase hex
        // are all legal input spellings; the writer has exactly one of each.
        assert_eq!(canonical(r#""A\/B""#), r#""A/B""#);
        assert_eq!(canonical(r#""\u0041""#), r#""A""#);
        assert_eq!(canonical(r#""\u000A""#), r#""\n""#);
        assert_eq!(canonical(r#""\n""#), r#""\n""#);
        // A C0 control with no short escape takes the lowercase-hex form.
        assert_eq!(canonical(r#""\u0000""#), r#""\u0000""#);
        assert_eq!(canonical(r#""\u001F""#), r#""\u001f""#);
        // Non-ASCII scalars stay raw UTF-8, including one written as a pair.
        assert_eq!(canonical(r#""\u00E9""#), "\"\u{e9}\"");
        assert_eq!(canonical(r#""\uD83D\uDE00""#), "\"\u{1f600}\"");
        // Quote and reverse solidus keep their escapes; `/` and DEL do not gain one.
        assert_eq!(canonical(r#""a\"b\\c/d""#), r#""a\"b\\c/d""#);
        assert_eq!(canonical("\"\u{7f}\""), "\"\u{7f}\"");
    }

    #[test]
    fn writing_a_parsed_document_is_a_fixpoint() {
        for text in [
            r#"{"a":[1,-2,null,true,false,"x"],"b":{}}"#,
            r#"[]"#,
            r#"{}"#,
            r#""\n\t\u0000""#,
        ] {
            let once = canonical(text);
            let twice = canonical(&once);
            assert_eq!(
                once, twice,
                "canonical writing is not idempotent for {text}"
            );
        }
    }

    // --- fail-closed reader ------------------------------------------------------

    #[test]
    fn a_duplicate_key_is_a_typed_error_not_a_last_writer_win() {
        assert_eq!(
            parse(r#"{"a":1,"a":2}"#),
            Err(JsonError::DuplicateKey {
                at: 7,
                key: "a".to_owned()
            })
        );
        assert_eq!(
            Json::object([
                ("a".to_owned(), Json::Integer(1)),
                ("a".to_owned(), Json::Integer(2)),
            ]),
            Err(JsonError::DuplicateKey {
                at: 0,
                key: "a".to_owned()
            })
        );
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        assert_eq!(parse("{} {}"), Err(JsonError::TrailingBytes { at: 3 }));
        assert_eq!(parse("1 2"), Err(JsonError::TrailingBytes { at: 2 }));
    }

    #[test]
    fn every_truncation_of_a_document_is_a_typed_error() {
        let full = r#"{"a":[1,{"b":"c\n"}],"d":null}"#;
        for cut in 1..full.len() {
            let error = parse(&full[..cut]).expect_err("a truncated document must not decode");
            // Never a panic, never a silent partial value.
            assert!(
                matches!(
                    error,
                    JsonError::UnexpectedEnd { .. }
                        | JsonError::UnterminatedString { .. }
                        | JsonError::Unexpected { .. }
                        | JsonError::NumberSyntax { .. }
                ),
                "truncation at {cut} produced {error:?}"
            );
        }
    }

    #[test]
    fn a_raw_control_byte_and_a_bad_escape_are_both_rejected() {
        assert_eq!(
            parse("\"a\nb\""),
            Err(JsonError::ControlCharacter { at: 2 })
        );
        assert_eq!(parse(r#""\q""#), Err(JsonError::StringEscape { at: 2 }));
        assert_eq!(
            parse(r#""\uD800""#),
            Err(JsonError::LoneSurrogate { at: 2 })
        );
        assert_eq!(
            parse(r#""\uDC00""#),
            Err(JsonError::LoneSurrogate { at: 2 })
        );
        assert_eq!(
            parse(r#""\uD800A""#),
            Err(JsonError::LoneSurrogate { at: 2 })
        );
    }

    #[test]
    fn non_utf8_bytes_inside_a_string_are_rejected() {
        assert_eq!(
            Json::parse(&[b'"', 0xff, b'"']),
            Err(JsonError::NotUtf8 { at: 1 })
        );
    }

    #[test]
    fn nesting_past_the_bound_is_a_typed_error_and_not_a_stack_overflow() {
        let deep = format!(
            "{}1{}",
            "[".repeat(MAX_DEPTH + 5),
            "]".repeat(MAX_DEPTH + 5)
        );
        assert_eq!(parse(&deep), Err(JsonError::TooDeep { max: MAX_DEPTH }));
        // One below the bound still parses, so the bound is the bound and not an
        // off-by-one that quietly rejects legal documents.
        let shallow = format!(
            "{}1{}",
            "[".repeat(MAX_DEPTH - 1),
            "]".repeat(MAX_DEPTH - 1)
        );
        assert!(parse(&shallow).is_ok());
    }

    #[test]
    fn accessors_report_the_type_they_are_not() {
        let value = parse(r#"{"a":[1],"b":"x","c":true,"d":null}"#).expect("parses");
        let fields = value.as_object().expect("object");
        assert_eq!(fields["a"].as_array().expect("array").len(), 1);
        assert_eq!(fields["b"].as_str(), Some("x"));
        assert_eq!(fields["c"].as_bool(), Some(true));
        assert!(fields["d"].is_null());
        assert_eq!(fields["b"].as_integer(), None);
        assert_eq!(fields["a"].type_name(), "array");
        assert_eq!(value.to_string(), r#"{"a":[1],"b":"x","c":true,"d":null}"#);
    }
}
