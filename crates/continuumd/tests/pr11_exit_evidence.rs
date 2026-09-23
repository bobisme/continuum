//! PR-11 exit evidence: *the synthetic 200-event durability case compiles to a
//! replay-preserving core with substantial reduction* (bn-3m65; requirement id
//! `PR-11-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 11's Exit line;
//! the G0-DX-01 row's pass condition adds the third leg, *exact expansion handles*).
//!
//! # The adjudication this file records, and the re-run that superseded it
//!
//! **The exit is now closed at production grain (bn-1y4qc).** Its first close (bn-3m65) was at
//! the campaign's *declared* grain, with the ten-stage compiler named as the exit's owed
//! producer and [`the_compiler_is_served_at_production_grain`] pinning `context.compile`'s
//! refusal live — "so a landed compiler turns this file red and forces the exit evidence up to
//! production grain instead of letting the declared-grain close linger silently". The compiler
//! landed (bn-8g9uj's five children), the pin went red exactly as designed, and this file is
//! the re-run it forced. Nothing here relaxes: the closure that rejects an artifact for hiding
//! its grain, or for un-naming the producers it is still missing, is the same closure — only
//! the grain being declared changed.
//!
//! What moved, leg by leg: leg 1's *selection* is now RFC 0028 stages 1–2 running in
//! `continuum_context::compile` (its checks were always production, which is the point RFC
//! 0028's Validation section makes — "the checkers are independent of the compiler that made
//! the claim"); leg 2's *assembly* is now `continuum_context::pack::RootPack` (its counting
//! rule was always production); leg 3 was production end to end already, and the pack it
//! navigates is now one `context.compile` produced. What has *not* moved is that a grain is
//! declared at all: the campaign artifact states every leg's grain before any number, this
//! closure requires each declaration by name, and
//! [`anti_vacuity_doctored_campaign_artifacts_are_rejected`] proves each requirement can fire.
//!
//! The residual is smaller and still named. It is no longer "the compiler": it is the two
//! subsystems stages 5 and 7 *configured and refused* for (`continuum-proof-client` and
//! `continuum-refinement`, both PR-1/IMPL-01 scaffolds), the CIR causal-order producer that
//! leaves stage 2's input declared rather than produced, the crashpack producer, and
//! intent-registry resolution. A named, typed, routed absence does not block an exit whose
//! sentence is true at the delivered grain — the PR-5 owed-CBOR, PR-8 F16 and PR-6
//! wire-reachable-phase-set precedents — and a silent one would; so the closure rejects an
//! artifact that stops naming them, exactly as it rejected one that stopped naming the compiler.
//!
//! # What this layer is, and is not
//!
//! The independent exit layer over the bn-37gu campaign (the bn-3jtr/bn-2hmk shape): it touches
//! none of `dx01_falsification.rs`'s tests, closes the sentence against the dossier's own texts
//! rather than restating them, and re-derives every leg from the byte-pinned campaign artifact
//! instead of trusting its recorded verdict flags — the closure recomputes the reduction ratio
//! from the artifact's own byte counts, recomputes witness-equals-core from the two membership
//! lines, recomputes the INV-007 conservation equation, and recomputes the promise equality
//! from the two handles, so an artifact that lies about its own arithmetic is rejected by the
//! same function the genuine artifact passes
//! ([`anti_vacuity_doctored_campaign_artifacts_are_rejected`] proves each rejection arm fires).
//! The campaign itself keeps the facts fresh: `dx01_falsification.rs` re-renders the artifact
//! from two independent builds on every run and fails on drift, so citing its golden bytes here
//! cites a live campaign, not a memory.
//!
//! The one thing this layer does *not* take from the campaign is the compiler's own liveness.
//! [`the_compiler_is_served_at_production_grain`] builds its own daemon, its own three-node
//! causal order and its own registered projection — sharing no fixture with the campaign — and
//! compiles a pack through `Daemon::dispatch`. If `context.compile` ever stops being served, or
//! starts answering without a checked guarantee set, this file goes red on evidence it produced
//! itself.
//!
//! # Evidence map
//!
//! - the sentence, its source line, and its matrix twin, closed against the dossier —
//!   [`the_sentence_is_closed_against_the_dossier_not_restated`];
//! - the three legs, re-derived from the pinned campaign artifact —
//!   [`the_three_legs_close_over_the_pinned_campaign_artifact`];
//! - the rejection arms, one doctored artifact per leg —
//!   [`anti_vacuity_doctored_campaign_artifacts_are_rejected`];
//! - the grain guard, live against a real daemon —
//!   [`the_compiler_is_served_at_production_grain`];
//! - the whole exit rendered as one byte-stable artifact, pinned at
//!   `tests/golden/pr11_exit_evidence.txt` —
//!   [`the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden`]. Golden
//!   regeneration is explicit and cannot pass: `PR11_EXIT_BLESS=1` rewrites the golden
//!   and then panics, so a blessing run is always red and the diff is always reviewed
//!   as a contract change (INV-004).

