//! INV-007 — Omission transparency: evidence map (bn-kh8b).
//!
//! > Any bounded Context Pack, explanation, slice, or visualization names what was omitted
//! > and how to retrieve it.
//! >
//! > — `notes/plan/plan.md`, INV-007
//!
//! # What this file is, and is not
//!
//! Unlike `inv001`/`inv003`/`inv008`/`inv016`, which each landed *alongside* the machinery
//! they evidence, INV-007's substance was already delivered — mostly by PR-11 (bn-28jj: the
//! omission model and closed accounting; bn-38p2: the byte-budget packer; bn-3jrtz: the
//! schema closing `kind`) and by PR-6's budget half (bn-1gc: the `MeterSet` omission
//! discipline; bn-23j7s: its daemon wiring), with `bn-1604` pinning one interaction and
//! `bn-37gu`'s falsification campaign (`dx01_falsification.rs`) exercising the whole family
//! end to end against the PR-11 acceptance criterion. This file's job is therefore mostly a
//! **map**: facet → live enforcement site → the test that already proves it, cited rather
//! than re-run (`bn-1eqt`'s freshness-tripwire device — [`citations`] below), plus a small
//! amount of genuinely new evidence at the one grain nothing else exercises: the wire
//! field's own structural non-droppability, live, across a real dispatch.
//!
//! # Facet → site → test
//!
//! | Facet | Enforcement site | Test |
//! |---|---|---|
//! | closed five-member `OmissionReason`, schema order | `continuum-context::omission` | `omission::the_reason_set_is_exactly_the_schema_enum` (cited) |
//! | the schema's two conditionals are unrepresentable, not validated | `continuum-context::omission::Retrievability` | `omission::every_record_declares_its_retrievability`, `::an_expandable_record_carries_the_query_that_retrieves_it` (cited) |
//! | `kind` closed to the selection vocabulary at the schema itself (F13) | `context-pack.schema.json` `$defs.selection_kind` | `omission::the_schema_closes_omission_kind_to_the_selection_vocabulary` + mutant `::a_reopened_free_string_kind_is_caught` (cited) |
//! | closed accounting is a **constructor property**: candidates = selection + Σ manifest counts | `continuum-context::accounting::Accounting::close` | `accounting::the_counting_equation_holds_for_every_closed_accounting`, `::an_undispositioned_candidate_cannot_be_closed` (cited) |
//! | `reconcile` is an independent check over the two *published* halves | `ClosedAccounting::reconcile` | `accounting::a_shrunk_manifest_count_fails_reconciliation`, `::an_inflated_manifest_count_fails_reconciliation` (anti-vacuity mutants, cited) |
//! | expansion handles are content-derived (ID5 canonical encoding), not ledger-minted | `continuum-context::expansion::ExpansionHandle::derive` | `daemon_context_operations::the_handle_the_manifest_promised_is_the_handle_the_expansion_returns`, `::one_question_is_one_pack_whatever_the_idempotency_key_says` (cited) |
//! | a packed child is an **answer** (ok + budget omission); `BudgetExhausted` only below the minimal child | `continuum-context::budget::BudgetPacker` | `daemon_context_budget::an_over_budget_expansion_publishes_a_smaller_child_and_says_what_the_ceiling_cost`, `::a_ceiling_below_the_minimal_child_publishes_nothing` (cited) |
//! | measured `content_budget.bytes` is a least self-consistent fixed point | `continuum-context::pack::ChildPack::to_json` | `pack::the_recorded_size_is_the_least_self_consistent_one` (cited) |
//! | the wire manifest agrees with the pack's own manifest, record for record | `daemon::context::expand`/`project` | `daemon_context_operations::the_envelope_manifest_agrees_with_the_packs_manifest_record_for_record` (cited) |
//! | no arm of `context.expand` answers manifest-free (redaction, exhausted relation, budget, success) | `daemon::context::expand` | `daemon_context_operations::a_purged_anchor_expands_to_a_success_carrying_the_stub_and_the_redaction_record`, `::depth_moves_items_between_the_pack_and_the_manifest_and_changes_no_total` (cited) |
//! | `context.compile` is a typed refusal, never a manifest-free fake success | `daemon::context::ContextFamily::handle` | `daemon_context_operations::context_compile_decodes_and_is_refused_with_the_typed_reason` (cited) |
//! | `MeterSet`'s declared/unmetered cell: refused as a *charge*, not silently recorded | `continuum-task::budget` | `budget::charging_an_unmetered_dimension_is_refused_rather_than_invented`, `::a_declared_ceiling_with_no_meter_is_named_rather_than_silently_passed` (cited) |
//! | the daemon's omission list is *derived* from the meter set, not a hand-written array | `daemon::budget::omissions_of` | `pr6_impl02_budget_evidence::the_omission_manifest_is_derived_from_the_meter_set` (cited) |
//! | a redaction is never just an error: it also names itself in `omissions[]` | `daemon::evidence::redact_evidence` | `daemon_evidence::a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission` (cited) |
//! | INV-016 interaction: `ContextExpandRequest.anchor` is closed-vocabulary equality, never parsed as prose | `daemon::context::ContextPackRecord::resolves` | `inv016_untrusted_source_evidence` (cited) |
//! | the CLI's own manifest render is non-suppressible (no `--quiet-omissions`) | `continuum-cli::render` | cited (no continuumd dependency edge to dispatch through — see below) |
//! | the ≥10×-reduction / exact-handle / accounting-refuses-undercounts acceptance criterion, over a real 200+ event failure | `dx01_falsification.rs` | cited |
//! | **new here**: the wire field itself cannot be dropped from a real answer | this file, [`wire`] | `wire::the_omissions_field_cannot_be_dropped_from_a_real_wire_answer` |
//!
//! # Honest gaps — typed absences, not violations
//!
//! Per bn-n9a1's rule, an unbuilt *producer* is not a violation of a disclosure invariant:
//! nothing it would have disclosed exists to be dropped, and every boundary below is a typed
//! refusal or a documented scope statement, never a silent success.
//!
//! 1. **The ten-stage compiler is unwired.** `context.compile` refuses
//!    `UnsupportedSemanticFeature` rather than returning a compiled-looking pack with nothing
//!    behind it (`daemon::context`'s own module doc). `dx01_falsification.rs` — PR-11's own
//!    acceptance-criterion campaign — states the honest grain precisely: the *replay check*,
//!    the *counting rule*, and *expansion* run at production grain; candidate-set
//!    construction and stage-2 causal slicing are harness code standing in for the compiler
//!    that does not exist yet. This is INV-008's typed-refusal territory reaching into
//!    INV-007's, not an INV-007 gap on its own.
//! 2. **Whole-pack root assembly / store publication is declined**, by name, in
//!    `daemon::context`'s module doc: a pack travels on the wire and is not separately
//!    content-addressed in the reference store. The manifest still travels — INV-007's text
//!    only obligates naming the omission and how to retrieve it, and the wire-embedded
//!    manifest does both — so this is a scope boundary on pack *storage*, not on disclosure.
//! 3. **The crashpack producer is absent** (`dx01_falsification.rs`'s residual item 3): a
//!    pack's `replay` field names a class-checked `crash_*` handle with no artifact behind
//!    it. The manifest makes no claim that `replay` resolves; it is a typed field, honestly
//!    unproduced, not a place content is dropped without a record.
//! 4. **`continuum-benchmark::shell` deliberately summarizes** the manifest to a count plus
//!    a named retrieval command, rather than inlining every record the way `continuum-cli`
//!    does. Its own module doc calls this "the one place the projection is genuinely lossy"
//!    and states why (a terminal is not a place to print a structured list nobody asked
//!    for). It still names what was omitted (the count) and how to retrieve it (the
//!    command), so it satisfies INV-007's text at a coarser grain — recorded here as a
//!    deliberate, documented divergence in *style*, not a gap in substance.
//! 5. **The MeterSet `undeclared`×`unmetered` cell is silent, correctly.** A dimension
//!    nobody declared and nothing meters owes no omission record — it was never part of the
//!    request's scope, so there is nothing to disclose. Stated here because it can look like
//!    a silent drop to a reader who has not seen `continuum-task::budget::dimension`'s 2×2
//!    table; it is the one cell of four that is not a disclosure obligation at all.
//!
//! # `continuumd`'s dependency shape, and why some citations are textual
//!
//! `continuumd` depends on `continuum-context` and `continuum-task` (normal and dev), so the
//! rows above that name those crates' own types are checked as *live* Rust facts elsewhere in
//! this workspace and cited here by test name. `continuum-cli` and `continuum-benchmark`
//! depend on `continuumd`, never the reverse (`tools/check_crate_boundaries.py`), so this file
//! cannot dispatch through either — the two CLI-layer rows are freshness tripwires over their
//! tracked source text ([`citations::CITATIONS`]), the same shape `bn-1eqt` used for
//! Python-governed rules this crate cannot import.

