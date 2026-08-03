//! Dedicated exit evidence for `INV-003` (`notes/plan/plan.md:315`, the invariant bone
//! `bn-1eqt`).
//!
//! > `INV-003` — No prose-only machine interfaces
//! >
//! > Human text may accompany a result, but agents and integrations consume versioned
//! > schemas and enums.
//! >
//! > — `notes/plan/plan.md:315`
//!
//! # Audit: where "schemas decide, prose does not" already lives, cited rather than rebuilt
//!
//! Six things in this repository enforce a slice of INV-003 today. This file cites every
//! one of them and duplicates none:
//!
//! - **`notes/plan/schemas/README.md`** is the identity contract every artifact schema
//!   obeys — three identities (document/class/instance), the `schema_id`/`schema_epoch`
//!   instance header, and the rule that recovers a governing document mechanically from an
//!   instance's own header ("inserting `v<schema_epoch>/` before the last segment of
//!   `schema_id`"). `schema_resolution` below is that recovery rule, executed.
//! - **GOV-1-12** (`ambiguity-is-an-error`, `tools/governance/check_code_policy.py`) already
//!   walks every `notes/plan/schemas/*.schema.json` document and asserts top-level
//!   `additionalProperties: false`, `schema_id`/`schema_epoch` required-and-`const`-pinned
//!   for `schema_kind: "artifact"` documents, the closed three-member verdict vocabulary on
//!   `assurance-result.schema.json`, and that `TaskOutcome::Inconclusive` carries a
//!   non-optional `InconclusiveReason` (INV-008). `schema_closure` below is a **permanent
//!   Rust regression** of the first two rules, independent of that Python tool ever
//!   running — this file's own instruction is "mirror GOV-1-12's rule but as a permanent
//!   Rust regression", and `cited_enforcement_still_exists` is the tripwire that the Python
//!   original has not drifted out from under the citation.
//! - **GOV-1-10** (`pack-operation-contract`) and **GOV-1-11**
//!   (`reference-precedes-optimization`), the same file: every pack operation has a
//!   `domain-pack.schema.json`-closed contract, and every engine crate states plan §20's
//!   "results reach the trust base only as certificates checked from wire form" boundary
//!   before any optimized path is documented. Cited, not re-derived.
//! - **`tools/governance/check_claim_governance.py`** binds `docs/18`'s claim registry to
//!   its generated mirror and to `.bones/events` bidirectionally — a claim row is prose
//!   only if nothing machine-readable names it, and this checker is what makes that
//!   impossible silently. Cited, not re-derived.
//! - **`idl_conformance.rs`** is INV-003's wire-envelope leg: `continuumd-native-protocol.idl`
//!   is data, parsed independently of the Rust types it describes, and every request/
//!   response body, enum, and union is held to it field-for-field. Cited, not re-derived.
//! - **`inv016_untrusted_source_evidence.rs`** already proved the complementary half of this
//!   invariant: `Daemon::dispatch` keys on the operation *name* (a closed registry lookup),
//!   never on a free-text field's contents, across all 28 landed operations. INV-016 is
//!   "text cannot COMMAND"; INV-003 is "text cannot DEFINE". Where INV-016 proves dispatch
//!   *reads* structure, not prose, this file proves the *artifacts* dispatch reads and
//!   writes *are* structure, not prose — schema-closed or type-closed, every one.
//!   `dispatch_reads_structure_not_prose` below is the freshness tripwire on that citation.
//!
//! # Artifact-class table
//!
//! Every machine-consumed artifact class this repository defines today, and what makes each
//! one non-prose. `artifact_classes::SCHEMA_BOUND` is the schema-bound half of this table as
//! data, checked bidirectionally against the real `notes/plan/schemas/` directory listing —
//! the anti-drift device: a new schema file with no table row, or a table row naming a file
//! that no longer exists, fails loudly in `every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa`.
//!
//! | class | binding | checker / evidence |
//! |---|---|---|
//! | wire envelopes/operations (73, IDL) | typed wire — `continuumd-native-protocol.idl` | `idl_conformance.rs` (cited) |
//! | CONTCERT certificates (4 kernel crates) | typed wire — `wire.rs`'s byte grammar, `check_certificate(bytes: &[u8])` | `certificate_wire.rs`, `wire_form_boundary.rs` ×4 (cited); `wire_bytes_not_prose` below |
//! | the 19 `notes/plan/schemas/*.schema.json` instance classes | schema, `additionalProperties: false` + const-pinned identity | `schema_closure` below; GOV-1-12 (cited) |
//! | intent registry record (representative) | schema, daemon-emitted | `schema_resolution` below |
//! | evidence graph node (representative) | schema, daemon-emitted | `schema_resolution` below |
//! | recovery/reconciliation reports (`RecoveryReport`, plan §4.5) | typed wire — `Verdict` (3-variant enum), no schema file | `recovery_reports_are_typed` below; `g1_crash_recovery_evidence.rs` (cited) |
//! | governance/kernel-covenant evidence JSONs (`tools/*/evidence/*.json`) | typed wire — closed `"status": "pass"\|"fail"` + the producing checker's own self-test | the checkers themselves (cited, not re-derived here) |
//!
//! # Clause → test map
//!
//! | Clause | Test |
//! |---|---|
//! | every schema under `notes/plan/schemas/` closes its top level | [`schema_closure::every_schema_under_schemas_closes_its_top_level_and_const_pins_its_identity`] |
//! | mutant: an opened top level, a dropped `required` entry, and a dropped `const` pin are each caught | [`schema_closure::the_closure_check_is_not_vacuous`] |
//! | the artifact-class table and the real `schemas/` directory agree, bidirectionally | [`artifact_classes::every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa`] |
//! | mutant: dropping one table row breaks that comparison | [`artifact_classes::dropping_one_table_row_breaks_the_comparison`] |
//! | the daemon's own emitted intent-registry-record names a schema the tree resolves | [`schema_resolution::the_intent_registry_record_the_daemon_emits_names_a_schema_the_tree_resolves`] |
//! | the daemon's own emitted evidence-graph-node names a schema the tree resolves | [`schema_resolution::the_evidence_graph_node_the_daemon_emits_names_a_schema_the_tree_resolves`] |
//! | mutant: a fabricated `schema_id`, and a real one at the wrong epoch, resolve to nothing | [`schema_resolution::a_schema_id_that_names_no_real_class_does_not_resolve`] |
//! | all four kernel crates' certificate checkers take bytes, never text | [`wire_bytes_not_prose::every_kernel_checkers_entry_point_takes_bytes_never_text`] |
//! | mutant: a text-shaped signature is not mistaken for a bytes-shaped one | [`wire_bytes_not_prose::the_signature_check_is_not_vacuous`] |
//! | the kernel and the engine independently declare the same `CONTCERT` magic | [`wire_bytes_not_prose::the_kernel_and_the_engine_independently_declare_the_same_contcert_magic`] |
//! | `RecoveryReport`'s `Verdict` is a closed, compiler-enforced three-variant enum | [`recovery_reports_are_typed::verdict_is_a_closed_three_variant_enum_every_arm_named`] |
//! | the INV-016 structural leg this file cites for "dispatch reads structure, not prose" still exists | [`dispatch_reads_structure_not_prose::the_inv016_structural_leg_this_file_cites_still_exists`] |
//! | the cited GOV-1-10/11/12 rule names and the claim-registry bidirectional rule names have not drifted | [`cited_enforcement_still_exists`] (both tests) |
//!
//! # OOM hygiene
//!
//! Every schema document is read once via [`std::fs::read_to_string`] (the real,
//! pretty-printed files under `notes/plan/schemas/`) or `include_str!` (everything else:
//! `README.md`, the four kernel `check.rs` sources, `wire.rs`, `certificate.rs`, the
//! Python checkers, the INV-016 evidence file, and the Die Hard Intent Contract fixture),
//! held in a `Vec`/`const` once, and never doubled, repeated, or grown at runtime — this
//! bone's own rule, and the same one `inv016_untrusted_source_evidence.rs` states for its
//! hostile fixtures.
//!
//! # No `src/` file is touched or added by this file
//!
//! Every leg below either reads tracked files as data (the schemas, the kernel/engine
//! sources, the two Python checkers, the plan, the sibling INV-016 evidence file) or
//! dispatches through this crate's already-public `Daemon`/`OperationFamily` surface, the
//! same one `inv002`/`inv006`/`inv016`'s evidence files use.

