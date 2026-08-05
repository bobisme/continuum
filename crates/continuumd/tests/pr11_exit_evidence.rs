//! PR-11 exit evidence: *the synthetic 200-event durability case compiles to a
//! replay-preserving core with substantial reduction* (bn-3m65; requirement id
//! `PR-11-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 11's Exit line;
//! the G0-DX-01 row's pass condition adds the third leg, *exact expansion handles*).
//!
//! # The adjudication this file records
//!
//! RFC 0028's ten-stage compiler is not wired anywhere in this workspace —
//! `context.compile` is refused `UnsupportedSemanticFeature`, and the landed PR-11
//! bullets say in their own text that "the compiler itself … is unbuilt". So the exit
//! question is not whether the sentence's facts exist (bn-37gu's campaign established
//! them adversarially) but whether the word *compiles* may be read at the campaign's
//! declared grain, or demands the ten-stage pipeline as producer first. This bone
//! adjudicated **declared grain**, from four texts:
//!
//! 1. **RFC 0028 "Validation" makes every predicate of the sentence
//!    compiler-independent.** "Every guarantee has a checker, and the checkers are
//!    independent of the compiler that made the claim — INV-004's discipline applied
//!    here"; C6 adds that a promotion-relevant consumer "MUST re-run the checker rather
//!    than read the field". In this program *replay-preserving* is never something a
//!    compiler confers by having run; it is what the checker establishes over the
//!    artifact — and the campaign's replay/refutation checks, byte counting, and
//!    expansion handles are all the production checkers' own answers.
//! 2. **The exit-precedent conventions close a PR's sentence over the PR's delivered
//!    surface with the boundary named at the point it is crossed.** PR-5's exit closed
//!    with the CBOR half of every golden vector explicitly owed; PR-8's closed with the
//!    crashpack producer absent ("the field exists, the producer does not") and the 96
//!    corroborated one layer down under RFC 0026 F16; PR-6's quantifier was closed over
//!    the wire-reachable phase set. A named, typed, routed absence does not block an
//!    exit whose sentence is true at the delivered grain; a silent absence would.
//! 3. **The PR's own scope defines "compiler v0", and every bullet of it is
//!    delivered** — several of them compiler stages in substance: bn-38p2's
//!    `BudgetPacker` is stage 10 with the RFC's packing MUSTs held as tests, the closed
//!    accounting is the manifest-reconciliation validation row, the expansion protocol
//!    is wire-live, and RFC 0028 itself names "plain causal slice + expansion, with
//!    stage 9 disabled" as a configuration a deployment MUST be able to run. No bullet
//!    claims stages 1–9, and none silently does here: the sentence closes at the grain
//!    the annotation itself declares.
//! 4. **The controlling instance already exists.** The G0-DX-01 row — this sentence in
//!    the matrix's own words, under the heavier consequence ("redesign evidence
//!    model") — was accepted at exactly these grains, with the compiler named in the
//!    Decision cell's first sentence. Holding the PR's completion claim to a stricter
//!    standard than the kill-criteria row it instantiates has no text demanding it.
//!
//! The one text reading the sentence as compiler-demanding — the DX-01 row's routing
//! parenthetical "whose sentence is this row's experiment run *by the compiler*" — is
//! bn-37gu's routing gloss, submitted to this bone for adjudication rather than
//! settled; no normative text (START_HERE's PR-11 section, RFC 0028, plan §21/§25)
//! conditions the exit on the pipeline. Plan §25's own demonstration frames the case
//! consumer-side: "Agent receives a 200-event failing run through a 4-event Context
//! Pack". What stays true either way — and stays *said* — is that the ten-stage
//! compiler is PR-11's undelivered center: it is the exit's named owed producer, and
//! [`the_compiler_refusal_is_the_exits_grain_guard`] pins the refusal so a landed
//! compiler turns this file red and forces the exit evidence up to production grain
//! instead of letting the declared-grain close linger silently.
//!
//! # What this layer is, and is not
//!
//! The independent exit layer over the bn-37gu campaign (the bn-3jtr/bn-2hmk shape):
//! it touches none of `dx01_falsification.rs`'s tests and none of `src/`, closes the
//! sentence against the dossier's own texts rather than restating them, and re-derives
//! every leg from the byte-pinned campaign artifact instead of trusting its recorded
//! verdict flags — the closure recomputes the reduction ratio from the artifact's own
//! byte counts, recomputes witness-equals-core from the two membership lines,
//! recomputes the INV-007 conservation equation, and recomputes the promise equality
//! from the two handles, so an artifact that lies about its own arithmetic is rejected
//! by the same function the genuine artifact passes
//! ([`anti_vacuity_doctored_campaign_artifacts_are_rejected`] proves each rejection
//! arm fires). The campaign itself keeps the facts fresh: `dx01_falsification.rs`
//! re-renders the artifact from two independent builds on every run and fails on
//! drift, so citing its golden bytes here cites a live campaign, not a memory.
//!
//! Grain, exactly as the campaign declares it inside the artifact (and this closure
//! *requires* the declaration — a doctored artifact that stops declaring the harness
//! grain or stops naming the missing compiler is rejected, because the declared-grain
//! close is honest only while it is declared): leg 1's replay/refutation *checks* are
//! production and its stage-2 core *selection* is harness; leg 2's counting rule is
//! production (RFC 0028 correction 17) and its root assembly is harness; leg 3 is
//! production end to end through `Daemon::dispatch`.
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
//!   [`the_compiler_refusal_is_the_exits_grain_guard`];
//! - the whole exit rendered as one byte-stable artifact, pinned at
//!   `tests/golden/pr11_exit_evidence.txt` —
//!   [`the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden`]. Golden
//!   regeneration is explicit and cannot pass: `PR11_EXIT_BLESS=1` rewrites the golden
//!   and then panics, so a blessing run is always red and the diff is always reviewed
//!   as a contract change (INV-004).