use continuum_context::expansion::{
    ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_intent::canonical_json::Json;
use continuum_value::epoch::ProtocolWindow;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::{CodecError, from_bytes, to_bytes};
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, ContextHandle, EpochIdentity, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, ExpansionRelation, OmissionReason, ResultStatus,
};

// --- the fixture: a minimal registered pack, one group reachable at depth 1, one beyond it
// (the same idiom `daemon_context_operations.rs` (bn-28jj) and `daemon_context_budget.rs`
// (bn-38p2) each build independently — every evidence file in this crate is self-contained
// rather than sharing fixtures across files). --------------------------------------------

const NOW: &str = "2026-08-01T00:00:00.000Z";
const PARENT: &str = "ctx_parent1";
const SNAPSHOT: &str = "ws_demo1";

/// A conforming parent pack. `selected[]` carries the one anchor (`e_ack`) this daemon holds
/// a group for; the group itself is registered in [`pack_record`], not read from this text —
/// `ContextPackRecord::new` derives the manifest from the *payloads*, not from the document's
/// own `omissions[]`/`expansions[]` keys, which here are illustrative content only.
const PARENT_PACK: &str = r#"{
  "assurance": {"class": "bounded", "envelope": {}},
  "content_budget": {"bytes": 16384},
  "content_hash": "blake3-256:parentplaceholder",
  "context_id": "ctx_parent1",
  "evidence": ["ev_failure1"],
  "expansions": [{"anchor": "e_ack", "relation": "source_span"}],
  "guarantees": ["ReplayPreserving", "CausallyClosed"],
  "intent": "in_ack_v1",
  "omissions": [
    {"count": 2, "expandable": true,
     "expansion": {"anchor": "e_ack", "relation": "source_span"},
     "kind": "source", "reason": "budget"}
  ],
  "parent": null,
  "question": "why did AckImpliesDurable fail?",
  "redactions": [],
  "replay": "crash_demo1",
  "schema_epoch": 1,
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "selected": [{"artifact": "ev_ack1", "id": "e_ack", "kind": "event",
                "summary": "reply published before stable write"}],
  "semantic_epoch": "sem3-r3-demo",
  "snapshot": "ws_demo1",
  "verdict": "refuted"
}"#;

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor(name: &str) -> ActorId {
    ActorId::new(name).expect("a well-formed actor identity")
}

