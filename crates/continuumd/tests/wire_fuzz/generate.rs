//! The seed-driven, structure-aware generator.
//!
//! # Why hand-rolled and not `arbitrary` + `libfuzzer-sys`
//!
//! Three constraints, and the shape below is what satisfies all three at once:
//!
//! 1. `unsafe_code = "forbid"` is set workspace-wide with no crate-level override
//!    (`Cargo.toml`, `[workspace.lints.rust]`). A `libfuzzer-sys` harness expands to code
//!    that does not satisfy it, so a libFuzzer target cannot live inside this workspace.
//! 2. The toolchain is pinned to a stable release (`rust-toolchain.toml`) and `just check`
//!    must stay on it. `cargo fuzz` needs nightly, so a libFuzzer lane cannot be the thing
//!    the pinned gate runs.
//! 3. INV-005. Whatever runs in the gate has to be reproducible from a committed seed
//!    rather than from ambient entropy or a wall clock.
//!
//! Adding `arbitrary` would also have meant a second external dependency in a workspace
//! that deliberately has exactly one (`blake3`), with a `dependency-rationale.toml` entry
//! and a `dependency-audits.toml` full audit behind it (GOV-1-07, docs/09 T12). The
//! generator below is ninety lines of xorshift and a recursive builder; it costs less than
//! the audit would, and it is the same discipline the kernel fixtures already use
//! (`continuum-kernel-*/src/fixture.rs`: "docs/12 §1 forbids ambient randomness in the
//! semantic core, and a fuzz corpus that cannot be replayed byte for byte is not
//! evidence").
//!
//! # What "structure-aware" means here
//!
//! Half the inputs are built from the target's own grammar — a canonical JSON or CBOR
//! document, a CVNF-1 value, a snapshot tree, a `CONTCERT` header — so they get past the
//! outermost layer and reach the rules worth exercising. The other half are mutations of
//! the committed corpus, which is where the generator inherits everything the seeded
//! cases already know about the format.
//!
//! # Every bound is a construction bound
//!
//! `Budget::generate_depth` and `Budget::generate_nodes` are decremented as the builder
//! descends, so an input's size is bounded by the builder rather than by luck. Nothing
//! here doubles a buffer, and nothing allocates in proportion to a number an input
//! declares.

use std::collections::BTreeMap;

use continuum_value::value::Value;
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent};
use continuumd::codec::cbor::Cbor;
use continuumd::codec::json::Json;
use continuumd::daemon::identity::Blake3Identity;

use super::corpus::Case;
use super::outcome::Budget;
use super::target::{CORE_MAGIC, CORE_WIRE_EPOCH, Target};

/// A seeded xorshift generator.
///
/// The same shape, and for the same stated reason, as the one in
/// `continuum-kernel-core/src/fixture.rs`: no ambient randomness anywhere near a semantic
/// artifact, and a corpus that replays byte for byte from its seed.
#[derive(Debug)]
pub struct Xorshift {
    state: u64,
}

impl Xorshift {
    /// Seed the generator. Zero is xorshift's fixed point and is replaced.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    /// The next 64 bits.
    pub const fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// A value in `0..bound`, or zero when `bound` is zero.
    pub const fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % (bound as u64)) as usize
    }

    /// One byte.
    pub const fn byte(&mut self) -> u8 {
        (self.next_u64() & 0xff) as u8
    }

    /// `len` bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_be_bytes());
        }
        out.truncate(len);
        out
    }
}

/// An encoding-independent document, so one builder serves both canonical encodings.
#[derive(Debug, Clone)]
enum Doc {
    Null,
    Bool(bool),
    Uint(u64),
    Text(String),
    Blob(Vec<u8>),
    Array(Vec<Doc>),
    Map(Vec<(String, Doc)>),
}

/// The key pool. Small and protocol-flavoured so that map ordering, duplicate keys, and
/// envelope field names all occur often enough to matter.
const KEYS: &[&str] = &[
    "a",
    "b",
    "z",
    "actor",
    "arguments",
    "capability",
    "idempotency_key",
    "intent",
    "operation",
    "protocol_version",
    "request_id",
    "snapshot",
];

/// Short strings the builder draws from, including ones that stress the readers.
const TEXTS: &[&str] = &[
    "",
    "req_1",
    "ws_1",
    "cap_1",
    "agent:builder",
    "workspace.create",
    "3.2",
    "\u{7f}",
    "\u{80}",
    "\u{10ffff}",
];

