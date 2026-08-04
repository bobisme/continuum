//! The byte-budget branch of `context.expand` (PR-11 / IMPL-06, bn-38p2).
//!
//! # What this file is evidence for
//!
//! `tests/daemon_context_operations.rs` (bn-28jj) covers every other row of RFC 0028's
//! typed-outcome table; this one covers the row that had only half an answer:
//!
//! > | the budget is exhausted before the expansion completes | error `BudgetExhausted` with
//! > a continuation. Either nothing is published, or a child pack is published whose manifest
//! > records the shortfall with reason `budget`; a child claiming completeness MUST NOT be
//! > published (INV-009, INV-017) |
//! >
//! > — RFC 0028, "Expansion protocol", typed outcomes
//!
//! Four claims, over the wire, through `Daemon::dispatch`:
//!
//! - **an over-budget expansion is answered, not refused, while a conforming smaller child
//!   exists.** The child selects a prefix of what the expansion found and its manifest counts
//!   the rest under `budget`, exactly — and the envelope carries the same record, so a client
//!   that never parses the pack still learns that a ceiling cost it something.
//! - **the conservation law survives the ceiling.** `|selected| + Σ manifest counts` is the
//!   same number at every ceiling that publishes anything, and the same number as with no
//!   ceiling at all: packing moves an item from one half of the answer to the other and
//!   creates or destroys nothing (INV-007).
//! - **the branch flips at an exact boundary.** Below the smallest conforming child — the
//!   complete manifest beside an empty selection — nothing is published and the caller is
//!   refused `BudgetExhausted` with a typed `non_resumable_reason` (SD-13). The manifest is
//!   never what a budget squeezes out.
//! - **`content_budget.bytes` is the child's own measured size** (RFC 0028 correction 17),
//!   not the ceiling and not the parent's inherited number, and it is checkable against the
//!   bytes the daemon actually sent.
//!
//! # The fixture, and why it is a second one
//!
//! bn-28jj's fixture holds two items behind its anchor, which is too few to pack: dropping
//! one item saves less than the omission record that names it costs. This file registers its
//! own pack with a **twelve**-item group, so there is a range of ceilings between "everything
//! fits" and "nothing conforms" for the packer to be observed inside. Nothing in bn-28jj's
//! file moves; its own budget test still pins the refusal branch, now at the boundary this
//! file measures.
//!
//! # OOM hygiene
//!
//! The parent pack is a `const` string; the twelve items are built by one linear `map` over a
//! range, never by growing text. The largest value here is a 3 KiB JSON document.

use continuum_context::expansion::{
    ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::pack;
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_intent::canonical_json::Json;
use continuum_value::epoch::ProtocolWindow;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::family::{Arguments, Payload};
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
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, ExpansionRelation, OmissionReason, ResultStatus,
};

const NOW: &str = "2026-08-01T00:00:00.000Z";
const PARENT: &str = "ctx_wide001";
const SNAPSHOT: &str = "ws_demo1";
/// How many `source` items the one wide group behind `e_ack` holds.
const WIDE: u32 = 12;

/// A conforming parent whose manifest advertises the wide group this file expands.
const PARENT_PACK: &str = r#"{
  "assurance": {"class": "bounded", "envelope": {}},
  "content_budget": {"bytes": 16384},
  "content_hash": "blake3-256:parentplaceholder",
  "context_id": "ctx_wide001",
  "evidence": ["ev_failure1"],
  "expansions": [{"anchor": "e_ack", "relation": "source_span"}],
  "guarantees": ["ReplayPreserving", "CausallyClosed"],
  "intent": "in_ack_v1",
  "omissions": [
    {"count": 12, "expandable": true,
     "expansion": {"anchor": "e_ack", "relation": "source_span"},
     "kind": "source", "reason": "budget"}
  ],
  "parent": null,
  "question": "why did AckImpliesDurable fail?",
  "redactions": [{"redacted": true, "reason": "purged",
                  "commitment": "blake3-256:purged1", "original_class": "cir"}],
  "replay": "crash_demo1",
  "schema_epoch": 1,
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "selected": [{"artifact": "ev_ack1", "id": "e_ack", "kind": "event",
                "summary": "reply published before stable write"}],
  "semantic_epoch": "sem3-r3-demo",
  "snapshot": "ws_demo1",
  "verdict": "refuted"
}"#;

