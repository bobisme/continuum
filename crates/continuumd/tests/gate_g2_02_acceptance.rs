//! G2-02 acceptance — **independent re-derivation** of "no terminal parsing required".
//!
//! > no terminal parsing required
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:36`, gate G2, criterion **G2-02**
//!
//! The constitutional backdrop is the plan's own headline — "typed operations over explicit
//! handles, not terminal prose" — INV-003 ("Schemas decide, prose does not"; human text "may
//! accompany a machine result, never define it"), and the IDL's `rule envelope.no_prose`.
//!
//! # What this file is, and what it deliberately is not
//!
//! It is **not** a re-run of the delivering suite. `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md:784`
//! grades G2-02 "evidenced — `continuum-mcp/tests/typed_surface.rs`; every argument and every
//! answer, success or refusal, is a typed value; nothing is parsed out of a rendered line".
//! That suite asks a **presence** question: *is there a typed value in the answer?* Its
//! refusal test is named `a_daemon_refusal_is_a_typed_code_and_not_a_message` and it asserts
//! `refusal.code == ErrorCode::CapabilityDenied`. A protocol that answered every one of two
//! hundred distinct conditions with one code and put the distinguishing fact in `detail` would
//! pass it unchanged.
//!
//! This file asks the **sufficiency** question the criterion actually poses: *can an agent
//! finish the work reading only typed values?* Two halves, and they are separable:
//!
//! 1. **Forward closure.** For every step of every workflow this build can drive, is the
//!    input the next step needs recoverable from the previous answer at a **typed position**,
//!    or only from inside a human-text field?
//! 2. **Refusal determinacy.** At a refusal, do the code and the typed fields — together with
//!    what the agent already knows, namely its own request and the static registry — determine
//!    the next action, or must the agent read `detail`?
//!
//! Four method differences carry the independence from `typed_surface.rs`:
//!
//! 1. **The instrument is generic and spec-driven, not per-field.** [`leaves`] walks the answer
//!    *document* — the canonical JSON an agent actually receives from `Server::answer` — and
//!    names no field in advance. It gets each leaf's **declared IDL type** from the shipped
//!    `registry::NAMED_STRUCTS` / `UNIONS` tables rather than from the value's spelling, which
//!    is what makes it exact: a `TaskHandle`, a `Commitment` over a task artifact, and the
//!    `ResultStatus` member `task_started` are all the string `task_…` on the wire and are
//!    three different things. `typed_surface.rs` reads named accessors off decoded Rust
//!    values, so it can only see fields it already knows to look at. This walk also descends
//!    into spliced `Opaque` sub-documents, because `Opaque`'s codec inlines a canonical body
//!    rather than base64-ing it.
//! 2. **The workflows close over the wire, not over Rust.** [`recover`] pulls the next step's
//!    handle out of the answer *document* at positions the IDL declares to be that handle
//!    class, and the workflow is then driven from the recovered string. A handle that reached
//!    the agent only inside a sentence would leave [`recover`] with nothing typed and the
//!    workflow would stop. This is the criterion stated as a program, and
//!    [`the_census_flags_a_handle_that_reaches_the_agent_only_inside_a_sentence`] is its
//!    negative control.
//! 3. **The prose set is computed from the normative file, not declared here.**
//!    [`prose_fields`] parses `notes/plan/schemas/continuumd-native-protocol.idl` for every
//!    field whose name matches a human-text lexicon, splits them by *declared type*, and an
//!    adjudication table must name each one — so a prose field added to the IDL fails this
//!    file rather than slipping past it.
//! 4. **Refusals are classified by determinacy, not by typedness.** [`Determinacy`] is a
//!    five-rung ladder and exactly one rung — [`Determinacy::ProseOnly`] — is a G2-02 failure.
//!    Two refusals that share a code and need different recoveries are *not* a finding when
//!    the agent's own request separates them, or when a typed probe does, or when `detail` is
//!    uniform and separates nothing either; they are a finding when only `detail` does. The
//!    word in the criterion is "required".
//!
//! # Relationship to the G1-02 acceptance
//!
//! `gate_g1_02_acceptance.rs` classified all 666 leaves of the IDL and asked, of each, *is
//! this a handle?* This file does not repeat that sweep and does not re-derive its numbers.
//! Its question is the other one: *is prose ever the only carrier of a machine-needed fact?*
//! Its table 6 flagged that plan §4.4's artifact-class vocabulary travels as a bare `String`
//! in three places "as a G2-02 concern". [`the_artifact_class_spelling_is_a_typed_refusal_at_provisioning`]
//! guards its repair: see finding **F3** below.
//!
//! # The census, in numbers
//!
//! Every number below is computed at run time by the tests named beside it, and
//! [`report`] prints the whole census on demand. None is quoted from the exit package.
//!
//! | | |
//! |---|---|
//! | workflows driven end to end over a byte boundary | 8 |
//! | inter-step facts a workflow took from the previous answer | 27 |
//! | of those, recovered from a position the IDL declares as the needed type | **27** |
//! | of those, available **only** inside a human-text field | **0** |
//! | facts entering a workflow from outside the protocol | 3 ([`OUT_OF_BAND`]) |
//! | answers swept for the two declared recovery channels | 39 |
//! | of those carrying a non-empty `next_operations` or `Error.recovery` | **2** (stale-handle `Error.recovery` offers, bn-27mx7; `next_operations` on none) |
//! | refusal probes driven live | 14 |
//! | distinct `ErrorCode`s reached | 6 |
//! | probes decided by the typed answer alone | 2 |
//! | probes decided by the answer plus the agent's own request and the registry | 10 |
//! | probes decided only by a typed re-probe | 0 |
//! | probes decided **only by reading `detail`** | **0** |
//! | probes no channel decides — `detail` included | **2** (finding F2) |
//! | IDL fields matching the human-text lexicon | 23 |
//! | of those, already carried by a closed enum | 5 |
//! | of those, `Opaque` schema-governed sub-documents | 4 |
//! | of those, bare `String`/`list`/`map` — the prose surface proper | 14 |
//! | prose fields that are the **sole carrier** of a machine-needed fact | **0** |
//! | CLI commands, every one a registry operation | 16 |
//! | registry operations the CLI cannot reach | 59 |
//!
//! # Verdict
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** No workflow this build can drive requires an agent to
//! parse human-oriented text, and no refusal it can raise is decided by prose. The narrowing
//! is not softened, and it is two findings wide; the third, F3, is repaired and kept as a guard.
//!
//! | # | Scope statement | Test |
//! |---|---|---|
//! | 1 | Forward closure is **total** over the eight workflows: all 27 inter-step facts sit at a position whose *declared IDL type* is the type the next step needs, and the workflows are literally driven from the values recovered there | [`workflow_closure_is_total_over_every_workflow_this_build_can_drive`] |
//! | 2 | Terminal outcomes are typed too. A budget that cannot hold the model's own initial states is not an envelope refusal at all — it starts a task and fails it — and the record says `status = failed`, `failed_reason = BudgetExhausted` (an `ErrorCode`, not a sentence) with **no** continuation. The three facts an agent branches on are three typed fields | [`a_non_resumable_task_failure_is_typed_without_reading_its_sentence`] |
//! | 3 | The CLI is a strict subset — 16 commands, all 16 registry operations, none CLI-only — and its shipped binary carries `NullTransport`, so it reaches no daemon at all. An agent needing the terminal here is not merely unnecessary; it is impossible | [`the_cli_reaches_nothing_the_typed_protocol_does_not`], [`the_shipped_cli_binary_reaches_no_daemon_at_all`] |
//! | 4 | **FINDING F1**: the two typed recovery channels the IDL declares — `ResultEnvelope.next_operations` and `Error.recovery` — were empty on every answer this file collected (40 then, 39 since bn-3ncfp removed the `ws_` probe), success and refusal alike, because the daemon's own `Fault` type had no field that could fill `recovery`. bn-27mx7 wired `Error.recovery` for stale-handle refusals (RFC 0027 H8), so the pin is now a probe: every offer is executable as given. `next_operations` is still empty | [`next_operations_is_empty_and_every_recovery_offer_is_executable_as_given`] |
//! | 5 | **FINDING F2**: `retryable` is `false` on every refusal, `Error.data` present on none, `Error.continuation` on none. The per-occurrence typed discriminator is therefore the code alone, and **two** probes — an under-authorized principal and an absent handle — collapse onto one `CapabilityDenied` whose `detail` is byte-identical. **No channel separates them, `detail` included.** This is deliberate (RFC 0027 X1–X2, no existence oracle) and it is *not* a prose dependency; it is a typed underdetermination whose honest reading is that the recovery is a disjunction, not a lookup | [`the_only_undetermined_refusals_are_the_ones_prose_does_not_rescue_either`], [`no_refusal_carries_a_typed_discriminator_beyond_its_code`] |
//! | 6 | **FINDING F3, repaired (bn-3ncfp)**: plan §4.4's artifact-class vocabulary was a bare `String` in three places, with the *spelling* — `ws` or `ws_` — fixed only by doc comments that disagreed, so a capability scoped with the plan's literal prefix was silently scoped to nothing. IDL 1.13's `rule artifact_class.spelling` now fixes the class token (`ws`) and lists all nineteen, every site cites it, and the rule's list equals `ArtifactClass::ALL`. The prefix spelling is a typed `ProvisioningRefusal::ArtifactClass` at `Builder::try_build` and `register_capability`, which names the capability, the spelling, and the class it meant; the capability is never registered. The wire fields stay `String` inside major 3 (RFC 0026 F22) | [`the_artifact_class_spelling_is_a_typed_refusal_at_provisioning`] |
//! | 7 | The live surface is **30 of 75** operations. The other 45 have no `Arguments` variant and are refused by the codec before a body is read, so forward closure over them is **untested, not evidenced** | [`the_live_surface_is_thirty_of_the_seventy_five_and_this_is_which`] |
//! | 8 | Three facts enter the workflows from outside the protocol — staged content commitments, the initial intent handle, and the registered model catalog. No operation mints them. They are not prose either; they are absences | [`the_facts_that_enter_from_outside_the_protocol_are_exactly_these`] |
//!
//! # Why the coarse codes are not failures, stated precisely
//!
//! Ten of the fourteen probes share a code with a probe needing a different recovery, and none
//! of the ten is a G2-02 failure, for a reason worth naming rather than assuming. Two examples
//! carry it:
//!
//! - `MalformedRequest` covers both "a mutation with no idempotency key" and "a read carrying
//!   one" — *opposite* fixes under one code. The agent separates them without being told,
//!   because the registry declares `@mutation`/`@readonly` statically and the agent knows which
//!   operation it sent.
//! - `StaleSnapshot` covers "the snapshot is not sealed" and "the snapshot has been superseded"
//!   — different fixes, same operation, same request shape. The agent separates them because
//!   `WorkspaceCreateResponse.sealed` is a typed field it already read.
//!
//! In both, the deciding fact is typed and already in the agent's hands. That is the difference
//! between a coarse vocabulary and a prose dependency, and [`Determinacy`] is that difference
//! made a value.
//!
//! F1 is the one that would become a failure under a *stronger* reading of the criterion —
//! "an agent can discover its next move **from the answer**". This file does not read it that
//! way: the static registry is a machine-readable artifact, `continuum-mcp::register` computes
//! an admissible operation set from the caller's held handles without a frame, and neither
//! involves parsing a rendered line. F1 is reported as scope because the declared channel is
//! declared, and empty.
//!
//! # Negative controls
//!
//! Six, each a real assertion. Four weaken an instrument and require it to report; two require
//! an instrument to *disagree* with a doctored input rather than nod at it.
//!
//! | Control | What it doctors | What must happen |
//! |---|---|---|
//! | [`the_census_flags_a_handle_that_reaches_the_agent_only_inside_a_sentence`] | strikes a real `task_` handle from every typed position of a real answer and appends a `Warning` whose `detail` names it | the census reports it prose-only, and the typed surface is left naming a placeholder |
//! | [`the_census_does_not_call_a_typed_position_prose`] | leaves the same answer alone | the same handle is reported typed and nothing is scrapeable, so the instrument is not a constant |
//! | [`the_walk_reaches_inside_a_spliced_opaque_body_and_types_what_it_finds`] | — | the walk descends into `payload` under the operation's declared response shape, and separates the two declared types that both spell `task_…` |
//! | [`the_scraper_is_a_real_parser_and_not_a_stub`] | — | the prose parser really extracts a handle from a sentence, so "no prose hit" means something |
//! | [`the_determinacy_ladder_separates_a_prose_split_from_a_uniform_one`] | two synthetic refusal pairs, one with differing details and one with identical ones | the first classifies [`Determinacy::ProseOnly`], the second [`Determinacy::Undetermined`] |
//! | [`the_prose_table_is_not_a_wildcard`] and [`the_operation_name_derivation_is_not_vacuous`] | — | the adjudication table answers `None` for real typed fields, and a derived wire name the registry does not declare is refused |

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec;
use continuumd::codec::json::Json as Wire;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, is_mutation};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::intent::{IntentAcceptRequest, IntentGetRequest};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceDiffRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, EvidenceHandle,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{EvidenceQuery, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{
    Annotation, Nullable, Optional, ProtocolEnum, StructSpec, UnionSpec,
};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, DiffLayer, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind,
};
use continuumd::transport::{self, Server};

/// The repository root, reached from this crate's manifest.
const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The normative wire authority, relative to [`REPO`].
const IDL: &str = "notes/plan/schemas/continuumd-native-protocol.idl";

/// The human-facing adapter whose surface is measured against the protocol's.
const CLI_SOURCE: &str = "crates/continuum-cli/src";

/// Read a repository-relative file.
fn read(relative: &str) -> String {
    let path = Path::new(REPO).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative}: {error}"))
}

// =====================================================================================
// 1. What counts as prose — computed from the normative file
// =====================================================================================

/// Field-name stems that mark a field as written for a person to read.
///
/// A name matches when it equals a stem or ends in `_` followed by one, which is what puts
/// `non_resumable_reason`, `inconclusive_reason` and `failed_reason` in the sweep. The list is
/// deliberately wider than the IDL turns out to use: a lexicon that only named the fields that
/// exist would be a description of the answer rather than a test of it, and
/// [`the_lexicon_is_wider_than_the_protocol`] pins the difference.
const PROSE_LEXICON: &[&str] = &[
    "advice",
    "description",
    "detail",
    "doc",
    "explanation",
    "hint",
    "message",
    "note",
    "prose",
    "rationale",
    "reason",
    "reasons",
    "remediation",
    "summary",
    "text",
    "title",
];

/// Whether a wire field name is written for a person.
fn lexical_prose(field: &str) -> bool {
    PROSE_LEXICON
        .iter()
        .any(|stem| field == *stem || field.ends_with(&format!("_{stem}")))
}

/// How a human-text-named field actually carries its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Carriage {
    /// A closed enum. The name reads as prose; the type is a token an agent compares.
    Typed,
    /// An `Opaque` body governed by a `schemas/` document, spliced inline on the wire.
    Document,
    /// A bare `String`, `list<String>` or `map<String, String>`: the prose surface proper.
    Free,
}

/// Why a free-text field is, or is not, the sole carrier of a fact a workflow needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Load {
    /// A typed sibling in the same message carries the same fact.
    TypedSibling(&'static str),
    /// No step of any workflow consumes it; it accompanies a machine result (INV-003).
    Accompanies,
    /// The caller writes it; the daemon never reads it back as a machine fact.
    CallerAuthored,
    /// Prose is the only carrier of a fact some workflow needs. A G2-02 failure.
    #[expect(
        dead_code,
        reason = "the vocabulary is the finding; no field carries it today"
    )]
    SoleCarrier,
}

/// One field of the IDL whose name matches [`PROSE_LEXICON`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProseField {
    /// `Struct.field`, or `operation.direction.field` for an anonymous body.
    site: String,
    /// The wire key, which is what [`leaves`] sees.
    field: String,
    /// The declared type, annotations stripped.
    declared: String,
    /// 1-based line in the IDL.
    line: usize,
}

impl ProseField {
    /// How this field carries its value, from its declared type alone.
    fn carriage(&self) -> Carriage {
        match self.declared.as_str() {
            "String" | "list<String>" | "map<String, String>" => Carriage::Free,
            "Opaque" | "Bytes" => Carriage::Document,
            _ => Carriage::Typed,
        }
    }
}

/// Every field of the IDL, with the declaration that encloses it.
///
/// A second reader, and a third strategy: `idl_conformance.rs` tokenizes and descends,
/// `gate_g1_02_acceptance.rs` slices brace-balanced regions, and this one is a line-oriented
/// scan that keeps only what a *field* line can be — `name: Type presence;`. Requiring the
/// presence keyword is what makes it robust without a grammar: enum members
/// (`revise_intent = "revise-intent",`) and union variants (`produced(ProducedDimension),`)
/// carry none, so they cannot be mistaken for fields.
fn idl_fields() -> Vec<ProseField> {
    let text = read(IDL);
    let mut found = Vec::new();
    let mut owner = String::new();
    let mut operation = String::new();
    let mut in_rule = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        // Comments first: the header's own EBNF quotes the rule delimiter inside a `//`
        // line, and toggling on those would invert the flag for the rest of the file.
        if line.starts_with("//") {
            continue;
        }
        if line.contains("\"\"\"") {
            in_rule = !in_rule;
            continue;
        }
        if in_rule {
            continue;
        }
        if let Some(rest) = line.strip_prefix("struct ") {
            owner = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            operation.clear();
            continue;
        }
        if let Some(rest) = line.strip_prefix("operation ") {
            operation = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            owner.clear();
            continue;
        }
        if !operation.is_empty() {
            for direction in ["request", "response", "event"] {
                if line.starts_with(direction) && line.ends_with('{') {
                    owner = format!("{operation}.{direction}");
                }
            }
        }
        let Some(field) = field_declaration(line) else {
            continue;
        };
        if owner.is_empty() {
            continue;
        }
        found.push(ProseField {
            site: format!("{owner}.{}", field.0),
            field: field.0,
            declared: field.1,
            line: index + 1,
        });
    }
    found
}