fn build_doc(rng: &mut Xorshift, depth_left: usize, nodes_left: &mut usize) -> Doc {
    if *nodes_left == 0 {
        return Doc::Null;
    }
    *nodes_left = nodes_left.saturating_sub(1);
    let leaf_only = depth_left == 0;
    let choice = rng.below(if leaf_only { 5 } else { 7 });
    match choice {
        0 => Doc::Null,
        1 => Doc::Bool(rng.below(2) == 1),
        2 => Doc::Uint(match rng.below(4) {
            0 => 0,
            1 => rng.next_u64() % 1024,
            2 => (1_u64 << 53) - 1,
            _ => rng.next_u64(),
        }),
        3 => Doc::Text((*TEXTS.get(rng.below(TEXTS.len())).unwrap_or(&"")).to_owned()),
        4 => {
            let len = rng.below(8);
            Doc::Blob(rng.bytes(len))
        }
        5 => {
            let len = rng.below(4);
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(build_doc(rng, depth_left.saturating_sub(1), nodes_left));
            }
            Doc::Array(items)
        }
        _ => {
            let len = rng.below(4);
            let mut fields = Vec::with_capacity(len);
            for _ in 0..len {
                let key = (*KEYS.get(rng.below(KEYS.len())).unwrap_or(&"a")).to_owned();
                fields.push((
                    key,
                    build_doc(rng, depth_left.saturating_sub(1), nodes_left),
                ));
            }
            Doc::Map(fields)
        }
    }
}

fn to_json(doc: &Doc) -> Json {
    match doc {
        Doc::Null => Json::Null,
        Doc::Bool(value) => Json::Bool(*value),
        // `Json::Integer` is bounded at `MAX_EXACT_INTEGER`; a larger magnitude is a
        // decimal string in this encoding, which is what the writer would emit.
        Doc::Uint(value) => {
            if *value <= continuumd::codec::json::MAX_EXACT_INTEGER {
                Json::Integer(*value)
            } else {
                Json::String(value.to_string())
            }
        }
        Doc::Text(text) => Json::String(text.clone()),
        Doc::Blob(bytes) => Json::String(continuumd::codec::json::base64url(bytes)),
        Doc::Array(items) => Json::Array(items.iter().map(to_json).collect()),
        Doc::Map(fields) => {
            let mut map = BTreeMap::new();
            for (key, value) in fields {
                map.insert(key.clone(), to_json(value));
            }
            Json::Object(map)
        }
    }
}

fn to_cbor(doc: &Doc) -> Cbor {
    match doc {
        Doc::Null => Cbor::Null,
        Doc::Bool(value) => Cbor::Bool(*value),
        Doc::Uint(value) => Cbor::Unsigned(*value),
        Doc::Text(text) => Cbor::Text(text.clone()),
        Doc::Blob(bytes) => Cbor::Bytes(bytes.clone()),
        Doc::Array(items) => Cbor::Array(items.iter().map(to_cbor).collect()),
        Doc::Map(fields) => {
            let mut map = BTreeMap::new();
            for (key, value) in fields {
                map.insert(key.clone(), to_cbor(value));
            }
            Cbor::Map(map)
        }
    }
}

fn build_value(rng: &mut Xorshift, depth_left: usize, nodes_left: &mut usize) -> Value {
    if *nodes_left == 0 {
        return Value::Null;
    }
    *nodes_left = nodes_left.saturating_sub(1);
    let leaf_only = depth_left == 0;
    match rng.below(if leaf_only { 6 } else { 8 }) {
        0 => Value::Null,
        1 => Value::Bool(rng.below(2) == 1),
        2 => Value::nat(u128::from(rng.next_u64())),
        3 => Value::int(i128::from(rng.next_u64() as i64)),
        4 => Value::text((*TEXTS.get(rng.below(TEXTS.len())).unwrap_or(&"")).to_owned()),
        5 => {
            let len = rng.below(8);
            Value::bytes(rng.bytes(len))
        }
        6 => {
            let len = rng.below(4);
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(build_value(rng, depth_left.saturating_sub(1), nodes_left));
            }
            Value::seq(items).unwrap_or(Value::Null)
        }
        _ => {
            let len = 1 + rng.below(3);
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(build_value(rng, depth_left.saturating_sub(1), nodes_left));
            }
            Value::tuple(items).unwrap_or(Value::Null)
        }
    }
}