use continuum_context::assurance::Assurance;
use continuum_context::causal::CausalOrder;
use continuum_context::compile::{CausalCompile, RedactionPolicy};
use continuum_context::expansion::{ExpansionQuery, ExpansionRelation as PackRelation};
use continuum_context::pack::{self, PackProfile};
use continuum_context::replay::ReplayRef;
use continuum_context::selection::SelectionKind;
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
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, Verdict};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextCompileRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, EpochIdentity, OperationName, ProtocolVersion,
    RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, Encoding, EvaluationVerdict};

// --- the governing texts, read at compile time -------------------------------------------

/// The campaign's byte-pinned evidence artifact (bn-37gu). Its own suite re-renders and
/// re-compares it on every run, so these bytes are a live campaign's, not a snapshot's.
const CAMPAIGN: &str = include_str!("golden/dx01_falsification_evidence.txt");

/// The exit sentence's authoritative source.
const START_HERE: &str = include_str!("../../../notes/plan/notes/START_HERE_IMPLEMENTATION.md");

/// The matrix whose G0-DX-01 row is this sentence in the matrix's own words.
const MATRIX: &str = include_str!("../../../notes/plan/notes/G0_SPIKE_MATRIX.md");

/// This exit's own rendered evidence, pinned.
const EXIT_GOLDEN: &str = include_str!("golden/pr11_exit_evidence.txt");

/// The sentence, verbatim from START_HERE's PR 11 Exit line.
const SENTENCE: &str = "the synthetic 200-event durability case compiles to a \
                        replay-preserving core with substantial reduction";

// --- the closure: every leg re-derived from the artifact, able to fail -------------------

/// What the closure re-derived, for rendering.
#[derive(Debug)]
struct Closed {
    events: u64,
    reachable_states: u64,
    core: Vec<String>,
    pack_bytes: u64,
    raw_bytes: u64,
    ratio_x100: u64,
    candidates: u64,
    selected: u64,
    omitted: u64,
    expandable: u64,
    unachieved: u64,
    guarantees: String,
    promised: String,
    child_items: u64,
    packed_kept: u64,
    packed_dropped: u64,
    compile_status: String,
    compile_verdict: String,
    compile_class: String,
}

fn line_with<'a>(campaign: &'a str, prefix: &str) -> Result<&'a str, String> {
    campaign
        .lines()
        .find(|line| line.starts_with(prefix))
        .ok_or_else(|| format!("the campaign artifact has no `{prefix}` line"))
}