/// Split `name: Type presence [@annotation…];` into its name and its declared type.
///
/// Returns [`None`] for every other line, which is what keeps the scan honest without a
/// grammar: the presence keyword is mandatory in the IDL's own EBNF and appears nowhere else.
fn field_declaration(line: &str) -> Option<(String, String)> {
    let body = line.strip_suffix(';')?;
    let (name, rest) = body.split_once(':')?;
    if !name.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) || name.is_empty()
    {
        return None;
    }
    let mut declared = String::new();
    let mut presence = None;
    for token in rest.split_whitespace() {
        if token.starts_with('@') {
            break;
        }
        if matches!(token, "required" | "optional" | "nullable") {
            presence = Some(token);
            break;
        }
        if !declared.is_empty() {
            declared.push(' ');
        }
        declared.push_str(token);
    }
    presence?;
    if declared.is_empty() {
        return None;
    }
    Some((name.to_owned(), declared))
}

/// Every IDL field whose name matches [`PROSE_LEXICON`], in declaration order.
fn prose_fields() -> Vec<ProseField> {
    idl_fields()
        .into_iter()
        .filter(|field| lexical_prose(&field.field))
        .collect()
}

/// The wire keys that a walk must treat as prose positions.
///
/// Only [`Carriage::Free`] fields qualify. An enum-typed `reason` is a token an agent
/// compares, not a sentence it reads, and an `Opaque` `summary` is a schema-governed document
/// whose own leaves the walk descends into under their own names.
fn prose_keys() -> BTreeSet<String> {
    prose_fields()
        .into_iter()
        .filter(|field| field.carriage() == Carriage::Free)
        .map(|field| field.field)
        .collect()
}

/// The adjudication of every free-text field: what fact it carries, and what else carries it.
///
/// Closed and exhaustive by assertion. A free-text field added to the IDL and not added here
/// fails [`every_free_text_field_is_adjudicated`], so prose cannot enter the protocol without
/// someone deciding whether a workflow depends on it.
const PROSE_ADJUDICATION: &[(&str, Load)] = &[
    // The refusal channel. `code` is the branch and it is an enum; F1 and F2 above record how
    // coarse that branch is, but `detail` never carries a fact the code does not name.
    ("Error.detail", Load::TypedSibling("Error.code")),
    (
        "Error.non_resumable_reason",
        Load::TypedSibling("Error.continuation, absent exactly when this is present"),
    ),
    (
        "TaskRecord.non_resumable_reason",
        Load::TypedSibling("TaskRecord.continuation and TaskRecord.failed_reason"),
    ),
    (
        "ServerReject.detail",
        Load::TypedSibling("ServerReject.code"),
    ),
    // Diagnostics and warnings: `code` is the machine half, `detail` the reading half.
    ("Warning.detail", Load::TypedSibling("Warning.code")),
    (
        "Diagnostic.detail",
        Load::TypedSibling("Diagnostic.code and Diagnostic.severity"),
    ),
    // Assurance dimensions. `ProducedDimension.engine` names the producer; the summary is the
    // sentence beside it. `UnsupportedDimension.reason` is documented as a *token*, and the
    // daemon emits one, but the IDL declares no domain — recorded as accompanying rather than
    // load-bearing because no operation's input is derived from it.
    (
        "ProducedDimension.summary",
        Load::TypedSibling("ProducedDimension.engine"),
    ),
    ("UnsupportedDimension.reason", Load::Accompanies),
    // The certificate rejection payload relays a kernel's own vocabulary verbatim, by design
    // (IDL 1719-1735): re-spelling it in an enum would make the daemon an authority on a
    // verdict it did not reach. An agent's next action is fixed by `Error.code` alone.
    (
        "CertificateRejection.checker",
        Load::TypedSibling("Error.code = CertificateRejected"),
    ),
    (
        "CertificateRejection.reason",
        Load::TypedSibling("Error.code = CertificateRejected"),
    ),
    // The typed recovery channel's own prose slot. Empty everywhere, with the rest of it.
    (
        "NextOperation.rationale",
        Load::TypedSibling("NextOperation.operation and NextOperation.arguments"),
    ),
    // Caller-authored. The daemon stores or forwards these; nothing reads them back.
    ("IntentChangeSet.rationale", Load::CallerAuthored),
    ("intent.reject.request.reason", Load::CallerAuthored),
    ("repair.reject.request.reason", Load::CallerAuthored),
    // `query.explain_reuse` is one of the 45 operations this build does not serve; its
    // `reasons` map is a `map<String, String>` with no declared domain on either side. It is
    // adjudicated as accompanying *at this build's scope* and the absence is stated: nothing
    // here can drive it, so nothing here can show a workflow depending on it.
    ("query.explain_reuse.response.reasons", Load::Accompanies),
];

/// The adjudication of one site, or [`None`] when the table does not name it.
fn adjudication(site: &str) -> Option<Load> {
    PROSE_ADJUDICATION
        .iter()
        .find(|(named, _)| *named == site)
        .map(|(_, load)| *load)
}

#[test]
fn every_free_text_field_is_adjudicated() {
    let unnamed: Vec<String> = prose_fields()
        .into_iter()
        .filter(|field| field.carriage() == Carriage::Free)
        .filter(|field| adjudication(&field.site).is_none())
        .map(|field| format!("{}:{} `{}`", field.line, field.site, field.declared))
        .collect();
    assert!(
        unnamed.is_empty(),
        "the IDL declares free-text fields this file has not adjudicated: {unnamed:?}"
    );
}

#[test]
fn no_free_text_field_is_the_sole_carrier_of_a_machine_needed_fact() {
    let sole: Vec<&str> = PROSE_ADJUDICATION
        .iter()
        .filter(|(_, load)| matches!(load, Load::SoleCarrier))
        .map(|(site, _)| *site)
        .collect();
    assert!(
        sole.is_empty(),
        "prose is the only carrier of a machine-needed fact at {sole:?}; G2-02 fails"
    );
}

#[test]
fn the_human_text_named_fields_split_three_ways_and_this_is_the_split() {
    let fields = prose_fields();
    let counted = |carriage: Carriage| {
        fields
            .iter()
            .filter(|field| field.carriage() == carriage)
            .count()
    };
    assert_eq!(fields.len(), 23, "human-text-named fields in the IDL");
    assert_eq!(
        counted(Carriage::Typed),
        5,
        "already a closed enum: RedactionReason, OmissionReason, two InconclusiveReasons, \
         and TaskRecord.failed_reason, which is an ErrorCode"
    );
    assert_eq!(
        counted(Carriage::Document),
        4,
        "schema-governed Opaque bodies: three `summary`s and whiteboard.compile's `note`"
    );
    assert_eq!(
        counted(Carriage::Free),
        14,
        "the prose surface proper, every one of which PROSE_ADJUDICATION names"
    );
    assert_eq!(
        counted(Carriage::Typed) + counted(Carriage::Document) + counted(Carriage::Free),
        fields.len(),
        "the three carriages partition the sweep"
    );
}

#[test]
fn the_lexicon_is_wider_than_the_protocol() {
    let used: BTreeSet<String> = prose_fields()
        .into_iter()
        .map(|field| field.field)
        .collect();
    let unused: Vec<&str> = PROSE_LEXICON
        .iter()
        .filter(|stem| !used.iter().any(|field| field.ends_with(*stem)))
        .copied()
        .collect();
    assert!(
        unused.len() >= 6,
        "the lexicon must reach beyond the fields that exist, or it is a description rather \
         than a test; unreached stems were {unused:?}"
    );
    for absent in [
        "message",
        "description",
        "doc",
        "hint",
        "advice",
        "remediation",
    ] {
        assert!(
            unused.contains(&absent),
            "`{absent}` names no IDL field today; if it does now, adjudicate it"
        );
    }
}

#[test]
fn the_prose_table_is_not_a_wildcard() {
    for real in [
        "Error.code",
        "Warning.code",
        "TaskRecord.status",
        "ArtifactRef.handle",
        "workspace.create.response.snapshot",
    ] {
        assert!(
            adjudication(real).is_none(),
            "{real} is a typed field and the prose table must not claim it"
        );
    }
    assert!(adjudication("Error.detail").is_some(), "and it does answer");
}

// =====================================================================================
// 2. The instrument: a generic walk of the answer document
// =====================================================================================

/// One leaf of a wire document, with the path that reaches it and the type the IDL
/// declares for it.
///
/// The declared type is what makes this instrument exact rather than a spelling heuristic.
/// A `TaskHandle`, a `Commitment` over a task artifact, and the `ResultStatus` member
/// `task_started` are all the string `task_…` on the wire and are three different things; only
/// the shipped spec separates them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Leaf {
    /// Dotted path from the document root, with `[n]` for array positions.
    path: String,
    /// The nearest enclosing object key.
    key: String,
    /// The IDL type spelling, or `?` where the spec table does not reach.
    declared: String,
    /// The scalar, rendered exactly as canonical JSON spells it.
    text: String,
}

/// The shipped description of a named struct.
fn named_struct(name: &str) -> Option<&'static StructSpec> {
    registry::NAMED_STRUCTS
        .iter()
        .find(|spec| spec.name == name)
}