// --- the fixture ---------------------------------------------------------------------

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
        client: "continuumd-context-budget-evidence".to_owned(),
        actor: actor("agent:reader"),
        capability: capability("cap_reader"),
        features: Optional::Absent,
    }
}

fn grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability("cap_reader"),
        actor: actor("agent:reader"),
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

/// The pack and its two groups:
///
/// - `source_span@e_ack` → [`WIDE`] `source` items dropped for budget — the group a ceiling
///   is observed cutting into;
/// - `source_span@s_0000` → one further `source` item, slice-irrelevant, reachable only at
///   depth 2, so a depth-1 answer always carries a residual record no ceiling may touch.
fn pack_record() -> ContextPackRecord {
    let wide = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            WIDE,
            PackReason::Budget,
            ExpansionQuery::new(PackRelation::SourceSpan, name("e_ack")),
        ),
        (0..WIDE)
            .map(|index| source_item(&format!("s_{index:04}"), index + 1))
            .collect(),
    )
    .expect("twelve items for a count of twelve");
    let far = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            1,
            PackReason::SliceIrrelevant,
            ExpansionQuery::new(PackRelation::SourceSpan, name("s_0000")),
        ),
        vec![source_item("s_9999", 99)],
    )
    .expect("one item for a count of one");
    ContextPackRecord::new(
        parent_document(),
        WorkspaceHandle::new(SNAPSHOT).expect("a workspace handle"),
        [wide, far],
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

fn envelope(request_id: &str, ceiling: Optional<ByteCount>) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a request identity"),
        actor: actor("agent:reader"),
        capability: capability("cap_reader"),
        operation: OperationName::new("context.expand").expect("a declared operation"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        budget: Optional::Present(budget(ceiling)),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        trace: Optional::Absent,
        arguments: continuumd::protocol::scalar::Opaque::from_bytes(b"{}".to_vec()),
        output_policy: Optional::Absent,
        page: Optional::Absent,
    }
}

/// Expand `source_span@e_ack` at depth 1 under `ceiling`.
fn expand(daemon: &mut Daemon, request_id: &str, ceiling: Optional<ByteCount>) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope(request_id, ceiling),
        arguments: Arguments::ContextExpand(ContextExpandRequest {
            context: ContextHandle::new(PARENT).expect("a context handle"),
            anchor: "e_ack".to_owned(),
            relation: ExpansionRelation::SourceSpan,
            depth: Optional::Absent,
        }),
    })
}

fn under(daemon: &mut Daemon, request_id: &str, ceiling: u64) -> OperationOutcome {
    expand(
        daemon,
        request_id,
        Optional::Present(ByteCount::new(ceiling)),
    )
}

fn child_pack(outcome: &OperationOutcome) -> Json {
    let Payload::ContextExpand(response) = &outcome.payload else {
        panic!("expected a context.expand payload");
    };
    Json::parse(response.pack.as_bytes()).expect("the pack is canonical JSON")
}

fn pack_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let Payload::ContextExpand(response) = &outcome.payload else {
        panic!("expected a context.expand payload");
    };
    response.pack.as_bytes().to_vec()
}

fn array<'a>(document: &'a Json, key: &str) -> &'a [Json] {
    document.as_object().expect("object")[key]
        .as_array()
        .expect("array")
}

fn selected_ids(document: &Json) -> Vec<String> {
    array(document, "selected")
        .iter()
        .map(|item| {
            item.as_object().expect("object")["id"]
                .as_str()
                .expect("string")
                .to_owned()
        })
        .collect()
}

/// `|selected| + Σ manifest counts`, read off a child pack alone.
fn accounted(document: &Json) -> i64 {
    let selected = i64::try_from(array(document, "selected").len()).expect("small");
    let omitted: i64 = array(document, "omissions")
        .iter()
        .map(|record| {
            record.as_object().expect("object")["count"]
                .as_integer()
                .expect("an exact integer count")
        })
        .sum();
    selected + omitted
}

