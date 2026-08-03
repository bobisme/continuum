//! Primitives, handles, and string aliases (IDL §3 and §4).
//!
//! # Why the IDL's names are the Rust names
//!
//! Each IDL primitive is a Rust type of the same name — `Bool`, `U32`, `U64`, `Bytes`
//! are aliases, the rest are newtypes. That is not cosmetic: [`protocol_struct!`] records
//! a field's declared type by `stringify!`ing the token the field was declared with, so
//! the type in the struct and the type in the [`FieldSpec`] are one token. A mapping
//! table between "IDL type" and "Rust type" would be a second place for the two to
//! disagree, and there isn't one.
//!
//! # Handles are identifiers, not secrets — with one exception
//!
//! > Content-addressed identities are derivable by anyone holding the content, so
//! > handles are identifiers, not secrets, and confer no authority; authorization is
//! > always checked independently of handle possession (plan §4.4, ADR-0037). Capability
//! > tokens (`cap_*`) are the sole exception: they are minted randomly and DO confer
//! > authority.
//! >
//! > — IDL §4
//!
//! [`CapabilityHandle`] therefore carries a deliberately reticent
//! [`Debug`](core::fmt::Debug): a capability "MUST NOT be logged in request traces, MUST
//! NOT appear in error text or in `next_operations` arguments" (RFC 0026, "Transport,
//! encoding, authentication"), and a derived `Debug` is how a secret reaches a log.
//!
//! # What is validated here, and what is not
//!
//! Every `@pattern` the IDL declares is enforced by the corresponding constructor, so a
//! malformed identifier is a typed rejection at the boundary rather than a string that
//! travels. Nothing here bounds *length*: the IDL fixes no length limit on any
//! identifier, `ServerLimits` bounds result bytes rather than field widths, and
//! inventing a cap would be inventing wire semantics. Callers bound their input.
//!
//! [`protocol_struct!`]: crate::protocol_struct
//! [`FieldSpec`]: crate::protocol::spec::FieldSpec

use core::fmt;

use crate::protocol::spec::ProtocolHandle;

pub use continuum_value::epoch::{EpochIdentity, EpochIdentityError};

/// Boolean. JSON `true`/`false`; CBOR major type 7 simple values 20/21.
pub type Bool = bool;

/// Unsigned 32-bit integer.
pub type U32 = u32;

/// Unsigned 64-bit integer.
///
/// JSON encodes it as a number only when it is exactly representable; otherwise as a
/// decimal string. CBOR uses an unsigned integer.
pub type U64 = u64;

/// Opaque byte string. JSON encodes it base64url without padding; CBOR uses a byte
/// string.
pub type Bytes = Vec<u8>;

/// The protocol version, `MAJOR.MINOR`.
///
/// > `ProtocolEpoch::negotiate` is the typed form of this rule and `ProtocolWindow` of
/// > the major window; the daemon and that module MUST agree.
/// >
/// > — RFC 0026, "Version window and negotiation"
///
/// This is an alias rather than a newtype so that "agree" is not something anyone has to
/// maintain: there is one type, `continuum_value::epoch::ProtocolEpoch`, whose
/// parse/render round-trip is exactly the IDL's
/// `@pattern("^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")`.
pub type ProtocolVersion = continuum_value::epoch::ProtocolEpoch;

/// IETF RFC3339 timestamp in UTC with millisecond precision.
///
/// The IDL fixes all three: RFC3339 syntax, the `Z` offset, and three fractional
/// digits. The spelling is therefore `YYYY-MM-DDTHH:MM:SS.sssZ` and nothing else, and
/// [`Timestamp::new`] rejects any other. Field ranges are checked (month `01`-`12`, day
/// `01`-`31`, hour `00`-`23`, minute `00`-`59`, second `00`-`60` for a leap second);
/// calendar validity is not, because the IDL states no calendar rule and a rejection
/// this type invented would be a rejection no client could have predicted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(String);

