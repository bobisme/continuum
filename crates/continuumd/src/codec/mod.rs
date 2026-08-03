//! The canonical codec: protocol values to bytes and back, under the three rules
//! protocol 3.2 fixed.
//!
//! # The three decisions this module implements, and where each came from
//!
//! The type layer stopped short of a codec deliberately, and said why: RFC 0026 fixes two
//! encodings with "identical canonical field order" and never says what that order is; it
//! calls a verdict "a tagged union value" and never says how the tag is spelled; and the
//! IDL's open item 3 left the envelope's `Opaque` payloads without a declared shape.
//! Choosing values for those three without a normative source would have been inventing
//! wire format. bn-i4aem/bn-3bhkp took the three decisions in the IDL — `rule
//! encoding.canonical_form`, `rule encoding.union_tagging`, `rule
//! encoding.opaque_payloads` — and this module implements them. Each follows a discipline
//! the project already had rather than a preference:
//!
//! 1. **Field order is ascending Unicode code-point order of the field name.** That is
//!    RFC 0037's ID5 rule, whose reference implementation is
//!    `crates/continuum-intent/src/canonical_json.rs`, applied to the wire. It is a
//!    property of the *message* rather than of this file: an `optional` field a message
//!    omits has no declaration position to occupy, a `map<String, T>` has no declaration
//!    order at all, and a reader that needed the IDL to know the order could not read a
//!    document it can otherwise parse. `Json::Object` is a `BTreeMap`, so there is no
//!    other order to present. The same order binds the CBOR encoder, which therefore does
//!    *not* use the CBOR specification's own length-first map ordering — the protocol fixes one order for
//!    both encodings and the IDL is where it is fixed.
//! 2. **Unions are externally tagged**: `{"semantic": {…}}`, one key, the variant
//!    identifier the IDL declares. The IDL's `variant` production is an identifier plus a
//!    payload type, so the encoding adds nothing to the declaration. Internal tagging was
//!    rejected on a rule rather than on taste: the tag is not a declared field of the
//!    payload struct, and `rule envelope.unknown_fields` forbids a daemon from emitting a
//!    field the negotiated version does not define, so an internally tagged union is
//!    non-conforming against the IDL by construction.
//! 3. **`Opaque` is resolved by the carrying field, not by the codec.**
//!    `RequestEnvelope.arguments` is the request struct of the operation the same envelope
//!    names, `ResultEnvelope.payload` is that operation's response struct, and
//!    `NextOperation.arguments` is the request struct of the operation that struct itself
//!    names. Every resolution is a function of the message alone, because the naming field
//!    is `required` in the same object. Every other `Opaque` is a schema-governed value
//!    carried verbatim.
//!
//! # Where the encoding boundary is, and why the daemon is still codec-free
//!
//! `daemon` takes an already-decoded `Arguments` and emits a typed `Payload` beside the
//! envelope. That is not a limitation this module removes: it is the seam that keeps the
//! operation layer a pure function of typed values, with no bytes to parse and no encoding
//! to choose. The codec sits *outside* it, in `transport`, which decodes bytes into
//! `(RequestEnvelope, Arguments)` before dispatch and encodes the typed `Payload` into
//! `ResultEnvelope.payload` after it. So `payload` carries real bytes on the wire and
//! `null` only where the IDL says it must — on `status = error`.
//!
//! # Failing closed
//!
//! Every decode failure is typed and none is a repair. [`CodecError::code`] maps each to
//! the wire code RFC 0026's malformed-input list names for it, and the one case that is
//! *not* `MalformedRequest` is worth stating: an unknown member of an `@open` enum. RFC
//! 0026 requires such a member to be "surfaced verbatim, not rejected", and the type layer
//! types every enum field as the closed Rust enum, so the token cannot be carried into a
//! request body. It is surfaced verbatim as [`CodecError::UnknownOpenMember`] and answered
//! `UnsupportedSemanticFeature` — "the request needs a semantic feature this daemon does
//! not implement", which is exactly true of an unknown `TargetKind` or
//! `ExplorationStrategy` and is not a claim that the message was malformed.
//!
//! # Two encodings, one type layer
//!
//! The IDL fixes `canonical_json` **and** `canonical_cbor`, and requires them to carry
//! "identical canonical field order". The way that identity is made structural here rather
//! than maintained is [`Document`]: the trait a *document model* implements, with one
//! implementation per encoding ([`json::Json`], [`cbor::Cbor`]). [`ProtocolValue::encode`]
//! is generic over it, so the code that decides which key a field is written under, in
//! what order, and under which presence rule exists **once** and both encodings run it.
//!
//! That is stronger than emitting two encoders from one declaration, which was the other
//! way to spend the same discipline. Two encoders generated from one text can still
//! diverge if the two generators are edited apart; one encoder parameterized by its output
//! model cannot diverge at all, because there is nothing to edit apart. A field's key, its
//! position in the sequence, and its presence discipline are properties of
//! [`protocol_struct!`](crate::protocol_struct)'s expansion, and the encoding contributes
//! only the *spelling of the leaves*.
//!
//! Exactly two leaves are spelled differently, and the IDL is where both are fixed
//! (IDL §3): `Bytes` is base64url text in JSON and a byte string in CBOR, and a `U64`
//! beyond exact JSON representation is a decimal string there and an ordinary unsigned
//! integer here. Both live in the [`Document`] implementations —
//! [`from_byte_string`](Document::from_byte_string), [`from_unsigned`](Document::from_unsigned),
//! and their readers — which is why the type layer never mentions an encoding.
//!
//! An [`Opaque`] is the one value that carries *bytes* across this boundary, and it
//! carries them in the negotiated encoding by definition ("carried verbatim as a canonical
//! value of the negotiated encoding"). So [`ProtocolValue`] for `Opaque` reads and writes
//! through the same `Document` the enclosing message is being read or written through, and
//! a message containing an `Opaque` therefore has *different* `Opaque` bytes in the two
//! encodings while having the same field sequence. That is the contract, not a defect: the
//! payload is a canonical value of the connection's encoding.