/// The `budget` record of a child's manifest, if the ceiling cost it anything.
fn shortfall(document: &Json) -> Option<&Json> {
    array(document, "omissions").iter().find(|record| {
        record.as_object().expect("object")["reason"] == Json::String("budget".into())
    })
}

/// What an unconstrained expansion of the same question measures — every test's yardstick.
fn unconstrained(daemon: &mut Daemon, request_id: &str) -> u64 {
    let outcome = expand(daemon, request_id, Optional::Absent);
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let document = child_pack(&outcome);
    let measured = pack::budget_bytes_of(&document).expect("a measured child");
    // The parent's own `content_budget.bytes` (16384) is the ceiling this call ran under, so
    // "the whole answer fits" is a real fact about this fixture and not an absent check.
    assert!(measured < 16384);
    assert_eq!(array(&document, "selected").len(), WIDE as usize);
    measured
}

// --- the second branch: a smaller child, and what it says ------------------------------

#[test]
fn an_over_budget_expansion_publishes_a_smaller_child_and_says_what_the_ceiling_cost() {
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    let outcome = under(&mut daemon, "req_packed", whole - 400);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "a ceiling a smaller child fits under is answered, not refused: {:?}",
        outcome.envelope.error
    );
    let child = child_pack(&outcome);
    let kept = array(&child, "selected").len();
    assert!(kept < WIDE as usize, "the ceiling cost the answer nothing");
    assert!(
        kept > 0,
        "a ceiling this size still admits some of the answer"
    );

    // The shortfall, in the child's own manifest: exact, `budget`, expandable, and naming the
    // query that retrieves it.
    let record = shortfall(&child).expect("a packed child records what it dropped");
    let fields = record.as_object().expect("object");
    assert_eq!(fields["kind"], Json::String("source".to_owned()));
    assert_eq!(
        fields["count"],
        Json::Integer(i64::from(WIDE) - kept as i64)
    );
    assert_eq!(fields["expandable"], Json::Bool(true));
    assert_eq!(
        fields["expansion"],
        ExpansionQuery::new(PackRelation::SourceSpan, name("e_ack")).to_json()
    );

    // And on the envelope, which is where a client that does not parse the pack sees it.
    let wire = outcome
        .envelope
        .omissions
        .iter()
        .find(|omission| omission.reason == OmissionReason::Budget)
        .expect("the wire projection carries the shortfall too");
    assert_eq!(wire.subject, "selected.source");
    assert!(
        !wire.recoverable_by.is_absent(),
        "INV-007: a budget omission names how it is retrieved"
    );

    // The child is still a pack, and still claims nothing it did not check.
    pack::required_keys_present(&child).expect("the packed child carries the required keys");
    assert_eq!(
        child.as_object().expect("object")["guarantees"],
        Json::Array(Vec::new())
    );
    assert_eq!(
        child.as_object().expect("object")["redactions"],
        parent_document().as_object().expect("object")["redactions"],
        "packing does not touch what the child inherits"
    );
}

#[test]
fn the_wire_projection_agrees_with_the_packed_manifest_record_for_record() {
    // "The result envelope's `omissions` list […] is the wire projection of the same facts
    // and MUST agree with the pack's manifest record for record" — RFC 0028. The packed
    // manifest is the one that has to agree, not the pre-packing one.
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    for (index, ceiling) in [whole, whole - 300, whole - 600].into_iter().enumerate() {
        let outcome = under(&mut daemon, &format!("req_{index}"), ceiling);
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
        let child = child_pack(&outcome);
        let records = array(&child, "omissions");
        assert_eq!(
            records.len(),
            outcome.envelope.omissions.len(),
            "a ceiling of {ceiling}: one envelope omission per manifest record"
        );
        for (record, wire) in records.iter().zip(&outcome.envelope.omissions) {
            let object = record.as_object().expect("object");
            assert_eq!(
                object["reason"],
                Json::String(wire.reason.as_wire().to_owned())
            );
            assert_eq!(
                wire.subject,
                format!(
                    "selected.{}",
                    object["kind"].as_str().expect("a kind token")
                )
            );
        }
    }
}

