//! The declared seed corpus: what every committed case is, and why.
//!
//! # The rule this file exists to enforce
//!
//! > A seed that the codec rejects at the frame layer before the case's actual property is
//! > exercised is not coverage — assert where each seed lands.
//!
//! So every seed below is *constructed here*, against a documented production bound, and
//! declares the landing token it must reach. `wire_fuzz.rs` asserts three things about
//! that: the committed corpus file is byte-identical to the construction, the case still
//! lands where it says, and the (target × class) matrix has no silent hole — a class a
//! target does not seed must carry a stated reason, never an absence.
//!
//! # Adversarial inputs are constructed linear-size
//!
//! Every case here is under two kilobytes and most are under a hundred bytes. Nothing is
//! built by doubling, and nothing materializes the quantity it is testing:
//!
//! - the oversized frame is a four-byte length prefix declaring
//!   `MAX_FRAME_BYTES + 1` above a one-byte body;
//! - the oversized CBOR byte string is a nine-byte head declaring `u64::MAX` above no
//!   payload at all;
//! - the oversized certificate is a four-byte count declaring `MAX_STATES + 1`;
//! - the deep cases nest **one side only**, at exactly `production_max + 1` levels, which
//!   costs one to eighteen bytes per level depending on the format.
//!
//! That is the difference between testing a bound and reproducing the outage the bound
//! exists to prevent.

use continuum_intent::contract::IntentContract;
use continuum_value::value::{Value, ValueKind};
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
use continuumd::codec::{to_bytes, to_cbor_bytes};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::{Budget as ProtocolBudget, RequestEnvelope};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Opaque, OperationName, ProtocolVersion, RequestId,
};
use continuumd::protocol::spec::{Nullable, Optional};

use super::corpus::{Case, Class, Origin};
use super::generate::snapshot_prefix;
use super::target::{
    CODEC_MAX_DEPTH, CORE_MAGIC, CORE_MAX_STATES, CORE_WIRE_EPOCH, MAX_FRAME_BYTES,
    SNAPSHOT_MAX_DEPTH, VALUE_MAX_DEPTH,
};

/// The Intent Contract fixture the intent cases are built from.
///
/// `continuum-intent`'s own, `include_str!`'d rather than restated — the same choice, for
/// the same reason, that every other `continuumd` integration test makes.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// One row of the (target × class) matrix.
#[derive(Debug, Clone)]
pub enum Row {
    /// The class is seeded, by this case.
    Seeded(Case),
    /// The class does not apply to this target, for this reason.
    NotApplicable {
        /// The target.
        target: &'static str,
        /// The class.
        class: Class,
        /// Why the class has no meaning for this decoder. Never "we did not get to it".
        reason: &'static str,
    },
}

fn case(
    target: &'static str,
    name: &'static str,
    class: Class,
    expect: &'static str,
    note: &'static str,
    bytes: Vec<u8>,
) -> Row {
    Row::Seeded(Case {
        name: name.to_owned(),
        target: target.to_owned(),
        class,
        expect: expect.to_owned(),
        origin: Origin::Seed,
        note: note.to_owned(),
        bytes,
    })
}

const fn absent(target: &'static str, class: Class, reason: &'static str) -> Row {
    Row::NotApplicable {
        target,
        class,
        reason,
    }
}

// --- constructors ---------------------------------------------------------------------

/// `MAX_DEPTH + 1` nested arrays with a value at the centre, nesting one side only.
///
/// The value matters: `[[[]]]` closes without ever asking the reader for another value,
/// so the innermost frame the depth counter sees is one shallower than the bracket count
/// suggests. `null` at the centre is what makes the deepest level a *value*, which is
/// what the bound is stated over. The cost is one byte per level plus four.
fn nested_arrays(levels: usize) -> Vec<u8> {
    let mut out = vec![b'['; levels];
    out.extend_from_slice(b"null");
    out.extend(std::iter::repeat_n(b']', levels));
    out
}

/// `MAX_DEPTH + 1` nested single-element CBOR arrays with `null` at the centre.
fn nested_cbor_arrays(levels: usize) -> Vec<u8> {
    let mut out = vec![0x81_u8; levels];
    out.push(0xf6);
    out
}