use std::collections::BTreeSet;

use continuum_intent::canonical_json::Json as IntentJson;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::EvidenceGetRequest;
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, EpochIdentity, EvidenceHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, DataGrant, Encoding, ResultStatus};

// =====================================================================================
// Tracked text this file reads as data, never as code
// =====================================================================================

/// The absolute path of `notes/plan/schemas/`, for the runtime directory listing
/// `schema_closure::schema_files` and `artifact_classes`'s anti-drift check both need.
/// `CARGO_MANIFEST_DIR` is this crate's own root (`crates/continuumd`), so two `..` reach
/// the repository root — the same arithmetic `idl_conformance.rs`'s `IDL_PATH` uses.
const SCHEMAS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan/schemas");

const KERNEL_CORE_CHECK: &str = include_str!("../../continuum-kernel-core/src/check.rs");
const KERNEL_SAT_CHECK: &str = include_str!("../../continuum-kernel-sat/src/check.rs");
const KERNEL_SMT_CHECK: &str = include_str!("../../continuum-kernel-smt/src/check.rs");
const KERNEL_TEMPORAL_CHECK: &str = include_str!("../../continuum-kernel-temporal/src/check.rs");
const KERNEL_CORE_WIRE: &str = include_str!("../../continuum-kernel-core/src/wire.rs");
const ENGINE_CERTIFICATE: &str =
    include_str!("../../continuum-engine-reference/src/certificate.rs");

/// The sibling INV-016 evidence file, read as text so its citation above can be held to a
/// fact rather than to this file's own say-so.
const INV016_SOURCE: &str = include_str!("inv016_untrusted_source_evidence.rs");

const CHECK_CODE_POLICY: &str = include_str!("../../../tools/governance/check_code_policy.py");
const CHECK_CLAIM_GOVERNANCE: &str =
    include_str!("../../../tools/governance/check_claim_governance.py");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites and `inv002`/`inv016`
/// use it — one copy in this repository, `include_str!`'d rather than duplicated.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

// =====================================================================================
// A whitespace-tolerant JSON reader for the schema *files* (pretty-printed)
// =====================================================================================

mod schema_doc {
    //! `continuumd::codec::json::Json::parse` is for this daemon's own **canonical**
    //! (whitespace-free) wire bytes and rejects a pretty-printed document by construction
    //! — its own doc comment: "a canonical document has no insignificant whitespace, so
    //! the parser accepts none." The files under `notes/plan/schemas/` are hand-authored
    //! and pretty-printed, so this module is a second, independent, whitespace-tolerant
    //! reader for them — the same "two parsers, independent paths" device
    //! `idl_conformance.rs` and `budget_dimensions.rs` each state for their own pair.
    //!
    //! Numbers are kept as their source text (`Number`) rather than evaluated as floats:
    //! every number this file's callers read (`schema_epoch`) is a small non-negative
    //! integer, and keeping the raw text sidesteps ever needing float arithmetic at all.