pub mod cbor;
pub mod json;
pub mod operations;

use std::collections::BTreeMap;

use cbor::{Cbor, CborError};
use json::{Json, JsonError};

use crate::protocol::scalar::{
    ActorId, ArtifactHandle, AuditCorrelationId, ByteCount, Bytes, Commitment, DurationMs,
    EpochIdentity, Opaque, OperationName, PageToken, ProtocolVersion, RequestId, Timestamp,
};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{Encoding, ErrorCode};

/// The canonical document model of one encoding.
///
/// One implementation per encoding the IDL fixes, and the seam that lets a single
/// [`ProtocolValue`] implementation serve both. Everything structural — which keys, in
/// what order, present or absent — belongs to the caller; everything below is this
/// trait's, and there are exactly two places where the two implementations disagree
/// (`Bytes` and a large `U64`), both because IDL §3 says they disagree.
///
/// The accessors return [`CodecError`] rather than [`Option`] wherever the two encodings
/// have different *reasons* for refusing a value, so that a refusal reads the same
/// whichever encoding produced it.
pub trait Document: Sized + Clone + PartialEq + core::fmt::Debug {
    /// The IDL vocabulary member naming this encoding.
    const ENCODING: Encoding;

    /// The `null` literal: a *named* absence (INV-007), never an omission.
    fn from_null() -> Self;

    /// A boolean.
    fn from_bool(value: bool) -> Self;

    /// An unsigned integer, in whichever spelling this encoding gives that magnitude.
    fn from_unsigned(value: u64) -> Self;

    /// A UTF-8 string.
    fn from_text(value: &str) -> Self;

    /// A `Bytes` value, in this encoding's spelling of one.
    fn from_byte_string(value: &[u8]) -> Self;

    /// An array, in order.
    fn from_items(items: Vec<Self>) -> Self;

    /// A map, in canonical key order by construction.
    fn from_entries(entries: BTreeMap<String, Self>) -> Self;

    /// Whether this is the null literal.
    fn is_null(&self) -> bool;

    /// The boolean this value denotes, when it is one.
    fn as_bool(&self) -> Option<bool>;

