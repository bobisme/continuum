//! The decoders this harness drives, and the closed landing vocabulary of each.
//!
//! # What a target is
//!
//! A pure function from `&[u8]` to a [`Probe`]: where the input landed, spelled as a
//! token from a vocabulary the target declares up front, plus the re-encoding the
//! canonicity oracle needs. Nothing here decides whether an input is *good*; the oracles
//! in `engine.rs` do that, and they apply to every target identically.
//!
//! # Why the landing token exists
//!
//! Two reasons, and both are requirements rather than conveniences.
//!
//! 1. **A seed has to demonstrably reach the decoder it targets.** A "duplicate id" case
//!    that the frame layer refuses before the duplicate is ever compared is not coverage
//!    of duplicate-id handling. Every committed corpus case therefore declares the token
//!    it must land at, and `wire_fuzz.rs` asserts it — so a seed that starts landing
//!    somewhere shallower fails the gate instead of silently becoming decoration.
//! 2. **INV-008.** The vocabulary is closed and checked. A decoder that grew an answer
//!    outside it — or collapsed two answers into one — fails the [`Oracle::Untyped`]
//!    oracle rather than being quietly re-interpreted here.
//!
//! # Which surfaces are covered
//!
//! The four the bone names, each reached through its real public entry point:
//!
//! | surface | targets |
//! |---|---|
//! | native protocol frames | `frame.length-prefix`, `frame.request-envelope.json`, `frame.request-envelope.cbor` |
//! | canonical decoders | `canonical.json`, `canonical.cbor`, `canonical.value`, `canonical.intent-json` |
//! | snapshot / intent schemas | `schema.snapshot`, `schema.intent-contract` |
//! | wire-form certificates | `certificate.wire` |
//!
//! Every one of them is hand-rolled in this workspace — there is no `serde`, no
//! `serde_json` and no `ciborium` anywhere in the dependency graph — so no upstream
//! library is absorbing malformed input on their behalf.

use continuum_certificate::continuum_kernel_core::{Verdict as CoreVerdict, wire as core_wire};
use continuum_certificate::{
    Outcome, RoutingFault, check_certificate, continuum_kernel_sat, continuum_kernel_smt,
    continuum_kernel_temporal,
};
use continuum_intent::canonical_json::JsonError as IntentJsonError;
use continuum_intent::contract::{ContractDecodeError, IntentContract};
use continuum_intent::property::{PropertyDecodeError, PropertyError};
use continuum_value::value::{DecodeError as ValueDecodeError, Value};
use continuum_workspace::snapshot::{Snapshot, SnapshotDecodeError};
use continuumd::codec::cbor::{Cbor, CborError};
use continuumd::codec::json::{Json, JsonError};
use continuumd::codec::{CodecError, from_bytes, from_cbor_bytes, to_bytes, to_cbor_bytes};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::RequestEnvelope;
use continuumd::transport::{Channel, FrameError};

use super::outcome::{Budget, Probe};

/// The landing token every target uses for "this input decoded".
pub const ACCEPTED: &str = "accepted";

/// One decoder under test.
pub trait Target: Sync {
    /// The stable machine name. Also the corpus subdirectory.
    fn name(&self) -> &'static str;

    /// The resource ceilings this target runs under.
    fn budget(&self) -> Budget {
        Budget::DEFAULT
    }

    /// Every token [`Target::probe`] is allowed to return.
    ///
    /// Closed. A landing outside it is an [`super::outcome::Oracle::Untyped`] defect.
    fn vocabulary(&self) -> &'static [&'static str];

    /// Decode `bytes` and say where they landed.
    ///
    /// MUST be a pure function of `bytes` — the determinism oracle checks it.
    fn probe(&self, bytes: &[u8]) -> Probe;
}