fn field(line: &str, key: &str) -> Result<String, String> {
    let marker = format!("{key}=");
    let start = line
        .find(&marker)
        .ok_or_else(|| format!("`{key}=` is missing on `{line}`"))?
        + marker.len();
    Ok(line[start..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned())
}

fn number(line: &str, key: &str) -> Result<u64, String> {
    let raw = field(line, key)?;
    raw.parse::<u64>()
        .map_err(|_| format!("`{key}={raw}` is not a number"))
}

/// Close the exit sentence over the campaign artifact. Every check re-derives rather
/// than trusts: recorded verdict flags must *agree with* recomputation, never stand in
/// for it. Each rejection arm carries a distinct message so the doctored-artifact test
/// can prove it fired for its own reason.
fn close(campaign: &str) -> Result<Closed, String> {
    // The artifact is the row's experiment, said in its own header.
    if !campaign.starts_with("G0-DX-01 falsification evidence") {
        return Err("the artifact is not the G0-DX-01 campaign's".to_owned());
    }
    let row = line_with(campaign, "row: ")?;
    if row != "row: 200+ event noisy durability failure -> Context Pack" {
        return Err(format!("the experiment drifted: `{row}`"));
    }
    let legs = line_with(campaign, "legs: ")?;
    if legs != "legs: causal core replay-preserving | >=10x reduction | exact expansion handles" {
        return Err(format!("the pass-condition legs drifted: `{legs}`"));
    }

    // The case: 200+ events, synthetic durability failure, refuted by the production check.
    let trace = line_with(campaign, "events=")?;
    let events = number(trace, "events")?;
    if events < 200 {
        return Err(format!("{events} events is not the 200-event case"));
    }
    let verdict_line = line_with(campaign, "engine_check_verdict=")?;
    let verdict = field(verdict_line, "engine_check_verdict")?;
    if verdict != "refuted" {
        return Err(format!("the engine verdict is `{verdict}`, not refuted"));
    }
    if field(verdict_line, "invariant")? != "AckImpliesDurable" {
        return Err("the refuted invariant is not the durability invariant".to_owned());
    }
    let reachable_states = number(verdict_line, "reachable_states")?;

    // Leg 1 — replay-preserving core. Witness-equals-core is recomputed from the two
    // membership lines; the recorded flag must agree.
    let core_line = line_with(campaign, "core=")?;
    let core: Vec<String> = field(core_line, "core")?
        .split(',')
        .map(|event| {
            event
                .split(':')
                .nth(1)
                .map(str::to_owned)
                .ok_or_else(|| format!("`{event}` is not an id:action core entry"))
        })
        .collect::<Result<_, _>>()?;
    if core.len() != 4 {
        // Plan §25's own concrete number: "a 200-event failing run through a 4-event
        // Context Pack".
        return Err(format!(
            "the core has {} events, not plan §25's four",
            core.len()
        ));
    }
    let witness_line = line_with(campaign, "shortest_witness=")?;
    let witness: Vec<String> = field(witness_line, "shortest_witness")?
        .split(',')
        .map(str::to_owned)
        .collect();
    if witness != core {
        return Err("witness::shortest does not corroborate the core".to_owned());
    }
    if field(witness_line, "equals_core")? != "true" {
        return Err("the artifact's own equals_core flag disagrees".to_owned());
    }
    let replay = field(line_with(campaign, "core_replay=")?, "core_replay")?;
    if replay != "refutes" {
        return Err(format!(
            "the core replay is `{replay}`, not a reproduced refutation"
        ));
    }
    let drops: Vec<&str> = campaign
        .lines()
        .filter(|line| line.starts_with("drop_"))
        .collect();
    if drops.len() != 4 {
        return Err(format!(
            "{} drop-one controls, not one per core event",
            drops.len()
        ));
    }
    let mut disabled = 0u32;
    let mut lost_refutation = 0u32;
    for line in &drops {
        let head = line.split_whitespace().next().unwrap_or("");
        let (name, observed) = head
            .split_once('=')
            .ok_or_else(|| format!("`{head}` is not a drop-one record"))?;
        let expected = field(line, "pre_registered")?;
        if observed != expected || field(line, "held")? != "true" {
            return Err(format!(
                "control `{name}` did not fail its pre-registered way \
                 (observed `{observed}`, pre-registered `{expected}`)"
            ));
        }
        match observed {
            _ if observed.starts_with("disabled_at_") => disabled += 1,
            "completes_without_refuting" => lost_refutation += 1,
            other => return Err(format!("control `{name}` failed an unknown way: `{other}`")),
        }
    }
    if disabled == 0 || lost_refutation == 0 {
        return Err("the drop-one controls do not exercise both failure modes".to_owned());
    }
    let textual = line_with(campaign, "textual_selection:")?;
    if number(textual, "admits_noise")? == 0
        || field(textual, "misses")? != "begin_txn,power_loss"
        || !field(textual, "strict")?.starts_with("disabled_at_")
        || field(textual, "forgiving")? != "completes_without_refuting"
    {
        return Err("the textual-relevance control (RFC 0028 C2) did not fail".to_owned());
    }

    // Leg 2 — substantial reduction, graded at the row's own >=10x and recomputed from
    // the artifact's byte counts under the packer's counting rule.
    let pack_line = line_with(campaign, "pack_bytes=")?;
    let pack_bytes = number(pack_line, "pack_bytes")?;
    let candidates = number(pack_line, "candidates")?;
    let selected = number(pack_line, "selected")?;
    let omitted = number(pack_line, "omitted")?;
    let expandable = number(pack_line, "expandable")?;
    let unachieved = number(pack_line, "unachieved_guarantee")?;
    if candidates != selected + omitted {
        return Err(format!(
            "INV-007 conservation fails: {candidates} != {selected} + {omitted}"
        ));
    }
    if omitted != expandable + unachieved {
        return Err(format!(
            "the manifest does not split: {omitted} != {expandable} + {unachieved}"
        ));
    }
    // Rule C1, at the artifact: a guarantee the caller requested and no checker established is
    // an omission record, never an echoed claim. The closure requires the pack to *say* which
    // guarantees it claims, and requires the requested-and-unachieved one to be absent from
    // that list — the one direction C1's first sentence forbids.
    let guarantee_line = line_with(campaign, "guarantees=")?;
    let guarantees = field(guarantee_line, "guarantees")?;
    let requested = field(guarantee_line, "requested")?;
    if guarantees.split(',').any(|claimed| claimed == requested) {
        return Err(format!(
            "rule C1: the pack echoes the requested `{requested}` as a claim"
        ));
    }
    if unachieved > 0 && field(guarantee_line, "unachieved_c1")? != "unknown:unsupported" {
        return Err("rule C1's owed record is not the `unknown`/`unsupported` omission".to_owned());
    }
    let graded = line_with(campaign, "graded raw_named_json=")?;
    let raw_bytes = number(graded, "raw_named_json")?;
    let ratio_x100 = number(graded, "ratio_x100")?;
    let recomputed = raw_bytes * 100 / pack_bytes;
    if recomputed != ratio_x100 {
        return Err(format!(
            "the recorded ratio does not recompute: {raw_bytes}*100/{pack_bytes} = \
             {recomputed}, recorded {ratio_x100}"
        ));
    }
    if ratio_x100 < 1000 {
        return Err(format!(
            "the reduction is {ratio_x100} (x100), below the 10x the row grades \
             `substantial` at"
        ));
    }
    if field(graded, "pass_10x")? != "true" {
        return Err("the artifact's own pass_10x flag disagrees".to_owned());
    }
    let decomposition = line_with(campaign, "pack_decomposition ")?;
    let parts = number(decomposition, "selected")?
        + number(decomposition, "manifest+expansions")?
        + number(decomposition, "header")?;
    if parts != pack_bytes {
        return Err(format!(
            "the pack decomposition does not sum: {parts} != {pack_bytes}"
        ));
    }

    // Leg 3 — exact expansion handles, wire-live. The promise equality is recomputed
    // from the two handles; the recorded flag must agree.
    let parent = field(line_with(campaign, "parent=")?, "parent")?;
    let promise_line = line_with(campaign, "promised=")?;
    let promised = field(promise_line, "promised")?;
    let returned = field(promise_line, "returned")?;
    if promised != returned {
        return Err(format!(
            "the promise is broken: promised {promised}, returned {returned}"
        ));
    }
    if field(promise_line, "equal")? != "true" {
        return Err("the artifact's own equal flag disagrees".to_owned());
    }
    if !promised.starts_with("ctx_") || promised == parent {
        return Err(format!("`{promised}` is not a child ctx_* handle"));
    }
    let child_line = line_with(campaign, "child_items=")?;
    let child_items = number(child_line, "child_items")?;
    if child_items != expandable {
        return Err(format!(
            "the expansion did not return the expandable omissions: {child_items} != \
             {expandable}"
        ));
    }
    if field(child_line, "exact_match")? != "true"
        || field(child_line, "child_manifest_empty")? != "true"
    {
        return Err("the expansion is not exact under an empty residual manifest".to_owned());
    }
    let packed = line_with(campaign, "packed(")?;
    let packed_kept = number(packed, "kept")?;
    let packed_dropped = number(packed, "dropped")?;
    if packed_kept + packed_dropped != child_items {
        return Err(format!(
            "budget-branch conservation fails: {packed_kept} + {packed_dropped} != \
             {child_items}"
        ));
    }
    if field(packed, "recoverable_by_promised")? != "true" {
        return Err("the packed child's shortfall is not recoverable by the promise".to_owned());
    }

    // The compile itself, wire-live: the grain that moved. `context.compile` answers `ok`,
    // and the envelope's verdict and assurance class are the pack's own (RFC 0028, "Wire
    // surface"), which is what makes the compiled pack and the answer about it one statement
    // rather than two that agree today.
    let compile_line = line_with(campaign, "context.compile=")?;
    let compile_status = field(compile_line, "context.compile")?;
    if compile_status != "ok" {
        return Err(format!(
            "`context.compile` answered `{compile_status}`; the production-grain close \
             requires a served compiler"
        ));
    }
    let compile_verdict = field(compile_line, "verdict")?;
    if compile_verdict != verdict {
        return Err(format!(
            "the envelope verdict `{compile_verdict}` is not the evaluation's `{verdict}`"
        ));
    }
    let compile_class = field(compile_line, "assurance_class")?;
    if field(
        line_with(campaign, "audience_invariant=")?,
        "audience_invariant",
    )? != "true"
    {
        return Err(
            "`audience` no longer selects rendering only: two compiles differing in it \
             disagree (RFC 0028, \"Views and rendering\")"
                .to_owned(),
        );
    }

    // The grain declaration is load-bearing, and it always was: the close is honest only while
    // the artifact keeps declaring each leg's grain and naming the producers it is still
    // missing. This is the same requirement bn-3m65 wrote, with the grain it demands moved from
    // harness to production — an artifact that stops declaring, or that declares less than it
    // has, is rejected rather than accepted more strongly.
    let leg1 = line_with(campaign, "leg1: ")?;
    if !leg1.contains("selection=production") || !leg1.contains("check=production") {
        return Err(
            "leg 1 no longer declares its selection=production/check=production grain".to_owned(),
        );
    }
    if !leg1.contains("order=declared") {
        return Err(
            "leg 1 no longer declares that stage 2's causal order is a declared input — the \
             CIR producer is still absent and the close requires the declaration"
                .to_owned(),
        );
    }
    let leg2 = line_with(campaign, "leg2: ")?;
    if !leg2.contains("counting=production") || !leg2.contains("assembly=production") {
        return Err(
            "leg 2 no longer declares its counting=production/assembly=production grain".to_owned(),
        );
    }
    if !line_with(campaign, "leg3: ")?.contains("production end-to-end") {
        return Err("leg 3 no longer declares production end-to-end".to_owned());
    }
    let missing = line_with(campaign, "missing_producers:")?;
    for producer in [
        "proof service",
        "correspondence graph",
        "CIR causal-order producer",
        "crashpack producer",
        "intent registry",
    ] {
        if !missing.contains(producer) {
            return Err(format!(
                "the artifact no longer names the missing {producer} — the close requires \
                 every absence to stay declared"
            ));
        }
    }

    Ok(Closed {
        events,
        reachable_states,
        core,
        pack_bytes,
        raw_bytes,
        ratio_x100,
        candidates,
        selected,
        omitted,
        expandable,
        unachieved,
        guarantees,
        promised,
        child_items,
        packed_kept,
        packed_dropped,
        compile_status,
        compile_verdict,
        compile_class,
    })
}

// --- the live daemon, for the grain guard (transport_local.rs idioms, local copies) ------

const NOW: &str = "2026-08-01T00:00:00.000Z";

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
        client: "continuumd-pr11-exit-evidence".to_owned(),
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
        instances: Optional::Absent,
    }
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
    Daemon::builder(Blake3Identity, negotiated, capability())
        .epochs(epochs())
        .now(Timestamp::new(NOW).expect("a timestamp"))
        .capability(grant(), None)
        .family(ContextFamily)
        .build()
}