fn capability(name: &str) -> CapabilityHandle {
    CapabilityHandle::new(name).expect("a well-formed capability handle")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
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

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-inv007-evidence".to_owned(),
        actor: actor("agent:reader"),
        capability: capability("cap_reader"),
        features: Optional::Absent,
    }
}

fn grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability("cap_reader"),
        actor: actor("agent:reader"),
        // `context.expand` is `authority read` (RFC 0027's registry).
        level: AuthorityLevel::Read,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
    }
}

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn source_item(id: &str, line: u32) -> SelectedItem {
    let span = SourceSpan::new(
        WorkspacePath::new("src/ack.rs").expect("a repo-relative path"),
        line,
        1,
        line,
        40,
    )
    .expect("a well-formed span");
    SourceRef::new(span).into_selected_item(name(id))
}

fn parent_document() -> Json {
    Json::parse(PARENT_PACK.as_bytes()).expect("the fixture is admissible canonical JSON")
}

/// The pack, and the two groups its manifest accounts for: `source_span@e_ack` (two `source`
/// items, dropped for `budget`, reachable at depth 1) and `source_span@s_1` (one further
/// `source` item, dropped as `slice-irrelevant`, reachable only at depth 2 — the group that
/// stays *outstanding*, and therefore in the manifest, on a depth-1 expansion of the first).
fn pack_record() -> ContextPackRecord {
    let near = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            2,
            PackReason::Budget,
            ExpansionQuery::new(PackRelation::SourceSpan, name("e_ack")),
        ),
        vec![source_item("s_1", 10), source_item("s_2", 20)],
    )
    .expect("two items for a count of two");
    let far = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            1,
            PackReason::SliceIrrelevant,
            ExpansionQuery::new(PackRelation::SourceSpan, name("s_1")),
        ),
        vec![source_item("s_3", 30)],
    )
    .expect("one item for a count of one");
    ContextPackRecord::new(
        parent_document(),
        WorkspaceHandle::new(SNAPSHOT).expect("a workspace handle"),
        [near, far],
    )
    .expect("a conforming pack and a well-formed expansion graph")
}