/// Every target, in a fixed order. The order is part of the campaign's determinism.
#[must_use]
pub fn all() -> Vec<&'static dyn Target> {
    vec![
        &FrameLengthPrefix,
        &RequestEnvelopeJson,
        &RequestEnvelopeCbor,
        &CanonicalJson,
        &CanonicalCbor,
        &CanonicalValue,
        &IntentCanonicalJson,
        &SchemaSnapshot,
        &SchemaIntentContract,
        &CertificateWire,
    ]
}

/// The target with this name, when there is one.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static dyn Target> {
    all().into_iter().find(|target| target.name() == name)
}

// --- shared landing spellings --------------------------------------------------------
//
// `continuumd`'s two document readers are used both on their own and underneath the
// envelope decoder, so their vocabularies are spelled once and shared. That is what lets
// a corpus case say "this envelope seed lands in the JSON layer, at the duplicate-key
// rule" rather than only "the envelope refused it".

/// Tokens `continuumd::codec::json::JsonError` can produce.
pub const JSON_LANDINGS: &[&str] = &[
    "json::not-utf8",
    "json::truncated",
    "json::trailing-bytes",
    "json::too-deep",
    "json::duplicate-key",
    "json::key-order",
    "json::unexpected",
    "json::floating-point",
    "json::signed",
    "json::leading-zero",
    "json::integer-range",
    "json::raw-control",
    "json::bad-escape",
];

const fn json_landing(error: &JsonError) -> &'static str {
    match error {
        JsonError::NotUtf8 => "json::not-utf8",
        JsonError::Truncated => "json::truncated",
        JsonError::TrailingBytes => "json::trailing-bytes",
        JsonError::TooDeep => "json::too-deep",
        JsonError::DuplicateKey { .. } => "json::duplicate-key",
        JsonError::KeyOrder { .. } => "json::key-order",
        JsonError::Unexpected { .. } => "json::unexpected",
        JsonError::FloatingPoint { .. } => "json::floating-point",
        JsonError::Signed { .. } => "json::signed",
        JsonError::LeadingZero { .. } => "json::leading-zero",
        JsonError::IntegerRange { .. } => "json::integer-range",
        JsonError::RawControl { .. } => "json::raw-control",
        JsonError::BadEscape { .. } => "json::bad-escape",
    }
}

/// Tokens `continuumd::codec::cbor::CborError` can produce.
pub const CBOR_LANDINGS: &[&str] = &[
    "cbor::truncated",
    "cbor::trailing-bytes",
    "cbor::too-deep",
    "cbor::duplicate-key",
    "cbor::key-order",
    "cbor::non-text-key",
    "cbor::indefinite-length",
    "cbor::non-shortest-head",
    "cbor::negative-integer",
    "cbor::floating-point",
    "cbor::tag",
    "cbor::simple-value",
    "cbor::reserved",
    "cbor::not-utf8",
];

const fn cbor_landing(error: &CborError) -> &'static str {
    match error {
        CborError::Truncated => "cbor::truncated",
        CborError::TrailingBytes => "cbor::trailing-bytes",
        CborError::TooDeep => "cbor::too-deep",
        CborError::DuplicateKey { .. } => "cbor::duplicate-key",
        CborError::KeyOrder { .. } => "cbor::key-order",
        CborError::NonTextKey { .. } => "cbor::non-text-key",
        CborError::IndefiniteLength { .. } => "cbor::indefinite-length",
        CborError::NonShortestHead { .. } => "cbor::non-shortest-head",
        CborError::NegativeInteger { .. } => "cbor::negative-integer",
        CborError::FloatingPoint { .. } => "cbor::floating-point",
        CborError::Tag { .. } => "cbor::tag",
        CborError::SimpleValue { .. } => "cbor::simple-value",
        CborError::Reserved { .. } => "cbor::reserved",
        CborError::NotUtf8 { .. } => "cbor::not-utf8",
    }
}