/// The seven bytes every snapshot stream starts with, taken from a real encoding rather
/// than transcribed. A drifting `ENCODING_VERSION` changes this automatically.
#[must_use]
pub fn snapshot_prefix() -> Vec<u8> {
    let empty = Snapshot::build(&WorkspaceContent::new(), &Blake3Identity)
        .expect("an empty workspace has a snapshot");
    empty.encode().into_iter().take(7).collect()
}

fn build_snapshot_stream(rng: &mut Xorshift, depth_left: usize, nodes_left: &mut usize) -> Vec<u8> {
    let mut out = snapshot_prefix();
    build_snapshot_node(rng, depth_left, nodes_left, &mut out, true);
    out
}

fn build_snapshot_node(
    rng: &mut Xorshift,
    depth_left: usize,
    nodes_left: &mut usize,
    out: &mut Vec<u8>,
    force_directory: bool,
) {
    *nodes_left = nodes_left.saturating_sub(1);
    let directory = force_directory || (depth_left > 0 && *nodes_left > 0 && rng.below(2) == 1);
    if directory {
        let count = rng.below(4);
        out.push(0x01);
        out.extend_from_slice(&(count as u64).to_be_bytes());
        for index in 0..count {
            // Ascending by construction most of the time; the mutator is what breaks it.
            let name = format!("{}{index}", ["a", "b", "c", "dir"][rng.below(4)]);
            out.extend_from_slice(&(name.len() as u64).to_be_bytes());
            out.extend_from_slice(name.as_bytes());
            build_snapshot_node(rng, depth_left.saturating_sub(1), nodes_left, out, false);
        }
    } else {
        let content_len = rng.below(8);
        let content = rng.bytes(content_len);
        out.push(0x00);
        out.extend_from_slice(&(content.len() as u64).to_be_bytes());
        out.extend_from_slice(&content);
    }
}

/// Append a length-prefixed printable-ASCII token, the kernel wire form's `token`.
fn push_token(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    out.extend_from_slice(&(u16::try_from(bytes.len()).unwrap_or(u16::MAX)).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Build a `CONTCERT` byte string from the documented layout.
///
/// The point is to get *past routing*: eight bytes of magic route to
/// `continuum-kernel-core`, and everything after them is read by that kernel's own
/// decoder. Random bytes with no magic land at `cert::unroutable::unknown-family` and
/// exercise nothing.
fn build_certificate(rng: &mut Xorshift) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&CORE_MAGIC);
    // Occasionally an epoch or kind this build does not implement: those are `Unsupported`,
    // a different outcome from a rejection, and the harness should see both.
    let epoch = if rng.below(8) == 0 {
        rng.byte().into()
    } else {
        CORE_WIRE_EPOCH
    };
    out.extend_from_slice(&epoch.to_be_bytes());
    let kind: u16 = if rng.below(8) == 0 {
        u16::from(rng.byte())
    } else {
        1 + u16::try_from(rng.below(2)).unwrap_or(0)
    };
    out.extend_from_slice(&kind.to_be_bytes());
    for token in ["model", "sem-1", "prop", "scope", "assume", "engine-1"] {
        push_token(&mut out, token);
    }
    out.extend_from_slice(&epoch.to_be_bytes()); // schema_epoch, usually matching
    out.extend_from_slice(&0_u16.to_be_bytes()); // domain_pack_count
    let variables = 1 + u16::try_from(rng.below(3)).unwrap_or(0);
    out.extend_from_slice(&variables.to_be_bytes());
    for index in 0..variables {
        push_token(&mut out, &format!("v{index}"));
        out.extend_from_slice(&0_i64.to_be_bytes());
        out.extend_from_slice(&4_i64.to_be_bytes());
    }
    let states = 1 + u32::try_from(rng.below(3)).unwrap_or(0);
    out.extend_from_slice(&states.to_be_bytes());
    for state in 0..states {
        for _ in 0..variables {
            out.extend_from_slice(&i64::from(state).to_be_bytes());
        }
    }
    out
}