fn daemon() -> Daemon {
    let hello = hello();
    let negotiated = negotiate(
        &[
            ProtocolVersion::new(3, 0),
            ProtocolVersion::new(3, 1),
            version(),
        ],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("3.2 is served");
    let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_reader"))
        .epochs(epochs())
        .now(Timestamp::new(NOW).expect("a timestamp"))
        .capability(grant(), None)
        .family(ContextFamily)
        .build();
    daemon.state_mut().put_context_pack(
        ContextHandle::new(PARENT).expect("a context handle"),
        pack_record(),
    );
    daemon
}

fn budget(bytes: Optional<ByteCount>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(0),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes,
    }
}

fn envelope(operation: &str, request_id: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a request identity"),
        actor: actor("agent:reader"),
        capability: capability("cap_reader"),
        operation: OperationName::new(operation).expect("a declared operation"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        budget: Optional::Present(budget(Optional::Absent)),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        trace: Optional::Absent,
        arguments: continuumd::protocol::scalar::Opaque::from_bytes(b"{}".to_vec()),
        output_policy: Optional::Absent,
        page: Optional::Absent,
    }
}

fn request(anchor: &str) -> ContextExpandRequest {
    ContextExpandRequest {
        context: ContextHandle::new(PARENT).expect("a context handle"),
        anchor: anchor.to_owned(),
        relation: ExpansionRelation::SourceSpan,
        depth: Optional::Absent,
    }
}

fn expand(daemon: &mut Daemon, request_id: &str, anchor: &str) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("context.expand", request_id),
        arguments: Arguments::ContextExpand(request(anchor)),
    })
}

// --- live: the field is populated by real production code, across two different outcomes,
// from one fixture; then a mutant proves the field's own presence cannot be forged away. ---

mod wire {
    use super::*;