/// The exit layer's **own** compile projection — three writes, one dependence chain, sharing no
/// fixture with the campaign.
///
/// Small on purpose. What this has to establish is that `context.compile` is *served* at
/// production grain and answers with a pack whose guarantee set a checker licensed; the
/// campaign establishes the numbers. A fixture this layer built itself is what keeps the two
/// independent, which is the whole shape of an exit layer (bn-3jtr/bn-2hmk).
fn exit_projection() -> (ContextCompileSource, ArtifactHandle) {
    let id = |text: &str| Name::new(text).expect("a canonical identifier");
    let delta = |name: &str, variable: &str, before: i64, after: i64| {
        StateDeltaRef::new(
            id(variable),
            StateDeltaClass::Concrete,
            Some(Value::int(i128::from(before))),
            Value::int(i128::from(after)),
        )
        .expect("a changed variable is a real delta")
        .into_selected_item(id(name))
    };
    // d_begin -> d_write -> d_ack, and one write nothing downstream reads.
    let order = CausalOrder::new(
        [
            (id("d_ack"), SelectionKind::StateDelta),
            (id("d_begin"), SelectionKind::StateDelta),
            (id("d_noise"), SelectionKind::StateDelta),
            (id("d_write"), SelectionKind::StateDelta),
        ],
        [(id("d_ack"), id("d_write")), (id("d_write"), id("d_begin"))],
    )
    .expect("the order is a DAG");
    let digest = |text: &str| Blake3Hasher::hash(text.as_bytes()).to_token();
    let header = CompileHeader {
        snapshot: WorkspaceHandle::new("ws_pr11exitlayer").expect("a workspace handle"),
        semantic_epoch: "sem3-pr11-exit".to_owned(),
        intent: StoreHandle::new(ArtifactClass::IntentContract, &digest("pr11-exit-intent"))
            .expect("a digest token is a well-formed identity"),
        evidence: vec![
            StoreHandle::new(ArtifactClass::Evidence, &digest("pr11-exit-evidence"))
                .expect("a digest token is a well-formed identity"),
        ],
        replay: Some(
            ReplayRef::new(
                StoreHandle::new(ArtifactClass::Crashpack, &digest("pr11-exit-crashpack"))
                    .expect("a digest token is a well-formed identity"),
            )
            .expect("a crash_* handle"),
        ),
        verdict: Some(PackVerdict::Refuted),
        assurance: Assurance::new(
            AssuranceLevel::Bounded,
            AssuranceEnvelope::all_unsupported(
                &UnsupportedReason::new("outside-this-exit-layer").expect("a plain token"),
            ),
        ),
        profile: PackProfile::Failure,
        redactions: Vec::new(),
    };
    let root = ArtifactHandle::new(&header.evidence[0].to_string()).expect("an artifact handle");
    let source = ContextCompileSource::new(
        CausalCompile::new(order, RedactionPolicy::permitting_everything()),
        [id("d_ack")],
        ExpansionQuery::new(PackRelation::SameOwner, id("d_ack")),
        [
            delta("d_ack", "client_acked", 0, 1),
            delta("d_begin", "txn", 0, 1),
            delta("d_noise", "telemetry", 0, 1),
            delta("d_write", "wal_buffered", 0, 1),
        ],
        header,
    )
    .expect("every candidate of the order has a registered body");
    (source, root)
}