/// The shipped description of a union.
fn named_union(name: &str) -> Option<&'static UnionSpec> {
    registry::UNIONS.iter().find(|spec| spec.name == name)
}

/// The class prefix of a declared handle type.
fn handle_prefix(class: &str) -> Option<&'static str> {
    registry::HANDLES
        .iter()
        .find(|spec| spec.name == class)
        .map(|spec| spec.prefix)
}

/// The element type of a `list<..>` or the value type of a `map<..,..>`.
fn element(declared: &str) -> &str {
    if let Some(inner) = declared
        .strip_prefix("list<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        inner.trim()
    } else if let Some(inner) = declared
        .strip_prefix("map<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        inner
            .split_once(',')
            .map_or(inner, |(_, value)| value)
            .trim()
    } else {
        declared
    }
}

/// Every scalar leaf of an answer, each carrying the type the IDL declares for it.
///
/// The walk is driven by `continuumd::protocol::registry`'s own spec table, not by a list of
/// fields written here: it starts at `ResultEnvelope`, resolves each member's declared type
/// from the struct's `FieldSpec`, follows named structs and externally tagged unions, and
/// descends into the `payload` under the *operation's* declared response shape. `Opaque`
/// splices its body inline rather than base64-ing it, so the walk reaches inside a
/// schema-governed document too.
fn leaves(document: &Wire, response: Option<&'static StructSpec>) -> Vec<Leaf> {
    let mut found = Vec::new();
    walk(
        document,
        "ResultEnvelope",
        None,
        String::new(),
        String::new(),
        response,
        &mut found,
    );
    found.sort();
    found
}

/// The recursive half of [`leaves`].
fn walk(
    value: &Wire,
    declared: &str,
    over: Option<&'static StructSpec>,
    path: String,
    key: String,
    response: Option<&'static StructSpec>,
    out: &mut Vec<Leaf>,
) {
    let join = |name: &str| {
        if path.is_empty() {
            name.to_owned()
        } else {
            format!("{path}.{name}")
        }
    };
    match value {
        Wire::Object(members) => {
            if let Some(spec) = over.or_else(|| named_struct(declared)) {
                for (name, member) in members {
                    let field = spec.fields.iter().find(|field| field.name == *name);
                    let ty = field.map_or("?", |field| field.ty);
                    // The one `Opaque` whose shape the request already fixed.
                    if spec.name == "ResultEnvelope" && name == "payload" {
                        walk(
                            member,
                            "<payload>",
                            response,
                            join(name),
                            name.clone(),
                            response,
                            out,
                        );
                    } else {
                        walk(member, ty, None, join(name), name.clone(), response, out);
                    }
                }
            } else if let Some(union) = named_union(declared) {
                for (name, member) in members {
                    let ty = union
                        .variants
                        .iter()
                        .find(|variant| variant.ident == *name)
                        .map_or("?", |variant| variant.ty);
                    walk(member, ty, None, join(name), name.clone(), response, out);
                }
            } else {
                for (name, member) in members {
                    walk(member, "?", None, join(name), name.clone(), response, out);
                }
            }
        }
        Wire::Array(items) => {
            let inner = element(declared).to_owned();
            for (index, item) in items.iter().enumerate() {
                walk(
                    item,
                    &inner,
                    None,
                    format!("{path}[{index}]"),
                    key.clone(),
                    response,
                    out,
                );
            }
        }
        scalar => out.push(Leaf {
            path,
            key,
            declared: declared.to_owned(),
            text: match scalar {
                Wire::String(text) => text.clone(),
                Wire::Integer(number) => number.to_string(),
                Wire::Bool(flag) => flag.to_string(),
                _ => "null".to_owned(),
            },
        }),
    }
}

/// Whether a leaf sits in a prose position: a free-text type under a human-text name.
///
/// Both halves are required, and that is the adjudication [`PROSE_ADJUDICATION`] makes
/// concrete. `Omission.reason` is named like prose and typed as an enum, so an agent reading
/// it is comparing a token. `Milestone.name` is a bare `String` and is not named like prose;
/// it is an identifier, and the census would not call it a sentence.
fn prose_position(leaf: &Leaf) -> bool {
    lexical_prose(&leaf.key)
        && matches!(
            leaf.declared.as_str(),
            "String" | "list<String>" | "map<String, String>"
        )
}

/// Where in an answer a value of a given handle class can be found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Recovered {
    /// Distinct values sitting at a position the IDL declares to be that handle class.
    typed: BTreeMap<String, Vec<String>>,
    /// Distinct values a reader could only extract by parsing a prose field.
    prose: BTreeMap<String, Vec<String>>,
}

impl Recovered {
    /// The one value at a typed position, or a panic naming what was found instead.
    ///
    /// The panic is the point. A workflow driven through this function stops dead when the
    /// handle it needs is not typed, which is the criterion stated as control flow.
    fn sole(&self, what: &str) -> String {
        assert_eq!(
            self.typed.len(),
            1,
            "expected exactly one typed {what} in the answer; typed={:?} prose={:?}",
            self.typed,
            self.prose
        );
        self.typed
            .keys()
            .next()
            .cloned()
            .expect("a one-element map has a key")
    }
}

/// Find every value of one handle class in an answer, split by whether prose carries it.
///
/// A *typed* hit is a leaf whose declared type is exactly that class. A *prose* hit is a value
/// spelled inside a free-text field, extracted the way an agent scraping a message would have
/// to: find the class prefix, take the maximal run of handle-legal characters after it. The
/// split is the whole measurement.
fn recover(answer: &Answer, class: &str) -> Recovered {
    let prefix = handle_prefix(class)
        .unwrap_or_else(|| panic!("{class} is not a handle class the registry declares"));
    let mut found = Recovered::default();
    for leaf in answer.leaves() {
        if prose_position(&leaf) {
            for candidate in scrape(&leaf.text, prefix) {
                found
                    .prose
                    .entry(candidate)
                    .or_default()
                    .push(leaf.path.clone());
            }
        } else if leaf.declared == class {
            found
                .typed
                .entry(leaf.text.clone())
                .or_default()
                .push(leaf.path.clone());
        }
    }
    found
}

/// Pull every handle-shaped token beginning with `prefix` out of a sentence.
///
/// This is the parser the criterion says an agent must never need. It exists here so the
/// census can prove it is never needed, and so
/// [`the_census_flags_a_handle_that_reaches_the_agent_only_inside_a_sentence`] has something
/// to succeed with when the typed position is taken away.
fn scrape(text: &str, prefix: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes: Vec<char> = text.chars().collect();
    let needle: Vec<char> = prefix.chars().collect();
    let mut index = 0;
    while index + needle.len() <= bytes.len() {
        if bytes[index..index + needle.len()] == needle[..] {
            let mut end = index + needle.len();
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == '_' || bytes[end] == '-')
            {
                end += 1;
            }
            if end > index + needle.len() {
                found.insert(bytes[index..end].iter().collect::<String>());
            }
            index = end;
        } else {
            index += 1;
        }
    }
    found
}

/// Read a decision fact — a status, a flag, a verdict — off a typed position.
///
/// The handle half of the census is about *names*; this is the other half, about the values a
/// workflow branches on. `sealed` gating `verification.start`, `status` gating whether to poll
/// again, `verdict` gating whether the work is done.
fn typed_decision(answer: &Answer, key: &str) -> Option<Leaf> {
    let found = answer
        .leaves()
        .into_iter()
        .find(|leaf| leaf.key == key && !prose_position(leaf))?;
    assert!(
        !prose_position(&found),
        "`{key}` was read from a prose position at {}",
        found.path
    );
    Some(found)
}

// =====================================================================================
// 3. The fixtures — two daemons, both spoken to over a real byte boundary
// =====================================================================================

/// The TV-009 port's model, verbatim.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// A production trace, as a captured bundle would arrive.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// A second trace, so a checker has something to file a receipt over.
const RECEIPT: &str = "{\"receipt\":\"kernel-core\"}\n";

/// The instrumentation profile the trace was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

/// The protocol version every connection in this file negotiates.
fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

/// A capability handle.
fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

/// An actor identity.
fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

/// An operation name.
fn opname(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

/// An epoch identity.
fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

/// The daemon's clock reading.
fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// The epochs both daemons serve.
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

/// A capability profile.
fn profile(privileged: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| opname(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: true,
    }
}

/// A capability descriptor.
fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
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
        // Root is registered at 4 and every delegation at 3, so D6/D7's "a child's admission
        // set is a subset of its parent's" holds by construction rather than by luck.
        delegation_depth: if handle == "cap_root" { 4 } else { 3 },
        profile,
    }
}

/// The negotiated connection both fixtures answer on.
fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g2-02-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// An empty budget.
fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(count) => Optional::Present(count),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A request envelope whose obligations are read off the registry rather than hand-decided.
///
/// `@mutation` needs a key, `@readonly` must not carry one, `@task_starting` needs a budget.
/// Getting that wrong would refuse a probe at step 6 for a reason with nothing to do with this
/// file's subject, so it is derived rather than typed out.
fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    let spec = registry::operation(operation);
    let mutation = spec.is_some_and(|spec| spec.has(Annotation::Mutation));
    let task_starting = spec.is_some_and(|spec| spec.has(Annotation::TaskStarting));
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: if mutation {
            Optional::Present(format!("idem-{request}"))
        } else {
            Optional::Absent
        },
        actor: who(actor),
        capability: cap(capability),
        operation: opname(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: if task_starting {
            Optional::Present(budget(Some(64)))
        } else {
            Optional::Absent
        },
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

/// Name a snapshot on an envelope.
fn on(mut envelope: RequestEnvelope, snapshot: &str) -> RequestEnvelope {
    envelope.snapshot =
        Nullable::Value(WorkspaceHandle::new(snapshot).expect("a well-formed workspace handle"));
    envelope
}

/// Set the state budget on an envelope that already declares one.
fn states(mut envelope: RequestEnvelope, count: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(count));
    envelope
}

/// One answer, exactly as an agent receives it.
#[derive(Debug, Clone)]
struct Answer {
    /// The operation that was asked, which is what names the `payload`'s declared shape.
    operation: String,
    /// The canonical document the answer bytes are.
    document: Wire,
    /// The same bytes, decoded to the declared envelope.
    envelope: ResultEnvelope,
}

impl Answer {
    /// Every leaf of this answer, typed against the shipped spec.
    fn leaves(&self) -> Vec<Leaf> {
        leaves(
            &self.document,
            registry::operation(&self.operation).map(|spec| &spec.response),
        )
    }

    /// The status token, from the typed envelope.
    fn status(&self) -> ResultStatus {
        self.envelope.status
    }

    /// The error code, when the answer is a refusal.
    fn code(&self) -> Option<ErrorCode> {
        self.envelope.error.value().map(|error| error.code)
    }

    /// The refusal's `detail`, when the answer is a refusal.
    fn detail(&self) -> Option<String> {
        self.envelope
            .error
            .value()
            .map(|error| error.detail.clone())
    }
}

/// Write a request as a frame, hand it to the server, and read the answer back as a document.
///
/// Every answer this file measures goes through here. Nothing reads a Rust value the daemon
/// built in process: the census subject is bytes on a connection.
fn ask(server: &mut Server, mut envelope: RequestEnvelope, arguments: &Arguments) -> Answer {
    let operation = envelope.operation.as_str().to_owned();
    envelope.arguments = transport::encode_arguments(arguments).expect("the body encodes");
    let frame = codec::to_bytes(&envelope).expect("the envelope encodes");
    let bytes = server.answer(&frame).expect("a result is encodable");
    Answer {
        operation,
        document: Wire::parse(&bytes).expect("the answer is a canonical document"),
        envelope: codec::from_bytes::<ResultEnvelope>(&bytes).expect("the answer decodes"),
    }
}

/// The verification lane: workspace, intent, verification and task, over one connection.
struct Lane {
    /// The connection.
    server: Server,
    /// The intent handle the lane's snapshots bind, which arrives out of band.
    intent: IntentHandle,
    /// The staged components a `workspace.create` names.
    components: SnapshotComponents,
}

/// A `propose` capability scoped to the snapshot and intent classes, spelled as class
/// tokens.
fn tokened() -> CapabilityDescriptor {
    let mut token = grant(
        "cap_tokened",
        "agent:tokened",
        AuthorityLevel::Propose,
        Optional::Absent,
    );
    token.artifact_classes = vec!["ws".to_owned(), "in".to_owned()];
    token
}

/// The same descriptor under another handle, scoped with plan §4.4's literal prefix
/// spelling. Finding F3's instrument.
fn prefixed() -> CapabilityDescriptor {
    let mut literal = grant(
        "cap_prefixed",
        "agent:prefixed",
        AuthorityLevel::Propose,
        Optional::Absent,
    );
    literal.artifact_classes = vec!["ws_".to_owned(), "in_".to_owned()];
    literal
}

/// Build the verification lane.
fn lane() -> Lane {
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                )),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                )),
            ),
            root.clone(),
        )
        // Scoped with class tokens, the one spelling `rule artifact_class.spelling` admits.
        // The plan's literal prefix spelling, `ws_`, cannot be provisioned at all: finding
        // F3 is now a typed refusal at `try_build`, driven in
        // `the_artifact_class_spelling_is_a_typed_refusal_at_provisioning`.
        .capability(tokened(), root)
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build();

    let contract = IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes())
        .expect("the fixture decodes");
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle");
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );

    let components = SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![configuration],
        file_components: Optional::Absent,
    };

    Lane {
        server: Server::new(daemon, negotiated()),
        intent,
        components,
    }
}