    /// Two real answers from one dispatch: a success whose manifest is non-empty (the
    /// `far` group is still outstanding at depth 1) and a refusal whose manifest is empty —
    /// both `Vec`s, never conditionally present. This is the compact cross-status contrast
    /// none of the per-family suites states in one place; the record-for-record agreement
    /// and the redaction/exhaustion rows are cited, not re-proved, in the module doc's map.
    #[test]
    fn a_real_success_and_a_real_refusal_each_carry_a_typed_omissions_list() {
        let mut fixture = daemon();

        let success = expand(&mut fixture, "req_ok", "e_ack");
        assert_eq!(
            success.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            success.envelope.error
        );
        assert_eq!(
            success.envelope.omissions.len(),
            1,
            "the `far` group is still outstanding one hop beyond this expansion"
        );
        let omitted = &success.envelope.omissions[0];
        assert_eq!(omitted.reason, OmissionReason::SliceIrrelevant);
        assert_eq!(omitted.subject, "selected.source");
        assert!(
            matches!(omitted.recoverable_by, Optional::Present(_)),
            "an expandable omission names how to retrieve it (INV-007's second half)"
        );

        let refused = expand(&mut fixture, "req_bad", "no-such-anchor");
        assert_eq!(refused.envelope.status, ResultStatus::Error);
        assert_eq!(
            refused.error_code(),
            Some(ErrorCode::MalformedRequest),
            "an anchor absent from the parent's own advertised anchors is malformed"
        );
        assert!(
            refused.envelope.omissions.is_empty(),
            "a refusal still carries the field — an empty Vec, never an absent one"
        );
    }

    /// The mutant. `ResultEnvelope.omissions` is `list<Omission> required` (RFC 0026: "Empty
    /// list means nothing was omitted; the field is never absent"). This proves that fact at
    /// the codec's own boundary, over a REAL answer this file's own dispatch produced: the
    /// bytes decode as sent, and once `"omissions":[...]` is textually removed — the same
    /// single-field-drop device `codec_canonical_form.rs`'s
    /// `presence_is_three_valued_in_both_directions` uses for `intent` — decoding refuses
    /// with the field named, rather than silently defaulting to an empty manifest or
    /// dropping the disagreement.
    #[test]
    fn the_omissions_field_cannot_be_dropped_from_a_real_wire_answer() {
        let mut fixture = daemon();
        let success = expand(&mut fixture, "req_ok", "e_ack");
        assert!(
            !success.envelope.omissions.is_empty(),
            "a non-trivial fixture"
        );

        let bytes = to_bytes(&success.envelope).expect("a real answer encodes");
        let round_tripped = from_bytes(&bytes).expect("a real answer decodes");
        assert_eq!(
            success.envelope, round_tripped,
            "the honest bytes round-trip before any mutation"
        );

        let text = String::from_utf8(bytes).expect("canonical JSON is UTF-8");
        let stripped = without_field(&text, "omissions");
        assert_ne!(
            stripped, text,
            "the mutation must actually remove something"
        );

        assert_eq!(
            from_bytes::<continuumd::protocol::envelope::ResultEnvelope>(stripped.as_bytes())
                .expect_err("a required field dropped from the wire must not decode"),
            CodecError::MissingField {
                declared_by: "ResultEnvelope",
                field: "omissions",
            }
        );
    }

    /// Remove `"<key>":<value>,` (or the trailing-comma-free form) from a canonical JSON
    /// object's top level, by bracket-depth matching rather than by assuming the value's
    /// shape — `omissions` is an array of objects here, but this makes no assumption about
    /// what is inside it.
    fn without_field(text: &str, key: &str) -> String {
        let needle = format!("\"{key}\":");
        let start = text.find(&needle).expect("the field is present to remove");
        let value_start = start + needle.len();
        let bytes = text.as_bytes();
        let mut depth: i32 = 0;
        let mut opened = false;
        let mut end = value_start;
        for (offset, &byte) in bytes[value_start..].iter().enumerate() {
            match byte {
                b'[' | b'{' => {
                    depth += 1;
                    opened = true;
                }
                b']' | b'}' => depth -= 1,
                _ => {}
            }
            if opened && depth == 0 {
                end = value_start + offset + 1;
                break;
            }
        }
        // Consume one trailing comma so the surrounding object stays syntactically valid —
        // `omissions` is neither the first nor the last key in ID5 sort order here.
        if bytes.get(end) == Some(&b',') {
            end += 1;
        }
        format!("{}{}", &text[..start], &text[end..])
    }
}