    /// The unsigned integer this value denotes in the encoding's *number* spelling.
    ///
    /// # Errors
    ///
    /// [`CodecError::TypeMismatch`] when the value is not a number.
    fn as_integer(&self, expected: &'static str) -> Result<u64, CodecError>;

    /// The `U64` this value denotes, in every spelling the IDL gives `U64` here.
    ///
    /// # Errors
    ///
    /// [`CodecError::IntegerRange`] for a spelling this encoding does not admit for the
    /// magnitude, and [`CodecError::TypeMismatch`] for a value of another kind.
    fn as_u64(&self, expected: &'static str) -> Result<u64, CodecError>;

    /// The string, when this is one.
    fn as_text(&self) -> Option<&str>;

    /// The `Bytes` this value denotes, in this encoding's spelling of one.
    ///
    /// # Errors
    ///
    /// [`CodecError::Bytes`] when the spelling is not canonical, and
    /// [`CodecError::TypeMismatch`] for a value of another kind.
    fn as_byte_string(&self, expected: &'static str) -> Result<Vec<u8>, CodecError>;

    /// The array's items, when this is an array.
    fn as_items(&self) -> Option<&[Self]>;

    /// The map's entries, when this is a map.
    fn as_entries(&self) -> Option<&BTreeMap<String, Self>>;

    /// The name of this value's kind, for a typed mismatch report.
    fn kind(&self) -> &'static str;

    /// Read a canonical document.
    ///
    /// # Errors
    ///
    /// [`CodecError`] when the bytes are not a canonical document of this encoding.
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CodecError>;

    /// The canonical encoding of this document.
    fn to_canonical_bytes(&self) -> Vec<u8>;
}

/// The `canonical_json` document model.
///
/// Every method is a plain match rather than a call to the same-named inherent method:
/// [`Json`] has inherent `is_null`, `kind`, and `to_canonical_bytes` of its own, and
/// spelling the bodies out here keeps "which one is this" from being a question about
/// method-resolution order.
impl Document for Json {
    const ENCODING: Encoding = Encoding::CanonicalJson;

    fn from_null() -> Self {
        Self::Null
    }

    fn from_bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// The IDL's `U64` rule, writer side: a number while the value is exactly
    /// representable, a decimal string beyond that. A writer that chose per *call site*
    /// would give one number two spellings depending on where it was written from.
    fn from_unsigned(value: u64) -> Self {
        if value <= json::MAX_EXACT_INTEGER {
            Self::Integer(value)
        } else {
            Self::String(value.to_string())
        }
    }

    fn from_text(value: &str) -> Self {
        Self::String(value.to_owned())
    }

    fn from_byte_string(value: &[u8]) -> Self {
        Self::String(json::base64url(value))
    }

    fn from_items(items: Vec<Self>) -> Self {
        Self::Array(items)
    }

    fn from_entries(entries: BTreeMap<String, Self>) -> Self {
        Self::Object(entries)
    }

    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_integer(&self, expected: &'static str) -> Result<u64, CodecError> {
        match self {
            Self::Integer(value) => Ok(*value),
            other => Err(CodecError::TypeMismatch {
                expected,
                found: Json::kind(other),
            }),
        }
    }

    /// The IDL's `U64` rule, reader side. A decimal string inside the exactly-
    /// representable range is rejected because the value already has a spelling there,
    /// and a non-shortest decimal string (`"0100…"`) is rejected for the same reason. A
    /// *number* beyond the range never reaches here: [`Json::parse`] refuses it.
    fn as_u64(&self, expected: &'static str) -> Result<u64, CodecError> {
        match self {
            Self::Integer(value) => Ok(*value),
            Self::String(text) => {
                let parsed: u64 = text
                    .parse()
                    .map_err(|_| CodecError::IntegerRange { expected })?;
                if parsed <= json::MAX_EXACT_INTEGER || text != &parsed.to_string() {
                    return Err(CodecError::IntegerRange { expected });
                }
                Ok(parsed)
            }
            other => Err(CodecError::TypeMismatch {
                expected,
                found: Json::kind(other),
            }),
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    fn as_byte_string(&self, expected: &'static str) -> Result<Vec<u8>, CodecError> {
        match self {
            Self::String(text) => json::from_base64url(text).map_err(|_| CodecError::Bytes),
            other => Err(CodecError::TypeMismatch {
                expected,
                found: Json::kind(other),
            }),
        }
    }

    fn as_items(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    fn as_entries(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(fields) => Some(fields),
            _ => None,
        }
    }

    fn kind(&self) -> &'static str {
        Json::kind(self)
    }

    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self::parse(bytes)?)
    }

    fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_canonical(&mut out);
        out
    }
}

/// The `canonical_cbor` document model.
///
/// The two rows where this differs from the JSON model are
/// [`from_unsigned`](Document::from_unsigned) and
/// [`from_byte_string`](Document::from_byte_string) with their readers, and both
/// differences are IDL §3's: "CBOR uses an unsigned integer" and "CBOR uses a byte
/// string". Everything else is the same match against a different value model, which is
/// what makes "identical canonical field order" structural rather than maintained.
impl Document for Cbor {
    const ENCODING: Encoding = Encoding::CanonicalCbor;

    fn from_null() -> Self {
        Self::Null
    }

    fn from_bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// One spelling for every magnitude: major type 0 carries the whole `u64` range
    /// exactly, so CBOR has no `2^53 − 1` boundary and no second spelling beyond it.
    fn from_unsigned(value: u64) -> Self {
        Self::Unsigned(value)
    }

    fn from_text(value: &str) -> Self {
        Self::Text(value.to_owned())
    }

    fn from_byte_string(value: &[u8]) -> Self {
        Self::Bytes(value.to_vec())
    }

    fn from_items(items: Vec<Self>) -> Self {
        Self::Array(items)
    }