impl Timestamp {
    /// Parse a timestamp in the one spelling the IDL fixes.
    ///
    /// # Errors
    ///
    /// [`TimestampError`] when the text is not `YYYY-MM-DDTHH:MM:SS.sssZ`, or when a
    /// field is out of range.
    pub fn new(text: &str) -> Result<Self, TimestampError> {
        let bytes = text.as_bytes();
        if bytes.len() != 24 {
            return Err(TimestampError::NotCanonical);
        }
        let shape = b"____-__-__T__:__:__.___Z";
        for (index, expected) in shape.iter().enumerate() {
            let actual = bytes[index];
            let ok = match expected {
                b'_' => actual.is_ascii_digit(),
                other => actual == *other,
            };
            if !ok {
                return Err(TimestampError::NotCanonical);
            }
        }
        let field = |from: usize, to: usize| -> u32 {
            text[from..to]
                .parse()
                .expect("the shape check accepted only ASCII digits here")
        };
        let (month, day) = (field(5, 7), field(8, 10));
        let (hour, minute, second) = (field(11, 13), field(14, 16), field(17, 19));
        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 60
        {
            return Err(TimestampError::OutOfRange);
        }
        Ok(Self(text.to_owned()))
    }

    /// The canonical spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a string is not a protocol timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampError {
    /// The text is not `YYYY-MM-DDTHH:MM:SS.sssZ`.
    NotCanonical,
    /// A field is outside its range.
    OutOfRange,
}

impl fmt::Display for TimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCanonical => {
                f.write_str("timestamp is not RFC3339 UTC with millisecond precision")
            }
            Self::OutOfRange => f.write_str("timestamp has a field outside its range"),
        }
    }
}

impl core::error::Error for TimestampError {}

/// Non-negative duration in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DurationMs(u64);

impl DurationMs {
    /// A duration of `millis` milliseconds.
    #[must_use]
    pub const fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// The duration in milliseconds.
    #[must_use]
    pub const fn millis(self) -> u64 {
        self.0
    }
}

/// Non-negative byte count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteCount(u64);

impl ByteCount {
    /// A count of `bytes` bytes.
    #[must_use]
    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    /// The count in bytes.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.0
    }
}

/// A structured value whose shape is defined by a named schema or by a struct declared
/// in the IDL, carried inline.
///
/// > `Opaque` never means "free-form": there is no untyped payload anywhere in this
/// > protocol.
/// >
/// > — IDL §3
///
/// The governing schema is named by the carrying field's doc comment. Declaring it in
/// the IDL instead — an `@schema("<class identity>")` annotation the generator could
/// check against `schemas/` — is the IDL's own open item 3, and until it is closed there
/// is no machine-readable statement of which schema governs which field. This type
/// therefore carries bytes and does not pretend to know their shape; it deliberately
/// offers no accessor that interprets them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Opaque(Vec<u8>);

impl Opaque {
    /// Carry `bytes` as a schema-governed payload.
    #[must_use]
    pub const fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The carried bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

// --- string aliases ---------------------------------------------------------------

/// Why a string does not satisfy an alias's `@pattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternMismatch {
    /// The alias or handle class the string was offered to.
    pub declared: &'static str,
    /// The `@pattern` it failed, or the handle's prefix rule.
    pub pattern: &'static str,
}