// --- citations: freshness tripwires over enforcement this file does not re-run -----------

mod citations {
    use std::path::Path;

    /// `(facet, path relative to the workspace root, a byte-exact marker in that file)`.
    ///
    /// Every path is a tracked source or test file; every marker was read out of that file,
    /// not typed from memory, when this table was built. A row goes stale — the cited
    /// function renamed, the cited file rewritten, the cited sentence reworded — the moment
    /// [`every_citation_still_holds`] next runs, which is the point: fix the map or fix the
    /// citation, but do not let either drift silently (bn-1eqt's device).
    const CITATIONS: &[(&str, &str, &str)] = &[
        (
            "closed five-member OmissionReason, schema order",
            "crates/continuum-context/src/omission.rs",
            "pub const ALL: [Self; 5] = [",
        ),
        (
            "INV-016 interaction: kind narrowed because free prose could reach the artifact",
            "crates/continuum-context/src/omission.rs",
            "INV-016 — a free-string `kind` is a place source-derived prose could reach the",
        ),
        (
            "closed accounting: close() is the only constructor of a ClosedAccounting",
            "crates/continuum-context/src/accounting.rs",
            "pub fn close(self) -> Result<ClosedAccounting, AccountingError> {",
        ),
        (
            "reconcile: an independent check over the two published halves",
            "crates/continuum-context/src/accounting.rs",
            "pub fn reconcile(&self) -> Result<(), AccountingError> {",
        ),
        (
            "anti-vacuity mutant: a shrunk manifest count fails reconciliation",
            "crates/continuum-context/src/accounting.rs",
            "fn a_shrunk_manifest_count_fails_reconciliation",
        ),
        (
            "anti-vacuity mutant: an inflated manifest count fails reconciliation",
            "crates/continuum-context/src/accounting.rs",
            "fn an_inflated_manifest_count_fails_reconciliation",
        ),
        (
            "anti-vacuity mutant: a reopened free-string kind is caught (F13 payment)",
            "crates/continuum-context/src/omission.rs",
            "fn a_reopened_free_string_kind_is_caught",
        ),
        (
            "expansion handles are content-derived (ID5 canonical encoding), not minted",
            "crates/continuum-context/src/expansion.rs",
            "That is also what makes an expansion handle **exact** rather than advisory: a parent's",
        ),
        (
            "the packer reserves the manifest; only items are a droppable prefix",
            "crates/continuum-context/src/budget.rs",
            "the manifest is reserved.** Every residual record survives packing",
        ),
        (
            "packer boundary: below the minimal child, nothing is published",
            "crates/continuum-context/src/budget.rs",
            "below the minimal child, nothing is published | RFC 0028's budget row, first branch | `a_ceiling_below_the_minimal_child_publishes_nothing`",
        ),
        (
            "measured content_budget.bytes is the least self-consistent fixed point",
            "crates/continuum-context/src/pack.rs",
            "it is the *least* self-consistent length | this module's fixed-point reading | `the_recorded_size_is_the_least_self_consistent_one`",
        ),
        (
            "MeterSet 2x2: declared+unmetered is a typed omission, not silence",
            "crates/continuum-task/src/budget/dimension.rs",
            "*omitted* — [`DimensionOmission`], and a charge is refused",
        ),
        (
            "a charge against a declared-but-unmetered dimension is refused, not invented",
            "crates/continuum-task/src/budget.rs",
            "fn charging_an_unmetered_dimension_is_refused_rather_than_invented",
        ),
        (
            "a declared ceiling with no meter is named, not silently passed",
            "crates/continuum-task/src/budget.rs",
            "fn a_declared_ceiling_with_no_meter_is_named_rather_than_silently_passed",
        ),
        (
            "the daemon's omission list is DERIVED from the meter set, not a hand array",
            "crates/continuumd/src/daemon/budget.rs",
            "the INV-007 omission manifest | `verification::unenforced`, an eight-name array | [`omissions_of`], derived from [`MeterSet::STATES_ONLY`]",
        ),
        (
            "context.expand's typed-outcome table: the manifest is not optional",
            "crates/continuumd/src/daemon/context.rs",
            "# What an expansion answers with, and why the manifest is not optional",
        ),
        (
            "ResultEnvelope.omissions is a required field of every result envelope",
            "crates/continuumd/src/protocol/envelope.rs",
            "omissions: list<Omission> required;",
        ),
        (
            "wire manifest agrees with the pack's own manifest, record for record",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn the_envelope_manifest_agrees_with_the_packs_manifest_record_for_record",
        ),
        (
            "the promised handle is the returned handle (exact expansion handles)",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn the_handle_the_manifest_promised_is_the_handle_the_expansion_returns",
        ),
        (
            "a purged anchor is a success carrying the redaction record, never a bare error",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn a_purged_anchor_expands_to_a_success_carrying_the_stub_and_the_redaction_record",
        ),
        (
            "depth moves items between pack and manifest and changes no total",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn depth_moves_items_between_the_pack_and_the_manifest_and_changes_no_total",
        ),
        (
            "a budget below the minimal child refuses rather than truncating",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn a_budget_below_the_minimal_child_refuses_rather_than_truncating",
        ),
        (
            "context.compile decodes and is refused with the typed reason, never a fake pack",
            "crates/continuumd/tests/daemon_context_operations.rs",
            "fn context_compile_decodes_and_is_refused_with_the_typed_reason",
        ),
        (
            "an over-budget expansion publishes a smaller child and says what the ceiling cost",
            "crates/continuumd/tests/daemon_context_budget.rs",
            "fn a_ceiling_below_the_minimal_child_publishes_nothing",
        ),
        (
            "a shortfall is recovered by the same question under a larger ceiling",
            "crates/continuumd/tests/daemon_context_budget.rs",
            "fn the_shortfall_is_recovered_by_the_same_question_under_a_larger_ceiling",
        ),
        (
            "the daemon's declared/unmetered omissions are derived from the meter set, live",
            "crates/continuumd/tests/pr6_impl02_budget_evidence.rs",
            "fn the_omission_manifest_is_derived_from_the_meter_set",
        ),
        (
            "a ceiling nothing meters is recorded and named on the answer",
            "crates/continuumd/tests/pr6_impl02_budget_evidence.rs",
            "fn a_ceiling_nothing_meters_is_recorded_and_named_on_the_answer",
        ),
        (
            "BudgetExhausted on the wire is never a silent dead end",
            "crates/continuumd/tests/pr6_impl02_budget_evidence.rs",
            "fn budget_exhausted_on_the_wire_is_never_a_silent_dead_end",
        ),
        (
            "a redacted reference names its own omission, not just a redacted stub",
            "crates/continuumd/tests/daemon_evidence.rs",
            "fn a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission",
        ),
        (
            "verifying over a redacted reference still names the redaction omission",
            "crates/continuumd/tests/daemon_evidence.rs",
            "fn verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction",
        ),
        (
            "INV-016 interaction, the wire side: anchor is closed-vocabulary equality",
            "crates/continuumd/tests/inv016_untrusted_source_evidence.rs",
            "`ContextExpandRequest.anchor` is compared for exact equality against the anchors",
        ),
        (
            "PR-11's own acceptance-criterion campaign states its honest grain up front",
            "crates/continuumd/tests/dx01_falsification.rs",
            "RFC 0028's **ten-stage compiler is not wired anywhere in this workspace**",
        ),
        (
            "the packed child is >=10x smaller under the packer's own counting rule",
            "crates/continuumd/tests/dx01_falsification.rs",
            "leg2_the_pack_is_ten_times_smaller_under_the_packers_own_counting_rule",
        ),
        (
            "the production accounting refuses every undercount on the durability dataset",
            "crates/continuumd/tests/dx01_falsification.rs",
            "a04_the_production_accounting_refuses_every_undercount_on_this_dataset",
        ),
        (
            "the promised handle is the returned handle, end to end over the campaign",
            "crates/continuumd/tests/dx01_falsification.rs",
            "leg3_the_promised_handle_is_the_returned_handle_and_the_expansion_is_exact",
        ),
        (
            "the budget branch keeps the promise under the campaign's own dataset",
            "crates/continuumd/tests/dx01_falsification.rs",
            "leg3b_the_budget_branch_keeps_the_promise",
        ),
        (
            "the CLI's manifest render is non-suppressible: no --quiet-omissions",
            "crates/continuum-cli/src/render.rs",
            "# INV-007 is non-suppressible here, on purpose",
        ),
        (
            "every format always carries the omissions key, empty array or not",
            "crates/continuum-cli/src/render.rs",
            "There is no `--quiet-omissions`, and JSON",
        ),
        (
            "explain compile renders the result envelope's own INV-007 manifest",
            "crates/continuum-cli/src/explain.rs",
            "the *result envelope's* INV-007 manifest is rendered under `omissions` by",
        ),
        (
            "the benchmark shell's one documented lossy rule: count plus expansion command",
            "crates/continuum-benchmark/src/shell.rs",
            "**List-valued fields are summarized, with an expansion command.**",
        ),
        (
            "the benchmark shell states why: omissions is a required field it chooses to summarize",
            "crates/continuum-benchmark/src/shell.rs",
            "`omissions` a **required** field of every result envelope",
        ),
    ];