    fn from_entries(entries: BTreeMap<String, Self>) -> Self {
        Self::Map(entries)
    }

    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_integer(&self, expected: &'static str) -> Result<u64, CodecError> {
        match self {
            Self::Unsigned(value) => Ok(*value),
            other => Err(CodecError::TypeMismatch {
                expected,
                found: Cbor::kind(other),
            }),
        }
    }

    /// A `U64` is major type 0 and nothing else. A decimal *text* string is refused as a
    /// [`TypeMismatch`](CodecError::TypeMismatch) rather than parsed, because in this
    /// encoding it is not a second spelling of the number — it is a string.
    fn as_u64(&self, expected: &'static str) -> Result<u64, CodecError> {
        self.as_integer(expected)
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    fn as_byte_string(&self, expected: &'static str) -> Result<Vec<u8>, CodecError> {
        match self {
            Self::Bytes(bytes) => Ok(bytes.clone()),
            other => Err(CodecError::TypeMismatch {
                expected,
                found: Cbor::kind(other),
            }),
        }
    }

    fn as_items(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    fn as_entries(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Map(fields) => Some(fields),
            _ => None,
        }
    }

    fn kind(&self) -> &'static str {
        Cbor::kind(self)
    }

    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        Ok(Self::parse(bytes)?)
    }

    fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_canonical(&mut out);
        out
    }
}

/// A value the wire declares, and the two directions it travels — in either encoding.
///
/// `encode` is fallible because [`Opaque`] is: it carries bytes that must already be a
/// canonical value of the negotiated encoding, and a daemon that staged something else
/// into one has produced a value the wire cannot carry. Reporting that is better than
/// emitting bytes no reader can parse.
pub trait ProtocolValue: Sized {
    /// This value as a canonical document of `D`.
    ///
    /// # Errors
    ///
    /// [`CodecError`] when a carried `Opaque` is not itself canonical.
    fn encode<D: Document>(&self) -> Result<D, CodecError>;

    /// Read this value from a canonical document of `D`.
    ///
    /// # Errors
    ///
    /// [`CodecError`] when the document is not this value's declared shape.
    fn decode<D: Document>(value: &D) -> Result<Self, CodecError>;
}

/// Encode a protocol value to canonical bytes of `D`.
///
/// # Errors
///
/// [`CodecError`] when a carried `Opaque` is not canonical.
pub fn write_in<D: Document, T: ProtocolValue>(value: &T) -> Result<Vec<u8>, CodecError> {
    Ok(value.encode::<D>()?.to_canonical_bytes())
}

/// Decode a protocol value from canonical bytes of `D`.
///
/// # Errors
///
/// [`CodecError`] when the bytes are not a canonical document, or not this value's shape.
pub fn read_in<D: Document, T: ProtocolValue>(bytes: &[u8]) -> Result<T, CodecError> {
    T::decode(&D::from_canonical_bytes(bytes)?)
}

/// Carry a protocol value in an `Opaque` field, in the encoding `D`.
///
/// # Errors
///
/// [`CodecError`] when the value carries a non-canonical `Opaque` of its own.
pub fn to_opaque_in<D: Document, T: ProtocolValue>(value: &T) -> Result<Opaque, CodecError> {
    Ok(Opaque::from_bytes(write_in::<D, T>(value)?))
}

/// Read a protocol value out of an `Opaque` field carrying the encoding `D`.
///
/// # Errors
///
/// [`CodecError`] when the carried bytes are not a canonical document of that shape.
pub fn from_opaque_in<D: Document, T: ProtocolValue>(opaque: &Opaque) -> Result<T, CodecError> {
    read_in::<D, T>(opaque.as_bytes())
}

/// Encode a protocol value to canonical JSON bytes.
///
/// # Errors
///
/// [`CodecError`] when a carried `Opaque` is not canonical.
pub fn to_bytes<T: ProtocolValue>(value: &T) -> Result<Vec<u8>, CodecError> {
    write_in::<Json, T>(value)
}

/// Decode a protocol value from canonical JSON bytes.
///
/// # Errors
///
/// [`CodecError`] when the bytes are not a canonical document, or not this value's shape.
pub fn from_bytes<T: ProtocolValue>(bytes: &[u8]) -> Result<T, CodecError> {
    read_in::<Json, T>(bytes)
}

/// Encode a protocol value to canonical CBOR bytes.
///
/// # Errors
///
/// [`CodecError`] when a carried `Opaque` is not canonical.
pub fn to_cbor_bytes<T: ProtocolValue>(value: &T) -> Result<Vec<u8>, CodecError> {
    write_in::<Cbor, T>(value)
}

/// Decode a protocol value from canonical CBOR bytes.
///
/// # Errors
///
/// [`CodecError`] when the bytes are not a canonical document, or not this value's shape.
pub fn from_cbor_bytes<T: ProtocolValue>(bytes: &[u8]) -> Result<T, CodecError> {
    read_in::<Cbor, T>(bytes)
}

