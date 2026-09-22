//! G2-05 acceptance — **independent re-derivation** of the criterion's first two conjuncts
//! (bn-1iljt).
//!
//! > Context Packs are bounded, carry omission manifests and expansion handles, and improve
//! > agent benchmark effectiveness
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:33`, gate G2, criterion **G2-05**
//!
//! # What this file is, and what it deliberately is not
//!
//! It is **not** a re-run of the delivering suites. `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md`
//! re-ran those from clean state and read their results, and says so itself: "That is a
//! different and weaker thing than an independent re-derivation." This file is the
//! re-derivation. It shares no fixture with `pr11_exit_evidence.rs`, `daemon_context_budget.rs`,
//! `daemon_context_operations.rs`, `inv007_omission_transparency_evidence.rs` or
//! `pr11_impl06_byte_and_token_budgets.rs`, and it re-computes every predicate it asserts from
//! the *input projection* rather than reading it back off the compiler's own ledger.
//!
//! Three method differences carry the independence:
//!
//! 1. **The compiler is driven at adversarial scale, through `context.compile`.** The landed
//!    suites drive `context.expand` over hand-written `const` parent packs (twelve items), or
//!    `context.compile` over a four-node order. This file registers a [`DEPTH`]-deep causal
//!    chain with a [`FANOUT`]-wide successor fan-out and oversized item bodies — 384 candidates
//!    — and compiles it live.
//! 2. **Every number is measured externally.** Pack size is `response.pack.as_bytes().len()`,
//!    the bytes the daemon actually sent, never `content_budget.bytes`. The pack's own
//!    self-report is then checked *against* that measurement rather than trusted as it
//!    ([`the_recorded_size_is_confirmed_by_an_external_measurement`]).
//! 3. **The manifest is cross-checked against ground truth this file computes.** [`ancestry`]
//!    is a private backward walk over the raw edge list, written here, never
//!    `CausalOrder::backward_closure`. The selection, the omitted set, and the counting equation
//!    are all checked against *that* walk, so a compiler and a checker that agreed with each
//!    other but not with the graph would still fail.
//!
//! # The bound, and its normative source
//!
//! "Bounded" has two normative readings and this file separates them, because only one of them
//! is enforced today:
//!
//! - **The structural bound.** "A Context Pack is a bounded, typed, property-directed
//!   compilation of the evidence graph for one question" (RFC 0028, "Summary"); everything else
//!   travels in the manifest and is reached "by expansion, never by dumping". This is a bound
//!   *relative to the question*: the pack holds the causal closure of the roots and nothing
//!   else. [`the_root_pack_holds_the_closure_of_the_question_and_nothing_else`] measures it.
//! - **The byte ceiling.** "Bytes are the enforced contract" (RFC 0028, "Budgets and packing");
//!   the wire spelling is `Budget.bytes` and `OutputPolicy.max_bytes` ("Enforced byte ceiling
//!   on the result payload", `notes/plan/schemas/continuumd-native-protocol.idl`), and
//!   `content_budget.bytes` is the measured spend, not the ceiling (RFC 0028 correction 17).
//!   [`the_expansion_ceiling_is_exact_at_a_boundary_this_file_derives_independently`] probes it
//!   at, one under, and one over — on both boundaries the RFC names.
//!
//! **The gap this re-derivation found, and its repair.** The byte ceiling was first enforced on
//! `context.expand` only: `context.compile` read no ceiling, its handler never consulted
//! `envelope.budget.bytes` or `envelope.output_policy.max_bytes`, and the IDL's declared
//! `BudgetExhausted` was a branch no code took. bn-2ga1c paid it. The compile now refuses a
//! root over any stated ceiling with `BudgetExhausted` and publishes nothing — RFC 0028's
//! "A guaranteed core is never truncated" branch, since a root's selection is its
//! `CausallyClosed` core — and records `OutputPolicy.max_nodes` at `content_budget.nodes`.
//! [`the_compile_budget_bytes_ceiling_is_exact_at_the_whole_root`],
//! [`the_compile_max_bytes_ceiling_is_exact_at_the_whole_payload`] and
//! [`the_compile_node_ceiling_is_recorded_and_exact_at_the_selection`] are the regression
//! guards, each at the limit and one over it.
//!
//! # Conjunct 3 is not probed here, and the reason is not an opinion
//!
//! "improve agent benchmark effectiveness" has **no instrument anywhere in the tree** — unbuilt,
//! which is a different fact from measured-and-failed.
//! [`the_benchmark_effectiveness_instrument_does_not_exist`] is a freshness tripwire that
//! establishes the absence mechanically and goes red the moment an ablation arm lands, so the
//! debt cannot be silently paid or silently forgotten. Building the ablation is out of this
//! bone's scope; the instrument specification it would need is recorded on `bn-1iljt`.
//!
//! # Verdicts this file supports
//!
//! | Conjunct | Verdict | Evidence here |
//! |---|---|---|
//! | (1) bounded — structural | **SUPPORTED** | [`the_root_pack_holds_the_closure_of_the_question_and_nothing_else`] |
//! | (1) bounded — byte ceiling on `context.expand` | **SUPPORTED** | [`the_expansion_ceiling_is_exact_at_a_boundary_this_file_derives_independently`] |
//! | (1) bounded — byte and node ceilings on `context.compile` | **SUPPORTED** (bn-2ga1c; refusal branch, no root packer) | [`the_compile_budget_bytes_ceiling_is_exact_at_the_whole_root`], [`the_compile_max_bytes_ceiling_is_exact_at_the_whole_payload`], [`the_compile_node_ceiling_is_recorded_and_exact_at_the_selection`] |
//! | (2) omission manifests | **SUPPORTED** | [`the_manifest_enumerates_exactly_what_the_question_left_out`] |
//! | (2) expansion handles | **SUPPORTED** | [`every_omitted_candidate_comes_back_through_the_handle_the_manifest_named`] |
//! | negative controls | all fire | [`the_cross_checks_fail_on_artifacts_this_file_doctors`], [`a_handle_that_should_not_resolve_does_not`] |
//! | (3) benchmark effectiveness | **UNSUPPORTED (instrument unbuilt)** | [`the_benchmark_effectiveness_instrument_does_not_exist`] |
//!
//! # OOM and runtime hygiene
//!
//! The order is 384 nodes and the largest document built is about 40 KiB. Item bodies are grown
//! by one `repeat`, never by concatenation in a loop, and no test binary-searches the daemon:
//! the byte boundary is *predicted* from the vocabulary types and then confirmed with four
//! calls.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use continuum_context::assurance::Assurance;
use continuum_context::causal::CausalOrder;
use continuum_context::compile::{CausalCompile, RedactionPolicy};
use continuum_context::expansion::{
    Depth, ExpansionHandle, ExpansionQuery, ExpansionQuestion, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{Manifest, OmissionReason as PackReason, OmissionRecord};
use continuum_context::pack::{self, ChildPack, PackProfile};
use continuum_context::replay::ReplayRef;
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::state_delta::{StateDeltaClass, StateDeltaRef};
use continuum_context::verdict::Verdict as PackVerdict;
use continuum_intent::canonical_json::Json;
use continuum_value::assurance::{AssuranceEnvelope, AssuranceLevel, UnsupportedReason};
use continuum_value::epoch::ProtocolWindow;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_value::value::{Name, Value};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle as StoreHandle};
use continuumd::daemon::context::{CompileHeader, ContextCompileSource, ContextFamily};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, OutputPolicy, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::{ContextCompileRequest, ContextExpandRequest};
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, ByteCount, CapabilityHandle, ContextHandle, EpochIdentity,
    OperationName, ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, ExpansionRelation, ResultStatus,
};

// --- the adversarial projection ----------------------------------------------------------

/// How deep the causal chain is. Stage 2's backward closure has to traverse all of it, so a
/// truncating slicer produces a selection this file's own walk disagrees with.
const DEPTH: u32 = 128;

/// How wide the successor fan-out is. None of these are ancestors of the root, so every one of
/// them is a candidate the manifest must name and an expansion must return.
const FANOUT: u32 = 256;

/// How many bytes an oversized item body carries.
const OVERSIZED: usize = 2048;

/// Which chain nodes carry an oversized body — enough to make the root pack far larger than any
/// ceiling a caller could plausibly state.
const BIG_CHAIN: [u32; 4] = [0, 41, 83, DEPTH - 1];

/// How many fan-out nodes carry an oversized body, so a ceiling that cuts through the group cuts
/// through items of very different sizes and the packer's prefix scan is not exercised on a
/// uniform list.
const BIG_FANOUT: u32 = 12;

const NOW: &str = "2026-08-11T00:00:00.000Z";
const SNAPSHOT: &str = "ws_g2o5probe";
const SEMANTIC_EPOCH: &str = "sem3-g2-05-acceptance";
const QUESTION: &str = "which writes precede the acknowledged commit?";

fn id(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn chain_id(index: u32) -> String {
    format!("c_{index:04}")
}

fn fanout_id(index: u32) -> String {
    format!("n_{index:04}")
}

/// The root the question names: the tip of the chain.
fn root_id() -> String {
    chain_id(DEPTH - 1)
}

/// The raw edge list, `(node, immediate predecessor)`.
///
/// Declared once and consumed twice — by [`CausalOrder`], which is the compiler's premise, and by
/// [`ancestry`], which is this file's own independent walk. Two consumers of one declaration is
/// what makes the cross-check a check rather than a restatement.
fn edges() -> Vec<(String, String)> {
    let mut out = Vec::with_capacity((DEPTH - 1 + FANOUT) as usize);
    for index in 1..DEPTH {
        out.push((chain_id(index), chain_id(index - 1)));
    }
    for index in 0..FANOUT {
        out.push((fanout_id(index), root_id()));
    }
    out
}

fn nodes() -> Vec<String> {
    (0..DEPTH)
        .map(chain_id)
        .chain((0..FANOUT).map(fanout_id))
        .collect()
}

/// **This file's own** backward closure: `root` plus every transitive predecessor, computed from
/// the raw edge list by a worklist that shares no code with `continuum-context`.
///
/// The ground truth every selection assertion below is stated against.
fn ancestry(root: &str) -> BTreeSet<String> {
    let mut predecessors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (node, predecessor) in edges() {
        predecessors.entry(node).or_default().push(predecessor);
    }
    let mut closed: BTreeSet<String> = BTreeSet::new();
    let mut frontier: Vec<String> = vec![root.to_owned()];
    while let Some(node) = frontier.pop() {
        if !closed.insert(node.clone()) {
            continue;
        }
        if let Some(preds) = predecessors.get(&node) {
            frontier.extend(preds.iter().cloned());
        }
    }
    closed
}

/// One candidate's body: a concrete state delta whose summary is small, or oversized where the
/// probe wants weight.
fn body(name: &str, oversized: bool) -> SelectedItem {
    let after = if oversized {
        Value::text("w".repeat(OVERSIZED))
    } else {
        Value::int(1)
    };
    StateDeltaRef::new(
        id(&format!("v{name}")),
        StateDeltaClass::Concrete,
        Some(Value::int(0)),
        after,
    )
    .expect("a changed variable is a real delta")
    .into_selected_item(id(name))
}

fn bodies() -> Vec<SelectedItem> {
    let mut out = Vec::with_capacity(nodes().len());
    for index in 0..DEPTH {
        out.push(body(&chain_id(index), BIG_CHAIN.contains(&index)));
    }
    for index in 0..FANOUT {
        out.push(body(&fanout_id(index), index < BIG_FANOUT));
    }
    out
}

/// The expansion query the compile leaves its residual behind: the fan-out is exactly the set of
/// causal successors of the root, and the root is selected, so the anchor resolves in the
/// published pack ("expansion is navigation over a published pack" — RFC 0028).
fn residual() -> ExpansionQuery {
    ExpansionQuery::new(PackRelation::CausalSuccessors, id(&root_id()))
}

fn digest(text: &str) -> String {
    Blake3Hasher::hash(text.as_bytes()).to_token()
}

fn projection() -> (ContextCompileSource, ArtifactHandle) {
    let order = CausalOrder::new(
        nodes().into_iter().map(|node| {
            // One kind throughout, so the manifest partitions into a single cell and the
            // counting equation is checkable against a number this file knows exactly.
            (id(&node), SelectionKind::StateDelta)
        }),
        edges()
            .into_iter()
            .map(|(node, predecessor)| (id(&node), id(&predecessor))),
    )
    .expect("a chain plus a fan-out is a DAG");
    let header = CompileHeader {
        snapshot: WorkspaceHandle::new(SNAPSHOT).expect("a workspace handle"),
        semantic_epoch: SEMANTIC_EPOCH.to_owned(),
        intent: StoreHandle::new(ArtifactClass::IntentContract, &digest("g2-05-intent"))
            .expect("a digest token is a well-formed identity"),
        evidence: vec![
            StoreHandle::new(ArtifactClass::Evidence, &digest("g2-05-evidence"))
                .expect("a digest token is a well-formed identity"),
        ],
        replay: Some(
            ReplayRef::new(
                StoreHandle::new(ArtifactClass::Crashpack, &digest("g2-05-crashpack"))
                    .expect("a digest token is a well-formed identity"),
            )
            .expect("a crash_* handle"),
        ),
        verdict: Some(PackVerdict::Refuted),
        assurance: Assurance::new(
            AssuranceLevel::Bounded,
            AssuranceEnvelope::all_unsupported(
                &UnsupportedReason::new("outside-this-acceptance-probe").expect("a plain token"),
            ),
        ),
        profile: PackProfile::Failure,
        redactions: Vec::new(),
    };
    let root = ArtifactHandle::new(&header.evidence[0].to_string()).expect("an artifact handle");
    let source = ContextCompileSource::new(
        CausalCompile::new(order, RedactionPolicy::permitting_everything()),
        [id(&root_id())],
        residual(),
        bodies(),
        header,
    )
    .expect("every candidate of the order has a registered body");
    (source, root)
}

// --- the live daemon ----------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor() -> ActorId {
    ActorId::new("agent:reader").expect("a well-formed actor identity")
}

fn capability() -> CapabilityHandle {
    CapabilityHandle::new("cap_reader").expect("a well-formed capability handle")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(EpochIdentity::new("semantic-1").expect("an epoch")),
        intent: Nullable::Value(EpochIdentity::new("intent-1").expect("an epoch")),
        evidence: Nullable::Null,
        proof: Nullable::Value(EpochIdentity::new("proof-1").expect("an epoch")),
        corpus: Nullable::Null,
        engine: Nullable::Value(EpochIdentity::new("engine-reference-1").expect("an epoch")),
    }
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g2-05-acceptance".to_owned(),
        actor: actor(),
        capability: capability(),
        features: Optional::Absent,
    }
}

fn grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability(),
        actor: actor(),
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

/// A daemon with the adversarial projection registered, and the evidence root that names it.
fn daemon() -> (Daemon, ArtifactHandle) {
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
    let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability())
        .epochs(epochs())
        .now(Timestamp::new(NOW).expect("a timestamp"))
        .capability(grant(), None)
        .family(ContextFamily)
        .build();
    let (source, root) = projection();
    daemon.state_mut().put_compile_source(&root, source);
    (daemon, root)
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

fn envelope(
    operation: &str,
    request_id: &str,
    bytes: Optional<ByteCount>,
    policy: Optional<OutputPolicy>,
) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a request identity"),
        actor: actor(),
        capability: capability(),
        operation: OperationName::new(operation).expect("a declared operation"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        budget: Optional::Present(budget(bytes)),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        trace: Optional::Absent,
        arguments: continuumd::protocol::scalar::Opaque::from_bytes(b"{}".to_vec()),
        output_policy: policy,
        page: Optional::Absent,
    }
}

fn compile(
    daemon: &mut Daemon,
    root: &ArtifactHandle,
    request_id: &str,
    bytes: Optional<ByteCount>,
    policy: Optional<OutputPolicy>,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("context.compile", request_id, bytes, policy),
        arguments: Arguments::ContextCompile(ContextCompileRequest {
            evidence_root: root.clone(),
            question: QUESTION.to_owned(),
            audience: Optional::Absent,
            // No guarantee is requested, so rule C1 owes no record and the manifest
            // partitions into exactly the cell this file's ground truth predicts.
            guarantees: Optional::Absent,
        }),
    })
}

fn expand(
    daemon: &mut Daemon,
    context: &ContextHandle,
    request_id: &str,
    anchor: &str,
    relation: ExpansionRelation,
    ceiling: Optional<ByteCount>,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope("context.expand", request_id, ceiling, Optional::Absent),
        arguments: Arguments::ContextExpand(ContextExpandRequest {
            context: context.clone(),
            anchor: anchor.to_owned(),
            relation,
            depth: Optional::Absent,
        }),
    })
}

// --- reading an answer, always by external measurement --------------------------------------

/// The bytes the daemon actually sent, and the document parsed from exactly those bytes.
fn answered(outcome: &OperationOutcome) -> (Vec<u8>, ContextHandle, Json) {
    let (handle, bytes) = match &outcome.payload {
        Payload::ContextCompile(response) => {
            (response.context.clone(), response.pack.as_bytes().to_vec())
        }
        Payload::ContextExpand(response) => {
            (response.context.clone(), response.pack.as_bytes().to_vec())
        }
        other => panic!("expected a context payload, got {other:?}"),
    };
    let document = Json::parse(&bytes).expect("the pack is canonical JSON");
    (bytes, handle, document)
}

fn array<'a>(document: &'a Json, key: &str) -> &'a [Json] {
    document.as_object().expect("object")[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is an array"))
}

fn text<'a>(value: &'a Json, key: &str) -> &'a str {
    value.as_object().expect("object")[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` is a string"))
}