/// Tokens the envelope decoder adds above the document layer.
const CODEC_LANDINGS: &[&str] = &[
    "codec::missing-field",
    "codec::unexpected-null",
    "codec::type-mismatch",
    "codec::pattern",
    "codec::unknown-member",
    "codec::unknown-open-member",
    "codec::union-arity",
    "codec::unknown-variant",
    "codec::bytes",
    "codec::integer-range",
    "codec::unknown-operation",
    "codec::undeclared-error-data",
];

const fn codec_landing(error: &CodecError) -> &'static str {
    match error {
        CodecError::Json(inner) => json_landing(inner),
        CodecError::Cbor(inner) => cbor_landing(inner),
        CodecError::MissingField { .. } => "codec::missing-field",
        CodecError::UnexpectedNull { .. } => "codec::unexpected-null",
        CodecError::TypeMismatch { .. } => "codec::type-mismatch",
        CodecError::Pattern { .. } => "codec::pattern",
        CodecError::UnknownMember { .. } => "codec::unknown-member",
        CodecError::UnknownOpenMember { .. } => "codec::unknown-open-member",
        CodecError::UnionArity { .. } => "codec::union-arity",
        CodecError::UnknownVariant { .. } => "codec::unknown-variant",
        CodecError::Bytes => "codec::bytes",
        CodecError::IntegerRange { .. } => "codec::integer-range",
        CodecError::UnknownOperation => "codec::unknown-operation",
        CodecError::UndeclaredErrorData => "codec::undeclared-error-data",
    }
}

/// Concatenate landing groups into one closed vocabulary.
macro_rules! vocabulary {
    ($($group:expr),* $(,)?) => {{
        // A `&'static [&'static str]` built once and leaked deliberately: the vocabulary
        // is a compile-time constant of the target and outlives every probe.
        static CELL: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        CELL.get_or_init(|| {
            let mut out: Vec<&'static str> = vec![ACCEPTED];
            $(out.extend_from_slice($group);)*
            out.sort_unstable();
            out.dedup();
            out
        })
        .as_slice()
    }};
}

// --- native protocol frames ----------------------------------------------------------

/// The length-prefixed transport framing (`continuumd::transport`).
///
/// The input is a *stream*, not a message: the reader is handed the whole byte string and
/// asked for one frame. That is where the oversized class belongs, because a four-byte
/// prefix is all it costs to declare a sixteen-megabyte body — the input stays linear and
/// the decoder's `MAX_FRAME_BYTES` check is reached before any allocation.
#[derive(Debug)]
pub struct FrameLengthPrefix;

const FRAME_LANDINGS: &[&str] = &[
    "frame::too-large",
    "frame::incomplete",
    "frame::trailing-bytes",
];

impl Target for FrameLengthPrefix {
    fn name(&self) -> &'static str {
        "frame.length-prefix"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(FRAME_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        let mut channel = Channel::new();
        channel.put_bytes(bytes);
        match channel.take_frame() {
            Err(FrameError::TooLarge) => Probe::rejected("frame::too-large"),
            Ok(None) => Probe::rejected("frame::incomplete"),
            Ok(Some(payload)) => {
                if !channel.is_empty() {
                    // A stream carrying more than one frame is a stream, not a message.
                    // Accepting it here would make the canonicity oracle compare a
                    // re-framing of the first frame against the whole input.
                    return Probe::rejected("frame::trailing-bytes");
                }
                let mut reframed = Channel::new();
                match reframed.put_frame(&payload) {
                    Ok(()) => {
                        let mut out = Vec::with_capacity(payload.len() + 4);
                        // `Channel` has no byte accessor, so the framing is re-derived
                        // the only way a reader can: the four-byte big-endian prefix the
                        // module documents, then the payload.
                        let length = u32::try_from(payload.len()).unwrap_or(u32::MAX);
                        out.extend_from_slice(&length.to_be_bytes());
                        out.extend_from_slice(&payload);
                        Probe::canonical(out)
                    }
                    // A payload the writer refuses cannot have come out of the reader;
                    // reporting an empty re-encoding makes the round-trip oracle fire.
                    Err(FrameError::TooLarge) => Probe::canonical(Vec::new()),
                }
            }
        }
    }
}