#[test]
fn the_conservation_law_survives_the_ceiling() {
    // INV-007 over the wire, under a budget: thirteen candidates — the twelve behind `e_ack`
    // and the one behind `s_0000` — are accounted for at every ceiling that publishes.
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    let mut packed_seen = 0;
    for (index, ceiling) in (whole - 900..=whole).step_by(60).enumerate() {
        let outcome = under(&mut daemon, &format!("req_{index}"), ceiling);
        if outcome.envelope.status != ResultStatus::Ok {
            continue;
        }
        let child = child_pack(&outcome);
        assert_eq!(
            accounted(&child),
            i64::from(WIDE) + 1,
            "a ceiling of {ceiling} lost or invented a candidate"
        );
        if array(&child, "selected").len() < WIDE as usize {
            packed_seen += 1;
        }
        // The residual group is outstanding whatever the ceiling: a manifest is never what a
        // budget squeezes out.
        assert!(
            array(&child, "omissions").iter().any(|record| {
                record.as_object().expect("object")["reason"]
                    == Json::String("slice-irrelevant".to_owned())
            }),
            "a ceiling of {ceiling} dropped a manifest record"
        );
    }
    assert!(
        packed_seen > 3,
        "the sweep must actually exercise packing: {packed_seen}"
    );
}

#[test]
fn a_larger_ceiling_extends_the_same_prefix_over_the_wire() {
    // RFC 0028's prefix monotonicity and its frontier order, between two answers to one
    // question: the smaller selection is a prefix of the larger, and the larger omits less.
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    let narrow = child_pack(&under(&mut daemon, "req_narrow", whole - 700));
    let wide = child_pack(&under(&mut daemon, "req_wide", whole - 300));

    let narrow_ids = selected_ids(&narrow);
    let wide_ids = selected_ids(&wide);
    assert!(
        narrow_ids.len() < wide_ids.len(),
        "the sweep is not vacuous"
    );
    assert_eq!(
        narrow_ids.as_slice(),
        &wide_ids[..narrow_ids.len()],
        "a larger ceiling extends the packed prefix; it never reorders it"
    );
    let count = |document: &Json| -> i64 {
        array(document, "omissions")
            .iter()
            .map(|record| {
                record.as_object().expect("object")["count"]
                    .as_integer()
                    .expect("exact")
            })
            .sum()
    };
    assert!(count(&wide) < count(&narrow), "a larger ceiling omits less");
}

#[test]
fn the_shortfall_is_recovered_by_the_same_question_under_a_larger_ceiling() {
    // The consequence of "budget is not key material", stated as a test rather than left for
    // a reader to discover: one question is one `ctx_*` whatever the ceiling, so the handle a
    // budget omission is recoverable by is the child's own — and the two documents behind it
    // are frontier-ordered rather than conflicting (INV-009).
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    let packed = under(&mut daemon, "req_packed", whole - 500);
    let child = child_pack(&packed);
    let Payload::ContextExpand(response) = &packed.payload else {
        panic!("a payload");
    };
    let promised = packed
        .envelope
        .omissions
        .iter()
        .find(|omission| omission.reason == OmissionReason::Budget)
        .expect("the shortfall")
        .recoverable_by
        .value()
        .cloned()
        .expect("an expandable record names the pack that retrieves it");
    assert_eq!(
        promised.as_str(),
        response.context.as_str(),
        "the question that retrieves the shortfall is the one this child answers"
    );

    // Asking it again with room returns that same `ctx_*` and a strictly fuller answer.
    let full = under(&mut daemon, "req_full", whole);
    let Payload::ContextExpand(fuller) = &full.payload else {
        panic!("a payload");
    };
    assert_eq!(fuller.context.as_str(), promised.as_str());
    let complete = child_pack(&full);
    assert_ne!(
        pack_bytes(&packed),
        pack_bytes(&full),
        "two budgets, two documents — RFC 0028's determinism clause is per (key, budget)"
    );
    let narrow_ids = selected_ids(&child);
    let full_ids = selected_ids(&complete);
    assert_eq!(narrow_ids.as_slice(), &full_ids[..narrow_ids.len()]);
    assert!(shortfall(&child).is_some());
    assert!(
        shortfall(&complete).is_none(),
        "with room, nothing is dropped for budget"
    );
}

// --- the first branch: the boundary where nothing is published --------------------------