fn count(record: &Json) -> u64 {
    u64::try_from(
        record.as_object().expect("object")["count"]
            .as_integer()
            .expect("an exact count"),
    )
    .expect("a non-negative count")
}

fn selected_ids(document: &Json) -> BTreeSet<String> {
    array(document, "selected")
        .iter()
        .map(|item| text(item, "id").to_owned())
        .collect()
}

// --- ground-truth cross-checks, and the shapes they refuse -----------------------------------

/// How a doctored artifact fails this file's own reconciliation.
#[derive(Debug, PartialEq, Eq)]
enum Discrepancy {
    /// `|selected| + Σ manifest counts` is not the candidate universe.
    Unreconciled { candidates: u64, accounted: u64 },
    /// The selection holds an identity the question's closure does not.
    SelectedOffClosure { id: String },
    /// The closure holds an identity the selection does not.
    ClosureNotSelected { id: String },
    /// A manifest record is silent about how what it counts is retrieved (INV-007's second half).
    NoRetrievability { kind: String },
    /// The recorded `content_budget.bytes` is not the document's own byte length.
    Misreported { recorded: u64, measured: u64 },
    /// The published bytes are larger than the ceiling they were admitted against.
    OverBound { measured: u64, ceiling: u64 },
}

/// INV-007's counting equation, recomputed over the **published halves** against a candidate
/// universe this file knows independently.
///
/// > For a compile over a candidate set with a selection and a manifest, the size of the
/// > candidate set equals the size of the selection plus the sum of the manifest counts […]
/// > A count MUST NOT be an estimate, a sample, or a bucket.
/// >
/// > — RFC 0028, "Omission manifest"
///
/// `candidates` is supplied by the caller from the projection it built, never read out of the
/// artifact: the pack carries no candidate-count field, so this really is an external number.
fn reconcile(document: &Json, candidates: u64) -> Result<(), Discrepancy> {
    let selected = array(document, "selected").len() as u64;
    let omitted: u64 = array(document, "omissions").iter().map(count).sum();
    let accounted = selected + omitted;
    if accounted != candidates {
        return Err(Discrepancy::Unreconciled {
            candidates,
            accounted,
        });
    }
    for record in array(document, "omissions") {
        let fields = record.as_object().expect("object");
        // INV-007 names *both* halves: what was dropped, and how to retrieve it. A record that
        // is silent about retrievability satisfies half the invariant (RFC 0028, F12).
        if !fields.contains_key("expandable") {
            return Err(Discrepancy::NoRetrievability {
                kind: text(record, "kind").to_owned(),
            });
        }
        let expandable = fields["expandable"].as_bool().expect("a boolean");
        if expandable && !fields.contains_key("expansion") {
            return Err(Discrepancy::NoRetrievability {
                kind: text(record, "kind").to_owned(),
            });
        }
    }
    Ok(())
}