/// Carry a protocol value in an `Opaque` field, in canonical JSON.
///
/// # Errors
///
/// [`CodecError`] when the value carries a non-canonical `Opaque` of its own.
pub fn to_opaque<T: ProtocolValue>(value: &T) -> Result<Opaque, CodecError> {
    to_opaque_in::<Json, T>(value)
}

/// Read a protocol value out of an `Opaque` field carrying canonical JSON.
///
/// # Errors
///
/// [`CodecError`] when the carried bytes are not a canonical document of that shape.
pub fn from_opaque<T: ProtocolValue>(opaque: &Opaque) -> Result<T, CodecError> {
    from_opaque_in::<Json, T>(opaque)
}

// --- errors -------------------------------------------------------------------------

/// Why a message is not the value it claims to be.
///
/// No variant carries content from the wire. `rule envelope.no_prose` and INV-016 forbid
/// interpolating a caller's bytes into an explanation, and a struct name, a field name, or
/// an enum name is this file's own text. [`UnknownOpenMember`](CodecError::UnknownOpenMember)
/// is the single exception and it is deliberate: `rule versioning.enums` requires an
/// unknown `@open` member to be surfaced *verbatim*, so the token is carried in a typed
/// field for a caller to switch on, and MUST NOT be interpolated into prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The bytes are not a canonical JSON document.
    Json(JsonError),
    /// The bytes are not a canonical CBOR document.
    Cbor(CborError),
    /// A `required` or `nullable` field is absent.
    MissingField {
        /// The struct that declares it.
        declared_by: &'static str,
        /// The field name.
        field: &'static str,
    },
    /// A `required` or `optional` field is present and null. Absent and null are distinct
    /// in both directions.
    UnexpectedNull {
        /// The struct that declares it.
        declared_by: &'static str,
        /// The field name.
        field: &'static str,
    },
    /// A value is not the kind its declaration names.
    TypeMismatch {
        /// The declared type, in the IDL's spelling.
        expected: &'static str,
        /// The kind the document actually carries.
        found: &'static str,
    },
    /// A string does not satisfy its alias's `@pattern` or its handle's prefix rule.
    Pattern {
        /// The alias or handle class.
        declared: &'static str,
    },
    /// An unrecognized member of a *closed* enum, which `rule versioning.enums` makes
    /// malformed.
    UnknownMember {
        /// The enum the token was offered to.
        enum_name: &'static str,
    },
    /// An unrecognized member of an `@open` enum, surfaced verbatim rather than rejected.
    UnknownOpenMember {
        /// The enum the token was offered to.
        enum_name: &'static str,
        /// The token, verbatim. Never interpolate it into prose.
        token: Box<str>,
    },
    /// A union object does not carry exactly one key.
    UnionArity {
        /// The union.
        union: &'static str,
    },
    /// A union's single key is not one of its declared variants.
    UnknownVariant {
        /// The union.
        union: &'static str,
    },
    /// A `Bytes` value is not canonical base64url.
    Bytes,
    /// An integer is outside the range its declared type admits.
    IntegerRange {
        /// The declared type.
        expected: &'static str,
    },
    /// The envelope names an operation this daemon has no request shape for.
    UnknownOperation,
}

impl CodecError {
    /// The wire code RFC 0026's malformed-input list names for this failure.
    ///
    /// > an unknown member of a *closed* enum ⇒ `MalformedRequest` (an unknown member of
    /// > an `@open` enum is surfaced verbatim, not rejected); an `arguments` payload that
    /// > does not validate against the operation's request struct ⇒ `MalformedRequest`; a
    /// > `required` field absent or null ⇒ `MalformedRequest` […]
    /// >
    /// > — RFC 0026, "Malformed input"
    ///
    /// Every variant but one is on that list. [`UnknownOpenMember`](Self::UnknownOpenMember)
    /// is not, because the same list says such a member is not rejected — the message is
    /// well formed and names something this daemon does not implement, which is what
    /// `UnsupportedSemanticFeature` says. That code is inside every operation's union as
    /// of protocol 3.2 (`rule errors.common`), so answering with it is admissible wherever
    /// a request is decoded.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::UnknownOpenMember { .. } | Self::UnknownOperation => {
                ErrorCode::UnsupportedSemanticFeature
            }
            _ => ErrorCode::MalformedRequest,
        }
    }
}