    use std::collections::BTreeMap;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        Null,
        Bool(bool),
        Number(String),
        Str(String),
        Array(Vec<Value>),
        Object(BTreeMap<String, Value>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(map) => map.get(key),
                _ => None,
            }
        }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(text) => Some(text.as_str()),
                _ => None,
            }
        }

        pub fn as_bool(&self) -> Option<bool> {
            match self {
                Value::Bool(value) => Some(*value),
                _ => None,
            }
        }

        pub fn as_array(&self) -> Option<&[Value]> {
            match self {
                Value::Array(items) => Some(items),
                _ => None,
            }
        }

        pub fn as_u64(&self) -> Option<u64> {
            match self {
                Value::Number(text) => text.parse().ok(),
                _ => None,
            }
        }

        pub fn contains_str(&self, needle: &str) -> bool {
            self.as_array()
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(needle)))
        }
    }

    /// Parse `text` as a JSON document.
    ///
    /// # Panics
    ///
    /// On malformed input. Every file this reads is a tracked, hand-authored schema
    /// document (or a deliberate local mutant built from one); a parser that silently
    /// recovered from garbage could hide a real corruption instead of reporting it.
    pub fn parse(text: &str) -> Value {
        let chars: Vec<char> = text.chars().collect();
        let mut at = 0;
        let value = value(&chars, &mut at);
        skip_ws(&chars, &mut at);
        assert_eq!(at, chars.len(), "trailing bytes after the top-level value");
        value
    }

    fn skip_ws(chars: &[char], at: &mut usize) {
        while matches!(chars.get(*at), Some(c) if c.is_whitespace()) {
            *at += 1;
        }
    }

    fn value(chars: &[char], at: &mut usize) -> Value {
        skip_ws(chars, at);
        match chars.get(*at) {
            Some('{') => object(chars, at),
            Some('[') => array(chars, at),
            Some('"') => Value::Str(string(chars, at)),
            Some('t') => {
                literal(chars, at, "true");
                Value::Bool(true)
            }
            Some('f') => {
                literal(chars, at, "false");
                Value::Bool(false)
            }
            Some('n') => {
                literal(chars, at, "null");
                Value::Null
            }
            Some(c) if *c == '-' || c.is_ascii_digit() => number(chars, at),
            other => panic!("unexpected token at offset {at}: {other:?}"),
        }
    }

    fn literal(chars: &[char], at: &mut usize, text: &str) {
        for expected in text.chars() {
            assert_eq!(
                chars.get(*at),
                Some(&expected),
                "malformed literal at offset {at} (wanted {text:?})"
            );
            *at += 1;
        }
    }

    fn string(chars: &[char], at: &mut usize) -> String {
        assert_eq!(
            chars.get(*at),
            Some(&'"'),
            "expected a string at offset {at}"
        );
        *at += 1;
        let mut out = String::new();
        loop {
            match chars.get(*at) {
                Some('"') => {
                    *at += 1;
                    break;
                }
                Some('\\') => {
                    *at += 1;
                    match chars.get(*at) {
                        Some('"') => out.push('"'),
                        Some('\\') => out.push('\\'),
                        Some('/') => out.push('/'),
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some('b') => out.push('\u{8}'),
                        Some('f') => out.push('\u{c}'),
                        Some('u') => {
                            let hex: String = chars[*at + 1..*at + 5].iter().collect();
                            let code = u32::from_str_radix(&hex, 16).expect("a valid \\u escape");
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                            *at += 4;
                        }
                        other => panic!("unsupported escape at offset {at}: {other:?}"),
                    }
                    *at += 1;
                }
                Some(&character) => {
                    out.push(character);
                    *at += 1;
                }
                None => panic!("unterminated string"),
            }
        }
        out
    }

    fn number(chars: &[char], at: &mut usize) -> Value {
        let start = *at;
        if chars.get(*at) == Some(&'-') {
            *at += 1;
        }
        while matches!(
            chars.get(*at),
            Some(c) if c.is_ascii_digit() || *c == '.' || *c == 'e' || *c == 'E' || *c == '+' || *c == '-'
        ) {
            *at += 1;
        }
        Value::Number(chars[start..*at].iter().collect())
    }

    fn array(chars: &[char], at: &mut usize) -> Value {
        assert_eq!(chars.get(*at), Some(&'['));
        *at += 1;
        let mut items = Vec::new();
        skip_ws(chars, at);
        if chars.get(*at) == Some(&']') {
            *at += 1;
            return Value::Array(items);
        }
        loop {
            items.push(value(chars, at));
            skip_ws(chars, at);
            match chars.get(*at) {
                Some(',') => *at += 1,
                Some(']') => {
                    *at += 1;
                    break;
                }
                other => panic!("malformed array at offset {at}: {other:?}"),
            }
        }
        Value::Array(items)
    }

    fn object(chars: &[char], at: &mut usize) -> Value {
        assert_eq!(chars.get(*at), Some(&'{'));
        *at += 1;
        let mut map = BTreeMap::new();
        skip_ws(chars, at);
        if chars.get(*at) == Some(&'}') {
            *at += 1;
            return Value::Object(map);
        }
        loop {
            skip_ws(chars, at);
            let key = string(chars, at);
            skip_ws(chars, at);
            assert_eq!(chars.get(*at), Some(&':'), "expected ':' after key {key:?}");
            *at += 1;
            let field_value = value(chars, at);
            map.insert(key, field_value);
            skip_ws(chars, at);
            match chars.get(*at) {
                Some(',') => *at += 1,
                Some('}') => {
                    *at += 1;
                    break;
                }
                other => panic!("malformed object at offset {at}: {other:?}"),
            }
        }
        Value::Object(map)
    }
}