/// The selection is exactly the closure of the question — no more (a wrong pack) and no less (a
/// truncated one).
fn selection_is_the_closure(
    document: &Json,
    closure: &BTreeSet<String>,
) -> Result<(), Discrepancy> {
    let selected = selected_ids(document);
    if let Some(extra) = selected.difference(closure).next() {
        return Err(Discrepancy::SelectedOffClosure { id: extra.clone() });
    }
    if let Some(missing) = closure.difference(&selected).next() {
        return Err(Discrepancy::ClosureNotSelected {
            id: missing.clone(),
        });
    }
    Ok(())
}

/// `content_budget.bytes` is a measurement (RFC 0028 correction 17), so it is checkable against
/// the bytes the caller received. This is the check that keeps every "bounded" claim below from
/// resting on the compiler's own self-report.
fn size_is_measured(document: &Json, wire: &[u8]) -> Result<u64, Discrepancy> {
    let recorded = pack::budget_bytes_of(document).expect("a measured pack");
    let measured = wire.len() as u64;
    if recorded != measured {
        return Err(Discrepancy::Misreported { recorded, measured });
    }
    Ok(measured)
}

/// The bound itself, applied to bytes rather than to a field.
fn within_ceiling(wire: &[u8], ceiling: u64) -> Result<(), Discrepancy> {
    let measured = wire.len() as u64;
    if measured > ceiling {
        return Err(Discrepancy::OverBound { measured, ceiling });
    }
    Ok(())
}

