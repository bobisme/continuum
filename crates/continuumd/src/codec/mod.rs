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

pub mod json;
pub mod operations;

use std::collections::BTreeMap;

use json::{Json, JsonError, base64url, from_base64url};

use crate::protocol::scalar::{
    ActorId, ArtifactHandle, AuditCorrelationId, ByteCount, Bytes, Commitment, DurationMs,
    EpochIdentity, Opaque, OperationName, PageToken, ProtocolVersion, RequestId, Timestamp,
};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::ErrorCode;

/// A value the wire declares, and the two directions it travels.
///
/// `encode` is fallible because [`Opaque`] is: it carries bytes that must already be a
/// canonical value of the negotiated encoding, and a daemon that staged something else
/// into one has produced a value the wire cannot carry. Reporting that is better than
/// emitting bytes no reader can parse.
pub trait ProtocolValue: Sized {
    /// This value as a canonical document.
    ///
    /// # Errors
    ///
    /// [`CodecError`] when a carried `Opaque` is not itself canonical.
    fn encode(&self) -> Result<Json, CodecError>;

    /// Read this value from a canonical document.
    ///
    /// # Errors
    ///
    /// [`CodecError`] when the document is not this value's declared shape.
    fn decode(value: &Json) -> Result<Self, CodecError>;
}

/// Encode a protocol value to canonical bytes.
///
/// # Errors
///
/// [`CodecError`] when a carried `Opaque` is not canonical.
pub fn to_bytes<T: ProtocolValue>(value: &T) -> Result<Vec<u8>, CodecError> {
    Ok(value.encode()?.to_canonical_bytes())
}

/// Decode a protocol value from canonical bytes.
///
/// # Errors
///
/// [`CodecError`] when the bytes are not a canonical document, or not this value's shape.
pub fn from_bytes<T: ProtocolValue>(bytes: &[u8]) -> Result<T, CodecError> {
    T::decode(&Json::parse(bytes)?)
}

/// Carry a protocol value in an `Opaque` field.
///
/// # Errors
///
/// [`CodecError`] when the value carries a non-canonical `Opaque` of its own.
pub fn to_opaque<T: ProtocolValue>(value: &T) -> Result<Opaque, CodecError> {
    Ok(Opaque::from_bytes(to_bytes(value)?))
}

/// Read a protocol value out of an `Opaque` field.
///
/// # Errors
///
/// [`CodecError`] when the carried bytes are not a canonical document of that shape.
pub fn from_opaque<T: ProtocolValue>(opaque: &Opaque) -> Result<T, CodecError> {
    from_bytes(opaque.as_bytes())
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
    /// The bytes are not a canonical document.
    Json(JsonError),
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

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "{error}"),
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
pub fn object_of<'a>(
    value: &'a Json,
    expected: &'static str,
) -> Result<&'a BTreeMap<String, Json>, CodecError> {
    value.as_object().ok_or(CodecError::TypeMismatch {
        expected,
        found: value.kind(),
    })
}

/// Write a `required` field.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_required<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &T,
) -> Result<(), CodecError> {
    into.insert(name.to_owned(), value.encode()?);
    Ok(())
}

/// Write a `nullable` field: always present, `null` for a named absence.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_nullable<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &Nullable<T>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => Json::Null,
        Nullable::Value(inner) => inner.encode()?,
    };
    into.insert(name.to_owned(), encoded);
    Ok(())
}

/// Write an `optional` field: omitted when absent, never `null`.
///
/// # Errors
///
/// [`CodecError`] when the value cannot be encoded.
pub fn put_optional<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &Optional<T>,
) -> Result<(), CodecError> {
    if let Optional::Present(inner) = value {
        into.insert(name.to_owned(), inner.encode()?);
    }
    Ok(())
}

fn encode_list<T: ProtocolValue>(items: &[T]) -> Result<Json, CodecError> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(item.encode()?);
    }
    Ok(Json::Array(out))
}

fn decode_list<T: ProtocolValue>(value: &Json) -> Result<Vec<T>, CodecError> {
    let items = value.as_array().ok_or(CodecError::TypeMismatch {
        expected: "list",
        found: value.kind(),
    })?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(T::decode(item)?);
    }
    Ok(out)
}

fn encode_map<T: ProtocolValue>(entries: &BTreeMap<String, T>) -> Result<Json, CodecError> {
    let mut out = BTreeMap::new();
    for (key, value) in entries {
        out.insert(key.clone(), value.encode()?);
    }
    Ok(Json::Object(out))
}