/// Apply one bounded mutation to `bytes`.
///
/// Every arm is size-preserving or size-*reducing* except `Splice`, which inserts at most
/// eight bytes. Nothing here doubles the buffer: an adversarial input is built by
/// declaring a large number, never by materializing one.
fn mutate(rng: &mut Xorshift, bytes: &[u8], ceiling: usize) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.is_empty() {
        let len = rng.below(16);
        return rng.bytes(len);
    }
    match rng.below(6) {
        0 => {
            let index = rng.below(out.len());
            out[index] = rng.byte();
        }
        1 => {
            let index = rng.below(out.len());
            out[index] ^= 1 << rng.below(8);
        }
        2 => {
            let cut = rng.below(out.len());
            out.truncate(cut);
        }
        3 => {
            let index = rng.below(out.len());
            let extra_len = 1 + rng.below(8);
            let extra = rng.bytes(extra_len);
            for (offset, byte) in extra.into_iter().enumerate() {
                let at = (index + offset).min(out.len());
                out.insert(at, byte);
            }
        }
        4 => {
            // Push a length field to its extreme without materializing anything.
            let index = rng.below(out.len());
            let width = [1_usize, 2, 4, 8][rng.below(4)];
            for offset in 0..width {
                if let Some(slot) = out.get_mut(index + offset) {
                    *slot = 0xff;
                }
            }
        }
        _ => {
            let start = rng.below(out.len());
            let end = (start + 1 + rng.below(8)).min(out.len());
            out.drain(start..end);
        }
    }
    out.truncate(ceiling);
    out
}

/// Generates inputs for one target.
#[derive(Debug)]
pub struct Generator<'a> {
    name: &'static str,
    seeds: Vec<&'a [u8]>,
}

impl<'a> Generator<'a> {
    /// Bind a generator to `target`, seeded from the committed corpus.
    #[must_use]
    pub fn new(target: &'static dyn Target, corpus: &'a [Case]) -> Self {
        let seeds = corpus
            .iter()
            .filter(|case| case.target == target.name())
            .map(|case| case.bytes.as_slice())
            .collect();
        Self {
            name: target.name(),
            seeds,
        }
    }

    /// One input.
    #[must_use]
    pub fn generate(&self, rng: &mut Xorshift, budget: Budget) -> Vec<u8> {
        let built = match rng.below(8) {
            0..=2 => self.grammar(rng, budget),
            3..=5 => self.corpus_mutation(rng, budget),
            6 => {
                let base = self.grammar(rng, budget);
                mutate(rng, &base, budget.input_bytes)
            }
            _ => {
                let len = rng.below(64);
                rng.bytes(len)
            }
        };
        let mut out = built;
        out.truncate(budget.input_bytes);
        out
    }

    fn corpus_mutation(&self, rng: &mut Xorshift, budget: Budget) -> Vec<u8> {
        if self.seeds.is_empty() {
            let len = rng.below(32);
            return rng.bytes(len);
        }
        let base = self.seeds[rng.below(self.seeds.len())];
        mutate(rng, base, budget.input_bytes)
    }

    fn grammar(&self, rng: &mut Xorshift, budget: Budget) -> Vec<u8> {
        let mut nodes = budget.generate_nodes;
        match self.name {
            "canonical.json" | "canonical.intent-json" | "frame.request-envelope.json" => {
                to_json(&build_doc(rng, budget.generate_depth, &mut nodes)).to_canonical_bytes()
            }
            "canonical.cbor" | "frame.request-envelope.cbor" => {
                to_cbor(&build_doc(rng, budget.generate_depth, &mut nodes)).to_canonical_bytes()
            }
            "canonical.value" => build_value(rng, budget.generate_depth, &mut nodes).encode(),
            "schema.snapshot" => {
                build_snapshot_stream(rng, budget.generate_depth.min(8), &mut nodes)
            }
            "certificate.wire" => build_certificate(rng),
            "frame.length-prefix" => {
                let payload = to_json(&build_doc(rng, budget.generate_depth, &mut nodes))
                    .to_canonical_bytes();
                let mut out = Vec::with_capacity(payload.len() + 4);
                out.extend_from_slice(
                    &(u32::try_from(payload.len()).unwrap_or(u32::MAX)).to_be_bytes(),
                );
                out.extend_from_slice(&payload);
                out
            }
            // `schema.intent-contract` has no grammar builder: a contract is ten
            // interlocking groups and a generator that emitted plausible ones would be a
            // second implementation of the schema. It is driven from the corpus instead,
            // which is stated here rather than left to be inferred from coverage.
            _ => self.corpus_mutation(rng, budget),
        }
    }
}