// --- conjunct 1: bounded ---------------------------------------------------------------------

#[test]
fn the_root_pack_holds_the_closure_of_the_question_and_nothing_else() {
    // The structural bound, at adversarial scale: 384 candidates, of which the question's
    // backward closure is 128. Everything else is manifest, not content.
    let (mut daemon, root) = daemon();
    let outcome = compile(
        &mut daemon,
        &root,
        "req_structural",
        Optional::Absent,
        Optional::Absent,
    );
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "the compiler is served: {:?}",
        outcome.envelope.error
    );
    let (wire, _, document) = answered(&outcome);
    pack::required_keys_present(&document).expect("all seventeen required keys");

    let closure = ancestry(&root_id());
    assert_eq!(
        closure.len(),
        DEPTH as usize,
        "this file's own walk finds the whole chain and nothing else"
    );
    selection_is_the_closure(&document, &closure)
        .expect("the selection is exactly the closure of the question");
    reconcile(&document, u64::from(DEPTH) + u64::from(FANOUT))
        .expect("INV-007's counting equation holds over the whole candidate universe");

    // Depth is traversed in full: the far end of the chain — 127 hops from the root — is in the
    // pack. A slicer that truncated at any depth would leave it out.
    let selected = selected_ids(&document);
    assert!(
        selected.contains(&chain_id(0)),
        "the deepest ancestor is selected; stage 2 did not truncate the chain"
    );
    assert!(
        (0..FANOUT).all(|index| !selected.contains(&fanout_id(index))),
        "no fan-out successor reached the pack"
    );

    // The bound, measured rather than declared: the pack is far smaller than the bodies of the
    // universe it was compiled from. No threshold is asserted — RFC 0028 correction 15 forbids
    // this file inventing a reduction target — only that the pack is bounded away from the dump.
    let universe: u64 = bodies()
        .iter()
        .map(|item| item.to_canonical_bytes().len() as u64)
        .sum();
    let measured = size_is_measured(&document, &wire).expect("the recorded size is the real one");
    assert!(
        measured < universe,
        "the pack ({measured} B) is not bounded away from the dump of its candidate universe \
         ({universe} B)"
    );
}