fn compile_request(root: &ArtifactHandle) -> OperationRequest {
    // The exit's own case, asked for at production grain: compile a pack from an
    // evidence root, requesting exactly the guarantee the sentence turns on.
    OperationRequest {
        envelope: RequestEnvelope {
            protocol_version: version(),
            request_id: RequestId::new("req_grain_guard").expect("a request identity"),
            actor: actor(),
            capability: capability(),
            operation: OperationName::new("context.compile").expect("a declared operation"),
            idempotency_key: Optional::Present("idem-req_grain_guard".to_owned()),
            budget: Optional::Present(Budget {
                wall_ms: Optional::Absent,
                cpu_ms: Optional::Absent,
                memory_bytes: Optional::Absent,
                states: Optional::Present(0),
                solver_ms: Optional::Absent,
                proof_ms: Optional::Absent,
                tokens: Optional::Absent,
                candidates: Optional::Absent,
                bytes: Optional::Absent,
            }),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            trace: Optional::Absent,
            arguments: continuumd::protocol::scalar::Opaque::from_bytes(b"{}".to_vec()),
            output_policy: Optional::Absent,
            page: Optional::Absent,
        },
        arguments: Arguments::ContextCompile(ContextCompileRequest {
            evidence_root: root.clone(),
            question: "why did AckImpliesDurable fail?".to_owned(),
            audience: Optional::Absent,
            guarantees: Optional::Present(vec!["ReplayPreserving".to_owned()]),
        }),
    }
}

/// What a live `context.compile` answered, for rendering and for the guard.
struct LiveCompile {
    status: String,
    guarantees: Vec<String>,
    verdict: String,
    unachieved: u64,
}