// =====================================================================================
// `schema_closure` — GOV-1-12's "schemas-are-closed" / "instances-name-their-contract",
// mirrored as a permanent Rust regression
// =====================================================================================

mod schema_closure {
    use super::SCHEMAS_DIR;
    use super::schema_doc::{self, Value};

    /// Every `notes/plan/schemas/*.schema.json` document, read fresh at test time — a real
    /// directory listing, not a hand-copied list, so a new file is seen without editing
    /// this function.
    pub(crate) fn schema_files() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(SCHEMAS_DIR).expect("the schemas directory is readable") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            if !name.ends_with(".schema.json") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!("{}: {error}", path.display());
            });
            out.push((name, text));
        }
        out.sort();
        out
    }

    pub(crate) fn schema_file_names() -> super::BTreeSet<String> {
        schema_files().into_iter().map(|(name, _)| name).collect()
    }

    /// What GOV-1-12's `schemas-are-closed` and `instances-name-their-contract` rules ask
    /// of one schema document, read back as booleans this file's tests can assert on.
    pub(crate) struct SchemaCheck {
        closed: bool,
        artifact_kind: bool,
        requires_schema_id: bool,
        requires_schema_epoch: bool,
        schema_id_const_pinned: bool,
        schema_epoch_const_pinned: bool,
        pub(crate) id: Option<String>,
        pub(crate) declared_epoch: Option<u64>,
    }

    impl SchemaCheck {
        /// `additionalProperties: false` at the document's own top level (`schemas-are-closed`).
        pub(crate) fn is_closed(&self) -> bool {
            self.closed
        }

        /// `schema_kind: "artifact"` documents require and `const`-pin both header fields
        /// (`instances-name-their-contract`); `schema_kind: "value"` documents (today, only
        /// `redacted.schema.json`) are exempt by schemas/README.md's own rule, so this is
        /// vacuously true for them rather than a gap.
        pub(crate) fn instance_header_is_const_pinned(&self) -> bool {
            !self.artifact_kind
                || (self.requires_schema_id
                    && self.requires_schema_epoch
                    && self.schema_id_const_pinned
                    && self.schema_epoch_const_pinned)
        }
    }

    pub(crate) fn check_schema_doc(text: &str) -> SchemaCheck {
        let doc = schema_doc::parse(text);
        let closed = doc.get("additionalProperties").and_then(Value::as_bool) == Some(false);
        let artifact_kind = doc.get("schema_kind").and_then(Value::as_str) == Some("artifact");
        let required = doc.get("required");
        let requires_schema_id =
            required.is_some_and(|required| required.contains_str("schema_id"));
        let requires_schema_epoch =
            required.is_some_and(|required| required.contains_str("schema_epoch"));
        let properties = doc.get("properties");
        let schema_id_const_pinned = properties
            .and_then(|properties| properties.get("schema_id"))
            .and_then(|schema_id| schema_id.get("const"))
            .is_some();
        let schema_epoch_const_pinned = properties
            .and_then(|properties| properties.get("schema_epoch"))
            .and_then(|schema_epoch| schema_epoch.get("const"))
            .is_some();
        let id = doc.get("$id").and_then(Value::as_str).map(str::to_owned);
        let declared_epoch = doc.get("schema_epoch").and_then(Value::as_u64);
        SchemaCheck {
            closed,
            artifact_kind,
            requires_schema_id,
            requires_schema_epoch,
            schema_id_const_pinned,
            schema_epoch_const_pinned,
            id,
            declared_epoch,
        }
    }

    // --- the real run --------------------------------------------------------------------

    #[test]
    fn every_schema_under_schemas_closes_its_top_level_and_const_pins_its_identity() {
        let files = schema_files();
        assert!(
            files.len() >= 19,
            "expected at least the 19 tracked schema documents, found {}",
            files.len()
        );
        for (name, text) in &files {
            let check = check_schema_doc(text);
            assert!(
                check.is_closed(),
                "{name}: top-level `additionalProperties` is not `false` (schemas/README.md; \
                 GOV-1-12 `schemas-are-closed`)"
            );
            assert!(
                check.instance_header_is_const_pinned(),
                "{name}: an artifact schema does not require-and-const-pin both \
                 `schema_id` and `schema_epoch` (GOV-1-12 `instances-name-their-contract`)"
            );
        }
    }

    // --- mutant: three independent ways to break closure, each caught --------------------

    #[test]
    fn the_closure_check_is_not_vacuous() {
        let (_, text) = schema_files()
            .into_iter()
            .find(|(name, _)| name == "intent-registry-record.schema.json")
            .expect("the fixture schema is tracked");
        let baseline = check_schema_doc(&text);
        assert!(baseline.is_closed());
        assert!(baseline.instance_header_is_const_pinned());

        // Mutant 1: the top-level (first) `additionalProperties: false` reopened. This file
        // also has two nested ones (`acceptance`, `chain` items); `replacen(..., 1)` hits
        // the first occurrence in file order, which is the top-level one.
        let reopened = text.replacen(
            "\"additionalProperties\": false,",
            "\"additionalProperties\": true,",
            1,
        );
        assert_ne!(reopened, text, "the mutant must actually change the text");
        assert!(
            !check_schema_doc(&reopened).is_closed(),
            "reopening the top level must be caught, or the closure check is vacuous"
        );

        // Mutant 2: `schema_id` dropped from the top-level `required` list.
        let dropped_required = text.replacen(
            "\"required\": [\n    \"schema_id\",\n    \"schema_epoch\",\n    \"record_id\",\n    \
             \"intent\",\n    \"status\"\n  ],",
            "\"required\": [\n    \"schema_epoch\",\n    \"record_id\",\n    \"intent\",\n    \
             \"status\"\n  ],",
            1,
        );
        assert_ne!(
            dropped_required, text,
            "the mutant must actually change the text"
        );
        assert!(
            !check_schema_doc(&dropped_required).instance_header_is_const_pinned(),
            "dropping `schema_id` from `required` must be caught, or the check is vacuous"
        );

        // Mutant 3: the `const` pin dropped from `properties.schema_id` (the property stays,
        // named in `required`, but no longer pinned to one value).
        let dropped_const = text.replacen(
            "\"schema_id\": {\n      \"const\": \
             \"https://continuum.dev/schema/intent-registry-record.json\",\n      \
             \"description\"",
            "\"schema_id\": {\n      \"description\"",
            1,
        );
        assert_ne!(
            dropped_const, text,
            "the mutant must actually change the text"
        );
        assert!(
            !check_schema_doc(&dropped_const).instance_header_is_const_pinned(),
            "dropping the `const` pin must be caught, or the check is vacuous"
        );
    }
}