use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextCompileRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, EpochIdentity, OperationName, ProtocolVersion,
    RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, Encoding, ErrorCode, ResultStatus};

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
    promised: String,
    child_items: u64,
    packed_kept: u64,
    packed_dropped: u64,
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
    if candidates != selected + omitted {
        return Err(format!(
            "INV-007 conservation fails: {candidates} != {selected} + {omitted}"
        ));
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
    if child_items != omitted {
        return Err(format!(
            "the expansion did not return the omitted items: {child_items} != {omitted}"
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

    // The grain declaration is load-bearing: the declared-grain close is honest only
    // while the artifact keeps declaring the harness grain and naming the missing
    // producer. An artifact that stops is rejected, not accepted more strongly.
    let leg1 = line_with(campaign, "leg1: ")?;
    if !leg1.contains("selection=harness") || !leg1.contains("check=production") {
        return Err(
            "leg 1 no longer declares its selection=harness/check=production grain".to_owned(),
        );
    }
    let leg2 = line_with(campaign, "leg2: ")?;
    if !leg2.contains("counting=production") || !leg2.contains("assembly=harness") {
        return Err(
            "leg 2 no longer declares its counting=production/assembly=harness grain".to_owned(),
        );
    }
    if !line_with(campaign, "leg3: ")?.contains("production end-to-end") {
        return Err("leg 3 no longer declares production end-to-end".to_owned());
    }
    if !line_with(campaign, "missing_producers:")?.contains("ten-stage compiler") {
        return Err(
            "the artifact no longer names the ten-stage compiler as the missing \
                    producer — the declared-grain close requires the declaration"
                .to_owned(),
        );
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
        promised,
        child_items,
        packed_kept,
        packed_dropped,
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

fn compile_request() -> OperationRequest {
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
            evidence_root: ArtifactHandle::new("ev_dx01").expect("an artifact handle"),
            question: "why did AckImpliesDurable fail?".to_owned(),
            audience: Optional::Absent,
            guarantees: Optional::Present(vec!["ReplayPreserving".to_owned()]),
        }),
    }
}

/// The refusal's error-code token, live off a real dispatch.
fn live_compile_refusal() -> String {
    let mut daemon = daemon();
    let outcome = daemon.dispatch(&compile_request());
    assert_eq!(outcome.envelope.status, ResultStatus::Error);
    assert!(matches!(outcome.payload, Payload::None));
    let error = outcome
        .envelope
        .error
        .value()
        .expect("a refusal carries an error object")
        .clone();
    format!("{:?}", error.code)
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
    assert!(
        exit_line.contains("ten-stage compiler"),
        "the Exit annotation no longer names the compiler residual — the declared-grain \
         close requires it (the PR-5 owed-CBOR / PR-8 F16 precedent)"
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
        (2791, 48764, 1747),
        "the reduction, recomputed under the packer's counting rule"
    );
    assert_eq!(
        (closed.candidates, closed.selected, closed.omitted),
        (258, 10, 248),
        "INV-007 conservation"
    );
    assert_eq!(closed.child_items, 248, "the exact expansion");
    assert_eq!(
        (closed.packed_kept, closed.packed_dropped),
        (243, 5),
        "the budget branch"
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
            "ratio_x100=1747 pass_10x=true",
            "ratio_x100=2747 pass_10x=true",
        ),
        "does not recompute",
    );
    // A consistently doctored sub-10x reduction: 27000*100/2791 = 967, arithmetic
    // agrees, the claim itself is below the bar — and the still-recorded pass_10x=true
    // cannot rescue it, because the closure grades the recomputation, not the flag.
    rejection(
        doctored(
            "graded raw_named_json=48764 ratio_x100=1747",
            "graded raw_named_json=27000 ratio_x100=967",
        ),
        "below the 10x",
    );
    // A broken promise: the returned handle is not the one the record derived.
    rejection(
        doctored("returned=ctx_4d77", "returned=ctx_dead"),
        "promise is broken",
    );
    // An artifact that stops declaring the harness grain.
    rejection(
        doctored("selection=harness", "selection=production"),
        "no longer declares",
    );
    // An artifact that stops naming the missing compiler: the declared-grain close
    // must never quietly become a production-grain claim.
    rejection(
        doctored(
            "missing_producers: ten-stage compiler",
            "missing_producers: none",
        ),
        "ten-stage compiler",
    );
}

#[test]
fn the_compiler_refusal_is_the_exits_grain_guard() {
    // The declared-grain close rests on `context.compile` being genuinely refused —
    // "a compiled-looking pack with no compiler behind it is exactly the degradation
    // that rule names" (`daemon::context`). Re-pinned here, live, so the day a
    // compiler lands this file goes red and the exit evidence is re-run at production
    // grain instead of the declared-grain close lingering silently.
    let mut daemon = daemon();
    let outcome = daemon.dispatch(&compile_request());
    assert_eq!(outcome.envelope.status, ResultStatus::Error);
    let error = outcome
        .envelope
        .error
        .value()
        .expect("a refusal carries an error object");
    assert_eq!(
        error.code,
        ErrorCode::UnsupportedSemanticFeature,
        "context.compile is now served: upgrade this exit layer to production grain \
         before touching this pin"
    );
    assert!(
        matches!(outcome.payload, Payload::None),
        "a refusal serves nothing"
    );
}

// --- the rendered exit artifact ----------------------------------------------------------

fn render_exit_evidence() -> String {
    use std::fmt::Write as _;

    let closed = close(CAMPAIGN).expect("the campaign artifact closes the sentence");
    let refusal = live_compile_refusal();

    let mut out = String::new();
    let _ = writeln!(out, "PR-11 exit evidence (bn-3m65)");
    let _ = writeln!(out, "sentence: {SENTENCE}");
    let _ = writeln!(
        out,
        "source: notes/plan/notes/START_HERE_IMPLEMENTATION.md, PR 11 Exit; matrix twin: \
         G0-DX-01 pass condition (adds: exact expansion handles)"
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "adjudication: closed at the declared grain; the ten-stage compiler is the \
         exit's named owed producer"
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
        "- grain guard: context.compile -> {refusal} (re-pinned live in this file); a \
         landed compiler turns the pin red and this exit re-runs at production grain"
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
         check=production, selection=harness(stage 2)",
        closed.core.join(","),
        closed.core.len()
    );
    let _ = writeln!(
        out,
        "[substantial reduction] pack={} B vs graded raw={} B = {}.{:02}x (recomputed; \
         graded at the row's >=10x); conservation {} = {} + {}; \
         counting=production(correction 17), assembly=harness",
        closed.pack_bytes,
        closed.raw_bytes,
        closed.ratio_x100 / 100,
        closed.ratio_x100 % 100,
        closed.candidates,
        closed.selected,
        closed.omitted
    );
    let _ = writeln!(
        out,
        "[exact expansion handles] promised == returned ({}); {} items exact under an \
         empty residual manifest; budget branch keeps the promise ({} kept / {} \
         dropped); production end-to-end (Daemon::dispatch)",
        closed.promised, closed.child_items, closed.packed_kept, closed.packed_dropped
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "residual, owed to the compiler's own bone: ten-stage compiler (candidate-set \
         construction, stage-2 slicing licensing CausallyClosed/ReplayPreserving, \
         auditable intermediates per RFC 0030 compared artifact 5); root-pack \
         assembly; crashpack producer; intent-registry resolution"
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