fn live_compile() -> LiveCompile {
    let (source, root) = exit_projection();
    let mut daemon = daemon();
    daemon
        .state_mut()
        .put_compile_source(&root, source)
        .expect("an `ev_` root");
    let outcome = daemon.dispatch(&compile_request(&root));
    let Payload::ContextCompile(response) = &outcome.payload else {
        panic!(
            "context.compile must be served at production grain: {:?}",
            outcome.envelope.error
        );
    };
    let document = Json::parse(response.pack.as_bytes()).expect("the pack is canonical JSON");
    pack::required_keys_present(&document).expect("all seventeen required keys");
    let fields = document.as_object().expect("object");
    let guarantees: Vec<String> = fields["guarantees"]
        .as_array()
        .expect("array")
        .iter()
        .map(|token| token.as_str().expect("a string").to_owned())
        .collect();
    let unachieved: u64 = fields["omissions"]
        .as_array()
        .expect("array")
        .iter()
        .filter(|record| record.as_object().expect("object")["kind"].as_str() == Some("unknown"))
        .map(|record| {
            u64::try_from(
                record.as_object().expect("object")["count"]
                    .as_integer()
                    .expect("an exact count"),
            )
            .expect("a non-negative count")
        })
        .sum();
    LiveCompile {
        status: format!("{:?}", outcome.envelope.status).to_lowercase(),
        guarantees,
        verdict: match outcome.envelope.verdict.value().expect("a verdict") {
            Verdict::Evaluation(value) => {
                assert_eq!(value.verdict, EvaluationVerdict::Refuted);
                fields["verdict"].as_str().expect("a string").to_owned()
            }
            other => panic!("context.compile answers with an evaluation verdict: {other:?}"),
        },
        unachieved,
    }
}

// --- the tests ---------------------------------------------------------------------------

#[test]
fn the_sentence_is_closed_against_the_dossier_not_restated() {
    // START_HERE: the PR 11 section's Exit line carries the sentence verbatim, the
    // delivered annotation points at this bone, and the annotation keeps the compiler
    // residual named — erasing the residual without upgrading the grain fails here.
    let pr11 = START_HERE
        .split("### PR 11 — Context Pack schema and compiler v0 [G2]")
        .nth(1)
        .expect("START_HERE carries the PR 11 section")
        .split("### PR 12")
        .next()
        .expect("PR 12 follows PR 11");
    let exit_line = pr11
        .lines()
        .find(|line| line.starts_with("**Exit:**"))
        .expect("PR 11 has an Exit line");
    assert!(
        exit_line.starts_with(&format!("**Exit:** {SENTENCE}")),
        "the exit sentence was reworded; this layer's closure no longer applies:\n{exit_line}"
    );
    assert!(
        exit_line.contains("(delivered: bn-3m65"),
        "the Exit line does not carry this bone's delivered annotation"
    );
    // The annotation's residual, at the grain it now closes at. bn-3m65's assertion required
    // the *compiler* to be named; the compiler is delivered, so what the close requires named
    // is what is still absent — the same requirement, over the residual that is actually
    // outstanding (the PR-5 owed-CBOR / PR-8 F16 precedent).
    for producer in [
        "proof service",
        "correspondence graph",
        "CIR causal-order producer",
        "crashpack producer",
        "intent-registry resolution",
    ] {
        assert!(
            exit_line.contains(producer),
            "the Exit annotation no longer names the missing {producer}"
        );
    }
    assert!(
        exit_line.contains("production grain"),
        "the Exit annotation no longer declares the grain it closes at"
    );

    // The matrix twin: the G0-DX-01 row is the same experiment with the same legs, and
    // its pass condition adds the third leg this layer closes.
    let dx01 = MATRIX
        .lines()
        .find(|line| line.contains("| G0-DX-01 |"))
        .expect("the matrix carries the G0-DX-01 row");
    assert!(
        dx01.contains("200+ event noisy durability failure → Context Pack"),
        "the row's experiment drifted from the exit's case"
    );
    assert!(
        dx01.contains(
            "causal core replay-preserving; ≥10× context reduction; exact expansion handles"
        ),
        "the row's pass condition drifted from the exit's legs"
    );

    // And the campaign artifact says, in its own header, that it is that experiment —
    // so the three texts (sentence, row, evidence) cannot drift apart silently.
    assert!(
        CAMPAIGN.contains("row: 200+ event noisy durability failure -> Context Pack"),
        "the campaign artifact is not the row's experiment"
    );
    assert!(
        CAMPAIGN.contains(
            "legs: causal core replay-preserving | >=10x reduction | exact expansion handles"
        ),
        "the campaign artifact does not carry the row's legs"
    );
}

#[test]
fn the_three_legs_close_over_the_pinned_campaign_artifact() {
    let closed = close(CAMPAIGN).expect("the genuine campaign artifact closes the sentence");

    // The load-bearing values, pinned as freshness: if the campaign regenerates
    // differently, dx01's own golden comparison fails first and these say what moved.
    assert_eq!(closed.events, 225, "the case");
    assert_eq!(closed.reachable_states, 5248, "the complete exploration");
    assert_eq!(
        closed.core,
        [
            "begin_txn",
            "submit_write",
            "ack_before_flush",
            "power_loss"
        ],
        "the replay-preserving core"
    );
    assert_eq!(
        (closed.pack_bytes, closed.raw_bytes, closed.ratio_x100),
        (2423, 48764, 2012),
        "the reduction, recomputed under the packer's counting rule"
    );
    assert_eq!(
        (closed.candidates, closed.selected, closed.omitted),
        (255, 5, 250),
        "INV-007 conservation"
    );
    assert_eq!(
        (closed.expandable, closed.unachieved),
        (249, 1),
        "the manifest splits into the expandable group and rule C1's owed record"
    );
    assert_eq!(
        closed.guarantees, "CausallyClosed",
        "the pack claims exactly what a checker licensed"
    );
    assert_eq!(closed.child_items, 249, "the exact expansion");
    assert_eq!(
        (closed.packed_kept, closed.packed_dropped),
        (244, 5),
        "the budget branch"
    );
    assert_eq!(
        (
            closed.compile_status.as_str(),
            closed.compile_verdict.as_str(),
            closed.compile_class.as_str()
        ),
        ("ok", "refuted", "bounded"),
        "the compile is served and its wire answer is the pack's own"
    );
}