// =====================================================================================
// `artifact_classes` — the audit's own table, anti-drift-checked against the repository
// =====================================================================================

mod artifact_classes {
    use super::BTreeSet;
    use super::schema_closure;

    /// One schema-bound artifact class: a name for reporting, and the
    /// `notes/plan/schemas/*.schema.json` file that is its normative contract.
    pub(crate) struct SchemaBound {
        pub(crate) class: &'static str,
        pub(crate) file: &'static str,
    }

    /// Every artifact class this repository binds to a schema document today, hand-
    /// maintained and checked bidirectionally against the real directory listing by
    /// `every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa` below — the
    /// same "hand-maintained inventory vs. mechanical walk" device
    /// `inv016_untrusted_source_evidence.rs`'s structural leg uses for its own free-text
    /// inventory. A schema file with no row here, or a row naming a file that no longer
    /// exists, is exactly "a new machine artifact class without a schema" (or a stale one) —
    /// and it fails this comparison, not silently.
    pub(crate) const SCHEMA_BOUND: &[SchemaBound] = &[
        SchemaBound {
            class: "assurance-result",
            file: "assurance-result.schema.json",
        },
        SchemaBound {
            class: "benchmark-task",
            file: "benchmark-task.schema.json",
        },
        SchemaBound {
            class: "cir",
            file: "cir.schema.json",
        },
        SchemaBound {
            class: "context-pack",
            file: "context-pack.schema.json",
        },
        SchemaBound {
            class: "corpus-port",
            file: "corpus-port.schema.json",
        },
        SchemaBound {
            class: "crashpack",
            file: "crashpack.schema.json",
        },
        SchemaBound {
            class: "domain-pack",
            file: "domain-pack.schema.json",
        },
        SchemaBound {
            class: "evidence-graph-edge",
            file: "evidence-graph-edge.schema.json",
        },
        SchemaBound {
            class: "evidence-graph-node",
            file: "evidence-graph-node.schema.json",
        },
        SchemaBound {
            class: "intent-contract",
            file: "intent-contract.schema.json",
        },
        SchemaBound {
            class: "intent-registry-record",
            file: "intent-registry-record.schema.json",
        },
        SchemaBound {
            class: "promotion-receipt",
            file: "promotion-receipt.schema.json",
        },
        SchemaBound {
            class: "proof-receipt",
            file: "proof-receipt.schema.json",
        },
        SchemaBound {
            class: "redacted",
            file: "redacted.schema.json",
        },
        SchemaBound {
            class: "repair-transaction",
            file: "repair-transaction.schema.json",
        },
        SchemaBound {
            class: "semantic-diff",
            file: "semantic-diff.schema.json",
        },
        SchemaBound {
            class: "synthesis-candidate",
            file: "synthesis-candidate.schema.json",
        },
        SchemaBound {
            class: "verification-task",
            file: "verification-task.schema.json",
        },
        SchemaBound {
            class: "workspace-snapshot",
            file: "workspace-snapshot.schema.json",
        },
    ];

    fn table_files() -> BTreeSet<&'static str> {
        SCHEMA_BOUND.iter().map(|entry| entry.file).collect()
    }

    /// `class` (the reporting name) is never read by the bidirectional comparison itself —
    /// only `file` is — so this is where its own integrity is checked: non-empty, and no
    /// two rows report under the same name.
    #[test]
    fn every_table_row_names_a_non_empty_unique_class() {
        let mut classes = BTreeSet::new();
        for entry in SCHEMA_BOUND {
            assert!(!entry.class.is_empty(), "{}: empty class name", entry.file);
            assert!(
                classes.insert(entry.class),
                "{:?} is reported as the class for more than one table row",
                entry.class
            );
        }
    }

    #[test]
    fn every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa() {
        let table = table_files();
        assert_eq!(
            table.len(),
            SCHEMA_BOUND.len(),
            "the table names one schema file more than once"
        );
        let real = schema_closure::schema_file_names();
        let table_owned: BTreeSet<String> = table.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(
            table_owned, real,
            "the hand-maintained artifact-class table and notes/plan/schemas/ disagree: a \
             new schema file with no table row (or a table row naming a file that no longer \
             exists) is exactly \"a new machine artifact class without a schema\""
        );
    }

    #[test]
    fn dropping_one_table_row_breaks_the_comparison() {
        let real = schema_closure::schema_file_names();
        let mut missing_one = table_files();
        let dropped = *missing_one.iter().next().expect("the table is non-empty");
        missing_one.remove(dropped);
        let missing_one_owned: BTreeSet<String> =
            missing_one.iter().map(|name| (*name).to_owned()).collect();
        assert_ne!(
            missing_one_owned, real,
            "removing {dropped:?} from the table must break the comparison, or the anti-drift \
             check is vacuous"
        );
    }
}