/// The evidence graph: observe and evidence, over one connection.
struct Graph {
    /// The connection.
    server: Server,
    /// A staged production trace, which arrives out of band.
    trace: Commitment,
    /// A staged receipt body, which arrives out of band.
    receipt: Commitment,
}

/// Build the evidence graph.
fn graph() -> Graph {
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root,
        )
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .build();

    let mut stage = |path: &str, content: &str| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content")
    };
    let trace = stage("traces/die-hard.jsonl", TRACE);
    let receipt = stage("evidence/receipt.json", RECEIPT);

    Graph {
        server: Server::new(daemon, negotiated()),
        trace,
        receipt,
    }
}

/// The acceptance record `intent.accept` carries.
fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, ContractJson> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), ContractJson::String(value.to_owned()));
    }
    Opaque::from_bytes(ContractJson::Object(fields).to_canonical_bytes())
}

// =====================================================================================
// 4. Forward closure — the workflows, driven from the wire
// =====================================================================================

/// One inter-step fact: where the next step's input came from in the previous answer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Closure {
    /// The workflow this step belongs to.
    workflow: &'static str,
    /// The operation that answered.
    from: &'static str,
    /// The operation that consumed the fact.
    to: &'static str,
    /// What the fact is.
    fact: &'static str,
    /// The path in the answering document that carried it.
    path: String,
}

/// Record one recovered handle as a closure, asserting it was typed rather than scraped.
fn closed(
    census: &mut Vec<Closure>,
    workflow: &'static str,
    from: &'static str,
    to: &'static str,
    fact: &'static str,
    answer: &Answer,
    class: &str,
) -> String {
    let found = recover(answer, class);
    let value = found.sole(fact);
    let path = found.typed[&value]
        .first()
        .cloned()
        .expect("a typed hit names its path");
    census.push(Closure {
        workflow,
        from,
        to,
        fact,
        path,
    });
    value
}

/// Record one decision fact as a closure, asserting it sits at a typed position.
fn decided(
    census: &mut Vec<Closure>,
    workflow: &'static str,
    from: &'static str,
    to: &'static str,
    key: &'static str,
    answer: &Answer,
) -> String {
    let leaf = typed_decision(answer, key)
        .unwrap_or_else(|| panic!("{from} answered no typed `{key}` for {to} to branch on"));
    census.push(Closure {
        workflow,
        from,
        to,
        fact: key,
        path: leaf.path,
    });
    leaf.text
}

/// Every answer this file collects, kept so [`next_operations_is_empty_and_every_recovery_offer_is_executable_as_given`]
/// can measure the recovery channels over all of them at once.
fn all_answers() -> Vec<Answer> {
    let mut collected = workflow_answers();
    collected.extend(refusal_probes().into_iter().map(|probe| probe.answer));
    collected
}

/// Drive every workflow, returning the census and every answer produced on the way.
#[expect(
    clippy::too_many_lines,
    reason = "eight workflows in one function is the point: they share one connection each, \
              and splitting them would hide which facts cross which step"
)]
fn drive_workflows() -> (Vec<Closure>, Vec<Answer>) {
    let mut census = Vec::new();
    let mut answers = Vec::new();

    // --- W1 verify: accept an intent, publish a sealed snapshot, run a campaign, read the
    // verdict. The whole Phase A lane, and every handle in it comes off the wire.
    let mut fixture = lane();
    let accepted = ask(
        &mut fixture.server,
        envelope(
            "intent.accept",
            "human:steward",
            "cap_steward",
            "req_w1_accept",
        ),
        &Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    );
    assert_eq!(
        accepted.status(),
        ResultStatus::Ok,
        "{:?}",
        accepted.detail()
    );
    let intent = closed(
        &mut census,
        "W1 verify",
        "intent.accept",
        "workspace.create",
        "intent handle",
        &accepted,
        "IntentHandle",
    );
    answers.push(accepted);

    let mut components = fixture.components.clone();
    components.intent = IntentHandle::new(&intent).expect("a well-formed intent handle");
    let created = ask(
        &mut fixture.server,
        envelope(
            "workspace.create",
            "agent:builder",
            "cap_builder",
            "req_w1_create",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(created.status(), ResultStatus::Ok, "{:?}", created.detail());
    let snapshot = closed(
        &mut census,
        "W1 verify",
        "workspace.create",
        "verification.start",
        "snapshot handle",
        &created,
        "WorkspaceHandle",
    );
    let sealed = decided(
        &mut census,
        "W1 verify",
        "workspace.create",
        "verification.start",
        "sealed",
        &created,
    );
    assert_eq!(
        sealed, "true",
        "a campaign is legal only over a sealed snapshot"
    );
    answers.push(created);

    let started = ask(
        &mut fixture.server,
        on(
            envelope(
                "verification.start",
                "agent:runner",
                "cap_runner",
                "req_w1_start",
            ),
            &snapshot,
        ),
        &Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    assert_ne!(
        started.status(),
        ResultStatus::Error,
        "{:?}",
        started.detail()
    );
    let task = closed(
        &mut census,
        "W1 verify",
        "verification.start",
        "task.status",
        "task handle",
        &started,
        "TaskHandle",
    );
    answers.push(started);

    let polled = ask(
        &mut fixture.server,
        envelope("task.status", "agent:reader", "cap_reader", "req_w1_status"),
        &Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new(&task).expect("a well-formed task handle"),
        }),
    );
    assert_eq!(polled.status(), ResultStatus::Ok, "{:?}", polled.detail());
    let status = decided(
        &mut census,
        "W1 verify",
        "task.status",
        "verification.result",
        "status",
        &polled,
    );
    assert_eq!(status, "completed", "a 64-state budget finishes Die Hard");
    let for_result = closed(
        &mut census,
        "W1 verify",
        "task.status",
        "verification.result",
        "task handle",
        &polled,
        "TaskHandle",
    );
    answers.push(polled);

    let result = ask(
        &mut fixture.server,
        envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_w1_result",
        ),
        &Arguments::VerificationResult(VerificationResultRequest {
            task: TaskHandle::new(&for_result).expect("a well-formed task handle"),
        }),
    );
    assert_eq!(result.status(), ResultStatus::Ok, "{:?}", result.detail());
    let verdict = decided(
        &mut census,
        "W1 verify",
        "verification.result",
        "(terminal)",
        "verdict",
        &result,
    );
    assert!(
        !verdict.is_empty(),
        "the campaign's outcome is a typed token, not a sentence"
    );
    answers.push(result);

    // --- W2 resume: park a campaign under a budget that cannot hold it, then resume from the
    // continuation the status answered.
    let mut fixture = lane();
    prime(&mut fixture);
    let snapshot = sealed_snapshot(&mut fixture, "req_w2_create");
    let parked = ask(
        &mut fixture.server,
        on(
            states(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_w2_start",
                ),
                Some(4),
            ),
            &snapshot,
        ),
        &Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    let parked_task = closed(
        &mut census,
        "W2 resume",
        "verification.start",
        "task.status",
        "task handle",
        &parked,
        "TaskHandle",
    );
    answers.push(parked);

    let suspended = ask(
        &mut fixture.server,
        envelope("task.status", "agent:reader", "cap_reader", "req_w2_status"),
        &Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new(&parked_task).expect("a well-formed task handle"),
        }),
    );
    let suspended_status = decided(
        &mut census,
        "W2 resume",
        "task.status",
        "task.resume",
        "status",
        &suspended,
    );
    assert_eq!(suspended_status, "suspended", "a four-state budget parks");
    let continuation = closed(
        &mut census,
        "W2 resume",
        "task.status",
        "task.resume",
        "continuation handle",
        &suspended,
        "ContinuationHandle",
    );
    answers.push(suspended);

    let resumed = ask(
        &mut fixture.server,
        on(
            states(
                envelope("task.resume", "agent:runner", "cap_runner", "req_w2_resume"),
                Some(64),
            ),
            &snapshot,
        ),
        &Arguments::TaskResume(TaskResumeRequest {
            continuation: ContinuationHandle::new(&continuation)
                .expect("a well-formed continuation handle"),
            budget: Optional::Present(budget(Some(64))),
        }),
    );
    assert_ne!(
        resumed.status(),
        ResultStatus::Error,
        "{:?}",
        resumed.detail()
    );
    let _ = closed(
        &mut census,
        "W2 resume",
        "task.resume",
        "(terminal)",
        "task handle",
        &resumed,
        "TaskHandle",
    );
    answers.push(resumed);

    // --- W3 cancel: start, cancel, confirm the cancellation through a second read.
    let mut fixture = lane();
    prime(&mut fixture);
    let snapshot = sealed_snapshot(&mut fixture, "req_w3_create");
    let started = ask(
        &mut fixture.server,
        on(
            states(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_w3_start",
                ),
                Some(4),
            ),
            &snapshot,
        ),
        &Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    let task = closed(
        &mut census,
        "W3 cancel",
        "verification.start",
        "task.cancel",
        "task handle",
        &started,
        "TaskHandle",
    );
    answers.push(started);

    let cancelled = ask(
        &mut fixture.server,
        envelope("task.cancel", "agent:runner", "cap_runner", "req_w3_cancel"),
        &Arguments::TaskCancel(TaskCancelRequest {
            task: TaskHandle::new(&task).expect("a well-formed task handle"),
        }),
    );
    assert_eq!(
        cancelled.status(),
        ResultStatus::Ok,
        "{:?}",
        cancelled.detail()
    );
    let cancelled_task = closed(
        &mut census,
        "W3 cancel",
        "task.cancel",
        "task.status",
        "task handle",
        &cancelled,
        "TaskHandle",
    );
    answers.push(cancelled);

    let confirmed = ask(
        &mut fixture.server,
        envelope(
            "task.status",
            "agent:reader",
            "cap_reader",
            "req_w3_confirm",
        ),
        &Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new(&cancelled_task).expect("a well-formed task handle"),
        }),
    );
    let final_status = decided(
        &mut census,
        "W3 cancel",
        "task.status",
        "(terminal)",
        "status",
        &confirmed,
    );
    assert_eq!(final_status, "cancelled", "the obligation closed, typed");
    answers.push(confirmed);

    // --- W4 budget: re-admit a parked task through `task.update_budget` rather than resume.
    let mut fixture = lane();
    prime(&mut fixture);
    let snapshot = sealed_snapshot(&mut fixture, "req_w4_create");
    let parked = ask(
        &mut fixture.server,
        on(
            states(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_w4_start",
                ),
                Some(4),
            ),
            &snapshot,
        ),
        &Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    let task = closed(
        &mut census,
        "W4 budget",
        "verification.start",
        "task.update_budget",
        "task handle",
        &parked,
        "TaskHandle",
    );
    answers.push(parked);

    let raised = ask(
        &mut fixture.server,
        envelope(
            "task.update_budget",
            "agent:runner",
            "cap_runner",
            "req_w4_budget",
        ),
        &Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: TaskHandle::new(&task).expect("a well-formed task handle"),
            budget: budget(Some(64)),
        }),
    );
    assert_eq!(raised.status(), ResultStatus::Ok, "{:?}", raised.detail());
    let _ = decided(
        &mut census,
        "W4 budget",
        "task.update_budget",
        "(terminal)",
        "status",
        &raised,
    );
    answers.push(raised);

    // --- W5 publish: create a draft, fork it, seal the fork, diff the two. Publication and
    // lineage, with every snapshot named by a handle the previous answer minted.
    let mut fixture = lane();
    let accepted = ask(
        &mut fixture.server,
        envelope(
            "intent.accept",
            "human:steward",
            "cap_steward",
            "req_w5_accept",
        ),
        &Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    );
    assert_eq!(
        accepted.status(),
        ResultStatus::Ok,
        "{:?}",
        accepted.detail()
    );
    answers.push(accepted);

    let draft = ask(
        &mut fixture.server,
        envelope(
            "workspace.create",
            "agent:builder",
            "cap_builder",
            "req_w5_create",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: fixture.components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(false),
        }),
    );
    assert_eq!(draft.status(), ResultStatus::Ok, "{:?}", draft.detail());
    let base = closed(
        &mut census,
        "W5 publish",
        "workspace.create",
        "workspace.fork",
        "snapshot handle",
        &draft,
        "WorkspaceHandle",
    );
    let unsealed = decided(
        &mut census,
        "W5 publish",
        "workspace.create",
        "workspace.seal",
        "sealed",
        &draft,
    );
    assert_eq!(
        unsealed, "false",
        "the draft is not sealed, and says so typed"
    );
    answers.push(draft);

    let forked = ask(
        &mut fixture.server,
        envelope(
            "workspace.fork",
            "agent:builder",
            "cap_builder",
            "req_w5_fork",
        ),
        &Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: WorkspaceHandle::new(&base).expect("a well-formed workspace handle"),
            overlay: Optional::Absent,
            patches: Optional::Absent,
        }),
    );
    assert_eq!(forked.status(), ResultStatus::Ok, "{:?}", forked.detail());
    let child = closed(
        &mut census,
        "W5 publish",
        "workspace.fork",
        "workspace.seal",
        "snapshot handle",
        &forked,
        "WorkspaceHandle",
    );
    answers.push(forked);

    let put = ask(
        &mut fixture.server,
        envelope(
            "workspace.seal",
            "agent:builder",
            "cap_builder",
            "req_w5_seal",
        ),
        &Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: WorkspaceHandle::new(&child).expect("a well-formed workspace handle"),
        }),
    );
    assert_eq!(put.status(), ResultStatus::Ok, "{:?}", put.detail());
    let sealed_child = closed(
        &mut census,
        "W5 publish",
        "workspace.seal",
        "workspace.diff",
        "snapshot handle",
        &put,
        "WorkspaceHandle",
    );
    answers.push(put);

    let differed = ask(
        &mut fixture.server,
        envelope(
            "workspace.diff",
            "agent:builder",
            "cap_builder",
            "req_w5_diff",
        ),
        &Arguments::WorkspaceDiff(WorkspaceDiffRequest {
            before: WorkspaceHandle::new(&base).expect("a well-formed workspace handle"),
            after: WorkspaceHandle::new(&sealed_child).expect("a well-formed workspace handle"),
            layers: vec![DiffLayer::Structural],
        }),
    );
    // `workspace.diff` is one of the operations whose semantic depth this deployment does not
    // serve. That is a typed refusal, and the census records the refusal path rather than
    // pretending the step closed.
    if differed.status() == ResultStatus::Ok {
        let _ = closed(
            &mut census,
            "W5 publish",
            "workspace.diff",
            "(terminal)",
            "diff handle",
            &differed,
            "DiffHandle",
        );
    } else {
        let code = differed
            .code()
            .expect("a refusal carries a typed code, not a message");
        assert_eq!(
            code,
            ErrorCode::UnsupportedSemanticFeature,
            "an unserved diff depth refuses with a code naming the lane, not a sentence"
        );
    }
    answers.push(differed);

    // --- W6 intent: read the governing contract back through the protocol.
    let mut fixture = lane();
    let fetched = ask(
        &mut fixture.server,
        envelope("intent.get", "agent:reader", "cap_reader", "req_w6_get"),
        &Arguments::IntentGet(IntentGetRequest {
            intent: fixture.intent.clone(),
        }),
    );
    assert_eq!(fetched.status(), ResultStatus::Ok, "{:?}", fetched.detail());
    let _ = closed(
        &mut census,
        "W6 intent",
        "intent.get",
        "workspace.create",
        "intent handle",
        &fetched,
        "IntentHandle",
    );
    answers.push(fetched);

    // --- W7 evidence: ingest a trace, check it, publish a check edge, read the edge back.
    let mut evidence = graph();
    let ingested = ask(
        &mut evidence.server,
        envelope(
            "observe.ingest",
            "agent:observer",
            "cap_observer",
            "req_w7_ingest",
        ),
        &Arguments::ObserveIngest(ObserveIngestRequest {
            trace: evidence.trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    );
    assert_eq!(
        ingested.status(),
        ResultStatus::Ok,
        "{:?}",
        ingested.detail()
    );
    let node = closed(
        &mut census,
        "W7 evidence",
        "observe.ingest",
        "evidence.verify",
        "evidence handle",
        &ingested,
        "EvidenceHandle",
    );
    answers.push(ingested);

    let verified = ask(
        &mut evidence.server,
        envelope(
            "evidence.verify",
            "agent:reader",
            "cap_reader",
            "req_w7_verify",
        ),
        &Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: EvidenceHandle::new(&node).expect("a well-formed evidence handle"),
            expected_status: Optional::Absent,
        }),
    );
    assert_eq!(
        verified.status(),
        ResultStatus::Ok,
        "{:?}",
        verified.detail()
    );
    let checked = closed(
        &mut census,
        "W7 evidence",
        "evidence.verify",
        "evidence.link",
        "evidence handle",
        &verified,
        "EvidenceHandle",
    );
    let claim_status = decided(
        &mut census,
        "W7 evidence",
        "evidence.verify",
        "evidence.link",
        "status",
        &verified,
    );
    assert_eq!(claim_status, "observed", "the lattice position is a token");
    answers.push(verified);

    let linked = ask(
        &mut evidence.server,
        envelope(
            "evidence.link",
            "service:kernel-core",
            "cap_checker",
            "req_w7_link",
        ),
        &Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: EvidenceHandle::new(&checked).expect("a well-formed evidence handle"),
            receipt: evidence.receipt.clone(),
            checker_profile: "kernel-core/1".to_owned(),
        }),
    );
    assert_eq!(linked.status(), ResultStatus::Ok, "{:?}", linked.detail());
    // A link mints two handles at once — the edge and the receipt — so the sole-value rule
    // does not apply and the census records which typed position each came from. Both are
    // `EvidenceHandle`, neither is the subject the request already named, and neither is
    // scrapeable from a sentence.
    let minted = recover(&linked, "EvidenceHandle");
    assert!(
        minted.prose.is_empty(),
        "a link's handles must not be scrapeable from prose: {:?}",
        minted.prose
    );
    let mut edge_handle = String::new();
    for (value, paths) in &minted.typed {
        assert_ne!(
            value.as_str(),
            checked.as_str(),
            "the subject is an argument, not something the answer mints"
        );
        let path = paths.first().cloned().expect("a typed hit names its path");
        census.push(Closure {
            workflow: "W7 evidence",
            from: "evidence.link",
            to: "evidence.get",
            fact: "minted evidence handle",
            path: path.clone(),
        });
        if path == "payload.edge" {
            edge_handle = value.clone();
        }
    }
    assert!(
        !edge_handle.is_empty(),
        "the link named no edge at a typed position: {:?}",
        minted.typed
    );
    answers.push(linked);

    let got = ask(
        &mut evidence.server,
        envelope("evidence.get", "agent:reader", "cap_reader", "req_w7_get"),
        &Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: EvidenceHandle::new(&edge_handle).expect("a well-formed evidence handle"),
            inline: Optional::Absent,
        }),
    );
    assert_eq!(got.status(), ResultStatus::Ok, "{:?}", got.detail());
    answers.push(got);

    // --- W8 query: find evidence by a typed filter, then dereference what it named.
    let queried = ask(
        &mut evidence.server,
        envelope(
            "evidence.query",
            "agent:reader",
            "cap_reader",
            "req_w8_query",
        ),
        &Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Absent,
                claim_id: Optional::Absent,
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
    );
    assert_eq!(queried.status(), ResultStatus::Ok, "{:?}", queried.detail());
    let found = recover(&queried, "EvidenceHandle");
    assert!(
        found.prose.is_empty(),
        "a query's frontier must not be scrapeable from prose: {:?}",
        found.prose
    );
    let first = found
        .typed
        .keys()
        .next()
        .cloned()
        .expect("the query names at least the ingested node");
    census.push(Closure {
        workflow: "W8 query",
        from: "evidence.query",
        to: "evidence.get",
        fact: "evidence handle",
        path: found.typed[&first]
            .first()
            .cloned()
            .expect("a typed hit names its path"),
    });
    answers.push(queried);

    let dereferenced = ask(
        &mut evidence.server,
        envelope("evidence.get", "agent:reader", "cap_reader", "req_w8_get"),
        &Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: EvidenceHandle::new(&first).expect("a well-formed evidence handle"),
            inline: Optional::Absent,
        }),
    );
    assert_eq!(
        dereferenced.status(),
        ResultStatus::Ok,
        "{:?}",
        dereferenced.detail()
    );
    answers.push(dereferenced);

    (census, answers)
}