#[test]
fn the_recorded_size_is_confirmed_by_an_external_measurement() {
    // Every "bounded" claim in this file is measured on `response.pack.as_bytes()`. This test is
    // the one that licenses reading `content_budget.bytes` anywhere else: it holds the pack's own
    // self-report to the bytes the daemon sent, on a root pack and on an expansion child.
    let (mut daemon, root) = daemon();
    let compiled = compile(
        &mut daemon,
        &root,
        "req_measured",
        Optional::Absent,
        Optional::Absent,
    );
    let (wire, context, document) = answered(&compiled);
    let root_bytes = size_is_measured(&document, &wire).expect("the root's size is measured");

    let expanded = expand(
        &mut daemon,
        &context,
        "req_measured_child",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    let (child_wire, _, child) = answered(&expanded);
    let child_bytes = size_is_measured(&child, &child_wire).expect("the child's size is measured");
    assert_ne!(
        child_bytes, root_bytes,
        "the child records its own measurement, not the parent's (RFC 0028 correction 17)"
    );
}

/// The minimal conforming child, derived here from RFC 0028's own words rather than found by
/// probing the daemon.
///
/// > The smallest child this module can build is **the complete manifest and an empty
/// > selection** — tier 2 with nothing of tier 3.
/// >
/// > — `continuum_context::budget`, quoting RFC 0028's packing order
///
/// Everything this needs is public vocabulary plus the published parent, so the prediction is
/// independent of whatever the packer does.
fn predicted_minimal_child(parent: &Json) -> u64 {
    let query = residual();
    let parent_id = pack::identity_of(parent).expect("a `ctx_*` parent");
    let identity = ExpansionHandle::derive::<Blake3Hasher>(&parent_id, &query, Depth::DEFAULT)
        .expect("the parent is a pack");
    let question = ExpansionQuestion::of(&query, Depth::DEFAULT);
    // At an empty selection every retrieved candidate is dropped for `budget`, retrieved by the
    // question this child answers — one cell, one record, an exact count.
    let manifest = Manifest::new([OmissionRecord::expandable(
        SelectionKind::StateDelta,
        FANOUT,
        PackReason::Budget,
        query.clone(),
    )])
    .expect("one record is a partition");
    let child = ChildPack {
        parent,
        identity: &identity,
        question: &question,
        selected: &[],
        manifest: &manifest,
        ceiling: u64::MAX,
    };
    child
        .to_json::<Blake3Hasher>()
        .expect("the parent is a conforming pack")
        .to_canonical_bytes()
        .len() as u64
}

#[test]
fn the_expansion_ceiling_is_exact_at_a_boundary_this_file_derives_independently() {
    // RFC 0028's enforced contract — "Bytes are the enforced contract" — probed at both
    // boundaries the RFC names, each at, one under, and one over. Admission is decided on the
    // bytes the daemon sent, never on the field the pack reports.
    let (mut daemon, root) = daemon();
    let compiled = compile(
        &mut daemon,
        &root,
        "req_boundary_root",
        Optional::Absent,
        Optional::Absent,
    );
    let (_, context, parent) = answered(&compiled);

    // --- the upper boundary: the whole answer -------------------------------------------
    let whole = expand(
        &mut daemon,
        &context,
        "req_whole",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    let (whole_wire, _, whole_document) = answered(&whole);
    let full = whole_wire.len() as u64;
    assert_eq!(
        selected_ids(&whole_document).len(),
        FANOUT as usize,
        "an unconstrained expansion returns the whole group"
    );
    assert!(
        array(&whole_document, "omissions").is_empty(),
        "an empty manifest is the assertion that nothing is outstanding (RFC 0028)"
    );

    // Exactly at: admitted, and nothing dropped.
    let at = expand(
        &mut daemon,
        &context,
        "req_at_full",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(full)),
    );
    let (at_wire, _, _) = answered(&at);
    assert_eq!(at.envelope.status, ResultStatus::Ok);
    within_ceiling(&at_wire, full).expect("a ceiling of n admits n");
    assert_eq!(at_wire, whole_wire, "at the whole size, the whole answer");

    // One under: still an answer, and a smaller one — RFC 0028's "a smaller pack with a larger
    // manifest", never a truncation and never a dump.
    let under = expand(
        &mut daemon,
        &context,
        "req_under_full",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(full - 1)),
    );
    let (under_wire, _, under_document) = answered(&under);
    assert_eq!(under.envelope.status, ResultStatus::Ok);
    within_ceiling(&under_wire, full - 1).expect("one byte under the whole answer still fits");
    let kept = selected_ids(&under_document).len() as u64;
    let dropped: u64 = array(&under_document, "omissions").iter().map(count).sum();
    assert!(
        dropped > 0,
        "the ceiling cost something and the manifest says so"
    );
    assert_eq!(
        kept + dropped,
        u64::from(FANOUT),
        "packing moves a candidate between the halves and creates none"
    );

    // --- the lower boundary: the minimal conforming child --------------------------------
    let minimal = predicted_minimal_child(&parent);
    let floor = expand(
        &mut daemon,
        &context,
        "req_at_floor",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(minimal)),
    );
    assert_eq!(
        floor.envelope.status,
        ResultStatus::Ok,
        "the predicted floor is admitted: {:?}",
        floor.envelope.error
    );
    let (floor_wire, _, floor_document) = answered(&floor);
    within_ceiling(&floor_wire, minimal).expect("the floor admits itself");
    assert_eq!(
        floor_wire.len() as u64,
        minimal,
        "the daemon's smallest child is exactly the one this file predicted from RFC 0028"
    );
    assert!(
        array(&floor_document, "selected").is_empty(),
        "the minimal child selects nothing"
    );
    assert_eq!(
        array(&floor_document, "omissions")
            .iter()
            .map(count)
            .sum::<u64>(),
        u64::from(FANOUT),
        "and still names every candidate the ceiling cost"
    );

    // One under the floor: nothing is published, and the refusal is typed. "A manifest is never
    // what a budget squeezes out" (INV-007).
    let below = expand(
        &mut daemon,
        &context,
        "req_below_floor",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(minimal - 1)),
    );
    assert_eq!(below.envelope.status, ResultStatus::Error);
    assert_eq!(
        below
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object")
            .code,
        ErrorCode::BudgetExhausted
    );
    assert!(
        matches!(below.payload, Payload::None),
        "nothing is published below the floor"
    );

    // One over the floor: an answer again, still the minimal one — a byte does not buy an item.
    let above = expand(
        &mut daemon,
        &context,
        "req_above_floor",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(minimal + 1)),
    );
    assert_eq!(above.envelope.status, ResultStatus::Ok);
    let (above_wire, _, _) = answered(&above);
    within_ceiling(&above_wire, minimal + 1).expect("one over the floor conforms");
    assert_eq!(above_wire.len() as u64, minimal);
}

/// A compile under `budget.bytes`, `output_policy.max_bytes`, or `output_policy.max_nodes`.
fn compile_under(
    daemon: &mut Daemon,
    root: &ArtifactHandle,
    request_id: &str,
    budget_bytes: Option<u64>,
    max_bytes: Option<u64>,
    max_nodes: Option<u64>,
) -> OperationOutcome {
    let policy = if max_bytes.is_none() && max_nodes.is_none() {
        Optional::Absent
    } else {
        Optional::Present(OutputPolicy {
            max_bytes: max_bytes.map_or(Optional::Absent, |b| Optional::Present(ByteCount::new(b))),
            max_tokens: Optional::Absent,
            max_nodes: max_nodes.map_or(Optional::Absent, Optional::Present),
            audience: Optional::Absent,
        })
    };
    compile(
        daemon,
        root,
        request_id,
        budget_bytes.map_or(Optional::Absent, |b| Optional::Present(ByteCount::new(b))),
        policy,
    )
}

/// The refusal every over-ceiling compile earns: `BudgetExhausted`, with nothing on the wire.
fn assert_exhausted(outcome: &OperationOutcome, what: &str) {
    assert_eq!(outcome.envelope.status, ResultStatus::Error, "{what}");
    let error = outcome
        .envelope
        .error
        .value()
        .expect("a refusal carries an error object");
    assert_eq!(error.code, ErrorCode::BudgetExhausted, "{what}");
    assert!(
        !error.non_resumable_reason.is_absent(),
        "the refusal carries a typed non_resumable_reason (SD-13): {what}"
    );
    assert!(
        matches!(outcome.payload, Payload::None),
        "nothing is published over a ceiling: {what}"
    );
}

/// The whole answer this compile gives when nothing bounds it: its wire bytes, its handle, and
/// the size of the result payload in the negotiated encoding (canonical JSON, see [`hello`]),
/// measured by this file's own call into the codec rather than read back off the daemon.
fn unbounded_compile(daemon: &mut Daemon, root: &ArtifactHandle) -> (Vec<u8>, ContextHandle, u64) {
    let outcome = compile_under(daemon, root, "req_unbounded", None, None, None);
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let payload = match &outcome.payload {
        Payload::ContextCompile(response) => {
            continuumd::codec::write_in::<continuumd::codec::json::Json, _>(response)
                .expect("the response encodes")
                .len() as u64
        }
        other => panic!("expected a compile payload, got {other:?}"),
    };
    let (wire, context, _) = answered(&outcome);
    (wire, context, payload)
}