/// `RequestEnvelope` decoded from canonical JSON (`continuumd::codec`).
#[derive(Debug)]
pub struct RequestEnvelopeJson;

impl Target for RequestEnvelopeJson {
    fn name(&self) -> &'static str {
        "frame.request-envelope.json"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(JSON_LANDINGS, CBOR_LANDINGS, CODEC_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match from_bytes::<RequestEnvelope>(bytes) {
            Err(error) => Probe::rejected(codec_landing(&error)),
            // A normal form, not a byte identity, and RFC 0026 is why: "an unknown
            // `optional` request field ⇒ ignored, request served". So an envelope
            // carrying a field this version does not declare decodes to the same value as
            // one without it, and two byte strings legitimately denote one message. The
            // first campaign run over this target found exactly that — a mutation that
            // misspelled `idempotency_key` was accepted and re-encoded without it — which
            // is the rule working, not a defect. What is still owed, and is checked, is
            // that the normal form is a fixpoint.
            Ok(envelope) => match to_bytes(&envelope) {
                Ok(reencoded) => Probe::normal_form(reencoded),
                // Accepted but not re-encodable is a round-trip defect either way.
                Err(_) => Probe::canonical(Vec::new()),
            },
        }
    }
}

/// `RequestEnvelope` decoded from canonical CBOR (`continuumd::codec`).
///
/// RFC 0026 requires malformed-input fuzzing to run on **both** encodings; this is the
/// other half of [`RequestEnvelopeJson`].
#[derive(Debug)]
pub struct RequestEnvelopeCbor;

impl Target for RequestEnvelopeCbor {
    fn name(&self) -> &'static str {
        "frame.request-envelope.cbor"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(JSON_LANDINGS, CBOR_LANDINGS, CODEC_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match from_cbor_bytes::<RequestEnvelope>(bytes) {
            Err(error) => Probe::rejected(codec_landing(&error)),
            // A normal form for the same reason as the JSON half: `rule
            // versioning.unknown_fields` makes an unknown `optional` field ignorable.
            Ok(envelope) => match to_cbor_bytes(&envelope) {
                Ok(reencoded) => Probe::normal_form(reencoded),
                Err(_) => Probe::canonical(Vec::new()),
            },
        }
    }
}

// --- canonical decoders --------------------------------------------------------------

/// The `canonical_json` document reader (`continuumd::codec::json`).
#[derive(Debug)]
pub struct CanonicalJson;

impl Target for CanonicalJson {
    fn name(&self) -> &'static str {
        "canonical.json"
    }

    /// A tighter ceiling than the default, and the reason is in the decoder: `Json::parse`
    /// collects its input into a `Vec<char>` before the grammar runs, so the parser's peak
    /// footprint is four times the byte string it is given. 2 KiB of input is 8 KiB of
    /// parser state — bounded, explicit, and nowhere near the 16 MiB a frame may carry.
    fn budget(&self) -> Budget {
        Budget {
            input_bytes: 2048,
            ..Budget::DEFAULT
        }
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(JSON_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match Json::parse(bytes) {
            Err(error) => Probe::rejected(json_landing(&error)),
            Ok(document) => Probe::canonical(document.to_canonical_bytes()),
        }
    }
}

/// The `canonical_cbor` document reader (`continuumd::codec::cbor`).
#[derive(Debug)]
pub struct CanonicalCbor;

impl Target for CanonicalCbor {
    fn name(&self) -> &'static str {
        "canonical.cbor"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(CBOR_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match Cbor::parse(bytes) {
            Err(error) => Probe::rejected(cbor_landing(&error)),
            Ok(document) => Probe::canonical(document.to_canonical_bytes()),
        }
    }
}

/// CVNF-1, the canonical value encoding (`continuum_value::value`).
#[derive(Debug)]
pub struct CanonicalValue;