// =====================================================================================
// Shared daemon fixture (the idioms `inv002`/`inv016` use, trimmed to `intent` + `evidence`
// + `observe` — the only families `schema_resolution` needs)
// =====================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn opname(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn profile(privileged: &[&str], data_grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| opname(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: data_grants.to_vec(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-inv003-evidence-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// A daemon with `intent` + `evidence` + `observe` registered — enough to accept an intent
/// (naming an `intent-registry-record`) and to ingest a trace then read it back
/// (naming an `evidence-graph-node`), and nothing more: `schema_resolution` below is the
/// only consumer.
fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                5,
                Optional::Present(profile(&["intent.accept"], &[DataGrant::ProductionTrace])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept"], &[])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root,
        )
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .build()
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: opname(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(states) => Optional::Present(states),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn mutating(mut envelope: RequestEnvelope, key: &str, states: Option<u64>) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope.budget = Optional::Present(budget(states));
    envelope
}

/// The acceptance record `intent.accept` requires — the same shape
/// `daemon_operations.rs`/`inv016`'s fixtures build.
fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, IntentJson> =
        std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-inv003-evidence"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), IntentJson::String(value.to_owned()));
    }
    Opaque::from_bytes(IntentJson::Object(fields).to_canonical_bytes())
}

// =====================================================================================
// `schema_resolution` — the daemon's own emitted artifacts, resolved to real schema
// documents by schemas/README.md's own recovery rule, mechanically applied
// =====================================================================================

mod schema_resolution {
    use super::schema_closure;
    use super::{
        Arguments, EvidenceGetRequest, EvidenceHandle, IntentAcceptRequest, IntentJson,
        IntentRecord, ObserveIngestRequest, OperationRequest, Optional, Payload, RegistryStatus,
        ResultStatus,
    };
    use super::{
        acceptance_bytes, daemon, die_hard_contract, envelope, intent_handle, keyed, mutating,
    };

    /// `schemas/README.md`'s own recovery rule, applied mechanically: the governing
    /// document is named by inserting `v<schema_epoch>/` before the last segment of the
    /// instance's `schema_id`, and its file lives at `notes/plan/schemas/<name>.schema.json`.
    /// Returns the resolved document's own text when — and only when — a real file exists
    /// at that name *and* its own `$id`/`schema_epoch` agree with what the instance named
    /// (an instance cannot resolve to a document that disagrees with it about its own
    /// identity).
    fn resolve(schema_id: &str, schema_epoch: u64) -> Option<(String, String)> {
        let class = schema_id
            .strip_prefix("https://continuum.dev/schema/")?
            .strip_suffix(".json")?;
        let file_name = format!("{class}.schema.json");
        let (_, text) = schema_closure::schema_files()
            .into_iter()
            .find(|(name, _)| *name == file_name)?;
        let check = schema_closure::check_schema_doc(&text);
        if check.declared_epoch != Some(schema_epoch) {
            return None;
        }
        let expected_id = format!("https://continuum.dev/schema/v{schema_epoch}/{class}.json");
        if check.id.as_deref() != Some(expected_id.as_str()) {
            return None;
        }
        Some((file_name, text))
    }

    fn schema_id_and_epoch(record: &IntentJson) -> (String, u64) {
        let IntentJson::Object(fields) = record else {
            panic!("the record is an object");
        };
        let schema_id = match fields.get("schema_id") {
            Some(IntentJson::String(text)) => text.clone(),
            other => panic!("schema_id is missing or not a string: {other:?}"),
        };
        let schema_epoch = match fields.get("schema_epoch") {
            Some(IntentJson::Integer(value)) => {
                u64::try_from(*value).expect("schema_epoch is non-negative")
            }
            other => panic!("schema_epoch is missing or not an integer: {other:?}"),
        };
        (schema_id, schema_epoch)
    }

    fn assert_resolves_and_is_closed(schema_id: &str, schema_epoch: u64) {
        let (file, text) = resolve(schema_id, schema_epoch).unwrap_or_else(|| {
            panic!("{schema_id} (epoch {schema_epoch}) does not resolve to any schemas/ document")
        });
        let check = schema_closure::check_schema_doc(&text);
        assert!(
            check.is_closed(),
            "{file}: the resolved document does not close its top level"
        );
        assert!(
            check.instance_header_is_const_pinned(),
            "{file}: the resolved document does not const-pin its own identity"
        );
    }

    #[test]
    fn the_intent_registry_record_the_daemon_emits_names_a_schema_the_tree_resolves() {
        let mut instance = daemon();
        let contract = die_hard_contract();
        let handle = intent_handle(&contract);
        instance.state_mut().put_intent(
            handle.clone(),
            IntentRecord {
                contract,
                status: RegistryStatus::Proposed,
                supersedes: None,
                superseded_by: None,
                acceptance: None,
            },
        );
        let accepted = instance.dispatch(&OperationRequest {
            envelope: keyed(
                envelope(
                    "intent.accept",
                    "human:steward",
                    "cap_steward",
                    "req_accept",
                ),
                "idem-accept",
            ),
            arguments: Arguments::IntentAccept(IntentAcceptRequest {
                proposal: handle,
                acceptance: acceptance_bytes(),
                bundle: Optional::Absent,
            }),
        });
        assert_eq!(
            accepted.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            accepted.envelope.error
        );
        let record = match &accepted.payload {
            Payload::IntentAccept(response) => {
                IntentJson::parse(response.record.as_bytes()).expect("a canonical record")
            }
            other => panic!("expected an intent.accept payload, got {other:?}"),
        };
        let (schema_id, schema_epoch) = schema_id_and_epoch(&record);
        assert_eq!(
            schema_id,
            "https://continuum.dev/schema/intent-registry-record.json"
        );
        assert_resolves_and_is_closed(&schema_id, schema_epoch);
    }