#[test]
fn the_compile_budget_bytes_ceiling_is_exact_at_the_whole_root() {
    // RFC 0028, "Budgets and packing": "A guaranteed core is never truncated" — a root's
    // selection is its CausallyClosed core, so over the ceiling the compile takes the
    // `BudgetExhausted` branch its IDL `errors` clause declares. Regression guard for bn-2ga1c:
    // a compile under `budget.bytes = 512` once answered Ok with a 23 KiB pack.
    let (mut served, root) = daemon();
    let (whole, context, _) = unbounded_compile(&mut served, &root);
    let n = whole.len() as u64;

    // At the limit: admitted, byte-identical to the unbounded answer.
    let at = compile_under(&mut served, &root, "req_budget_at", Some(n), None, None);
    assert_eq!(
        at.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        at.envelope.error
    );
    let (at_wire, _, _) = answered(&at);
    within_ceiling(&at_wire, n).expect("a ceiling of n admits n");
    assert_eq!(
        at_wire, whole,
        "budget is not key material; the answer is the same pack"
    );

    // One byte over the limit (the answer is limit + 1): refused, typed.
    let (mut fresh, fresh_root) = daemon();
    let over = compile_under(
        &mut fresh,
        &fresh_root,
        "req_budget_over",
        Some(n - 1),
        None,
        None,
    );
    assert_exhausted(&over, "the whole root is one byte over budget.bytes");
    // And nothing was registered: the `ctx_*` the unbounded compile names is not navigable in
    // a daemon whose only compile was refused. A refused `@mutation` commits nothing.
    let probe = expand(
        &mut fresh,
        &context,
        "req_budget_over_probe",
        &root_id(),
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    assert_eq!(
        probe.envelope.error.value().map(|error| error.code),
        Some(ErrorCode::CapabilityDenied),
        "the refused compile published no pack"
    );

    // The bn-2ga1c probe itself.
    let tiny = compile_under(
        &mut fresh,
        &fresh_root,
        "req_budget_512",
        Some(512),
        None,
        None,
    );
    assert_exhausted(&tiny, "budget.bytes = 512 against a root of tens of KiB");
}

#[test]
fn the_compile_max_bytes_ceiling_is_exact_at_the_whole_payload() {
    // IDL `OutputPolicy.max_bytes`: "Enforced byte ceiling on the result payload" — measured
    // on the payload in the negotiated encoding, as `task.status` is (`daemon::output`).
    let (mut daemon, root) = daemon();
    let (whole, _, payload) = unbounded_compile(&mut daemon, &root);
    assert!(
        payload > whole.len() as u64,
        "the payload carries the pack and more"
    );

    let at = compile_under(&mut daemon, &root, "req_max_at", None, Some(payload), None);
    assert_eq!(
        at.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        at.envelope.error
    );
    let (at_wire, _, _) = answered(&at);
    assert_eq!(at_wire, whole, "at the limit, the whole answer");
    assert!(
        at.envelope
            .omissions
            .iter()
            .all(|o| o.reason != continuumd::protocol::vocabulary::OmissionReason::Budget),
        "an answer that fits records no budget omission"
    );

    let over = compile_under(
        &mut daemon,
        &root,
        "req_max_over",
        None,
        Some(payload - 1),
        None,
    );
    assert_exhausted(
        &over,
        "the payload is one byte over output_policy.max_bytes",
    );

    let tiny = compile_under(
        &mut daemon,
        &root,
        "req_max_512",
        Some(512),
        Some(512),
        None,
    );
    assert_exhausted(&tiny, "the bn-2ga1c probe: both ceilings at 512");
}

#[test]
fn the_compile_node_ceiling_is_recorded_and_exact_at_the_selection() {
    // RFC 0028: "`content_budget.nodes` is the graph-node ceiling of `OutputPolicy.max_nodes`";
    // IDL: "Ceiling on returned graph nodes". The returned graph nodes are `selected[]`, which
    // this file's own walk says is the whole chain.
    let (mut daemon, root) = daemon();
    let (whole, _, _) = unbounded_compile(&mut daemon, &root);
    let unbounded = Json::parse(&whole).expect("canonical JSON");
    assert!(
        !unbounded.as_object().expect("object")["content_budget"]
            .as_object()
            .expect("object")
            .contains_key("nodes"),
        "no ceiling stated, none recorded"
    );
    let limit = u64::from(DEPTH);
    assert_eq!(selected_ids(&unbounded).len() as u64, limit);

    let at = compile_under(&mut daemon, &root, "req_nodes_at", None, None, Some(limit));
    assert_eq!(
        at.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        at.envelope.error
    );
    let (at_wire, _, at_document) = answered(&at);
    size_is_measured(&at_document, &at_wire).expect("still a measured pack");
    assert_eq!(
        at_document.as_object().expect("object")["content_budget"]
            .as_object()
            .expect("object")
            .get("nodes"),
        Some(&Json::Integer(i64::from(DEPTH))),
        "the stated ceiling is recorded at content_budget.nodes"
    );
    assert_eq!(selected_ids(&at_document).len() as u64, limit);

    let over = compile_under(
        &mut daemon,
        &root,
        "req_nodes_over",
        None,
        None,
        Some(limit - 1),
    );
    assert_exhausted(
        &over,
        "the selection is one node over output_policy.max_nodes",
    );
}

#[test]
fn the_compile_budget_exhausted_branch_is_declared_and_reachable() {
    // The declaration and the implementation agree: the IDL offers `BudgetExhausted` for
    // context.compile, and `daemon::context::FAULTS` lists it. Before bn-2ga1c the second half
    // was false.
    let idl = read_dossier("notes/plan/schemas/continuumd-native-protocol.idl");
    let declaration = idl
        .split("operation context.compile {")
        .nth(1)
        .expect("the IDL declares context.compile")
        .split("\n}")
        .next()
        .expect("the operation block closes");
    assert!(
        declaration.contains("BudgetExhausted"),
        "the IDL declares BudgetExhausted for context.compile"
    );
    assert!(
        continuumd::daemon::context::FAULTS
            .contains(&("context.compile", ErrorCode::BudgetExhausted)),
        "FAULTS lists the branch the handler now takes"
    );
}

// --- conjunct 2: omission manifests and expansion handles ------------------------------------

#[test]
fn the_manifest_enumerates_exactly_what_the_question_left_out() {
    // Not "a manifest is present" — the manifest is checked, cell by cell, against a set this
    // file computed from the projection.
    let (mut daemon, root) = daemon();
    let outcome = compile(
        &mut daemon,
        &root,
        "req_manifest",
        Optional::Absent,
        Optional::Absent,
    );
    let (_, _, document) = answered(&outcome);

    let closure = ancestry(&root_id());
    let expected: BTreeSet<String> = nodes()
        .into_iter()
        .filter(|node| !closure.contains(node))
        .collect();
    assert_eq!(expected.len(), FANOUT as usize);

    let records = array(&document, "omissions");
    assert_eq!(records.len(), 1, "one kind, one reason, one cell");
    let record = &records[0];
    assert_eq!(text(record, "kind"), "state_delta");
    assert_eq!(
        count(record),
        expected.len() as u64,
        "the count is exact against the set this file computed, not against the ledger's own"
    );
    // The order declares itself complete, so non-ancestry is a proof and the drop is
    // `slice-irrelevant` rather than `heuristic-cutoff` — RFC 0028 reserves the first for items
    // *provably* outside the slice.
    assert_eq!(text(record, "reason"), "slice-irrelevant");
    assert_eq!(
        record.as_object().expect("object")["expandable"].as_bool(),
        Some(true),
        "INV-007's second half: the record names how what it counts is retrieved"
    );

    reconcile(&document, u64::from(DEPTH) + u64::from(FANOUT))
        .expect("the counting equation holds against an externally known universe");

    // The wire projection is the same facts, record for record (RFC 0028, "Omission manifest").
    assert_eq!(outcome.envelope.omissions.len(), records.len());
    assert_eq!(
        outcome.envelope.omissions[0].subject,
        format!("selected.{}", text(record, "kind"))
    );
    assert_eq!(
        outcome.envelope.omissions[0].reason.as_wire(),
        text(record, "reason")
    );
}

#[test]
fn every_omitted_candidate_comes_back_through_the_handle_the_manifest_named() {
    // Typed, present, and **live**: the handle is dereferenced through `Daemon::dispatch` and the
    // items it returns are checked against the omitted set this file computed independently.
    let (mut daemon, root) = daemon();
    let compiled = compile(
        &mut daemon,
        &root,
        "req_handles",
        Optional::Absent,
        Optional::Absent,
    );
    let (_, context, document) = answered(&compiled);

    let record = &array(&document, "omissions")[0];
    let expansion = &record.as_object().expect("object")["expansion"];
    let anchor = text(expansion, "anchor").to_owned();
    let relation = text(expansion, "relation").to_owned();

    // Typed: the relation is a member of the closed eleven, and the anchor resolves in the parent.
    assert!(
        PackRelation::from_wire_str(&relation).is_some(),
        "`{relation}` is outside the closed ExpansionRelation vocabulary"
    );
    assert!(
        selected_ids(&document).contains(&anchor),
        "the anchor does not resolve in the parent's own selection"
    );
    // The pack advertises the same query in `expansions[]`.
    assert!(
        array(&document, "expansions").iter().any(|advertised| {
            text(advertised, "anchor") == anchor && text(advertised, "relation") == relation
        }),
        "the manifest's query is not among the pack's advertised expansions"
    );

    // Promised: the envelope names the exact `ctx_*` the record resolves to.
    let promised = compiled.envelope.omissions[0]
        .recoverable_by
        .value()
        .cloned()
        .expect("an expandable record names the pack that retrieves it");

    // Live: follow it.
    let expanded = expand(
        &mut daemon,
        &context,
        "req_deref",
        &anchor,
        ExpansionRelation::CausalSuccessors,
        Optional::Present(ByteCount::new(1 << 20)),
    );
    assert_eq!(
        expanded.envelope.status,
        ResultStatus::Ok,
        "the promised handle does not resolve: {:?}",
        expanded.envelope.error
    );
    let (_, child_handle, child) = answered(&expanded);
    assert_eq!(
        child_handle.as_str(),
        promised.as_str(),
        "the manifest promised a pack the expansion did not return"
    );

    // And it resolves to the omitted content — every one of it, and nothing else.
    let closure = ancestry(&root_id());
    let expected: BTreeSet<String> = nodes()
        .into_iter()
        .filter(|node| !closure.contains(node))
        .collect();
    assert_eq!(
        selected_ids(&child),
        expected,
        "the expansion did not return exactly the candidates the manifest counted"
    );
    assert!(
        array(&child, "omissions").is_empty(),
        "nothing is outstanding behind the group, and the empty manifest says so"
    );
    // C5: a child inherits identity, never guarantees.
    assert!(
        array(&child, "guarantees").is_empty(),
        "an expansion child claims no guarantee of its own (RFC 0028 C5)"
    );
    for key in ["snapshot", "intent", "semantic_epoch"] {
        assert_eq!(
            text(&child, key),
            text(&document, key),
            "the child does not inherit the parent's `{key}`"
        );
    }
}

// --- negative controls -------------------------------------------------------------------------

#[test]
fn the_cross_checks_fail_on_artifacts_this_file_doctors() {
    // Anti-vacuity for every predicate above: each cross-check is shown rejecting an artifact
    // built by mutating the genuine one, so a green run is a result rather than a tautology.
    let (mut daemon, root) = daemon();
    let outcome = compile(
        &mut daemon,
        &root,
        "req_controls",
        Optional::Absent,
        Optional::Absent,
    );
    let (wire, _, document) = answered(&outcome);
    let candidates = u64::from(DEPTH) + u64::from(FANOUT);
    let closure = ancestry(&root_id());

    // The genuine artifact passes every one of them.
    reconcile(&document, candidates).expect("the genuine pack reconciles");
    selection_is_the_closure(&document, &closure).expect("the genuine selection is the closure");
    size_is_measured(&document, &wire).expect("the genuine size is measured");

    // (a) a shrunk manifest count — the mutant RFC 0028's Validation section names.
    assert!(matches!(
        reconcile(
            &with_count(&document, count(&array(&document, "omissions")[0]) - 1),
            candidates
        ),
        Err(Discrepancy::Unreconciled { .. })
    ));
    // (b) an inflated one, in the other direction.
    assert!(matches!(
        reconcile(
            &with_count(&document, count(&array(&document, "omissions")[0]) + 1),
            candidates
        ),
        Err(Discrepancy::Unreconciled { .. })
    ));
    // (c) the whole record dropped — "a pack that cannot name what it dropped is malformed, not
    // compact".
    assert!(matches!(
        reconcile(
            &with_field(&document, "omissions", Json::Array(Vec::new())),
            candidates
        ),
        Err(Discrepancy::Unreconciled { .. })
    ));
    // (d) a record that stops saying how it is retrieved (INV-007's second half, F12's shape).
    let silent = with_field(
        &document,
        "omissions",
        Json::Array(vec![without_key(
            &array(&document, "omissions")[0],
            "expandable",
        )]),
    );
    // The counting equation still holds for this one, which is the point: it fails on the
    // *second* half of INV-007 rather than on the arithmetic.
    assert!(matches!(
        reconcile(&silent, candidates),
        Err(Discrepancy::NoRetrievability { .. })
    ));
    // (e) an item off the closure smuggled into the selection.
    let smuggled = with_field(
        &document,
        "selected",
        Json::Array(
            array(&document, "selected")
                .iter()
                .cloned()
                .chain([body(&fanout_id(0), false).to_json()])
                .collect(),
        ),
    );
    assert!(matches!(
        selection_is_the_closure(&smuggled, &closure),
        Err(Discrepancy::SelectedOffClosure { .. })
    ));
    // (f) a core event dropped from the selection.
    let truncated = with_field(
        &document,
        "selected",
        Json::Array(
            array(&document, "selected")
                .iter()
                .filter(|item| text(item, "id") != chain_id(0))
                .cloned()
                .collect(),
        ),
    );
    assert!(matches!(
        selection_is_the_closure(&truncated, &closure),
        Err(Discrepancy::ClosureNotSelected { .. })
    ));
    // (g) a pack that understates its own size — the self-report check is not vacuous.
    let understated = with_field(
        &document,
        "content_budget",
        Json::object([("bytes".to_owned(), Json::Integer(1))]).expect("one key"),
    );
    assert!(matches!(
        size_is_measured(&understated, &wire),
        Err(Discrepancy::Misreported { recorded: 1, .. })
    ));
    // (h) an over-bound artifact, constructed directly: the ceiling check flags bytes larger than
    // the ceiling they were admitted against, so the conformance observed above is a measurement
    // rather than an assumption.
    assert!(matches!(
        within_ceiling(&wire, (wire.len() as u64) - 1),
        Err(Discrepancy::OverBound { .. })
    ));
    within_ceiling(&wire, wire.len() as u64).expect("a ceiling of n admits n");
}

#[test]
fn a_handle_that_should_not_resolve_does_not() {
    // The liveness check in [`every_omitted_candidate_comes_back_through_the_handle_the_manifest_named`]
    // only means something if a handle that names nothing is refused. Both refusals are typed, and
    // they are two different refusals — which half of the request was wrong decides which.
    let (mut daemon, root) = daemon();
    let compiled = compile(
        &mut daemon,
        &root,
        "req_negative_handles",
        Optional::Absent,
        Optional::Absent,
    );
    let (_, context, _) = answered(&compiled);

    // An anchor nothing in the pack carries: `MalformedRequest`.
    let nowhere = expand(
        &mut daemon,
        &context,
        "req_nowhere",
        "zz_absent",
        ExpansionRelation::CausalSuccessors,
        Optional::Absent,
    );
    assert_eq!(
        nowhere
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object")
            .code,
        ErrorCode::MalformedRequest
    );
    assert!(matches!(nowhere.payload, Payload::None));

    // A relation that is not advertised at a resolvable anchor: `UnsupportedSemanticFeature`.
    let undefined = expand(
        &mut daemon,
        &context,
        "req_undefined",
        &root_id(),
        ExpansionRelation::ProofDependency,
        Optional::Absent,
    );
    assert_eq!(
        undefined
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object")
            .code,
        ErrorCode::UnsupportedSemanticFeature
    );
    assert!(matches!(undefined.payload, Payload::None));
}

// --- conjunct 3: the instrument that does not exist ----------------------------------------------

#[test]
fn the_benchmark_effectiveness_instrument_does_not_exist() {
    // G2-05's third conjunct — "improve agent benchmark effectiveness" — is **UNSUPPORTED
    // (instrument unbuilt)**. Unbuilt is not failed and is not inconclusive, and the difference
    // matters enough to be established mechanically rather than asserted in prose.
    //
    // This is a freshness tripwire, not an ablation: it goes red the moment the benchmark crate
    // grows a Context Pack arm, which forces the debt to be re-adjudicated instead of quietly
    // paid or quietly forgotten.
    let surface = read_dossier("crates/continuum-benchmark/src/surface.rs");
    let arms = surface
        .split("pub enum Arm {")
        .nth(1)
        .expect("the benchmark crate declares its arms")
        .split('}')
        .next()
        .expect("the enum closes");
    for member in ["Native", "Shell"] {
        assert!(arms.contains(member), "the `{member}` arm went missing");
    }
    let declared = arms
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("///") && !trimmed.starts_with("//")
        })
        .count();
    assert_eq!(
        declared, 2,
        "the benchmark comparison gained an arm; if it is the Context Pack ablation, G2-05's \
         third conjunct is no longer instrument-unbuilt and this pin must be re-adjudicated"
    );

    // And no source or test file in that crate touches a pack at all. The only occurrences of
    // "Context Pack" in it are the two quotations of the plan sentence that *requires* the
    // ablation — the sentence, not the instrument.
    let mut touching: Vec<String> = Vec::new();
    let mut quoting: Vec<String> = Vec::new();
    for path in rust_sources("crates/continuum-benchmark") {
        let text = std::fs::read_to_string(&path).expect("a readable source file");
        let name = path
            .strip_prefix(workspace_root())
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        if text.contains("Context Pack") {
            quoting.push(name.clone());
        }
        for marker in [
            "continuum_context",
            "ContextPack",
            "context.compile",
            "context.expand",
            "ContextHandle",
        ] {
            if text.contains(marker) {
                touching.push(format!("{name} ({marker})"));
                break;
            }
        }
    }
    assert!(
        touching.is_empty(),
        "the Phase A benchmark instrument now references Context Packs, so the ablation may \
         exist; re-adjudicate G2-05's third conjunct: {touching:?}"
    );
    quoting.sort();
    assert_eq!(
        quoting,
        [
            "crates/continuum-benchmark/src/separation.rs",
            "crates/continuum-benchmark/tests/pr10_family_separation.rs",
        ],
        "the set of files quoting the plan sentence changed; the quotation is not the instrument, \
         so a change here means the tree moved and this pin must be re-read"
    );

    // The §19.4 separation gate the plan requires *before* either result is accepted exists and is
    // the benchmark subset's, not a pack subset's: it is built for DX-10 and would have to be
    // re-run over whatever task set an ablation drew. That is the one piece of the instrument that
    // is already in the tree, and naming it is the actionable half of this absence.
    let separation = read_dossier("crates/continuum-benchmark/src/separation.rs");
    assert!(
        separation.contains("The Phase A benchmark subset used for DX-10 and the G2 Context Pack"),
        "separation.rs no longer quotes the plan §22 staging rule that binds an ablation to the \
         §19.4 check"
    );
    assert!(
        separation.contains("pub fn check"),
        "the §19.4 family/source-hash separation check is no longer callable, so an ablation \
         would have no gate to pass"
    );
}

// --- reading the tree ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

fn read_dossier(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Every `.rs` file under `relative`, sorted, so the scan is deterministic.
fn rust_sources(relative: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack = vec![workspace_root().join(relative)];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// --- doctoring helpers, used only by the negative controls -------------------------------------

fn with_field(document: &Json, key: &str, value: Json) -> Json {
    let mut fields = document.as_object().expect("object").clone();
    fields.insert(key.to_owned(), value);
    Json::Object(fields)
}

fn with_count(document: &Json, count: u64) -> Json {
    let records: Vec<Json> = array(document, "omissions")
        .iter()
        .enumerate()
        .map(|(index, record)| {
            if index != 0 {
                return record.clone();
            }
            let mut fields = record.as_object().expect("object").clone();
            fields.insert(
                "count".to_owned(),
                Json::Integer(i64::try_from(count).expect("a count in range")),
            );
            Json::Object(fields)
        })
        .collect();
    with_field(document, "omissions", Json::Array(records))
}

fn without_key(value: &Json, key: &str) -> Json {
    let mut fields = value.as_object().expect("object").clone();
    fields.remove(key);
    Json::Object(fields)
}