const VALUE_LANDINGS: &[&str] = &[
    "value::unexpected-end",
    "value::trailing-bytes",
    "value::unknown-tag",
    "value::invalid-bool",
    "value::invalid-sign",
    "value::non-minimal-integer",
    "value::negative-zero",
    "value::integer-too-wide",
    "value::length-too-large",
    "value::invalid-utf8",
    "value::invalid-name",
    "value::invalid-bitvec",
    "value::not-ascending",
    "value::invalid-multiplicity",
    "value::depth-exceeded",
];

impl Target for CanonicalValue {
    fn name(&self) -> &'static str {
        "canonical.value"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(VALUE_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match Value::decode(bytes) {
            Err(error) => Probe::rejected(match error {
                ValueDecodeError::UnexpectedEnd { .. } => "value::unexpected-end",
                ValueDecodeError::TrailingBytes { .. } => "value::trailing-bytes",
                ValueDecodeError::UnknownTag { .. } => "value::unknown-tag",
                ValueDecodeError::InvalidBool { .. } => "value::invalid-bool",
                ValueDecodeError::InvalidSign { .. } => "value::invalid-sign",
                ValueDecodeError::NonMinimalInteger { .. } => "value::non-minimal-integer",
                ValueDecodeError::NegativeZero { .. } => "value::negative-zero",
                ValueDecodeError::IntegerTooWide { .. } => "value::integer-too-wide",
                ValueDecodeError::LengthTooLarge { .. } => "value::length-too-large",
                ValueDecodeError::InvalidUtf8 { .. } => "value::invalid-utf8",
                ValueDecodeError::InvalidName { .. } => "value::invalid-name",
                ValueDecodeError::InvalidBitVec { .. } => "value::invalid-bitvec",
                ValueDecodeError::NotAscending { .. } => "value::not-ascending",
                ValueDecodeError::InvalidMultiplicity { .. } => "value::invalid-multiplicity",
                ValueDecodeError::DepthExceeded { .. } => "value::depth-exceeded",
            }),
            Ok(value) => Probe::canonical(value.encode()),
        }
    }
}

/// The intent dossier's own JSON reader (`continuum_intent::canonical_json`).
///
/// Deliberately a *second* JSON target: this parser has the opposite input discipline to
/// `continuumd`'s — "the input need not be canonical — any legal JSON spelling of an
/// admissible document is accepted" — so the canonicity contract it owes is a fixpoint,
/// not byte identity, and the harness must not hold it to the stronger one.
#[derive(Debug)]
pub struct IntentCanonicalJson;

/// Tokens `continuum_intent::canonical_json::JsonError` can produce.
pub const INTENT_JSON_LANDINGS: &[&str] = &[
    "ijson::unexpected-end",
    "ijson::unexpected",
    "ijson::trailing-bytes",
    "ijson::duplicate-key",
    "ijson::floating-point",
    "ijson::integer-out-of-range",
    "ijson::number-syntax",
    "ijson::unterminated-string",
    "ijson::control-character",
    "ijson::string-escape",
    "ijson::lone-surrogate",
    "ijson::not-utf8",
    "ijson::too-deep",
];

const fn intent_json_landing(error: &IntentJsonError) -> &'static str {
    match error {
        IntentJsonError::UnexpectedEnd { .. } => "ijson::unexpected-end",
        IntentJsonError::Unexpected { .. } => "ijson::unexpected",
        IntentJsonError::TrailingBytes { .. } => "ijson::trailing-bytes",
        IntentJsonError::DuplicateKey { .. } => "ijson::duplicate-key",
        IntentJsonError::FloatingPoint { .. } => "ijson::floating-point",
        IntentJsonError::IntegerOutOfRange { .. } => "ijson::integer-out-of-range",
        IntentJsonError::NumberSyntax { .. } => "ijson::number-syntax",
        IntentJsonError::UnterminatedString { .. } => "ijson::unterminated-string",
        IntentJsonError::ControlCharacter { .. } => "ijson::control-character",
        IntentJsonError::StringEscape { .. } => "ijson::string-escape",
        IntentJsonError::LoneSurrogate { .. } => "ijson::lone-surrogate",
        IntentJsonError::NotUtf8 { .. } => "ijson::not-utf8",
        IntentJsonError::TooDeep { .. } => "ijson::too-deep",
    }
}