/// A well-formed request envelope. `arguments` carries a canonical document of the
/// encoding the envelope will be written in — the `Opaque` rule, which is why there is a
/// parameter here rather than one fixed payload.
fn envelope(arguments: Opaque) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: ProtocolVersion::new(3, 2),
        request_id: RequestId::new("req_1").expect("a well-formed request id"),
        idempotency_key: Optional::Present("idem-1".to_owned()),
        actor: ActorId::new("agent:fuzz").expect("a well-formed actor"),
        capability: CapabilityHandle::new("cap_fuzz").expect("a well-formed capability"),
        operation: OperationName::new("workspace.create").expect("a well-formed operation"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments,
        budget: Optional::Present(ProtocolBudget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Present(64),
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
        }),
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

/// Replace the first occurrence of `from` with `to`, asserting that there was one.
///
/// Used to derive an adversarial spelling from a well-formed one by changing exactly the
/// bytes the case is about. The assertion is the guard: a fixture that stopped containing
/// the pattern would otherwise produce a seed that tests nothing.
fn substitute(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let position = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap_or_else(|| {
            panic!(
                "the fixture no longer contains {:?}, so this seed would test nothing",
                String::from_utf8_lossy(from)
            )
        });
    let mut out = Vec::with_capacity(bytes.len() + to.len());
    out.extend_from_slice(&bytes[..position]);
    out.extend_from_slice(to);
    out.extend_from_slice(&bytes[position + from.len()..]);
    out
}

/// A CVNF-1 natural number's body: a length byte and the big-endian magnitude.
fn nat_body(magnitude: &[u8]) -> Vec<u8> {
    let mut out = vec![u8::try_from(magnitude.len()).unwrap_or(u8::MAX)];
    out.extend_from_slice(magnitude);
    out
}

/// A well-formed snapshot of two files.
fn snapshot_bytes() -> Vec<u8> {
    let mut content = WorkspaceContent::new();
    content
        .insert(
            WorkspacePath::new("a.txt").expect("a usable path"),
            b"alpha".to_vec(),
        )
        .expect("a fresh path");
    content
        .insert(
            WorkspacePath::new("dir/b.txt").expect("a usable path"),
            b"beta".to_vec(),
        )
        .expect("a fresh path");
    Snapshot::build(&content, &Blake3Identity)
        .expect("a well-formed workspace has a snapshot")
        .encode()
}

/// A snapshot directory entry: an 8-byte name length, the name, then the child node.
fn snapshot_entry(name: &str, child: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(name.len() as u64).to_be_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(child);
    out
}

/// A snapshot file node with `content`.
fn snapshot_file(content: &[u8]) -> Vec<u8> {
    let mut out = vec![0x00];
    out.extend_from_slice(&(content.len() as u64).to_be_bytes());
    out.extend_from_slice(content);
    out
}

/// A snapshot directory node over `entries`.
fn snapshot_directory(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0x01];
    out.extend_from_slice(&(entries.len() as u64).to_be_bytes());
    for entry in entries {
        out.extend_from_slice(entry);
    }
    out
}

/// Append a kernel wire-form token: a big-endian `u16` length and printable ASCII.
fn push_token(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(u16::try_from(text.len()).unwrap_or(u16::MAX)).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// The header and claim envelope every `CONTCERT` state-type certificate carries.
///
/// Built from the layout `crates/continuum-kernel-core/src/wire.rs` documents, not from
/// that crate's encoder: the kernel deliberately ships no encoder, and a seed that used
/// one would be testing the producer and the checker against shared code — the
/// common-mode failure docs/03 §8 is written against.
fn certificate_prefix() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&CORE_MAGIC);
    out.extend_from_slice(&CORE_WIRE_EPOCH.to_be_bytes());
    out.extend_from_slice(&2_u16.to_be_bytes()); // kind: state-type
    for token in ["model", "sem-1", "prop", "scope", "assume", "engine-1"] {
        push_token(&mut out, token);
    }
    out.extend_from_slice(&CORE_WIRE_EPOCH.to_be_bytes()); // schema_epoch
    out.extend_from_slice(&0_u16.to_be_bytes()); // domain_pack_count
    out
}