    fn workspace_root() -> std::path::PathBuf {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).to_path_buf()
    }

    fn file_contains(path: &str, marker: &str) -> bool {
        let full = workspace_root().join(path);
        let text = std::fs::read_to_string(&full)
            .unwrap_or_else(|error| panic!("cited file `{path}` must be readable: {error}"));
        text.contains(marker)
    }

    #[test]
    fn every_citation_still_holds() {
        let missing: Vec<String> = CITATIONS
            .iter()
            .filter(|(_, path, marker)| !file_contains(path, marker))
            .map(|(facet, path, marker)| format!("{facet} ({path}: {marker:?})"))
            .collect();
        assert!(
            missing.is_empty(),
            "stale INV-007 citation(s) — the map or the cited file needs an update:\n{}",
            missing.join("\n")
        );
    }

    /// The lookup itself is not vacuous: a marker that cannot be in the file is reported
    /// missing, and a real row's marker is found in the real file. Without this, a bug that
    /// made [`file_contains`] always return `true` would leave [`every_citation_still_holds`]
    /// green regardless of what the cited files say.
    #[test]
    fn the_citation_check_is_not_vacuous() {
        assert!(!file_contains(
            "crates/continuum-context/src/omission.rs",
            "this exact marker does not appear in the tracked file, by construction",
        ));
        assert!(file_contains(
            "crates/continuum-context/src/omission.rs",
            "pub const ALL: [Self; 5] = [",
        ));
    }

    #[test]
    fn the_table_is_well_formed_and_cites_broadly() {
        for (facet, path, marker) in CITATIONS {
            assert!(
                !facet.is_empty(),
                "a row with no facet name hides what it proves"
            );
            assert!(path.ends_with(".rs"), "{path} is not a Rust source file");
            assert!(
                !marker.is_empty(),
                "an empty marker would match every file vacuously"
            );
        }
        assert!(
            CITATIONS.len() >= 30,
            "INV-007 is richly evidenced; a thin table would misstate that"
        );
    }
}