impl From<JsonError> for CodecError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<CborError> for CodecError {
    fn from(error: CborError) -> Self {
        Self::Cbor(error)
    }
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
            Self::Cbor(error) => write!(f, "{error}"),
            Self::MissingField { declared_by, field } => {
                write!(f, "`{declared_by}.{field}` is absent and is not `optional`")
            }
            Self::UnexpectedNull { declared_by, field } => {
                write!(f, "`{declared_by}.{field}` is null and is not `nullable`")
            }
            Self::TypeMismatch { expected, found } => {
                write!(f, "expected `{expected}`, found a {found}")
            }
            Self::Pattern { declared } => {
                write!(f, "a string does not satisfy `{declared}`")
            }
            Self::UnknownMember { enum_name } => {
                write!(f, "unrecognized member of the closed enum `{enum_name}`")
            }
            Self::UnknownOpenMember { enum_name, .. } => {
                write!(f, "unrecognized member of the open enum `{enum_name}`")
            }
            Self::UnionArity { union } => {
                write!(f, "`{union}` is encoded as an object with exactly one key")
            }
            Self::UnknownVariant { union } => {
                write!(f, "unrecognized variant of the union `{union}`")
            }
            Self::Bytes => f.write_str("a `Bytes` value is not canonical base64url"),
            Self::IntegerRange { expected } => {
                write!(f, "an integer outside the range of `{expected}`")
            }
            Self::UnknownOperation => {
                f.write_str("this daemon declares no request shape for that operation")
            }
        }
    }
}

impl core::error::Error for CodecError {}

// --- the field helpers the declaration macros emit ----------------------------------
//
// Nine of them, one per (presence x shape) pair the IDL's `field` production admits.
// They are functions rather than a trait so that `Bytes` — which is `Vec<u8>` and encodes
// as base64url — can be a `ProtocolValue` while `list<T>` goes through a different path;
// a blanket `ProtocolValue for Vec<T>` would overlap with it and neither impl would be
// admissible.

/// The object a value must be for a struct or union to decode from it.
///
/// # Errors
///
/// [`CodecError::TypeMismatch`] when it is anything else.
pub fn object_of<'a, D: Document>(
    value: &'a D,
    expected: &'static str,
) -> Result<&'a BTreeMap<String, D>, CodecError> {
    value.as_entries().ok_or(CodecError::TypeMismatch {
        expected,
        found: value.kind(),
    })
}

/// Write a `required` field.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_required<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &T,
) -> Result<(), CodecError> {
    into.insert(name.to_owned(), value.encode::<D>()?);
    Ok(())
}

/// Write a `nullable` field: always present, `null` for a named absence.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_nullable<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Nullable<T>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => D::from_null(),
        Nullable::Value(inner) => inner.encode::<D>()?,
    };
    into.insert(name.to_owned(), encoded);
    Ok(())
}

/// Write an `optional` field: omitted when absent, never `null`.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_optional<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Optional<T>,
) -> Result<(), CodecError> {
    if let Optional::Present(inner) = value {
        into.insert(name.to_owned(), inner.encode::<D>()?);
    }
    Ok(())
}

fn encode_list<D: Document, T: ProtocolValue>(items: &[T]) -> Result<D, CodecError> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(item.encode::<D>()?);
    }
    Ok(D::from_items(out))
}

fn decode_list<D: Document, T: ProtocolValue>(value: &D) -> Result<Vec<T>, CodecError> {
    let items = value.as_items().ok_or(CodecError::TypeMismatch {
        expected: "list",
        found: value.kind(),
    })?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(T::decode(item)?);
    }
    Ok(out)
}

fn encode_map<D: Document, T: ProtocolValue>(
    entries: &BTreeMap<String, T>,
) -> Result<D, CodecError> {
    let mut out = BTreeMap::new();
    for (key, value) in entries {
        out.insert(key.clone(), value.encode::<D>()?);
    }
    Ok(D::from_entries(out))
}

fn decode_map<D: Document, T: ProtocolValue>(value: &D) -> Result<BTreeMap<String, T>, CodecError> {
    let entries = object_of(value, "map")?;
    let mut out = BTreeMap::new();
    for (key, item) in entries {
        out.insert(key.clone(), T::decode(item)?);
    }
    Ok(out)
}

/// Write a `required` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when an item cannot be encoded.
pub fn put_required_list<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &[T],
) -> Result<(), CodecError> {
    into.insert(name.to_owned(), encode_list(value)?);
    Ok(())
}

/// Write a `nullable` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when an item cannot be encoded.
pub fn put_nullable_list<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Nullable<Vec<T>>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => D::from_null(),
        Nullable::Value(items) => encode_list(items)?,
    };
    into.insert(name.to_owned(), encoded);
    Ok(())
}

/// Write an `optional` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when an item cannot be encoded.
pub fn put_optional_list<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Optional<Vec<T>>,
) -> Result<(), CodecError> {
    if let Optional::Present(items) = value {
        into.insert(name.to_owned(), encode_list(items)?);
    }
    Ok(())
}