/// A `CONTCERT` state-type certificate over `variables` and `states`.
fn certificate(variables: &[&str], states: &[i64], declared_state_count: Option<u32>) -> Vec<u8> {
    let mut out = certificate_prefix();
    out.extend_from_slice(&(u16::try_from(variables.len()).unwrap_or(u16::MAX)).to_be_bytes());
    for name in variables {
        push_token(&mut out, name);
        out.extend_from_slice(&0_i64.to_be_bytes());
        out.extend_from_slice(&16_i64.to_be_bytes());
    }
    let count = declared_state_count.unwrap_or_else(|| {
        u32::try_from(states.len() / variables.len().max(1)).unwrap_or(u32::MAX)
    });
    out.extend_from_slice(&count.to_be_bytes());
    for value in states {
        out.extend_from_slice(&value.to_be_bytes());
    }
    out
}

// --- the matrix -----------------------------------------------------------------------

/// Every declared row, in a fixed order.
///
/// The order is part of the corpus's identity: the regeneration test writes files from
/// this list, so a reordering would be a diff.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one function, one table: splitting the matrix across helpers would let a \
              row go missing without the reader seeing a gap"
)]
pub fn rows() -> Vec<Row> {
    let envelope_json =
        to_bytes(&envelope(json_arguments())).expect("a well-formed envelope encodes");
    let contract = DIE_HARD_CONTRACT.trim_end().as_bytes().to_vec();
    let snapshot = snapshot_bytes();
    let prefix = snapshot_prefix();

    vec![
        // --- frame.length-prefix ------------------------------------------------------
        case(
            "frame.length-prefix",
            "well-formed-single-frame",
            Class::WellFormed,
            "accepted",
            "a two-byte payload under its own four-byte length prefix",
            {
                let mut out = 2_u32.to_be_bytes().to_vec();
                out.extend_from_slice(b"{}");
                out
            },
        ),
        case(
            "frame.length-prefix",
            "oversized-declared-length",
            Class::Oversized,
            "frame::too-large",
            "a prefix declaring MAX_FRAME_BYTES + 1 above a one-byte body: five bytes total",
            {
                let declared = u32::try_from(MAX_FRAME_BYTES)
                    .unwrap_or(u32::MAX)
                    .saturating_add(1);
                let mut out = declared.to_be_bytes().to_vec();
                out.push(0x00);
                out
            },
        ),
        case(
            "frame.length-prefix",
            "noncanonical-under-declared-length",
            Class::Noncanonical,
            "frame::trailing-bytes",
            "a prefix that under-declares its body, so the stream has two readings",
            {
                let mut out = 1_u32.to_be_bytes().to_vec();
                out.extend_from_slice(b"{}");
                out
            },
        ),
        absent(
            "frame.length-prefix",
            Class::Cyclic,
            "the framing layer has no recursion: a frame is a four-byte length and a flat \
             body, so there is no nesting for a cycle to appear in. The cyclic class is \
             seeded on every decoder that does nest.",
        ),
        absent(
            "frame.length-prefix",
            Class::DuplicateId,
            "the framing layer carries no identifiers — the prefix is a length and the \
             body is opaque to it, so there is nothing that could repeat.",
        ),
        // --- frame.request-envelope.json ----------------------------------------------
        case(
            "frame.request-envelope.json",
            "well-formed-workspace-create",
            Class::WellFormed,
            "accepted",
            "a complete workspace.create request envelope in canonical JSON",
            envelope_json.clone(),
        ),
        case(
            "frame.request-envelope.json",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "json::too-deep",
            "MAX_DEPTH + 1 nested arrays, one side only: one byte per level",
            nested_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "frame.request-envelope.json",
            "oversized-integer-beyond-exact",
            Class::Oversized,
            "json::integer-range",
            "2^53 as a JSON number: beyond MAX_EXACT_INTEGER, where a U64 must be a string",
            br#"{"budget":9007199254740992}"#.to_vec(),
        ),
        case(
            "frame.request-envelope.json",
            "duplicate-id-repeated-member",
            Class::DuplicateId,
            "json::key-order",
            "one object with two `request_id` members. It lands on the ordering rule and \
             not on `DuplicateKey`, because an equal key is not *strictly* ascending and \
             the reader checks ascent on the way in: strict ascent is what forbids the \
             duplicate, and `JsonError::DuplicateKey` is reachable only through the \
             `Json::object` constructor.",
            br#"{"request_id":"req_1","request_id":"req_2"}"#.to_vec(),
        ),
        case(
            "frame.request-envelope.json",
            "noncanonical-u64-as-string-in-range",
            Class::Noncanonical,
            "codec::integer-range",
            "a U64 inside the exactly-representable range spelled as a decimal string: a \
             second spelling, refused at the struct layer rather than the document layer",
            substitute(&envelope_json, br#""states":64"#, br#""states":"64""#),
        ),
        // --- frame.request-envelope.cbor ----------------------------------------------
        case(
            "frame.request-envelope.cbor",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "cbor::too-deep",
            "MAX_DEPTH + 1 single-element arrays, one side only: one byte per level",
            nested_cbor_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "frame.request-envelope.cbor",
            "oversized-declared-byte-string",
            Class::Oversized,
            "cbor::truncated",
            "a byte-string head declaring u64::MAX above no payload: nine bytes total",
            {
                let mut out = vec![0x5b_u8];
                out.extend_from_slice(&u64::MAX.to_be_bytes());
                out
            },
        ),
        case(
            "frame.request-envelope.cbor",
            "duplicate-id-repeated-key",
            Class::DuplicateId,
            "cbor::key-order",
            "a two-entry map whose keys are the same text string. As in JSON, strict \
             ascent fires first: `CborError::DuplicateKey` is reachable only through the \
             `Cbor::map` constructor.",
            vec![0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02],
        ),
        case(
            "frame.request-envelope.cbor",
            "noncanonical-non-shortest-head",
            Class::Noncanonical,
            "cbor::non-shortest-head",
            "zero written with a one-byte argument: a second spelling of major type 0",
            vec![0x18, 0x00],
        ),
        case(
            "frame.request-envelope.cbor",
            "well-formed-workspace-create",
            Class::WellFormed,
            "accepted",
            "the same workspace.create envelope in canonical CBOR. Its `arguments` differ \
             from the JSON envelope's by contract, not by accident: an Opaque is carried \
             verbatim as a canonical value of the negotiated encoding.",
            to_cbor_bytes(&envelope(cbor_arguments())).expect("a well-formed envelope encodes"),
        ),
        // --- canonical.json ------------------------------------------------------------
        case(
            "canonical.json",
            "well-formed-object",
            Class::WellFormed,
            "accepted",
            "an object with an array, an integer, and a named absence",
            br#"{"a":[1,2],"b":null}"#.to_vec(),
        ),
        case(
            "canonical.json",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "json::too-deep",
            "MAX_DEPTH + 1 nested arrays, one side only",
            nested_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "canonical.json",
            "oversized-integer-beyond-exact",
            Class::Oversized,
            "json::integer-range",
            "2^53 as a number: one past MAX_EXACT_INTEGER",
            b"9007199254740992".to_vec(),
        ),
        case(
            "canonical.json",
            "duplicate-id-repeated-key",
            Class::DuplicateId,
            "json::key-order",
            "two members with one key. Last-writer-wins would be a silent \
             reinterpretation; the reader refuses on strict ascent, which an equal key \
             fails before the map is consulted.",
            br#"{"a":1,"a":2}"#.to_vec(),
        ),
        case(
            "canonical.json",
            "noncanonical-leading-zero",
            Class::Noncanonical,
            "json::leading-zero",
            "the integer 1 written `01`: a second spelling of one number",
            b"01".to_vec(),
        ),
        case(
            "canonical.json",
            "noncanonical-key-order",
            Class::Noncanonical,
            "json::key-order",
            "members out of ascending code-point order: a second spelling of one document",
            br#"{"b":1,"a":2}"#.to_vec(),
        ),
        // --- canonical.cbor -------------------------------------------------------------
        case(
            "canonical.cbor",
            "well-formed-map",
            Class::WellFormed,
            "accepted",
            "a single-entry map with a text key and an unsigned value",
            vec![0xa1, 0x61, b'a', 0x01],
        ),
        case(
            "canonical.cbor",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "cbor::too-deep",
            "MAX_DEPTH + 1 single-element arrays, one side only",
            nested_cbor_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "canonical.cbor",
            "oversized-declared-byte-string",
            Class::Oversized,
            "cbor::truncated",
            "a byte-string head declaring u64::MAX above no payload",
            {
                let mut out = vec![0x5b_u8];
                out.extend_from_slice(&u64::MAX.to_be_bytes());
                out
            },
        ),
        case(
            "canonical.cbor",
            "duplicate-id-repeated-key",
            Class::DuplicateId,
            "cbor::key-order",
            "a two-entry map whose keys are the same text string: refused on strict \
             ascent, one entry before the map would notice the repeat",
            vec![0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02],
        ),
        case(
            "canonical.cbor",
            "noncanonical-non-shortest-head",
            Class::Noncanonical,
            "cbor::non-shortest-head",
            "zero written with a one-byte argument: a second spelling of major type 0",
            vec![0x18, 0x00],
        ),
        case(
            "canonical.cbor",
            "noncanonical-key-order",
            Class::Noncanonical,
            "cbor::key-order",
            "map keys out of the protocol's ascending order",
            vec![0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02],
        ),
        // --- canonical.value (CVNF-1) ----------------------------------------------------
        case(
            "canonical.value",
            "well-formed-sequence",
            Class::WellFormed,
            "accepted",
            "a two-element sequence of naturals",
            Value::seq([Value::nat(1), Value::nat(2)])
                .expect("a well-formed sequence")
                .encode(),
        ),
        case(
            "canonical.value",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "value::depth-exceeded",
            "MAX_DEPTH + 1 single-element sequence headers, one side only: three bytes per level",
            {
                let mut out = Vec::new();
                for _ in 0..=VALUE_MAX_DEPTH {
                    out.push(ValueKind::Seq.tag());
                    out.extend_from_slice(&nat_body(&[0x01]));
                }
                out.push(ValueKind::Null.tag());
                out
            },
        ),
        case(
            "canonical.value",
            "oversized-declared-blob-length",
            Class::Oversized,
            "value::length-too-large",
            "a byte string declaring 2^64 bytes above no payload: eleven bytes, and one \
             past what any 64-bit target can index",
            {
                let mut out = vec![ValueKind::Bytes.tag()];
                let mut magnitude = vec![0x01_u8];
                magnitude.extend_from_slice(&[0x00; 8]);
                out.extend_from_slice(&nat_body(&magnitude));
                out
            },
        ),
        case(
            "canonical.value",
            "duplicate-id-repeated-record-field",
            Class::DuplicateId,
            "value::not-ascending",
            "a record naming one field twice: strict ascent is what forbids the duplicate",
            {
                let mut out = vec![ValueKind::Record.tag()];
                out.extend_from_slice(&nat_body(&[0x02]));
                for _ in 0..2 {
                    out.extend_from_slice(&nat_body(&[0x01]));
                    out.push(b'a');
                    out.extend_from_slice(&Value::Null.encode());
                }
                out
            },
        ),
        case(
            "canonical.value",
            "noncanonical-leading-zero-magnitude",
            Class::Noncanonical,
            "value::non-minimal-integer",
            "the natural 1 written with a leading zero byte: a second spelling of one number",
            {
                let mut out = vec![ValueKind::Nat.tag()];
                out.extend_from_slice(&nat_body(&[0x00, 0x01]));
                out
            },
        ),
        // --- canonical.intent-json --------------------------------------------------------
        case(
            "canonical.intent-json",
            "well-formed-object",
            Class::WellFormed,
            "accepted",
            "a minimal admissible document",
            br#"{"a":1}"#.to_vec(),
        ),
        case(
            "canonical.intent-json",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "ijson::too-deep",
            "MAX_DEPTH + 1 nested arrays, one side only",
            nested_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "canonical.intent-json",
            "oversized-integer-beyond-i64",
            Class::Oversized,
            "ijson::integer-out-of-range",
            "a magnitude past this reader's i64 carrier",
            b"99999999999999999999".to_vec(),
        ),
        case(
            "canonical.intent-json",
            "duplicate-id-repeated-key",
            Class::DuplicateId,
            "ijson::duplicate-key",
            "two members with one key: refused even though this reader is otherwise lenient",
            br#"{"a":1,"a":2}"#.to_vec(),
        ),
        case(
            "canonical.intent-json",
            "noncanonical-spacing-and-order",
            Class::Noncanonical,
            "accepted",
            "whitespace and unordered keys: this reader normalizes rather than refusing, so \
             the case is accepted and the harness checks the normal form is a fixpoint",
            br#"{ "b" : 1, "a" : 2 }"#.to_vec(),
        ),
        // --- schema.snapshot ---------------------------------------------------------------
        case(
            "schema.snapshot",
            "well-formed-two-files",
            Class::WellFormed,
            "accepted",
            "a real Merkle snapshot of two files, encoded by the production encoder",
            snapshot,
        ),
        case(
            "schema.snapshot",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "snapshot::too-deep",
            "MAX_DEPTH + 1 nested single-entry directories, one side only: eighteen bytes per level",
            {
                let mut node = snapshot_file(b"");
                for _ in 0..=SNAPSHOT_MAX_DEPTH {
                    node = snapshot_directory(&[snapshot_entry("a", &node)]);
                }
                let mut out = prefix.clone();
                out.extend_from_slice(&node);
                out
            },
        ),
        case(
            "schema.snapshot",
            "oversized-declared-file-length",
            Class::Oversized,
            "snapshot::length-out-of-range",
            "a file node declaring u64::MAX bytes of content above no content",
            {
                let mut file = vec![0x00_u8];
                file.extend_from_slice(&u64::MAX.to_be_bytes());
                let mut out = prefix.clone();
                out.extend_from_slice(&snapshot_directory(&[snapshot_entry("a", &file)]));
                out
            },
        ),
        case(
            "schema.snapshot",
            "duplicate-id-repeated-entry-name",
            Class::DuplicateId,
            "snapshot::name-order",
            "a directory listing one name twice: strict ascent is what forbids the duplicate",
            {
                let leaf = snapshot_file(b"x");
                let mut out = prefix.clone();
                out.extend_from_slice(&snapshot_directory(&[
                    snapshot_entry("a", &leaf),
                    snapshot_entry("a", &leaf),
                ]));
                out
            },
        ),
        case(
            "schema.snapshot",
            "noncanonical-entry-order",
            Class::Noncanonical,
            "snapshot::name-order",
            "a directory listing `b` before `a`: a second spelling of one tree",
            {
                let leaf = snapshot_file(b"x");
                let mut out = prefix.clone();
                out.extend_from_slice(&snapshot_directory(&[
                    snapshot_entry("b", &leaf),
                    snapshot_entry("a", &leaf),
                ]));
                out
            },
        ),
        // --- schema.intent-contract -------------------------------------------------------
        case(
            "schema.intent-contract",
            "well-formed-die-hard",
            Class::WellFormed,
            "accepted",
            "continuum-intent's own Die Hard contract fixture, verbatim",
            contract.clone(),
        ),
        case(
            "schema.intent-contract",
            "cyclic-one-sided-nesting",
            Class::Cyclic,
            "ijson::too-deep",
            "MAX_DEPTH + 1 nested arrays: the contract decoder's JSON layer is where the \
             depth bound lives, and this asserts it is reached before any group runs",
            nested_arrays(CODEC_MAX_DEPTH + 1),
        ),
        case(
            "schema.intent-contract",
            "oversized-integer-beyond-i64",
            Class::Oversized,
            "ijson::integer-out-of-range",
            "a magnitude past the reader's carrier, refused before the schema is consulted",
            b"99999999999999999999".to_vec(),
        ),
        case(
            "schema.intent-contract",
            "duplicate-id-repeated-claim-id",
            Class::DuplicateId,
            "intent::claims::duplicate-unit-key",
            "the fixture with `TypeOK` renamed to `NotSolved`, so two claims share an id \
             (RFC 0031 W1). The whole contract is carried so the case reaches the claims \
             group rather than stopping at the JSON layer.",
            substitute(&contract, br#""id":"TypeOK""#, br#""id":"NotSolved""#),
        ),
        absent(
            "schema.intent-contract",
            Class::Noncanonical,
            "`IntentContract::decode` accepts any legal JSON spelling of an admissible \
             document by contract, so a non-canonical spelling is not a rejection class \
             for it — it is an acceptance whose normal form must be a fixpoint, which the \
             round-trip oracle checks on every accepted case. The rejecting half of the \
             class is seeded on `canonical.intent-json`, which is the same reader.",
        ),
        // --- certificate.wire ---------------------------------------------------------------
        case(
            "certificate.wire",
            "well-formed-state-type",
            Class::WellFormed,
            "cert::core::verified",
            "a minimal CONTCERT state-type certificate: one variable, two in-range states",
            certificate(&["v0"], &[0, 1], None),
        ),
        case(
            "certificate.wire",
            "oversized-declared-state-count",
            Class::Oversized,
            "cert::core::rejected::count-out-of-range",
            "a four-byte state count declaring MAX_STATES + 1 above two states",
            certificate(&["v0"], &[0, 1], Some(CORE_MAX_STATES.saturating_add(1))),
        ),
        case(
            "certificate.wire",
            "duplicate-id-repeated-variable-name",
            Class::DuplicateId,
            "cert::core::rejected::not-strictly-ascending",
            "two state variables with one name: strict ascent is what forbids the duplicate",
            certificate(&["v0", "v0"], &[0, 0, 1, 1], None),
        ),
        case(
            "certificate.wire",
            "noncanonical-descending-state-table",
            Class::Noncanonical,
            "cert::core::rejected::not-strictly-ascending",
            "a state table listing 1 before 0: a second spelling of one state set",
            certificate(&["v0"], &[1, 0], None),
        ),
        absent(
            "certificate.wire",
            Class::Cyclic,
            "the certificate wire form is flat: `Nothing nests, so the format has no \
             recursion for RFC 0005's cycle/recursion bounds to bound and the decoder \
             cannot overflow a stack` (crates/continuum-kernel-core/src/wire.rs). There is \
             no nesting for a cycle to appear in.",
        ),
    ]
}

/// Every seeded case, in declaration order.
#[must_use]
pub fn cases() -> Vec<Case> {
    rows()
        .into_iter()
        .filter_map(|row| match row {
            Row::Seeded(case) => Some(case),
            Row::NotApplicable { .. } => None,
        })
        .collect()
}

/// An empty canonical-JSON `arguments` payload.
#[must_use]
pub fn json_arguments() -> Opaque {
    Opaque::from_bytes(b"{}".to_vec())
}

/// An empty canonical-CBOR `arguments` payload: major type 5, zero entries.
///
/// Not the same bytes as [`json_arguments`], and that is the contract rather than an
/// inconvenience: an `Opaque` is "carried verbatim as a canonical value of the negotiated
/// encoding", so one message has different `arguments` bytes in the two encodings while
/// having the same field sequence.
#[must_use]
pub fn cbor_arguments() -> Opaque {
    Opaque::from_bytes(vec![0xa0])
}

/// The well-formed request envelope, for the cross-encoding identity check.
#[must_use]
pub fn envelope_value(arguments: Opaque) -> RequestEnvelope {
    envelope(arguments)
}

/// The Die Hard contract fixture, trimmed.
#[must_use]
pub fn die_hard_contract() -> Vec<u8> {
    DIE_HARD_CONTRACT.trim_end().as_bytes().to_vec()
}

/// Whether the fixture still decodes, so a fixture change is a loud failure.
#[must_use]
pub fn contract_decodes() -> bool {
    IntentContract::decode(&die_hard_contract()).is_ok()
}