fn decode_map<T: ProtocolValue>(value: &Json) -> Result<BTreeMap<String, T>, CodecError> {
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
pub fn put_required_list<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
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
pub fn put_nullable_list<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &Nullable<Vec<T>>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => Json::Null,
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
pub fn put_optional_list<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
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
pub fn put_required_map<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
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
pub fn put_nullable_map<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &Nullable<BTreeMap<String, T>>,
) -> Result<(), CodecError> {
    let encoded = match value {
        Nullable::Null => Json::Null,
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
pub fn put_optional_map<T: ProtocolValue>(
    into: &mut BTreeMap<String, Json>,
    name: &str,
    value: &Optional<BTreeMap<String, T>>,
) -> Result<(), CodecError> {
    if let Optional::Present(entries) = value {
        into.insert(name.to_owned(), encode_map(entries)?);
    }
    Ok(())
}

fn present<'a>(
    fields: &'a BTreeMap<String, Json>,
    declared_by: &'static str,
    field: &'static str,
) -> Result<&'a Json, CodecError> {
    fields
        .get(field)
        .ok_or(CodecError::MissingField { declared_by, field })
}

/// Read a `required` field: present and non-null.
///
/// # Errors
///
/// [`CodecError::MissingField`], [`CodecError::UnexpectedNull`], or a shape mismatch.
pub fn take_required<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_nullable<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_optional<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_required_list<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_nullable_list<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_optional_list<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_required_map<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_nullable_map<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
pub fn take_optional_map<T: ProtocolValue>(
    fields: &BTreeMap<String, Json>,
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
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::Bool(*self))
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        match value {
            Json::Bool(inner) => Ok(*inner),
            other => Err(CodecError::TypeMismatch {
                expected: "Bool",
                found: other.kind(),
            }),
        }
    }
}

impl ProtocolValue for u32 {
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::Integer(u64::from(*self)))
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        match value {
            Json::Integer(inner) => {
                Self::try_from(*inner).map_err(|_| CodecError::IntegerRange { expected: "U32" })
            }
            other => Err(CodecError::TypeMismatch {
                expected: "U32",
                found: other.kind(),
            }),
        }
    }
}

/// `U64` has two spellings and the *magnitude* picks which.
///
/// > `U64`. JSON encodes it as a number only when it is exactly representable; otherwise
/// > as a decimal string.
/// >
/// > — IDL §3
///
/// Both directions are strict: a decimal string inside the exactly-representable range is
/// rejected, because it would be a second spelling of a value that already has one, and a
/// number beyond the range is rejected by the parser before it reaches here.
impl ProtocolValue for u64 {
    fn encode(&self) -> Result<Json, CodecError> {
        if *self <= json::MAX_EXACT_INTEGER {
            Ok(Json::Integer(*self))
        } else {
            Ok(Json::String(self.to_string()))
        }
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        match value {
            Json::Integer(inner) => Ok(*inner),
            Json::String(text) => {
                let parsed: Self = text
                    .parse()
                    .map_err(|_| CodecError::IntegerRange { expected: "U64" })?;
                if parsed <= json::MAX_EXACT_INTEGER || text != &parsed.to_string() {
                    return Err(CodecError::IntegerRange { expected: "U64" });
                }
                Ok(parsed)
            }
            other => Err(CodecError::TypeMismatch {
                expected: "U64",
                found: other.kind(),
            }),
        }
    }
}

impl ProtocolValue for String {
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::String(self.clone()))
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or(CodecError::TypeMismatch {
                expected: "String",
                found: value.kind(),
            })
    }
}

/// `Bytes` is base64url without padding in JSON, and a byte string in CBOR.
impl ProtocolValue for Bytes {
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::String(base64url(self)))
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        let text = value.as_str().ok_or(CodecError::TypeMismatch {
            expected: "Bytes",
            found: value.kind(),
        })?;
        from_base64url(text).map_err(|_| CodecError::Bytes)
    }
}

/// An `Opaque` is a canonical value carried verbatim: the codec neither interprets it nor
/// re-shapes it (`rule encoding.opaque_payloads`).
impl ProtocolValue for Opaque {
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::parse(self.as_bytes())?)
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        Ok(Self::from_bytes(value.to_canonical_bytes()))
    }
}

/// Emit [`ProtocolValue`] for a string newtype whose constructor enforces its pattern.
macro_rules! string_value {
    ($name:ty, $declared:literal, $read:ident, $write:ident) => {
        impl ProtocolValue for $name {
            fn encode(&self) -> Result<Json, CodecError> {
                Ok(Json::String(self.$write().to_owned()))
            }

            fn decode(value: &Json) -> Result<Self, CodecError> {
                let text = value.as_str().ok_or(CodecError::TypeMismatch {
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
            fn encode(&self) -> Result<Json, CodecError> {
                Ok(Json::String(self.as_str().to_owned()))
            }

            fn decode(value: &Json) -> Result<Self, CodecError> {
                let text = value.as_str().ok_or(CodecError::TypeMismatch {
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
    fn encode(&self) -> Result<Json, CodecError> {
        Ok(Json::String(self.to_string()))
    }

    fn decode(value: &Json) -> Result<Self, CodecError> {
        let text = value.as_str().ok_or(CodecError::TypeMismatch {
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
            fn encode(&self) -> Result<Json, CodecError> {
                ProtocolValue::encode(&self.$write())
            }

            fn decode(value: &Json) -> Result<Self, CodecError> {
                Ok(Self::new(<u64 as ProtocolValue>::decode(value)?))
            }
        }
    };
}

integer_value!(DurationMs, millis);
integer_value!(ByteCount, bytes);