impl fmt::Display for PatternMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` requires {} and the offered string does not match",
            self.declared, self.pattern
        )
    }
}

impl core::error::Error for PatternMismatch {}

const fn is_opaque_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// A content commitment. The IDL declares no pattern for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Commitment(String);

impl Commitment {
    /// Carry `text` as a commitment.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// The commitment's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An opaque pagination cursor. The IDL declares no pattern for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageToken(String);

impl PageToken {
    /// Carry `text` as a page token.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// The token's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A client-unique request identifier, `@pattern("^req_[A-Za-z0-9_-]+$")`.
///
/// It is a tracing identifier, never an artifact identity (RFC 0026, "Request
/// envelope").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(String);

impl RequestId {
    /// The pattern this alias declares.
    pub const PATTERN: &'static str = "^req_[A-Za-z0-9_-]+$";

    /// Parse a request identifier.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the text does not match [`PATTERN`](RequestId::PATTERN).
    pub fn new(text: &str) -> Result<Self, PatternMismatch> {
        let mismatch = PatternMismatch {
            declared: "RequestId",
            pattern: Self::PATTERN,
        };
        let rest = text.strip_prefix("req_").ok_or(mismatch)?;
        if rest.is_empty() || !rest.bytes().all(is_opaque_byte) {
            return Err(mismatch);
        }
        Ok(Self(text.to_owned()))
    }

    /// The identifier's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An actor identity, `@pattern("^(agent|human|service|ci):[A-Za-z0-9._:-]+$")`.
///
/// The four schemes are a closed set (RFC 0026, "Request envelope"): an unrecognized
/// scheme is malformed, never a new kind of actor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorId(String);

impl ActorId {
    /// The pattern this alias declares.
    pub const PATTERN: &'static str = "^(agent|human|service|ci):[A-Za-z0-9._:-]+$";

    /// The four schemes, in the IDL's declaration order.
    pub const SCHEMES: [&'static str; 4] = ["agent", "human", "service", "ci"];

    /// Parse an actor identity.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the scheme is not one of [`SCHEMES`](ActorId::SCHEMES) or
    /// the remainder is empty or contains a character outside `[A-Za-z0-9._:-]`.
    pub fn new(text: &str) -> Result<Self, PatternMismatch> {
        let mismatch = PatternMismatch {
            declared: "ActorId",
            pattern: Self::PATTERN,
        };
        let (scheme, rest) = text.split_once(':').ok_or(mismatch)?;
        if !Self::SCHEMES.contains(&scheme) {
            return Err(mismatch);
        }
        let tail_ok =
            |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-');
        if rest.is_empty() || !rest.bytes().all(tail_ok) {
            return Err(mismatch);
        }
        Ok(Self(text.to_owned()))
    }

    /// The identity's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The scheme half.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0
            .split_once(':')
            .expect("a constructed ActorId contains a colon")
            .0
    }
}

/// An operation name, `@pattern("^[a-z]+\.[a-z_]+$")`.
///
/// The pattern is the wire constraint. Being a *registered* operation is a stronger
/// property, and [`registered`](OperationName::registered) is where a caller asks for it;
/// the two are deliberately separate, because an unregistered but well-formed name is
/// `MalformedRequest` at a different layer than a name that is not a name at all.
///
/// [`registered`]: OperationName::registered
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationName(String);

impl OperationName {
    /// The pattern this alias declares.
    pub const PATTERN: &'static str = r"^[a-z]+\.[a-z_]+$";

    /// Parse an operation name.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the text is not `namespace.verb` in the declared
    /// alphabets.
    pub fn new(text: &str) -> Result<Self, PatternMismatch> {
        let mismatch = PatternMismatch {
            declared: "OperationName",
            pattern: Self::PATTERN,
        };
        let (namespace, verb) = text.split_once('.').ok_or(mismatch)?;
        if namespace.is_empty() || !namespace.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err(mismatch);
        }
        if verb.is_empty() || !verb.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
            return Err(mismatch);
        }
        Ok(Self(text.to_owned()))
    }

    /// Parse a name and require that the registry declares it.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the text is not a well-formed name, and [`None`] through
    /// the [`Option`] when it is well-formed but not one of the 73 declared operations.
    pub fn registered(text: &str) -> Result<Option<Self>, PatternMismatch> {
        let name = Self::new(text)?;
        Ok(crate::protocol::registry::operation(name.as_str()).map(|_| name))
    }

    /// The name's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The correlation identity of an audit-log record,
/// `@pattern("^[A-Za-z0-9_-]+$")`, `@since("3.1")`.
///
/// > Deliberately not a handle and not a plan §4.4 artifact class: the audit log is not
/// > the content-addressed store, this identity is not dereferenceable through any
/// > operation in this file, and it carries no class prefix that would suggest otherwise.
/// >
/// > — IDL §4
///
/// It is therefore not a [`ProtocolHandle`] and has no prefix rule. The pattern admits no
/// `_`-separated class run *requirement*, which is the difference from
/// [`ArtifactHandle`]: `receipt_1` satisfies both patterns, and only the carrying field
/// says which one it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuditCorrelationId(String);

impl AuditCorrelationId {
    /// The pattern this alias declares.
    pub const PATTERN: &'static str = "^[A-Za-z0-9_-]+$";

    /// Parse an audit-correlation identity.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the text is empty or carries a character outside
    /// `[A-Za-z0-9_-]`.
    pub fn new(text: &str) -> Result<Self, PatternMismatch> {
        let mismatch = PatternMismatch {
            declared: "AuditCorrelationId",
            pattern: Self::PATTERN,
        };
        if text.is_empty() || !text.bytes().all(is_opaque_byte) {
            return Err(mismatch);
        }
        Ok(Self(text.to_owned()))
    }

    /// The identity's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A handle of any artifact class,
/// `@pattern("^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$")`.
///
/// Used where an operation accepts more than one class — `debug.open`'s `subject`,
/// `program.replay`'s `recording`, `query.explain_reuse`'s `derivation`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactHandle(String);

impl ArtifactHandle {
    /// The pattern this alias declares.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$";

    /// Parse a class-agnostic artifact handle.
    ///
    /// # Errors
    ///
    /// [`PatternMismatch`] when the text has no lowercase class prefix ending in `_`, or
    /// an empty or ill-formed opaque part.
    pub fn new(text: &str) -> Result<Self, PatternMismatch> {
        let mismatch = PatternMismatch {
            declared: "ArtifactHandle",
            pattern: Self::PATTERN,
        };
        let bytes = text.as_bytes();
        // `a_b` is the shortest string the pattern accepts.
        if bytes.len() < 3 || !bytes[0].is_ascii_lowercase() {
            return Err(mismatch);
        }
        if !bytes[1..].iter().copied().all(is_opaque_byte) {
            return Err(mismatch);
        }
        // The class run is the maximal prefix over `[a-z0-9_]`; the separator is an
        // underscore inside it that is not the final character.
        let class_run = bytes
            .iter()
            .position(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
            .unwrap_or(bytes.len());
        let searchable = class_run.min(bytes.len() - 1);
        if !bytes[1..searchable].contains(&b'_') {
            return Err(mismatch);
        }
        Ok(Self(text.to_owned()))
    }

    /// The handle's spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// --- handles ------------------------------------------------------------------------

/// Declare a handle class: a newtype whose constructor enforces the IDL's prefix rule.
///
/// The `@redacted` form omits the derived [`Debug`](core::fmt::Debug) so the class can
/// supply a reticent one. It exists for `cap_*`, the one class that confers authority,
/// and should not acquire a second user without a rule saying that class is a secret.
macro_rules! protocol_handle {
    (
        $(#[doc = $handle_doc:literal])*
        $name:ident = $prefix:literal
    ) => {
        $(#[doc = $handle_doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        protocol_handle!(@body $name = $prefix);
    };

    (
        @redacted
        $(#[doc = $handle_doc:literal])*
        $name:ident = $prefix:literal
    ) => {
        $(#[doc = $handle_doc])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        protocol_handle!(@body $name = $prefix);
    };

    (@body $name:ident = $prefix:literal) => {
        impl $name {
            #[doc = concat!("Parse a `", $prefix, "` handle.")]
            ///
            /// # Errors
            ///
            /// [`PatternMismatch`] when the prefix is absent or the opaque part is empty
            /// or contains a character outside `[A-Za-z0-9_-]`.
            pub fn new(text: &str) -> Result<Self, PatternMismatch> {
                let mismatch = PatternMismatch {
                    declared: stringify!($name),
                    pattern: concat!("`", $prefix, "` followed by `[A-Za-z0-9_-]+`"),
                };
                let rest = text.strip_prefix($prefix).ok_or(mismatch)?;
                if rest.is_empty() || !rest.bytes().all(is_opaque_byte) {
                    return Err(mismatch);
                }
                Ok(Self(text.to_owned()))
            }

            /// The handle's spelling.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl ProtocolHandle for $name {
            const HANDLE_NAME: &'static str = stringify!($name);
            const PREFIX: &'static str = $prefix;
        }

        impl $crate::codec::ProtocolValue for $name {
            fn encode<D: $crate::codec::Document>(
                &self,
            ) -> ::core::result::Result<D, $crate::codec::CodecError> {
                ::core::result::Result::Ok(
                    <D as $crate::codec::Document>::from_text(self.as_str()),
                )
            }

            fn decode<D: $crate::codec::Document>(
                value: &D,
            ) -> ::core::result::Result<Self, $crate::codec::CodecError> {
                let text = <D as $crate::codec::Document>::as_text(value).ok_or(
                    $crate::codec::CodecError::TypeMismatch {
                        expected: stringify!($name),
                        found: <D as $crate::codec::Document>::kind(value),
                    },
                )?;
                // The prefix rule is the constructor's, not a second copy of it here.
                Self::new(text).map_err(|_| $crate::codec::CodecError::Pattern {
                    declared: stringify!($name),
                })
            }
        }
    };
}

protocol_handle! {
    /// Workspace snapshot (`schemas/workspace-snapshot.schema.json`).
    WorkspaceHandle = "ws_"
}
protocol_handle! {
    /// Intent Contract (`schemas/intent-contract.schema.json`).
    IntentHandle = "in_"
}
protocol_handle! {
    /// Signed, content-addressed intent bundle (plan §4.2.1).
    IntentBundleHandle = "inb_"
}
protocol_handle! {
    /// Elaborated model.
    ModelHandle = "model_"
}
protocol_handle! {
    /// Causal execution graph (`schemas/cir.schema.json`).
    CausalGraphHandle = "cir_"
}
protocol_handle! {
    /// Crashpack (`schemas/crashpack.schema.json`).
    CrashpackHandle = "crash_"
}
protocol_handle! {
    /// Context Pack (`schemas/context-pack.schema.json`).
    ContextHandle = "ctx_"
}
protocol_handle! {
    /// Proof artifact.
    ProofArtifactHandle = "proof_"
}
protocol_handle! {
    /// Proof state.
    ProofStateHandle = "ps_"
}
protocol_handle! {
    /// Verification task (`schemas/verification-task.schema.json`).
    TaskHandle = "task_"
}
protocol_handle! {
    /// Evidence-graph node or edge.
    EvidenceHandle = "ev_"
}
protocol_handle! {
    /// Causal debugger branch.
    DebugHandle = "dbg_"
}
protocol_handle! {
    /// Promotion or proof receipt.
    ReceiptHandle = "receipt_"
}
protocol_handle! {
    /// Repair transaction (`schemas/repair-transaction.schema.json`).
    RepairHandle = "rt_"
}
protocol_handle! {
    /// Forge synthesis archive.
    ForgeHandle = "forge_"
}
protocol_handle! {
    /// Continuation of a suspended task.
    ContinuationHandle = "cont_"
}
protocol_handle! {
    @redacted
    /// Capability token. The one class that is not content-addressed, and the one that
    /// confers authority: see this module's documentation.
    CapabilityHandle = "cap_"
}
protocol_handle! {
    /// Semantic or intent diff (`schemas/semantic-diff.schema.json`).
    DiffHandle = "diff_"
}
protocol_handle! {
    /// Engine-defect report (plan §4.7).
    DefectHandle = "defect_"
}

impl fmt::Debug for CapabilityHandle {
    /// Redacts the token.
    ///
    /// A capability is a secret that "MUST NOT be logged in request traces, MUST NOT
    /// appear in error text or in `next_operations` arguments" (RFC 0026). A derived
    /// `Debug` is the shortest path from that rule to a log line that breaks it, so this
    /// one prints the class and nothing else.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapabilityHandle(<redacted>)")
    }
}