#[test]
fn anti_vacuity_doctored_campaign_artifacts_are_rejected() {
    let doctored = |from: &str, to: &str| {
        let mutated = CAMPAIGN.replacen(from, to, 1);
        assert_ne!(
            mutated, CAMPAIGN,
            "the mutation `{from}` must actually hit the artifact"
        );
        mutated
    };
    let rejection = |campaign: String, expected: &str| {
        let err = close(&campaign).expect_err("a doctored artifact must be rejected");
        assert!(
            err.contains(expected),
            "rejected for the wrong reason: wanted `{expected}`, got `{err}`"
        );
    };

    // The case is not a 200-event case.
    rejection(
        doctored("events=225", "events=150"),
        "not the 200-event case",
    );
    // The verdict flipped: no failure, nothing to diagnose.
    rejection(
        doctored(
            "engine_check_verdict=refuted",
            "engine_check_verdict=satisfied",
        ),
        "not refuted",
    );
    // A drop-one control that stops failing its pre-registered way.
    rejection(
        doctored(
            "drop_begin_txn=disabled_at_submit_write pre_registered",
            "drop_begin_txn=STILL_REFUTES pre_registered",
        ),
        "pre-registered",
    );
    // An artifact that lies about its own arithmetic: recorded ratio != recomputed.
    rejection(
        doctored(
            "ratio_x100=2012 pass_10x=true",
            "ratio_x100=3012 pass_10x=true",
        ),
        "does not recompute",
    );
    // A consistently doctored sub-10x reduction: 24000*100/2423 = 990, arithmetic
    // agrees, the claim itself is below the bar — and the still-recorded pass_10x=true
    // cannot rescue it, because the closure grades the recomputation, not the flag.
    rejection(
        doctored(
            "graded raw_named_json=48764 ratio_x100=2012",
            "graded raw_named_json=24000 ratio_x100=990",
        ),
        "below the 10x",
    );
    // A broken promise: the returned handle is not the one the record derived.
    rejection(
        doctored("returned=ctx_dcb2", "returned=ctx_dead"),
        "promise is broken",
    );
    // A manifest that does not split into its two halves.
    rejection(
        doctored("omitted=250 expandable=249", "omitted=250 expandable=250"),
        "does not split",
    );
    // Rule C1's first sentence: a daemon MUST NOT echo a requested guarantee it did not
    // achieve. An artifact whose pack claims the requested one is rejected.
    rejection(
        doctored(
            "guarantees=CausallyClosed requested=ReplayPreserving",
            "guarantees=CausallyClosed,ReplayPreserving requested=ReplayPreserving",
        ),
        "echoes the requested",
    );
    // An artifact that stops declaring the production grain. This is bn-3m65's own
    // grain-hiding arm, unchanged in shape: only the grain it demands moved.
    rejection(
        doctored("selection=production", "selection=harness"),
        "no longer declares",
    );
    // An artifact that stops declaring stage 2's order as a declared input — the one
    // producer the *selection* leg is still missing.
    rejection(
        doctored("order=declared", "order=production"),
        "no longer declares that stage 2's causal order is a declared input",
    );
    // An artifact that stops naming a missing producer: the production-grain close must
    // never quietly become a claim that nothing is owed.
    rejection(
        doctored(
            "missing_producers: proof service",
            "missing_producers: none",
        ),
        "no longer names the missing proof service",
    );
    // An artifact that claims the production grain while the compiler is not served: the
    // trip-wire in the other direction, and the one this bone's own re-run turns on.
    rejection(
        doctored("context.compile=ok", "context.compile=refused"),
        "requires a served compiler",
    );
    // And an artifact whose envelope verdict disagrees with the evaluation's.
    rejection(
        doctored(
            "context.compile=ok verdict=refuted",
            "context.compile=ok verdict=satisfied",
        ),
        "is not the evaluation's",
    );
}