impl Target for IntentCanonicalJson {
    fn name(&self) -> &'static str {
        "canonical.intent-json"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(INTENT_JSON_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match continuum_intent::canonical_json::Json::parse(bytes) {
            Err(error) => Probe::rejected(intent_json_landing(&error)),
            Ok(document) => Probe::normal_form(document.to_canonical_bytes()),
        }
    }
}

// --- snapshot and intent schemas -----------------------------------------------------

/// The workspace snapshot tree (`continuum_workspace::snapshot`).
///
/// Identities are not transmitted — the reader re-derives every one through a
/// [`ContentIdentifier`](continuum_workspace::publication::ContentIdentifier) seam — so
/// the seam is part of the target. The production seam is used, not a stub: a harness
/// that supplied a degenerate identifier would be fuzzing a decoder no daemon runs.
#[derive(Debug)]
pub struct SchemaSnapshot;

const SNAPSHOT_LANDINGS: &[&str] = &[
    "snapshot::magic",
    "snapshot::version",
    "snapshot::unexpected-end",
    "snapshot::unknown-tag",
    "snapshot::length-out-of-range",
    "snapshot::name-encoding",
    "snapshot::name",
    "snapshot::name-order",
    "snapshot::too-deep",
    "snapshot::root-is-not-a-directory",
    "snapshot::trailing-bytes",
    "snapshot::identity",
];

impl Target for SchemaSnapshot {
    fn name(&self) -> &'static str {
        "schema.snapshot"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(SNAPSHOT_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match Snapshot::decode(bytes, &Blake3Identity) {
            Err(error) => Probe::rejected(match error {
                SnapshotDecodeError::Magic => "snapshot::magic",
                SnapshotDecodeError::Version { .. } => "snapshot::version",
                SnapshotDecodeError::UnexpectedEnd { .. } => "snapshot::unexpected-end",
                SnapshotDecodeError::UnknownTag { .. } => "snapshot::unknown-tag",
                SnapshotDecodeError::LengthOutOfRange { .. } => "snapshot::length-out-of-range",
                SnapshotDecodeError::NameEncoding { .. } => "snapshot::name-encoding",
                SnapshotDecodeError::Name { .. } => "snapshot::name",
                SnapshotDecodeError::NameOrder { .. } => "snapshot::name-order",
                SnapshotDecodeError::TooDeep { .. } => "snapshot::too-deep",
                SnapshotDecodeError::RootIsNotADirectory => "snapshot::root-is-not-a-directory",
                SnapshotDecodeError::TrailingBytes { .. } => "snapshot::trailing-bytes",
                SnapshotDecodeError::Identity(_) => "snapshot::identity",
            }),
            Ok(snapshot) => Probe::canonical(snapshot.encode()),
        }
    }
}

/// The Intent Contract artifact (`continuum_intent::contract`).
///
/// The umbrella decoder: ten field groups, each with its own typed rejection, layered
/// over the lenient JSON reader [`IntentCanonicalJson`] drives on its own. The landing
/// vocabulary keeps the group, because "the claims group refused it" and "the bounds
/// group refused it" are different facts about a contract.
#[derive(Debug)]
pub struct SchemaIntentContract;

const INTENT_CONTRACT_LANDINGS: &[&str] = &[
    "intent::missing-field",
    "intent::type-mismatch",
    "intent::unknown-field",
    "intent::unknown-token",
    "intent::header-mismatch",
    "intent::contract",
    "intent::claims",
    "intent::claims::duplicate-unit-key",
    "intent::assumptions",
    "intent::observers",
    "intent::bounds",
    "intent::faults",
    "intent::fairness",
    "intent::assurance",
    "intent::optimization",
    "intent::policy",
];