    #[test]
    fn the_evidence_graph_node_the_daemon_emits_names_a_schema_the_tree_resolves() {
        let mut instance = daemon();
        let trace = instance
            .state_mut()
            .stage(
                &super::Blake3Identity,
                super::WorkspacePath::new("traces/inv003.jsonl").expect("a workspace path"),
                b"{\"events\":[{\"at\":0,\"op\":\"observe\"}]}\n".to_vec(),
            )
            .expect("staging names its content");
        let ingested = instance.dispatch(&OperationRequest {
            envelope: mutating(
                envelope(
                    "observe.ingest",
                    "agent:observer",
                    "cap_observer",
                    "req_ingest",
                ),
                "idem-ingest",
                None,
            ),
            arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                trace,
                instrumentation_profile: "inv003-evidence-profile".to_owned(),
            }),
        });
        assert_eq!(
            ingested.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            ingested.envelope.error
        );
        let handle: EvidenceHandle = match &ingested.payload {
            Payload::ObserveIngest(response) => response
                .evidence
                .first()
                .cloned()
                .expect("an ingest names the node it appended"),
            other => panic!("expected an observe.ingest payload, got {other:?}"),
        };

        let got = instance.dispatch(&OperationRequest {
            envelope: envelope("evidence.get", "agent:reader", "cap_reader", "req_get"),
            arguments: Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: handle,
                inline: Optional::Absent,
            }),
        });
        assert_eq!(
            got.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            got.envelope.error
        );
        let node = match &got.payload {
            Payload::EvidenceGet(response) => match &response.node {
                super::Nullable::Value(node) => {
                    IntentJson::parse(node.as_bytes()).expect("a canonical node record")
                }
                super::Nullable::Null => {
                    panic!("the node record is not null for a node the graph holds")
                }
            },
            other => panic!("expected an evidence.get payload, got {other:?}"),
        };
        let (schema_id, schema_epoch) = schema_id_and_epoch(&node);
        assert_eq!(
            schema_id,
            "https://continuum.dev/schema/evidence-graph-node.json"
        );
        assert_resolves_and_is_closed(&schema_id, schema_epoch);
    }

    // --- mutant: a schema_id that does not resolve, and one at the wrong epoch -----------

    #[test]
    fn a_schema_id_that_names_no_real_class_does_not_resolve() {
        // Sanity first: the positive case really does resolve, so a `None` below is the
        // resolver's own verdict, not a bug that makes it always fail.
        assert!(
            resolve(
                "https://continuum.dev/schema/intent-registry-record.json",
                1
            )
            .is_some()
        );

        assert!(
            resolve(
                "https://continuum.dev/schema/no-such-artifact-class.json",
                1
            )
            .is_none(),
            "a schema_id naming no real class must not resolve"
        );
        assert!(
            resolve(
                "https://continuum.dev/schema/intent-registry-record.json",
                99
            )
            .is_none(),
            "a real class at an epoch no document declares must not resolve"
        );
    }
}

// =====================================================================================
// `wire_bytes_not_prose` — the certificate checker's entry point, and the CONTCERT magic
// =====================================================================================

mod wire_bytes_not_prose {
    //! The certificate checker consumes wire **bytes**, never prose: all four
    //! `continuum-kernel-*` crates declare `pub fn check_certificate(bytes: &[u8]) ->
    //! Verdict` (`crates/continuum-kernel-{core,sat,smt,temporal}/src/check.rs`) — no
    //! `&str`/`String` overload exists anywhere in the trusted checking base. This is a
    //! text-scan citation of that fact (this test file has no dependency on the four
    //! kernel crates, so it reads their sources as data rather than calling them), which is
    //! exactly the "certificate checker consumes wire BYTES not prose (cite kernel-core's
    //! entry points mechanically)" instruction this file was written to satisfy.

    const SIGNATURE: &str = "pub fn check_certificate(bytes: &[u8]) -> Verdict {";

    #[test]
    fn every_kernel_checkers_entry_point_takes_bytes_never_text() {
        for (name, source) in [
            ("continuum-kernel-core", super::KERNEL_CORE_CHECK),
            ("continuum-kernel-sat", super::KERNEL_SAT_CHECK),
            ("continuum-kernel-smt", super::KERNEL_SMT_CHECK),
            ("continuum-kernel-temporal", super::KERNEL_TEMPORAL_CHECK),
        ] {
            assert!(
                source.contains(SIGNATURE),
                "{name}: check_certificate's entry point signature drifted from \
                 `bytes: &[u8]`"
            );
        }
    }

    #[test]
    fn the_signature_check_is_not_vacuous() {
        let mutated = super::KERNEL_CORE_CHECK.replacen(
            "pub fn check_certificate(bytes: &[u8]) -> Verdict {",
            "pub fn check_certificate(text: &str) -> Verdict {",
            1,
        );
        assert_ne!(
            mutated,
            super::KERNEL_CORE_CHECK,
            "the mutant must change the text"
        );
        assert!(
            !mutated.contains(SIGNATURE),
            "a text-shaped signature must not be mistaken for a bytes-shaped one"
        );
    }