/// Accept the lane's intent, so a snapshot can bind it.
fn prime(fixture: &mut Lane) {
    let accepted = ask(
        &mut fixture.server,
        envelope("intent.accept", "human:steward", "cap_steward", "req_prime"),
        &Arguments::IntentAccept(IntentAcceptRequest {
            proposal: fixture.intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    );
    assert_eq!(
        accepted.status(),
        ResultStatus::Ok,
        "{:?}",
        accepted.detail()
    );
}

/// Publish a sealed snapshot on the lane and return its handle, read off the wire.
fn sealed_snapshot(fixture: &mut Lane, request: &str) -> String {
    let created = ask(
        &mut fixture.server,
        envelope("workspace.create", "agent:builder", "cap_builder", request),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: fixture.components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(created.status(), ResultStatus::Ok, "{:?}", created.detail());
    recover(&created, "WorkspaceHandle").sole("snapshot handle")
}

/// Every answer the workflows produce.
fn workflow_answers() -> Vec<Answer> {
    drive_workflows().1
}

#[test]
fn workflow_closure_is_total_over_every_workflow_this_build_can_drive() {
    let (census, _) = drive_workflows();
    let workflows: BTreeSet<&str> = census.iter().map(|step| step.workflow).collect();
    assert_eq!(
        workflows.len(),
        8,
        "eight workflows are driven end to end: {workflows:?}"
    );
    assert_eq!(
        census.len(),
        27,
        "twenty-seven facts cross a step boundary; if this moved, the header's census moved \
         with it and both must be re-derived (`cargo test -- report --ignored --nocapture`)"
    );
    // Every recorded closure is typed by construction — `closed` and `decided` panic otherwise
    // — so the assertion that carries the criterion is that the census is *non-empty for every
    // workflow*, and that no workflow silently dropped its steps.
    for workflow in &workflows {
        let steps = census
            .iter()
            .filter(|step| step.workflow == *workflow)
            .count();
        assert!(
            steps >= 1,
            "{workflow} recorded no inter-step fact, so it evidences nothing"
        );
    }
    // And that no recorded path lies inside a prose field. `closed` already refuses one, but
    // the census is re-checked here against the IDL's own prose keys so the two halves of the
    // file have to agree.
    let prose = prose_keys();
    for step in &census {
        let terminal = step
            .path
            .rsplit('.')
            .next()
            .unwrap_or(&step.path)
            .split('[')
            .next()
            .unwrap_or(&step.path);
        assert!(
            !prose.contains(terminal),
            "{} → {} took `{}` from the prose field `{}`; G2-02 fails",
            step.from,
            step.to,
            step.fact,
            step.path
        );
    }
}

/// The facts that enter a workflow from outside the protocol.
///
/// Not prose, and not a failure of this criterion — absences, stated because INV-007 says an
/// omission is named rather than dropped.
const OUT_OF_BAND: &[(&str, &str)] = &[
    (
        "content commitments",
        "`DaemonState::stage` publishes bytes and mints a `Commitment`; no operation in the \
         registry does, so a trace or a file enters through the deployment, not the wire",
    ),
    (
        "the first intent handle",
        "`intent.get` and `intent.accept` both take an `IntentHandle` they do not mint; the \
         governing contract's identity reaches an agent from its deployment",
    ),
    (
        "the model catalog",
        "`ModelCatalog::register` binds a module set to an elaborated model out of band; this \
         daemon compiles no CML source, so `verification.start` can only verify what a \
         deployment registered",
    ),
];

#[test]
fn the_facts_that_enter_from_outside_the_protocol_are_exactly_these() {
    assert_eq!(OUT_OF_BAND.len(), 3, "three, and each is named");
    // The claim is checkable in one direction: no shipped operation names a `Commitment` in
    // its *response* that it minted from nothing. Every commitment an answer carries is one
    // the request already named, or one derived from content the daemon already held.
    let minting: Vec<&str> = OPERATIONS
        .iter()
        .filter(|spec| spec.name.starts_with("workspace.") || spec.name.starts_with("observe."))
        .filter(|spec| spec.name.ends_with(".stage") || spec.name.ends_with(".put"))
        .map(|spec| spec.name)
        .collect();
    assert!(
        minting.is_empty(),
        "the registry now has a staging operation and the first absence is closed: {minting:?}"
    );
}

// =====================================================================================
// 5. Refusal determinacy
// =====================================================================================

/// The action an agent must take to make progress after a refusal.
///
/// Coarser than the daemon's `detail` on purpose: the question is what an agent *does*, and
/// two conditions that lead to the same next operation are one action however differently
/// they read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Action {
    /// Re-send the same call with an idempotency key.
    AddIdempotencyKey,
    /// Re-send the same call without one.
    DropIdempotencyKey,
    /// Re-send with a different value for a named argument of the agent's own request.
    FixOwnArgument,
    /// Seal the snapshot, then re-send.
    SealSnapshot,
    /// Re-read the lineage head and re-send against the current snapshot.
    RefetchHead,
    /// Raise the declared budget and re-send.
    RaiseBudget,
    /// Obtain a capability that admits the operation.
    Escalate,
    /// Stop naming this object and re-derive the handle from a call that mints one.
    RederiveHandle,
    /// Re-open the connection under a version both sides serve.
    Renegotiate,
    /// The deployment does not serve this; no retry of any shape succeeds.
    Unshipped,
}

/// What channel decides the action, at the strictest level that suffices.
///
/// A ladder, and only its bottom rung is a G2-02 failure. The criterion's word is "required":
/// an agent that can compute its next move from what it already knows is not parsing anything,
/// however helpful the sentence beside the code would have been.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Determinacy {
    /// The typed answer alone fixes the action: no other probe shares its code.
    AnswerAlone,
    /// The code, plus the request the agent itself sent and the static registry.
    AnswerAndOwnRequest,
    /// Neither of the above, but re-issuing with one argument changed separates the cases.
    ///
    /// Never reached today, and declared anyway: the ladder's answer is a *value* rather than
    /// an absence of vocabulary, and a rung that existed only in the prose of this comment
    /// would be the exact thing this file exists to object to.
    #[expect(
        dead_code,
        reason = "the rung is the finding; no probe needs it in this build"
    )]
    TypedProbe,
    /// Only `detail` separates the cases. A G2-02 failure.
    ProseOnly,
    /// No channel separates the cases, `detail` included.
    Undetermined,
}

/// One live refusal, with its adjudication.
struct Probe {
    /// What was attempted.
    name: &'static str,
    /// The operation attempted.
    operation: &'static str,
    /// Whether the agent's own request distinguishes this case from every other probe that
    /// shares its code — that is, whether some field of the request the agent chose is the
    /// condition, so the agent can name it without being told.
    self_evident: bool,
    /// The action the agent must take.
    action: Action,
    /// The answer.
    answer: Answer,
}