#[test]
fn the_compiler_is_served_at_production_grain() {
    // The pin bn-3m65 left here read: "`context.compile` is now served: upgrade this exit
    // layer to production grain before touching this pin." It went red when the compiler
    // landed, that upgrade is this file, and the pin now points the other way — a compiler
    // that stops being served turns the exit red again, on evidence this layer produced.
    let live = live_compile();
    assert_eq!(live.status, "ok", "context.compile is served");

    // A served compiler is not enough: the answer has to be a *checked* one. Stage 2's
    // independent closure checker licensed `CausallyClosed` over the selection actually
    // published, and nothing else is claimed — no stage 3 automaton, no stage 8 transcript,
    // and `ProofRelevant` is unreachable by construction (`continuum_context::proof`).
    assert_eq!(
        live.guarantees,
        ["CausallyClosed"],
        "the pack claims exactly the guarantees a checker licensed"
    );
    // Rule C1, live: the requested `ReplayPreserving` is not echoed, and what it owes the
    // manifest is there.
    assert!(
        !live
            .guarantees
            .iter()
            .any(|claim| claim == "ReplayPreserving"),
        "a daemon MUST NOT echo a requested guarantee it did not achieve (C1)"
    );
    assert_eq!(live.unachieved, 1, "C1's owed omission record");
    assert_eq!(live.verdict, "refuted", "the pack's own verdict");
}

// --- the rendered exit artifact ----------------------------------------------------------

fn render_exit_evidence() -> String {
    use std::fmt::Write as _;

    let closed = close(CAMPAIGN).expect("the campaign artifact closes the sentence");
    let live = live_compile();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "PR-11 exit evidence (bn-3m65; re-run at production grain, bn-1y4qc)"
    );
    let _ = writeln!(out, "sentence: {SENTENCE}");
    let _ = writeln!(
        out,
        "source: notes/plan/notes/START_HERE_IMPLEMENTATION.md, PR 11 Exit; matrix twin: \
         G0-DX-01 pass condition (adds: exact expansion handles)"
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "adjudication: closed at PRODUCTION grain; the ten-stage compiler is delivered and \
         is the producer of every leg's selection and assembly"
    );
    let _ = writeln!(
        out,
        "- RFC 0028 Validation: checkers are independent of the compiler that made the \
         claim (INV-004), and C6 forbids trusting the producer, so every predicate the \
         sentence asserts is checker-established"
    );
    let _ = writeln!(
        out,
        "- precedent: PR-5 closed with the CBOR codec owed; PR-8 closed with the \
         crashpack producer absent and the 96 corroborated one layer down (RFC 0026 \
         F16); PR-6 closed over the wire-reachable phase set"
    );
    let _ = writeln!(
        out,
        "- grain guard: context.compile -> {} (live, on this layer's own projection); \
         guarantees={} (C1: requested ReplayPreserving not echoed, {} owed omission); a \
         compiler that stops being served turns this pin red",
        live.status,
        live.guarantees.join(","),
        live.unachieved
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "[the case] events={} (>=200), synthetic durability failure through the \
         production engine; verdict=refuted over {} reachable states",
        closed.events, closed.reachable_states
    );
    let _ = writeln!(
        out,
        "[replay-preserving core] core={} ({} events); replay refutes; witness::shortest \
         == core; 4 drop-one controls + reorder + textual control held; \
         check=production, selection=production(stages 1-2), order=declared(no CIR producer)",
        closed.core.join(","),
        closed.core.len()
    );
    let _ = writeln!(
        out,
        "[substantial reduction] pack={} B vs graded raw={} B = {}.{:02}x (recomputed; \
         graded at the row's >=10x); conservation {} = {} + {} ({} expandable + {} rule-C1); \
         guarantees={}; counting=production(correction 17), assembly=production(RootPack)",
        closed.pack_bytes,
        closed.raw_bytes,
        closed.ratio_x100 / 100,
        closed.ratio_x100 % 100,
        closed.candidates,
        closed.selected,
        closed.omitted,
        closed.expandable,
        closed.unachieved,
        closed.guarantees
    );
    let _ = writeln!(
        out,
        "[exact expansion handles] promised == returned ({}); {} items exact under an \
         empty residual manifest; budget branch keeps the promise ({} kept / {} \
         dropped); production end-to-end (Daemon::dispatch)",
        closed.promised, closed.child_items, closed.packed_kept, closed.packed_dropped
    );
    let _ = writeln!(
        out,
        "[the compile] context.compile={} verdict={} assurance_class={} (both equal the \
         pack's, RFC 0028 \"Wire surface\"); audience selects rendering only",
        closed.compile_status, closed.compile_verdict, closed.compile_class
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "residual, named and routed rather than paid: the proof service (stage 5 \
         configured and refused) and the §16 correspondence graph (stage 7 configured and \
         refused), both PR-1/IMPL-01 scaffolds; the CIR causal-order producer, which \
         leaves stage 2's input declared; the crashpack producer; intent-registry \
         resolution"
    );
    out
}

#[test]
fn the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden() {
    let actual = render_exit_evidence();
    if std::env::var_os("PR11_EXIT_BLESS").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr11_exit_evidence.txt"
        );
        std::fs::write(path, &actual).expect("the golden is writable");
        panic!(
            "PR11_EXIT_BLESS rewrote {path}; rerun without the variable and review the \
             diff as a contract change"
        );
    }
    assert_eq!(
        actual, EXIT_GOLDEN,
        "the exit evidence artifact drifted; to regenerate deliberately, run with \
         PR11_EXIT_BLESS=1 and review the diff as a contract change.\nactual:\n{actual}"
    );
    // Rendered twice, identical bytes: no ambient anything (INV-005).
    assert_eq!(actual, render_exit_evidence());
}