    /// The kernel and the reference engine each declare the certificate's magic
    /// independently (`continuum-kernel-core/src/wire.rs` is the trusted checking base;
    /// `continuum-engine-reference/src/certificate.rs` is the producer) — two hand-authored
    /// copies of one fact, in docs/03 §8's "independent paths" style, rather than one
    /// constant either side imports from the other (the checker may not depend on the
    /// engine at all — INV-004, plan §20).
    #[test]
    fn the_kernel_and_the_engine_independently_declare_the_same_contcert_magic() {
        const MAGIC_DECLARATION: &str = "pub const MAGIC: [u8; 8] = *b\"CONTCERT\";";
        assert!(super::KERNEL_CORE_WIRE.contains(MAGIC_DECLARATION));
        assert!(super::ENGINE_CERTIFICATE.contains(MAGIC_DECLARATION));
    }
}

// =====================================================================================
// `recovery_reports_are_typed` — a machine artifact class with no schema file, closed by
// the compiler instead
// =====================================================================================

mod recovery_reports_are_typed {
    //! `RecoveryReport` (plan §4.5, `crates/continuumd/src/daemon/recovery.rs`) is a
    //! machine-consumed artifact class with no `notes/plan/schemas/*.schema.json` document
    //! of its own — G1 crash recovery is typed Rust, not a wire artifact. INV-003 still
    //! binds it: its own doc comment states plainly that its machine-readable surface is
    //! its typed fields and accessor methods, and `render() -> String` is the deliberate
    //! prose sidecar — "human text may accompany a result" — never fed back as an input
    //! anywhere (`g1_crash_recovery_evidence.rs`, which this file cites rather than
    //! re-running). `Verdict`, the report's own headline judgment, is a plain three-variant
    //! enum with no `#[non_exhaustive]`: the `match` below has no wildcard arm, so a fourth
    //! variant added anywhere is a compile error at this call site — INV-003's "schemas
    //! decide" enforced by the compiler rather than by a runtime validator, which is the
    //! strongest closure a Rust type can offer.

    use continuumd::daemon::recovery::Verdict;

    fn describe(verdict: Verdict) -> &'static str {
        match verdict {
            Verdict::Clean => "clean",
            Verdict::Residue => "residue",
            Verdict::Defective => "defective",
        }
    }

    #[test]
    fn verdict_is_a_closed_three_variant_enum_every_arm_named() {
        assert_eq!(describe(Verdict::Clean), "clean");
        assert_eq!(describe(Verdict::Residue), "residue");
        assert_eq!(describe(Verdict::Defective), "defective");
    }
}

// =====================================================================================
// `dispatch_reads_structure_not_prose` — freshness tripwire on the INV-016 citation
// =====================================================================================

mod dispatch_reads_structure_not_prose {
    //! `inv016_untrusted_source_evidence.rs`'s structural leg already proves, mechanically,
    //! that `Daemon::dispatch` keys on the operation *name* (a closed registry lookup)
    //! across all 28 landed operations, and that the other 45 are inert for *any* payload —
    //! "the IDL has no operation whose dispatch reads free text", the fact this file's audit
    //! asserts rather than re-derives. This module does not re-run that proof; it is the
    //! tripwire that the citation stays true if the cited file is ever renamed or gutted
    //! underneath it.

    #[test]
    fn the_inv016_structural_leg_this_file_cites_still_exists() {
        for needle in [
            "fn the_free_text_inventory_reachable_from_every_landed_request_is_exactly_this_list",
            "fn every_unlanded_operation_is_inert_for_any_payload_whatsoever",
            "fn exactly_twenty_eight_of_the_seventy_three_operations_are_landed",
            "never against anything a free-text field carries",
        ] {
            assert!(
                super::INV016_SOURCE.contains(needle),
                "inv016_untrusted_source_evidence.rs no longer contains {needle:?}; this \
                 file's INV-003 citation of its structural leg needs updating"
            );
        }
    }
}

// =====================================================================================
// `cited_enforcement_still_exists` — freshness tripwires on the GOV-1-10/11/12 and
// claim-governance citations
// =====================================================================================

mod cited_enforcement_still_exists {
    #[test]
    fn gov_1_12_ambiguity_is_an_error_still_names_the_four_rules_this_file_leans_on() {
        for needle in [
            "GOV-1-12",
            "rule_ambiguity_is_an_error",
            "\"schemas-are-closed\"",
            "\"instances-name-their-contract\"",
            "\"verdict-vocabulary-is-closed\"",
            "\"inconclusive-carries-its-reason\"",
        ] {
            assert!(
                super::CHECK_CODE_POLICY.contains(needle),
                "tools/governance/check_code_policy.py no longer contains {needle:?}; the \
                 GOV-1-12 citation above needs updating"
            );
        }
    }

    #[test]
    fn gov_1_10_and_gov_1_11_are_still_the_pack_contract_and_reference_precedence_rules() {
        for needle in [
            "\"pack-operation-contract\"",
            "\"reference-precedes-optimization\"",
            "GOV-1-10",
            "GOV-1-11",
        ] {
            assert!(
                super::CHECK_CODE_POLICY.contains(needle),
                "missing {needle:?}"
            );
        }
    }

    #[test]
    fn check_claim_governance_still_binds_the_claim_registry_bidirectionally() {
        for needle in ["claim-registry-mirror", "claim-id-resolves", "docs/18"] {
            assert!(
                super::CHECK_CLAIM_GOVERNANCE.contains(needle),
                "tools/governance/check_claim_governance.py no longer contains {needle:?}"
            );
        }
    }
}