/// Drive every refusal probe.
#[expect(
    clippy::too_many_lines,
    reason = "one probe per condition, each with the fixture state that reaches it"
)]
fn refusal_probes() -> Vec<Probe> {
    let mut probes = Vec::new();

    // --- envelope obligations, on the lane.
    let mut fixture = lane();
    prime(&mut fixture);
    let snapshot = sealed_snapshot(&mut fixture, "req_p_seed");

    let mut unkeyed = envelope("workspace.create", "agent:builder", "cap_builder", "req_p1");
    unkeyed.idempotency_key = Optional::Absent;
    probes.push(Probe {
        name: "a mutation with no idempotency key",
        operation: "workspace.create",
        self_evident: true,
        action: Action::AddIdempotencyKey,
        answer: ask(
            &mut fixture.server,
            unkeyed,
            &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: fixture.components.clone(),
                overlay: Optional::Absent,
                seal: Optional::Present(true),
            }),
        ),
    });

    let mut keyed = envelope("task.status", "agent:reader", "cap_reader", "req_p2");
    keyed.idempotency_key = Optional::Present("idem-on-a-read".to_owned());
    probes.push(Probe {
        name: "a read carrying an idempotency key",
        operation: "task.status",
        self_evident: true,
        action: Action::DropIdempotencyKey,
        answer: ask(
            &mut fixture.server,
            keyed,
            &Arguments::TaskStatus(TaskStatusRequest {
                task: TaskHandle::new("task_absent0000000000000000000000000000000000000000")
                    .expect("a well-formed task handle"),
            }),
        ),
    });

    let mut budgetless = envelope("verification.start", "agent:runner", "cap_runner", "req_p3");
    budgetless.budget = Optional::Absent;
    probes.push(Probe {
        name: "a task-starting call with no budget",
        operation: "verification.start",
        self_evident: true,
        action: Action::FixOwnArgument,
        answer: ask(
            &mut fixture.server,
            on(budgetless, &snapshot),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    // --- the three `verification.start` refusals that share one code.
    probes.push(Probe {
        name: "a portfolio with no wire definition",
        operation: "verification.start",
        self_evident: true,
        action: Action::FixOwnArgument,
        answer: ask(
            &mut fixture.server,
            on(
                envelope("verification.start", "agent:runner", "cap_runner", "req_p4"),
                &snapshot,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Custom,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    probes.push(Probe {
        name: "a target kind no reachability campaign answers",
        operation: "verification.start",
        self_evident: true,
        action: Action::FixOwnArgument,
        answer: ask(
            &mut fixture.server,
            on(
                envelope("verification.start", "agent:runner", "cap_runner", "req_p5"),
                &snapshot,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::Refinement,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    probes.push(Probe {
        name: "a target naming a predicate the model does not declare",
        operation: "verification.start",
        self_evident: true,
        action: Action::FixOwnArgument,
        answer: ask(
            &mut fixture.server,
            on(
                envelope("verification.start", "agent:runner", "cap_runner", "req_p6"),
                &snapshot,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::Property,
                    id: "NoSuchPredicate".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    // --- a campaign over a snapshot whose modules no registered model matches. Every argument
    // is well-formed, so nothing in the agent's own request names the condition.
    let mut bare = lane();
    prime(&mut bare);
    let mut trimmed = bare.components.clone();
    trimmed.files.truncate(1);
    let other = bare
        .server
        .daemon_mut()
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("Other.ctm").expect("a workspace path"),
            b"-- not the registered module set\n".to_vec(),
        )
        .expect("staging names its content");
    let odd = ask(
        &mut bare.server,
        envelope(
            "workspace.create",
            "agent:builder",
            "cap_builder",
            "req_p7_create",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: vec![other],
                ..trimmed
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(odd.status(), ResultStatus::Ok, "{:?}", odd.detail());
    let unregistered = recover(&odd, "WorkspaceHandle").sole("snapshot handle");
    probes.push(Probe {
        name: "a snapshot whose modules no registered model matches",
        operation: "verification.start",
        self_evident: false,
        action: Action::Unshipped,
        answer: ask(
            &mut bare.server,
            on(
                envelope("verification.start", "agent:runner", "cap_runner", "req_p7"),
                &unregistered,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    // --- a campaign over an unsealed snapshot.
    let mut drafting = lane();
    prime(&mut drafting);
    let draft = ask(
        &mut drafting.server,
        envelope(
            "workspace.create",
            "agent:builder",
            "cap_builder",
            "req_p8_create",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: drafting.components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(false),
        }),
    );
    assert_eq!(draft.status(), ResultStatus::Ok, "{:?}", draft.detail());
    let unsealed = recover(&draft, "WorkspaceHandle").sole("snapshot handle");
    probes.push(Probe {
        name: "a campaign over an unsealed snapshot",
        operation: "verification.start",
        // The agent published this snapshot and read `sealed: false` off that answer, so the
        // condition is one its own prior typed answers already named.
        self_evident: true,
        action: Action::SealSnapshot,
        answer: ask(
            &mut drafting.server,
            on(
                envelope("verification.start", "agent:runner", "cap_runner", "req_p8"),
                &unsealed,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    // --- a campaign over a snapshot its lineage has moved past.
    let mut superseded = lane();
    prime(&mut superseded);
    let head = sealed_snapshot(&mut superseded, "req_p9_create");
    let forked = ask(
        &mut superseded.server,
        envelope(
            "workspace.fork",
            "agent:builder",
            "cap_builder",
            "req_p9_fork",
        ),
        &Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: WorkspaceHandle::new(&head).expect("a well-formed workspace handle"),
            overlay: Optional::Absent,
            patches: Optional::Absent,
        }),
    );
    if forked.status() == ResultStatus::Ok {
        probes.push(Probe {
            name: "a campaign over a snapshot its lineage moved past",
            operation: "verification.start",
            self_evident: false,
            action: Action::RefetchHead,
            answer: ask(
                &mut superseded.server,
                on(
                    envelope("verification.start", "agent:runner", "cap_runner", "req_p9"),
                    &head,
                ),
                &Arguments::VerificationStart(VerificationStartRequest {
                    target: Target {
                        kind: TargetKind::AllClaims,
                        id: "DieHard".to_owned(),
                    },
                    portfolio: Portfolio::Interactive,
                    context_policy: Optional::Absent,
                    priority_class: Optional::Absent,
                }),
            ),
        });
    }

    // --- a key spent on one request re-used for another.
    let mut reused = lane();
    prime(&mut reused);
    let mut first = envelope(
        "workspace.create",
        "agent:builder",
        "cap_builder",
        "req_p15a",
    );
    first.idempotency_key = Optional::Present("idem-shared".to_owned());
    let opened = ask(
        &mut reused.server,
        first,
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: reused.components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(opened.status(), ResultStatus::Ok, "{:?}", opened.detail());
    let mut again = envelope(
        "workspace.create",
        "agent:builder",
        "cap_builder",
        "req_p15b",
    );
    again.idempotency_key = Optional::Present("idem-shared".to_owned());
    probes.push(Probe {
        name: "an idempotency key spent on a different request",
        operation: "workspace.create",
        self_evident: true,
        action: Action::AddIdempotencyKey,
        answer: ask(
            &mut reused.server,
            again,
            &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: reused.components.clone(),
                overlay: Optional::Absent,
                seal: Optional::Present(false),
            }),
        ),
    });

    // --- a version this connection did not negotiate.
    let mut mismatched = envelope("task.status", "agent:reader", "cap_reader", "req_p16");
    mismatched.protocol_version = ProtocolVersion::new(3, 0);
    probes.push(Probe {
        name: "a protocol version this connection did not negotiate",
        operation: "task.status",
        self_evident: true,
        action: Action::Renegotiate,
        answer: ask(
            &mut reused.server,
            mismatched,
            &Arguments::TaskStatus(TaskStatusRequest {
                task: TaskHandle::new("task_absent0000000000000000000000000000000000000000")
                    .expect("a well-formed task handle"),
            }),
        ),
    });

    // --- the two capability refusals that finding F2 is about.
    let mut denied = lane();
    prime(&mut denied);
    let real = sealed_snapshot(&mut denied, "req_p11_create");
    probes.push(Probe {
        name: "a read capability attempting an execute operation",
        operation: "verification.start",
        self_evident: false,
        action: Action::Escalate,
        answer: ask(
            &mut denied.server,
            on(
                envelope(
                    "verification.start",
                    "agent:reader",
                    "cap_reader",
                    "req_p11",
                ),
                &real,
            ),
            &Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        ),
    });

    let mut absent = graph();
    probes.push(Probe {
        name: "a well-formed evidence handle the graph does not hold",
        operation: "evidence.get",
        self_evident: false,
        action: Action::RederiveHandle,
        answer: ask(
            &mut absent.server,
            envelope("evidence.get", "agent:reader", "cap_reader", "req_p12"),
            &Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: EvidenceHandle::new(
                    "ev_notheldbythisgraphaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("a well-formed evidence handle"),
                inline: Optional::Absent,
            }),
        ),
    });

    // --- an operation this deployment registers no family for.
    let mut unserved = lane();
    prime(&mut unserved);
    probes.push(Probe {
        name: "an operation whose subsystem this daemon does not serve",
        operation: "evidence.get",
        self_evident: true,
        action: Action::Unshipped,
        answer: ask(
            &mut unserved.server,
            envelope("evidence.get", "agent:reader", "cap_reader", "req_p14"),
            &Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: EvidenceHandle::new(
                    "ev_nofamilyonthisconnectionaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("a well-formed evidence handle"),
                inline: Optional::Absent,
            }),
        ),
    });

    probes
}

/// Classify one probe against the whole probe set.
fn determinacy(probe: &Probe, all: &[Probe]) -> Determinacy {
    let code = probe.answer.code();
    let sharing: Vec<&Probe> = all
        .iter()
        .filter(|other| other.answer.code() == code)
        .collect();
    let actions: BTreeSet<Action> = sharing.iter().map(|other| other.action).collect();
    if actions.len() == 1 {
        return Determinacy::AnswerAlone;
    }
    if probe.self_evident {
        return Determinacy::AnswerAndOwnRequest;
    }
    // Among the probes that share this code and are not separated by the agent's own request,
    // is `detail` the thing that separates them?
    let opaque: Vec<&&Probe> = sharing.iter().filter(|other| !other.self_evident).collect();
    let details: BTreeSet<Option<String>> =
        opaque.iter().map(|other| other.answer.detail()).collect();
    let residual: BTreeSet<Action> = opaque.iter().map(|other| other.action).collect();
    if residual.len() == 1 {
        return Determinacy::AnswerAndOwnRequest;
    }
    if details.len() == opaque.len() {
        Determinacy::ProseOnly
    } else {
        Determinacy::Undetermined
    }
}

#[test]
fn refusal_determinacy_is_measured_over_every_probe() {
    let probes = refusal_probes();
    assert!(
        probes.len() >= 14,
        "the probe set shrank; every probe is a condition this daemon can actually reach"
    );
    for probe in &probes {
        assert_eq!(
            probe.answer.status(),
            ResultStatus::Error,
            "`{}` on {} did not refuse; the probe no longer probes anything",
            probe.name,
            probe.operation
        );
        assert!(
            probe.answer.code().is_some(),
            "`{}` refused without a typed code",
            probe.name
        );
    }
    let codes: BTreeSet<&'static str> = probes
        .iter()
        .filter_map(|probe| probe.answer.code())
        .map(ProtocolEnum::as_wire)
        .collect();
    assert!(
        codes.len() >= 5,
        "the probes must reach several codes or the collision analysis is vacuous: {codes:?}"
    );

    let mut prose_bound = Vec::new();
    for probe in &probes {
        if determinacy(probe, &probes) == Determinacy::ProseOnly {
            prose_bound.push(probe.name);
        }
    }
    assert!(
        prose_bound.is_empty(),
        "these refusals are separated only by their `detail` string, so an agent must parse \
         prose to recover: {prose_bound:?}; G2-02 fails"
    );
}

#[test]
fn the_only_undetermined_refusals_are_the_ones_prose_does_not_rescue_either() {
    let probes = refusal_probes();
    let undetermined: Vec<&str> = probes
        .iter()
        .filter(|probe| determinacy(probe, &probes) == Determinacy::Undetermined)
        .map(|probe| probe.name)
        .collect();
    assert_eq!(
        undetermined.len(),
        2,
        "finding F2 is exactly two probes wide; the third, a mis-spelled capability scope, \
         became a typed provisioning refusal (bn-3ncfp): {undetermined:?}"
    );
    // And the reason they are undetermined rather than prose-bound is that their `detail` is
    // the same string. Parsing it recovers nothing, which is why this is a typed-coarseness
    // finding and not a G2-02 failure.
    let details: BTreeSet<Option<String>> = probes
        .iter()
        .filter(|probe| determinacy(probe, &probes) == Determinacy::Undetermined)
        .map(|probe| probe.answer.detail())
        .collect();
    assert_eq!(
        details.len(),
        1,
        "two undetermined refusals whose details differ would be a prose dependency, not a \
         uniform denial: {details:?}"
    );
    assert_eq!(
        details.into_iter().next().flatten(),
        Some("the presented capability does not admit this operation".to_owned()),
        "the uniform denial RFC 0027 X1-X2 requires"
    );
}

#[test]
fn a_non_resumable_task_failure_is_typed_without_reading_its_sentence() {
    // `BudgetExhausted` is the one code the IDL pairs with a free-text `non_resumable_reason`,
    // and it does not arrive as an envelope refusal at all: a budget that cannot hold the
    // model's own initial states starts a task and fails it. So the question is whether the
    // *task record* says "do not resume" typed. It does, in two fields, and the sentence beside
    // them adds nothing an agent branches on.
    let mut fixture = lane();
    prime(&mut fixture);
    let snapshot = sealed_snapshot(&mut fixture, "req_nr_create");
    let started = ask(
        &mut fixture.server,
        on(
            states(
                envelope(
                    "verification.start",
                    "agent:runner",
                    "cap_runner",
                    "req_nr_start",
                ),
                Some(0),
            ),
            &snapshot,
        ),
        &Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    );
    let task = recover(&started, "TaskHandle").sole("task handle");
    let record = ask(
        &mut fixture.server,
        envelope("task.status", "agent:reader", "cap_reader", "req_nr_status"),
        &Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new(&task).expect("a well-formed task handle"),
        }),
    );
    assert_eq!(record.status(), ResultStatus::Ok, "{:?}", record.detail());
    let leaves = record.leaves();
    let status = leaves
        .iter()
        .find(|leaf| leaf.path == "payload.status")
        .expect("a task record states its status");
    assert_eq!(status.declared, "TaskStatus", "and states it as an enum");
    assert_eq!(status.text, "failed");
    let reason = leaves
        .iter()
        .find(|leaf| leaf.path == "payload.failed_reason")
        .expect("a failed task is never silent (plan §4.5)");
    assert_eq!(
        reason.declared, "ErrorCode",
        "the terminal reason is the same closed vocabulary a refusal uses"
    );
    assert_eq!(reason.text, "BudgetExhausted");
    assert!(
        !leaves
            .iter()
            .any(|leaf| leaf.path == "payload.continuation"),
        "and the absence of a continuation is what says `not resumable`, typed"
    );
    // The free-text sibling exists, and nothing above needed it.
    let sentence = leaves
        .iter()
        .find(|leaf| leaf.path == "payload.non_resumable_reason");
    if let Some(sentence) = sentence {
        assert_eq!(sentence.declared, "String");
        assert!(
            prose_position(sentence),
            "`non_resumable_reason` is the prose position PROSE_ADJUDICATION calls a typed \
             sibling of `failed_reason` and `continuation`"
        );
    }
}

#[test]
fn next_operations_is_empty_and_every_recovery_offer_is_executable_as_given() {
    // `Error.recovery` became populated under bn-27mx7 (RFC 0027 H8: a stale-handle
    // refusal names the head it has to be re-derived against). This is the probe the
    // earlier as-is pin asked this file to grow: every offer on the wire names a registered
    // operation, its `arguments` decode as that operation's request struct in the
    // negotiated encoding, and its `rationale` is one of the daemon's declared tokens —
    // never prose (RFC 0026 N1, RFC 0027 S1).
    let answers = all_answers();
    assert!(
        answers.len() >= 30,
        "the sweep must see the whole run, not a slice: {} answers",
        answers.len()
    );
    let mut offered = 0_usize;
    for answer in &answers {
        offered += answer.envelope.next_operations.len();
        let Some(error) = answer.envelope.error.value() else {
            continue;
        };
        for offer in &error.recovery {
            assert!(
                continuumd::protocol::registry::operation(offer.operation.as_str()).is_some(),
                "{}: a recovery offer names an operation the registry declares",
                answer.operation
            );
            continuumd::codec::operations::decode_arguments(
                offer.operation.as_str(),
                &offer.arguments,
            )
            .unwrap_or_else(|failure| {
                panic!(
                    "{}: a recovery offer's arguments decode as its operation's request \
                     struct: {failure:?}",
                    answer.operation
                )
            });
            let rationale = offer
                .rationale
                .value()
                .expect("every offer this daemon makes states its typed rationale");
            assert!(
                [
                    continuumd::daemon::family::RATIONALE_RESEAL_LINEAGE_HEAD,
                    continuumd::daemon::family::RATIONALE_REISSUE_WITHOUT_SNAPSHOT_PIN,
                ]
                .contains(&rationale.as_str()),
                "{}: `{rationale}` is not a declared rationale token",
                answer.operation
            );
        }
    }
    assert_eq!(
        offered, 0,
        "`ResultEnvelope.next_operations` is populated; the typed discovery surface is now \
         checkable and this file must grow a probe for it"
    );

    // Not vacuous: the superseded-snapshot probe carries its offer on the wire, and the
    // offer names a snapshot other than the one the refused campaign pinned.
    let probes = refusal_probes();
    let moved = probes
        .iter()
        .find(|probe| probe.name == "a campaign over a snapshot its lineage moved past")
        .expect("the superseded-snapshot probe is in the set");
    let error = moved
        .answer
        .envelope
        .error
        .value()
        .expect("a refusal carries an error");
    assert_eq!(error.code, ErrorCode::StaleSnapshot);
    assert_eq!(
        error
            .recovery
            .iter()
            .map(|offer| offer.operation.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace.seal"],
        "a superseded-snapshot refusal offers exactly the re-seal of its lineage head"
    );
}

#[test]
fn no_refusal_carries_a_typed_discriminator_beyond_its_code() {
    let probes = refusal_probes();
    for probe in &probes {
        let error = probe
            .answer
            .envelope
            .error
            .value()
            .expect("a refusal carries an error");
        assert!(
            !error.retryable,
            "`{}` is the first retryable refusal this daemon raises; the determinacy ladder \
             must gain a rung for it",
            probe.name
        );
        assert!(
            error.data.value().is_none(),
            "`{}` carries typed `Error.data`; the determinacy analysis must read it",
            probe.name
        );
        assert!(
            error.continuation.value().is_none(),
            "`{}` carries a continuation on a refusal; the ladder must read it",
            probe.name
        );
    }
}

/// The token list `rule artifact_class.spelling` declares, read out of the rule body.
fn declared_class_tokens(idl: &str) -> Vec<String> {
    let start = idl
        .find("rule artifact_class.spelling {")
        .expect("the IDL declares `rule artifact_class.spelling`");
    let body = &idl[start..];
    let body = &body[..body.find("\n}\n").expect("the rule closes")];
    let list = &body[body
        .find("are exactly:")
        .expect("the rule lists the tokens")..];
    let list = &list[..list.find('.').expect("the list ends with a full stop")];
    list.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_artifact_class_spelling_is_a_typed_refusal_at_provisioning() {
    use continuum_workspace::artifact_path::ArtifactClass;
    use continuumd::daemon::provisioning::{ArtifactClassRefusal, ProvisioningRefusal};

    // The IDL decides the spelling in a rule, and every site cites that rule. The two doc
    // comments that disagreed are gone.
    let text = read(IDL);
    assert!(
        !text.contains("the plan §4.4 prefix without the underscore")
            && !text.contains("Artifact class of the redacted original (plan §4.4 prefix)."),
        "the disagreeing doc comments are corrected"
    );
    for site in [
        "original_class: String required;",
        "kind: String required;",
        "artifact_classes: list<String> required;",
    ] {
        let at = text.find(site).expect("the site is declared");
        let preceding = &text[text[..at].rfind("\n\n").unwrap_or(0)..at];
        let doc = &preceding[preceding.rfind(";\n").map_or(0, |end| end + 2)..];
        assert!(
            doc.contains("class token") && doc.contains("rule artifact_class.spelling"),
            "`{site}` cites the spelling rule: {doc}"
        );
    }

    // The rule's list is the store's class table, token for token and in plan §4.4 order,
    // so neither can drift from the other.
    let declared = declared_class_tokens(&text);
    let store: Vec<String> = ArtifactClass::ALL
        .into_iter()
        .map(|class| class.token().to_owned())
        .collect();
    assert_eq!(declared, store, "the rule and `ArtifactClass::ALL` agree");

    // The plan's own prefix spelling is a typed refusal where it is provisioned. It names the
    // capability, the spelling, and the class it meant.
    let refused = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                Optional::Absent,
            ),
            None,
        )
        .capability(tokened(), Some(cap("cap_root")))
        .capability(prefixed(), Some(cap("cap_root")))
        .try_build()
        .expect_err("the prefix spelling is refused, not provisioned as an empty scope");
    assert_eq!(
        refused,
        ProvisioningRefusal::ArtifactClass {
            capability: cap("cap_prefixed"),
            refusal: ArtifactClassRefusal::PrefixSpelling {
                given: "ws_".to_owned(),
                meant: ArtifactClass::WorkspaceSnapshot,
            },
        }
    );
    let message = refused.to_string();
    assert!(
        message.contains("cap_prefixed") && message.contains("`ws_`") && message.contains("`ws`"),
        "the refusal's rendering carries what its fields carry: {message}"
    );

    // A spelling that is neither a token nor a prefix is the other refusal, not the same one.
    let mut unknown = prefixed();
    unknown.artifact_classes = vec!["workspace".to_owned()];
    let refused = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(unknown, None)
        .try_build()
        .expect_err("an unknown spelling is refused");
    assert!(matches!(
        refused,
        ProvisioningRefusal::ArtifactClass {
            refusal: ArtifactClassRefusal::Unknown { ref given },
            ..
        } if given == "workspace"
    ));

    // The post-build provisioning surface refuses the same spelling, and registers nothing.
    let mut fixture = lane();
    prime(&mut fixture);
    let refused = fixture
        .server
        .daemon_mut()
        .state_mut()
        .register_capability(prefixed(), Some(cap("cap_root")))
        .expect_err("the prefix spelling is refused after build too");
    assert!(matches!(
        refused,
        ProvisioningRefusal::ArtifactClass {
            refusal: ArtifactClassRefusal::PrefixSpelling { .. },
            ..
        }
    ));
    assert!(
        fixture
            .server
            .daemon_mut()
            .state_mut()
            .grant(&cap("cap_prefixed"))
            .is_none(),
        "a refused capability is not registered"
    );

    // The token spelling is still admitted for the call the prefix spelling used to fail.
    let token = ask(
        &mut fixture.server,
        envelope(
            "workspace.create",
            "agent:tokened",
            "cap_tokened",
            "req_f3_b",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: fixture.components.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(
        token.status(),
        ResultStatus::Ok,
        "the token spelling is the canonical one: {:?}",
        token.detail()
    );
}

// =====================================================================================
// 6. The CLI boundary
// =====================================================================================

/// The wire operation name a daemon `Arguments` variant carries.
///
/// The protocol spells a variant `WorkspaceCreateByReference` for `workspace.create_by_reference`:
/// the first camel segment is the namespace, the rest is the snake-cased verb. Deriving it
/// rather than tabulating it is what lets the CLI's surface be *read* from the CLI's source
/// instead of restated here, and [`the_operation_name_derivation_is_not_vacuous`] is the
/// control that the derivation can fail.
fn wire_name(variant: &str) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    for character in variant.chars() {
        if character.is_ascii_uppercase() && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
        current.push(character.to_ascii_lowercase());
    }
    if !current.is_empty() {
        segments.push(current);
    }
    match segments.split_first() {
        Some((namespace, verb)) if !verb.is_empty() => {
            format!("{namespace}.{}", verb.join("_"))
        }
        _ => variant.to_ascii_lowercase(),
    }
}

/// Every operation the CLI can reach, read from the CLI's own source.
///
/// Two channels, because the CLI uses two: a typed `Arguments` variant where a body exists in
/// this build, and a bare operation-name literal where the operation is one of the 45 the
/// daemon refuses before a body is read. The literal channel is filtered through the registry,
/// so an unrelated dotted string in the source cannot inflate the count.
fn cli_operations() -> BTreeSet<String> {
    let mut source = String::new();
    let root = Path::new(REPO).join(CLI_SOURCE);
    let mut pending = vec![root];
    while let Some(entry) = pending.pop() {
        if entry.is_dir() {
            let mut children: Vec<_> = std::fs::read_dir(&entry)
                .unwrap_or_else(|error| panic!("{}: {error}", entry.display()))
                .map(|child| child.expect("a readable directory entry").path())
                .collect();
            children.sort();
            pending.extend(children);
        } else {
            source.push_str(&std::fs::read_to_string(&entry).unwrap_or_default());
            source.push('\n');
        }
    }

    let mut found = BTreeSet::new();
    let mut rest = source.as_str();
    while let Some(at) = rest.find("Arguments::") {
        rest = &rest[at + "Arguments::".len()..];
        let variant: String = rest
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !variant.is_empty() {
            let name = wire_name(&variant);
            if registry::operation(&name).is_some() {
                found.insert(name);
            }
        }
    }
    for candidate in source.split('"') {
        if registry::operation(candidate).is_some() {
            found.insert(candidate.to_owned());
        }
    }
    found
}

#[test]
fn the_cli_reaches_nothing_the_typed_protocol_does_not() {
    let cli = cli_operations();
    let registry: BTreeSet<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();
    let only_in_cli: Vec<&String> = cli
        .iter()
        .filter(|name| !registry.contains(name.as_str()))
        .collect();
    assert!(
        only_in_cli.is_empty(),
        "the CLI names operations the protocol does not declare: {only_in_cli:?}"
    );
    assert_eq!(
        cli.len(),
        16,
        "the CLI's operation surface is sixteen, matching its own command count: {cli:?}"
    );
    assert_eq!(
        registry.len() - cli.len(),
        59,
        "and fifty-nine registry operations have no CLI surface at all"
    );
}

#[test]
fn the_shipped_cli_binary_reaches_no_daemon_at_all() {
    // The strongest form of "an agent never needs the terminal": in this build the terminal
    // cannot answer. The binary is wired to a transport that refuses every frame, and says so
    // rather than pretending.
    let binary = read("crates/continuum-cli/src/bin/continuum.rs");
    assert!(
        binary.contains("NullTransport"),
        "the shipped binary now carries a real transport; the claim below must be re-derived"
    );
    let wire = read("crates/continuum-cli/src/wire.rs");
    assert!(
        wire.contains("NoTransportConfigured"),
        "the null transport's refusal is a typed variant, not a printed sentence"
    );
}

#[test]
fn the_live_surface_is_thirty_of_the_seventy_five_and_this_is_which() {
    // The 30 the codec can build a body for are the ones any of this file's evidence reaches.
    // Naming the boundary is the INV-007 half of the verdict: forward closure is *evidenced*
    // over 30 operations and *untested* over 45.
    assert_eq!(OPERATIONS.len(), 75, "the registry's own count");
    let served: BTreeSet<&str> = [
        "workspace.create",
        "workspace.create_by_reference",
        "workspace.fork",
        "workspace.diff",
        "workspace.seal",
        "intent.get",
        "intent.diff",
        "intent.propose_revision",
        "intent.accept",
        "intent.reject",
        "intent.lock",
        "evidence.get",
        "evidence.query",
        "evidence.verify",
        "evidence.subscribe",
        "evidence.link",
        "observe.ingest",
        "observe.classify",
        "observe.result",
        "verification.start",
        "verification.result",
        "verification.await",
        "task.status",
        "task.cancel",
        "task.resume",
        "task.subscribe",
        "task.update_budget",
        "context.compile",
        "context.expand",
        "whiteboard.compile",
    ]
    .into_iter()
    .collect();
    assert_eq!(served.len(), 30, "thirty have an `Arguments` variant");
    for name in &served {
        assert!(
            registry::operation(name).is_some(),
            "{name} is not a declared operation"
        );
    }
    // Every served name is a mutation or a read, and the registry says which — the fact an
    // agent needs before it can even build an envelope, and it is static, not printed.
    for name in &served {
        let _ = is_mutation(name);
    }
}

// =====================================================================================
// 7. Negative controls
// =====================================================================================

#[test]
fn the_census_flags_a_handle_that_reaches_the_agent_only_inside_a_sentence() {
    let (_, answers) = drive_workflows();
    let source = answers
        .iter()
        .find(|answer| !recover(answer, "TaskHandle").typed.is_empty())
        .expect("some answer names a task");
    let handle = recover(source, "TaskHandle").sole("task handle");

    // Doctor the answer the way a prose-carrying protocol would have shaped it: strip the
    // handle from every typed position, and put it inside `detail`.
    let doctored = Answer {
        operation: source.operation.clone(),
        document: plant(&source.document, &handle),
        envelope: source.envelope.clone(),
    };
    let found = recover(&doctored, "TaskHandle");
    assert!(
        !found.typed.contains_key(&handle),
        "the doctored answer still carries the handle typed at {:?}",
        found.typed
    );
    assert!(
        found.prose.contains_key(&handle),
        "the census failed to notice the handle inside the sentence: {found:?}"
    );
    // And the workflow that depended on it would proceed on the *wrong* value: the only typed
    // task position now holds the placeholder, so a step driven from the typed surface would
    // carry `struck` forward rather than the handle the sentence names. That is the failure
    // mode the criterion forbids, reproduced.
    assert_eq!(
        found.sole("task handle"),
        "struck",
        "the typed surface must be left naming something other than the handle"
    );
}

#[test]
fn the_census_does_not_call_a_typed_position_prose() {
    let (_, answers) = drive_workflows();
    let source = answers
        .iter()
        .find(|answer| !recover(answer, "TaskHandle").typed.is_empty())
        .expect("some answer names a task");
    let found = recover(source, "TaskHandle");
    assert!(
        found.prose.is_empty(),
        "an undoctored answer must carry no scrapeable handle: {:?}",
        found.prose
    );
    assert_eq!(
        found.typed.len(),
        1,
        "and exactly one typed one, so the instrument is not a constant"
    );
}

/// Rewrite an answer so `handle` appears only inside a declared prose position.
///
/// The doctoring is realistic rather than arbitrary: the handle is struck from every typed
/// position, and a `Warning` is appended to the envelope's own `warnings` list with the handle
/// spelled inside `Warning.detail` — a `String` under a human-text name, which is exactly the
/// shape a prose-carrying protocol would have. Nothing outside the IDL's declared fields is
/// invented, so the census is being tested against a document it could really receive.
fn plant(document: &Wire, handle: &str) -> Wire {
    let struck = strike(document, handle);
    let Wire::Object(mut members) = struck else {
        panic!("an answer is an object");
    };
    let mut warning: BTreeMap<String, Wire> = BTreeMap::new();
    warning.insert("code".to_owned(), Wire::String("task-parked".to_owned()));
    warning.insert(
        "detail".to_owned(),
        Wire::String(format!("the campaign {handle} parked; poll it to continue")),
    );
    let mut warnings = match members.remove("warnings") {
        Some(Wire::Array(existing)) => existing,
        _ => Vec::new(),
    };
    warnings.push(Wire::Object(warning));
    members.insert("warnings".to_owned(), Wire::Array(warnings));
    Wire::Object(members)
}

/// Replace every whole-value occurrence of `handle` with a placeholder.
fn strike(value: &Wire, handle: &str) -> Wire {
    match value {
        Wire::Object(members) => Wire::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), strike(member, handle)))
                .collect(),
        ),
        Wire::Array(items) => Wire::Array(items.iter().map(|item| strike(item, handle)).collect()),
        Wire::String(text) if text == handle => Wire::String("struck".to_owned()),
        other => other.clone(),
    }
}

#[test]
fn the_determinacy_ladder_separates_a_prose_split_from_a_uniform_one() {
    // Two synthetic pairs, both sharing a code and both needing different actions. The only
    // difference is whether `detail` separates them.
    let split = vec![
        synthetic(
            "a",
            ErrorCode::StatusConflict,
            "the claim moved",
            Action::RefetchHead,
        ),
        synthetic(
            "b",
            ErrorCode::StatusConflict,
            "the budget is spent",
            Action::RaiseBudget,
        ),
    ];
    assert_eq!(
        determinacy(&split[0], &split),
        Determinacy::ProseOnly,
        "differing details across differing actions is exactly the failure mode"
    );

    let uniform = vec![
        synthetic(
            "a",
            ErrorCode::StatusConflict,
            "denied",
            Action::RefetchHead,
        ),
        synthetic(
            "b",
            ErrorCode::StatusConflict,
            "denied",
            Action::RaiseBudget,
        ),
    ];
    assert_eq!(
        determinacy(&uniform[0], &uniform),
        Determinacy::Undetermined,
        "identical details across differing actions is underdetermination, not a prose \
         dependency: parsing the message recovers nothing"
    );

    let single = vec![synthetic(
        "a",
        ErrorCode::StatusConflict,
        "denied",
        Action::RefetchHead,
    )];
    assert_eq!(
        determinacy(&single[0], &single),
        Determinacy::AnswerAlone,
        "one action under one code is decided by the code"
    );
}

/// A probe built from a hand-written refusal, for the ladder's own controls.
fn synthetic(name: &'static str, code: ErrorCode, detail: &str, action: Action) -> Probe {
    let mut error: BTreeMap<String, Wire> = BTreeMap::new();
    error.insert("code".to_owned(), Wire::String(code.as_wire().to_owned()));
    error.insert("detail".to_owned(), Wire::String(detail.to_owned()));
    let mut root: BTreeMap<String, Wire> = BTreeMap::new();
    root.insert("status".to_owned(), Wire::String("error".to_owned()));
    root.insert("error".to_owned(), Wire::Object(error));
    Probe {
        name,
        operation: "synthetic",
        self_evident: false,
        action,
        answer: Answer {
            operation: "task.status".to_owned(),
            document: Wire::Object(root),
            envelope: ResultEnvelope {
                request_id: RequestId::new("req_synthetic").expect("a well-formed request id"),
                status: ResultStatus::Error,
                verdict: Nullable::Null,
                error: Optional::Present(continuumd::protocol::envelope::Error {
                    code,
                    detail: detail.to_owned(),
                    data: Optional::Absent,
                    recovery: Vec::new(),
                    continuation: Optional::Absent,
                    non_resumable_reason: Optional::Absent,
                    retryable: false,
                }),
                assurance: Optional::Absent,
                artifacts: Vec::new(),
                task: Optional::Absent,
                continuation: Optional::Absent,
                omissions: Vec::new(),
                warnings: Vec::new(),
                cost: continuumd::protocol::envelope::Cost {
                    wall_ms: Optional::Absent,
                    cpu_ms: Optional::Absent,
                    memory_bytes: Optional::Absent,
                    states: Optional::Absent,
                    solver_ms: Optional::Absent,
                    proof_ms: Optional::Absent,
                    tokens: Optional::Absent,
                    candidates: Optional::Absent,
                    bytes: Optional::Absent,
                    tokenizer_id: Optional::Absent,
                },
                epochs: epochs(),
                next_operations: Vec::new(),
                next_page_token: Optional::Absent,
                payload: Nullable::Null,
                audit: Optional::Absent,
            },
        },
    }
}

#[test]
fn the_operation_name_derivation_is_not_vacuous() {
    assert_eq!(wire_name("WorkspaceCreate"), "workspace.create");
    assert_eq!(
        wire_name("WorkspaceCreateByReference"),
        "workspace.create_by_reference"
    );
    assert_eq!(wire_name("TaskUpdateBudget"), "task.update_budget");
    assert!(
        registry::operation(&wire_name("WorkspaceInvent")).is_none(),
        "a derived name the registry does not declare must not be admitted"
    );
    assert!(
        registry::operation(&wire_name("NotAnOperation")).is_none(),
        "and neither must a variant from another namespace entirely"
    );
}

#[test]
fn the_walk_reaches_inside_a_spliced_opaque_body_and_types_what_it_finds() {
    // The instrument's own reach, pinned on a real answer: `Opaque` inlines a canonical body
    // rather than base64-ing it, so `payload` is walked under the operation's own declared
    // response shape. A walk that stopped at the `Opaque` boundary would miss the whole of
    // every response body and would report false closure over every workflow above.
    let (_, answers) = drive_workflows();
    let record = answers
        .iter()
        .find(|answer| answer.operation == "task.status" && answer.status() == ResultStatus::Ok)
        .expect("some task.status succeeded");
    let walked = record.leaves();
    let inside: Vec<&Leaf> = walked
        .iter()
        .filter(|leaf| leaf.path.starts_with("payload."))
        .collect();
    assert!(
        inside.len() > 5,
        "the walk did not descend into the spliced payload: {inside:?}"
    );
    assert!(
        inside
            .iter()
            .any(|leaf| leaf.path == "payload.task" && leaf.declared == "TaskHandle"),
        "and it did not type what it found: {inside:?}"
    );
    assert!(
        inside
            .iter()
            .any(|leaf| leaf.path == "payload.status" && leaf.declared == "TaskStatus"),
        "the decision fact must be typed as its own enum, not as a string"
    );
    // And the two `task_`-spelled values a spelling heuristic would have confused are
    // separated by their declared types rather than by their prefixes.
    let spelled: BTreeSet<&str> = walked
        .iter()
        .filter(|leaf| leaf.text.starts_with("task_"))
        .map(|leaf| leaf.declared.as_str())
        .collect();
    assert!(
        spelled.len() >= 2,
        "at least two declared types spell `task_…`; that is why the walk is spec-driven \
         rather than prefix-driven: {spelled:?}"
    );
}

#[test]
fn the_scraper_is_a_real_parser_and_not_a_stub() {
    // The prose half of the census only means something if it can actually pull a handle out
    // of a sentence. If it could not, every answer would look typed.
    let found = scrape(
        "the campaign task_abc-123 parked; resume from cont_def456 when ready",
        "task_",
    );
    assert_eq!(found.len(), 1);
    assert!(found.contains("task_abc-123"));
    assert!(scrape("no handle here", "task_").is_empty());
    assert!(
        scrape("task_", "task_").is_empty(),
        "a bare prefix names nothing"
    );
}

/// Print the whole census, so the header's numbers are re-derivable rather than asserted.
///
/// Ignored by default: it asserts nothing and its output is a table, not a verdict. Run it
/// with `cargo test -p continuumd --test gate_g2_02_acceptance -- report --ignored --nocapture`
/// when any number in this file's header needs to be checked or moved.
#[test]
#[ignore = "reporting aid: prints the census the header tabulates, and asserts nothing"]
fn report() {
    let (census, answers) = drive_workflows();
    let workflows: BTreeSet<&str> = census.iter().map(|step| step.workflow).collect();
    println!("workflows = {}", workflows.len());
    println!("inter-step facts = {}", census.len());
    for step in &census {
        println!(
            "  {} | {} -> {} | {} @ {}",
            step.workflow, step.from, step.to, step.fact, step.path
        );
    }
    let probes = refusal_probes();
    println!("refusal probes = {}", probes.len());
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for probe in &probes {
        let level = format!("{:?}", determinacy(probe, &probes));
        *tally.entry(level.clone()).or_default() += 1;
        println!(
            "  {:<28} | {:<20} | {:?} | {}",
            probe.answer.code().map_or("-", ProtocolEnum::as_wire),
            level,
            probe.action,
            probe.name
        );
    }
    println!("determinacy = {tally:?}");
    println!(
        "answers swept for recovery channels = {}",
        answers.len() + probes.len()
    );
    let fields = prose_fields();
    println!("human-text-named IDL fields = {}", fields.len());
    for field in &fields {
        println!(
            "  {}:{} `{}` {:?}",
            field.line,
            field.site,
            field.declared,
            field.carriage()
        );
    }
    let cli = cli_operations();
    println!("CLI operations = {} {:?}", cli.len(), cli);
}