/// Write a `required` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when a value cannot be encoded.
pub fn put_required_map<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &BTreeMap<String, T>,
) -> Result<(), CodecError> {
    into.insert(name.to_owned(), encode_map(value)?);
    Ok(())
}

/// Write a `nullable` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when a value cannot be encoded.
pub fn put_nullable_map<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Nullable<BTreeMap<String, T>>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => D::from_null(),
        Nullable::Value(entries) => encode_map(entries)?,
    };
    into.insert(name.to_owned(), encoded);
    Ok(())
}

/// Write an `optional` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when a value cannot be encoded.
pub fn put_optional_map<D: Document, T: ProtocolValue>(
    into: &mut BTreeMap<String, D>,
    name: &str,
    value: &Optional<BTreeMap<String, T>>,
) -> Result<(), CodecError> {
    if let Optional::Present(entries) = value {
        into.insert(name.to_owned(), encode_map(entries)?);
    }
    Ok(())
}

fn present<'a, D: Document>(
    fields: &'a BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<&'a D, CodecError> {
    fields
        .get(field)
        .ok_or(CodecError::MissingField { declared_by, field })
}

/// Read a `required` field: present and non-null.
///
/// # Errors
///
/// [`CodecError::MissingField`], [`CodecError::UnexpectedNull`], or a shape mismatch.
pub fn take_required<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<T, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    T::decode(value)
}

/// Read a `nullable` field: present, and `null` is a value.
///
/// # Errors
///
/// [`CodecError::MissingField`] when it is absent — omitting a `nullable` field is
/// malformed — or a shape mismatch.
pub fn take_nullable<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Nullable<T>, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Ok(Nullable::Null);
    }
    Ok(Nullable::Value(T::decode(value)?))
}

/// Read an `optional` field: absent is absent, and `null` is malformed.
///
/// # Errors
///
/// [`CodecError::UnexpectedNull`] when it is present and null, or a shape mismatch.
pub fn take_optional<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Optional<T>, CodecError> {
    let Some(value) = fields.get(field) else {
        return Ok(Optional::Absent);
    };
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    Ok(Optional::Present(T::decode(value)?))
}

/// Read a `required` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when absent, null, or not a list of that item shape.
pub fn take_required_list<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Vec<T>, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    decode_list(value)
}

/// Read a `nullable` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when absent, or not a list of that item shape.
pub fn take_nullable_list<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Nullable<Vec<T>>, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Ok(Nullable::Null);
    }
    Ok(Nullable::Value(decode_list(value)?))
}

/// Read an `optional` `list<T>` field.
///
/// # Errors
///
/// [`CodecError`] when present and null, or not a list of that item shape.
pub fn take_optional_list<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Optional<Vec<T>>, CodecError> {
    let Some(value) = fields.get(field) else {
        return Ok(Optional::Absent);
    };
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    Ok(Optional::Present(decode_list(value)?))
}

/// Read a `required` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when absent, null, or not a map of that value shape.
pub fn take_required_map<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<BTreeMap<String, T>, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    decode_map(value)
}

/// Read a `nullable` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when absent, or not a map of that value shape.
pub fn take_nullable_map<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Nullable<BTreeMap<String, T>>, CodecError> {
    let value = present(fields, declared_by, field)?;
    if value.is_null() {
        return Ok(Nullable::Null);
    }
    Ok(Nullable::Value(decode_map(value)?))
}

/// Read an `optional` `map<String, T>` field.
///
/// # Errors
///
/// [`CodecError`] when present and null, or not a map of that value shape.
pub fn take_optional_map<D: Document, T: ProtocolValue>(
    fields: &BTreeMap<String, D>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<Optional<BTreeMap<String, T>>, CodecError> {
    let Some(value) = fields.get(field) else {
        return Ok(Optional::Absent);
    };
    if value.is_null() {
        return Err(CodecError::UnexpectedNull { declared_by, field });
    }
    Ok(Optional::Present(decode_map(value)?))
}

// --- the leaf types -----------------------------------------------------------------

impl ProtocolValue for bool {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_bool(*self))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        value.as_bool().ok_or(CodecError::TypeMismatch {
            expected: "Bool",
            found: value.kind(),
        })
    }
}

impl ProtocolValue for u32 {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_unsigned(u64::from(*self)))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        // `U32` takes the encoding's plain number spelling and no other: a `U32` is always
        // inside the exactly-representable range, so the second spelling `U64` has in JSON
        // is not one this type ever needs.
        Self::try_from(value.as_integer("U32")?)
            .map_err(|_| CodecError::IntegerRange { expected: "U32" })
    }
}