impl Target for SchemaIntentContract {
    fn name(&self) -> &'static str {
        "schema.intent-contract"
    }

    /// A contract is the largest artifact in the set; the Die Hard fixture the seeds
    /// mutate is a few kilobytes on its own.
    fn budget(&self) -> Budget {
        Budget {
            input_bytes: 16_384,
            ..Budget::DEFAULT
        }
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(INTENT_JSON_LANDINGS, INTENT_CONTRACT_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match IntentContract::decode(bytes) {
            Err(error) => Probe::rejected(match error {
                ContractDecodeError::Json(inner) => intent_json_landing(&inner),
                ContractDecodeError::MissingField { .. } => "intent::missing-field",
                ContractDecodeError::TypeMismatch { .. } => "intent::type-mismatch",
                ContractDecodeError::UnknownField { .. } => "intent::unknown-field",
                ContractDecodeError::UnknownToken { .. } => "intent::unknown-token",
                ContractDecodeError::HeaderMismatch { .. } => "intent::header-mismatch",
                ContractDecodeError::Contract(_) => "intent::contract",
                // W1's unique-claim-id rule is the duplicate-id case class for this
                // surface, so it keeps its own token rather than collapsing into the
                // group's.
                ContractDecodeError::Claims(PropertyDecodeError::Property(
                    PropertyError::DuplicateUnitKey { .. },
                )) => "intent::claims::duplicate-unit-key",
                ContractDecodeError::Claims(_) => "intent::claims",
                ContractDecodeError::Assumptions(_) => "intent::assumptions",
                ContractDecodeError::Observers(_) => "intent::observers",
                ContractDecodeError::Bounds(_) => "intent::bounds",
                ContractDecodeError::Faults(_) => "intent::faults",
                ContractDecodeError::Fairness(_) => "intent::fairness",
                ContractDecodeError::Assurance(_) => "intent::assurance",
                ContractDecodeError::Optimization(_) => "intent::optimization",
                ContractDecodeError::Policy(_) => "intent::policy",
            }),
            // `to_artifact_bytes` is the contract's canonical spelling, and the reader
            // accepts non-canonical spellings of the same document, so what is owed here
            // is a fixpoint rather than byte identity.
            Ok(contract) => Probe::normal_form(contract.to_artifact_bytes()),
        }
    }
}

// --- wire-form certificates ----------------------------------------------------------

/// The certificate checking surface (`continuum_certificate::check_certificate`).
///
/// Bytes in, a routed kernel's own verdict out. There is no re-encoding to check and
/// there deliberately never will be: the kernels ship a checker and never an encoder, so
/// a producer cannot hand the checker a structure and the harness cannot either
/// (INV-004, plan §20).
///
/// The landing token names the family, the verdict class, and — for
/// `continuum-kernel-core`, the family the seeded cases target — that kernel's own
/// rejection reason, taken verbatim from
/// [`Rejection::reason`](continuum_certificate::continuum_kernel_core::Rejection::reason)
/// rather than transcribed into a second vocabulary.
#[derive(Debug)]
pub struct CertificateWire;