#[test]
fn a_ceiling_below_the_minimal_child_publishes_nothing() {
    // The boundary is the smallest conforming child — the complete manifest beside an empty
    // selection — and it is measured here rather than assumed: at that size the answer is a
    // pack with no items, and one byte below it there is no answer at all.
    let mut daemon = daemon();
    let minimal = {
        let outcome = under(&mut daemon, "req_floor", 1);
        let error = outcome
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object");
        assert_eq!(error.code, ErrorCode::BudgetExhausted);
        assert!(
            !error.non_resumable_reason.is_absent(),
            "SD-13: a BudgetExhausted carries a continuation or a typed non-resumable reason"
        );
        assert!(
            matches!(outcome.payload, Payload::None),
            "nothing published"
        );
        // Binary search for the boundary over the *published* answers, so the number below is
        // the daemon's own and not this test's arithmetic.
        let mut low = 1;
        let mut high = 4096;
        while low < high {
            let middle = low + (high - low) / 2;
            let probe = under(&mut daemon, &format!("req_probe_{middle}"), middle);
            if probe.envelope.status == ResultStatus::Ok {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    };

    let at_the_boundary = under(&mut daemon, "req_boundary", minimal);
    assert_eq!(at_the_boundary.envelope.status, ResultStatus::Ok);
    let child = child_pack(&at_the_boundary);
    assert!(
        array(&child, "selected").is_empty(),
        "the minimal child selects nothing"
    );
    assert_eq!(
        accounted(&child),
        i64::from(WIDE) + 1,
        "and still accounts for every candidate"
    );
    assert_eq!(
        pack::budget_bytes_of(&child).expect("measured"),
        minimal,
        "the boundary is the minimal child's own measured size"
    );

    let below = under(&mut daemon, "req_below", minimal - 1);
    assert_eq!(
        below
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object")
            .code,
        ErrorCode::BudgetExhausted
    );
    assert!(matches!(below.payload, Payload::None));
}

// --- the measurement --------------------------------------------------------------------

#[test]
fn the_child_records_its_own_measured_size_and_never_the_ceiling() {
    // RFC 0028 correction 17. The number in the pack is checkable against the bytes the
    // daemon sent, and it is neither the ceiling in force nor the parent's own value — the
    // two readings the interim placeholder used to write.
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    for (index, ceiling) in [
        Optional::Absent,
        Optional::Present(ByteCount::new(whole - 450)),
    ]
    .into_iter()
    .enumerate()
    {
        let outcome = expand(&mut daemon, &format!("req_{index}"), ceiling);
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
        let child = child_pack(&outcome);
        let recorded = pack::budget_bytes_of(&child).expect("a measured child");
        assert_eq!(
            recorded,
            pack_bytes(&outcome).len() as u64,
            "the recorded size is the size of the document that records it"
        );
        assert_ne!(recorded, 16384, "the parent's number is not the child's");
        if let Optional::Present(stated) = &ceiling {
            assert_ne!(recorded, stated.bytes(), "the ceiling is not the record");
            assert!(recorded <= stated.bytes());
        }
    }
}

#[test]
fn one_question_and_one_ceiling_are_one_pack_whatever_the_idempotency_key_says() {
    // The idempotency rule is untouched by packing: two calls under two different keys and
    // one ceiling return one `ctx_*` and byte-identical packs, because the identity is
    // derived from the question and the packing rule is a function of its inputs.
    let mut daemon = daemon();
    let whole = unconstrained(&mut daemon, "req_whole");

    let first = under(&mut daemon, "req_1", whole - 350);
    let second = under(&mut daemon, "req_2", whole - 350);
    assert_ne!(first.envelope.request_id, second.envelope.request_id);
    assert_eq!(pack_bytes(&first), pack_bytes(&second));
    let (Payload::ContextExpand(one), Payload::ContextExpand(two)) =
        (&first.payload, &second.payload)
    else {
        panic!("two payloads");
    };
    assert_eq!(one.context, two.context);
    // Not vacuous: a different ceiling is the same handle and different bytes.
    let third = under(&mut daemon, "req_3", whole - 700);
    let Payload::ContextExpand(other) = &third.payload else {
        panic!("a payload");
    };
    assert_eq!(one.context, other.context);
    assert_ne!(pack_bytes(&first), pack_bytes(&third));
}