/// `U64`'s spelling is the encoding's business, and IDL §3 gives each encoding one.
///
/// > `U64`. JSON encodes it as a number only when it is exactly representable; otherwise
/// > as a decimal string. CBOR uses an unsigned integer.
/// >
/// > — IDL §3
///
/// Both directions are strict in both encodings: JSON rejects a decimal string inside the
/// exactly-representable range and a number beyond it, and CBOR rejects a decimal string
/// altogether, because each would be a second spelling of a value that already has one.
/// Which rejection applies is [`Document::as_u64`]'s to decide.
impl ProtocolValue for u64 {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_unsigned(*self))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        value.as_u64("U64")
    }
}

impl ProtocolValue for String {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_text(self))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        value
            .as_text()
            .map(ToOwned::to_owned)
            .ok_or(CodecError::TypeMismatch {
                expected: "String",
                found: value.kind(),
            })
    }
}

/// `Bytes` is base64url without padding in JSON, and a byte string in CBOR.
impl ProtocolValue for Bytes {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_byte_string(self))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        value.as_byte_string("Bytes")
    }
}

/// An `Opaque` is a canonical value carried verbatim: the codec neither interprets it nor
/// re-shapes it (`rule encoding.opaque_payloads`).
///
/// "Carried verbatim as a canonical value of the **negotiated encoding**" is why this is
/// the one leaf that reads and writes bytes: the payload's bytes are a document of
/// whatever encoding the enclosing message is in, so it is parsed and re-written through
/// the same [`Document`] and never translated between the two.
impl ProtocolValue for Opaque {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        D::from_canonical_bytes(self.as_bytes())
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        Ok(Self::from_bytes(value.to_canonical_bytes()))
    }
}

/// Emit [`ProtocolValue`] for a string newtype whose constructor enforces its pattern.
macro_rules! string_value {
    ($name:ty, $declared:literal, $read:ident, $write:ident) => {
        impl ProtocolValue for $name {
            fn encode<D: Document>(&self) -> Result<D, CodecError> {
                Ok(D::from_text(self.$write()))
            }

            fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
                let text = value.as_text().ok_or(CodecError::TypeMismatch {
                    expected: $declared,
                    found: value.kind(),
                })?;
                Self::$read(text).map_err(|_| CodecError::Pattern {
                    declared: $declared,
                })
            }
        }
    };
}

string_value!(Timestamp, "Timestamp", new, as_str);
string_value!(RequestId, "RequestId", new, as_str);
string_value!(ActorId, "ActorId", new, as_str);
string_value!(OperationName, "OperationName", new, as_str);
string_value!(AuditCorrelationId, "AuditCorrelationId", new, as_str);
string_value!(ArtifactHandle, "ArtifactHandle", new, as_str);
string_value!(EpochIdentity, "EpochIdentity", new, as_str);

/// `Commitment` and `PageToken` declare no `@pattern`, so their constructors are total and
/// the decode cannot fail on shape. Inventing a constraint here would be inventing wire
/// semantics; the IDL says what it says.
macro_rules! unconstrained_string_value {
    ($name:ty, $declared:literal) => {
        impl ProtocolValue for $name {
            fn encode<D: Document>(&self) -> Result<D, CodecError> {
                Ok(D::from_text(self.as_str()))
            }

            fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
                let text = value.as_text().ok_or(CodecError::TypeMismatch {
                    expected: $declared,
                    found: value.kind(),
                })?;
                Ok(Self::new(text))
            }
        }
    };
}

unconstrained_string_value!(Commitment, "Commitment");
unconstrained_string_value!(PageToken, "PageToken");

/// `ProtocolVersion` is `continuum_value::epoch::ProtocolEpoch`, whose parse/render
/// round-trip *is* the IDL's `@pattern`. Restating the pattern here would be the second
/// spelling ADR-0013 forbids.
impl ProtocolValue for ProtocolVersion {
    fn encode<D: Document>(&self) -> Result<D, CodecError> {
        Ok(D::from_text(&self.to_string()))
    }

    fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
        let text = value.as_text().ok_or(CodecError::TypeMismatch {
            expected: "ProtocolVersion",
            found: value.kind(),
        })?;
        text.parse().map_err(|_| CodecError::Pattern {
            declared: "ProtocolVersion",
        })
    }
}

/// Emit [`ProtocolValue`] for a `u64` newtype.
macro_rules! integer_value {
    ($name:ty, $write:ident) => {
        impl ProtocolValue for $name {
            fn encode<D: Document>(&self) -> Result<D, CodecError> {
                ProtocolValue::encode(&self.$write())
            }

            fn decode<D: Document>(value: &D) -> Result<Self, CodecError> {
                Ok(Self::new(<u64 as ProtocolValue>::decode(value)?))
            }
        }
    };
}

integer_value!(DurationMs, millis);
integer_value!(ByteCount, bytes);