const CERTIFICATE_LANDINGS: &[&str] = &[
    "cert::unroutable::too-short-for-magic",
    "cert::unroutable::unknown-family",
    "cert::core::verified",
    "cert::core::unsupported",
    "cert::core::rejected::oversized",
    "cert::core::rejected::truncated",
    "cert::core::rejected::bad-magic",
    "cert::core::rejected::trailing-bytes",
    "cert::core::rejected::malformed-token",
    "cert::core::rejected::count-out-of-range",
    "cert::core::rejected::not-strictly-ascending",
    "cert::core::rejected::inverted-variable-range",
    "cert::core::rejected::schema-epoch-mismatch",
    "cert::core::rejected::no-initial-states",
    "cert::core::rejected::initial-state-not-in-table",
    "cert::core::rejected::unknown-action",
    "cert::core::rejected::closure-failure",
    "cert::core::rejected::property-violated",
    "cert::sat::verified",
    "cert::sat::rejected",
    "cert::sat::unsupported",
    "cert::smt::verified",
    "cert::smt::rejected",
    "cert::smt::unsupported",
    "cert::temporal::verified",
    "cert::temporal::rejected",
    "cert::temporal::unsupported",
];

impl Target for CertificateWire {
    fn name(&self) -> &'static str {
        "certificate.wire"
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        vocabulary!(CERTIFICATE_LANDINGS)
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        let landing = match check_certificate(bytes) {
            Outcome::Unroutable(RoutingFault::TooShortForMagic { .. }) => {
                "cert::unroutable::too-short-for-magic".to_owned()
            }
            Outcome::Unroutable(RoutingFault::UnknownFamily { .. }) => {
                "cert::unroutable::unknown-family".to_owned()
            }
            Outcome::Checked(verdict) => match verdict {
                continuum_certificate::KernelVerdict::Core(core) => match core {
                    CoreVerdict::Verified(_) => "cert::core::verified".to_owned(),
                    CoreVerdict::Unsupported(_) => "cert::core::unsupported".to_owned(),
                    CoreVerdict::Rejected(rejection) => {
                        format!("cert::core::rejected::{}", rejection.reason())
                    }
                },
                continuum_certificate::KernelVerdict::Sat(sat) => match sat {
                    continuum_kernel_sat::Verdict::Verified(_) => "cert::sat::verified",
                    continuum_kernel_sat::Verdict::Rejected(_) => "cert::sat::rejected",
                    continuum_kernel_sat::Verdict::Unsupported(_) => "cert::sat::unsupported",
                }
                .to_owned(),
                continuum_certificate::KernelVerdict::Smt(smt) => match smt {
                    continuum_kernel_smt::Verdict::Verified(_) => "cert::smt::verified",
                    continuum_kernel_smt::Verdict::Rejected(_) => "cert::smt::rejected",
                    continuum_kernel_smt::Verdict::Unsupported(_) => "cert::smt::unsupported",
                }
                .to_owned(),
                continuum_certificate::KernelVerdict::Temporal(temporal) => match temporal {
                    continuum_kernel_temporal::Verdict::Verified(_) => "cert::temporal::verified",
                    continuum_kernel_temporal::Verdict::Rejected(_) => "cert::temporal::rejected",
                    continuum_kernel_temporal::Verdict::Unsupported(_) => {
                        "cert::temporal::unsupported"
                    }
                }
                .to_owned(),
            },
        };
        Probe::rejected(landing)
    }
}

/// The kernel-core certificate magic, re-exported so seed construction does not spell it
/// a second time.
pub const CORE_MAGIC: [u8; 8] = core_wire::MAGIC;

/// The wire epoch kernel-core implements.
pub const CORE_WIRE_EPOCH: u16 = core_wire::WIRE_EPOCH;

/// The largest state count kernel-core admits.
pub const CORE_MAX_STATES: u32 = core_wire::MAX_STATES;

/// `continuumd`'s document depth bound, shared by both encodings.
pub const CODEC_MAX_DEPTH: usize = continuumd::codec::json::MAX_DEPTH;

/// The snapshot tree's depth bound.
pub const SNAPSHOT_MAX_DEPTH: usize = continuum_workspace::snapshot::MAX_DEPTH;

/// CVNF-1's depth bound.
pub const VALUE_MAX_DEPTH: usize = continuum_value::value::MAX_DEPTH;

/// The transport's frame ceiling.
pub const MAX_FRAME_BYTES: usize = continuumd::transport::MAX_FRAME_BYTES;
