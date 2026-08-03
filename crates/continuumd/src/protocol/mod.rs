//! The native protocol type layer: every plan §10.2 operation, mechanically held to
//! `notes/plan/schemas/continuumd-native-protocol.idl`.
//!
//! # What this module is
//!
//! The IDL is the wire authority:
//!
//! > This file is the single machine-readable definition of the continuumd native
//! > protocol. Generated Rust/TypeScript clients and generated JSON Schema derive from
//! > it. RFC 0026's prose summarizes this file; where the prose and this file disagree,
//! > this file decides and the RFC is corrected.
//! >
//! > — `notes/plan/schemas/continuumd-native-protocol.idl`, header
//!
//! This module is the Rust projection of that file: the 73 operations of 18 namespaces
//! with their request, response, verdict, and error declarations; the request and result
//! envelopes; the connection handshake and the protocol-major N/N−1 window; and the
//! closed and open vocabularies, including the complete plan §10.3 [`ErrorCode`]
//! taxonomy.
//!
//! # Mechanically checked, not generated — and how to reverse that
//!
//! The bone this module answers offers two ways to keep the types and the IDL from
//! drifting: generate the types from the IDL at build time, or check the committed types
//! against it. **This module takes the checked path**, for three reasons:
//!
//! 1. the workspace has no `build.rs` anywhere, and a build script is a new class of
//!    machinery — one that runs on every consumer's build, participates in the
//!    reproducible-build covenant of plan §20, and has to be audited as part of it;
//! 2. a generator's output is unreadable at the point of use, and this protocol's types
//!    are the surface every adapter is a projection of (ADR-0036/0038/0042). Doc comments
//!    that cite the rule a field obeys are worth more here than a build step;
//! 3. a build-time generator proves the types *were* derived from some IDL. The test
//!    proves they *agree with this one*, now — including agreeing with RFC 0027's
//!    independent authority registry, which a generator reading only the IDL cannot
//!    check at all.
//!
//! The choice is reversible and cheap to reverse. The check lives in
//! `tests/idl_conformance.rs`; the registry it checks
//! ([`registry::OPERATIONS`]) is already data. Emitting that data from a `build.rs` and
//! keeping the same test as the acceptance criterion would be a mechanical change, and
//! the test would then be checking the generator instead of the transcription. Nothing
//! in this module's shape depends on the transcription being hand-maintained.
//!
//! # Why the check can catch a renamed field
//!
//! A test can compare data against the IDL; it cannot see a Rust struct's field names.
//! So [`protocol_struct!`](crate::protocol_struct) emits the struct **and** its
//! [`FieldSpec`](spec::FieldSpec) list from one declaration: the field name is written
//! once and reaches both. Rename `overlay` to `overlays` in the struct and the spec says
//! `overlays` too, the IDL still says `overlay`, and
//! `every_operation_request_struct_matches_the_idl` fails naming the operation, the field
//! index, and both spellings. There is no edit that renames the field and leaves the test
//! green. The same construction covers enums (members and wire tokens), unions, and
//! handle prefixes.
//!
//! # The serialization boundary
//!
//! The IDL fixes **two encodings** — `canonical_json` and `canonical_cbor`, exactly one
//! negotiated per connection — and this module implements the part of that contract that
//! is *fixed*: the wire token of every enum member (including the members whose token
//! differs from their identifier, such as `revise-intent` and `bounded-schedules`), the
//! three-valued presence of every field as distinct [`Nullable`](spec::Nullable) and
//! [`Optional`](spec::Optional) types, the `@pattern` of every string alias, and every
//! handle prefix.
//!
//! Until protocol 3.2 it stopped short of a byte-level codec, and the reason was that
//! three things a codec needs were fixed by no normative source: the canonical field
//! order the two encodings share, how a union spells its tag, and what the envelope's
//! `Opaque` payloads are. Choosing values for them here would have been inventing wire
//! format.
//!
//! All three are now decided *in the IDL*, where a wire decision belongs — `rule
//! encoding.canonical_form`, `rule encoding.union_tagging`, `rule
//! encoding.opaque_payloads` — and [`crate::codec`] implements them. This module is still
//! the layer that stops at the type: it enforces what the declaration fixes, and the
//! codec turns a declaration into bytes. What connects them is that the codec is
//! *emitted* from the same macros — [`protocol_struct!`](crate::protocol_struct) writes
//! the struct, its [`FieldSpec`](spec::FieldSpec) list, and its encoder from one token —
//! so a renamed field cannot encode under its old key.
//!
//! The byte boundary itself is [`crate::transport`]: `daemon` still takes typed arguments
//! and emits a typed payload, and the transport is the only place bytes and dispatch meet.
//!
//! # Layout
//!
//! - [`spec`] — the declaration macros and the self-description they emit;
//! - [`scalar`] — IDL §3 primitives, §4 handles, and the string aliases;
//! - [`vocabulary`] — every enum, closed or `@open`;
//! - [`envelope`] — IDL §6: the request and result envelopes, errors, verdicts;
//! - [`handshake`] — IDL §7: `ClientHello`/`ServerWelcome` and version negotiation;
//! - [`task`] — IDL §8: task records and stream events;
//! - [`shared`] — IDL §9 plus `VerificationResult`;
//! - [`operations`] — one module per IDL namespace, 18 in all;
//! - [`registry`] — the 73 operations as data, and the protocol's identity constants.
//!
//! [`ErrorCode`]: vocabulary::ErrorCode

pub mod envelope;
pub mod handshake;
pub mod operations;
pub mod registry;
pub mod scalar;
pub mod shared;
pub mod spec;
pub mod task;
pub mod vocabulary;

/// The names the generated modules declare their fields with.
///
/// Each protocol module imports this glob so that a field declared `snapshot:
/// WorkspaceHandle nullable` resolves without a per-module import list. The glob is an
/// authoring convenience only: the type a field resolves to is still the one the IDL
/// names, because the IDL's name *is* the Rust name (see [`crate::protocol::scalar`]).
pub(crate) mod prelude {
    pub use super::envelope::*;
    pub use super::handshake::*;
    pub use super::scalar::*;
    pub use super::shared::*;
    pub use super::task::*;
    pub use super::vocabulary::*;
}
