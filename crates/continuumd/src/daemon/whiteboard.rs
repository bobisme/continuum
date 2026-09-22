//! The `whiteboard` family: `compile` — plan §11.5's note compiler, on the wire.
//!
//! # What this operation is, and what it deliberately is not
//!
//! > For complex invention tasks, agents may use a structured whiteboard […] The compiler
//! > turns whiteboard entries into typed graph proposals, rejecting references to
//! > nonexistent artifacts or unsupported status claims.
//! >
//! > — plan §11.5
//!
//! A whiteboard is where an agent *works*; the graph is what it may *claim*. This is the
//! one-way door between them, and every node that comes out of it lands at
//! [`ClaimStatus::BOTTOM`] — `proposed`. Nothing here writes a status, mints a
//! [`Promotion`](super::evidence::Promotion), or can reach RFC 0038's compare-and-set:
//! that type's fields are private to [`daemon::evidence`](super::evidence) and this module
//! is not that module, which is the same compile-time device
//! [`observe`](super::observe) gets its half of INV-004 from.
//!
//! # The two questions RFC 0038 left open, and where their answers live
//!
//! | Question | Answer | Where it is normative |
//! |---|---|---|
//! | daemon-side or client-side compilation | daemon-side | `rule whiteboard.compilation` clause 1, RFC 0038 W10 |
//! | the note as an `Opaque` or as a declared struct | `Opaque`, governed by the note schema | `rule whiteboard.compilation` clause 2, RFC 0038 W11 |
//!
//! **Daemon-side**, because docs/44's "references must resolve" is a refusal only the
//! holder of the graph can perform, and because a wire of *proposals* would be a wire of
//! caller-filled node kinds, claim identities, and provenance actors — and a field a
//! caller can fill is a field a caller can lie in (RFC 0038 D3). W2 says an author "MUST
//! NOT be able to name the kind directly: if it could, the seven headings would be
//! decoration", and only a daemon that reads the sections itself can hold that.
//!
//! **An `Opaque`**, because `notes/plan/schemas/whiteboard-note.schema.json` is normative
//! for a note's shape (INV-003) and a second declaration of the same twelve-member
//! document in the IDL would be the two-authority disagreement `rule
//! conformance.registry_agreement` exists to prevent one namespace down.
//! `intent.propose_revision`'s `changes` is the precedent, and [`note`] below is the
//! reader — the same seam [`daemon::evidence`](super::evidence)'s `node_record` sits on
//! from the other side: the daemon owns the wire's JSON spelling, and
//! `continuum-evidence` owns what a note *means*.
//!
//! # Which compiler runs
//!
//! The decisions are `continuum-evidence`'s and are consulted rather than restated:
//! [`Section::ALL`] is the section list, [`Section::node_kind`] is W2's mapping,
//! [`Section::label`] is the label every proposal carries, and the note's own types are
//! what refuse an unsupported conclusion ([`Decision::new`]) and a reserved note class
//! ([`WhiteboardNote::new`]). What is *not* borrowed is
//! [`WhiteboardNote::compile`] itself: that function resolves against a
//! [`EvidenceGraph`](continuum_evidence::graph::EvidenceGraph), whose keys are the
//! *within-graph* content keys RFC 0038 D4 says never reach the wire, while the graph this
//! operation compiles against is the daemon's, whose keys are the wire's `ev_` handles.
//! Mirroring one into the other would mean rebuilding every held node under a second set
//! of names, and a mirror node's identity would have to be derived from a provenance
//! record the daemon reconstructs — a fabrication in the one place D4 forbids one.
//! `tests/daemon_whiteboard.rs` pins the two against each other section by section, so the
//! mapping cannot drift.
//!
//! # The order of the refusals, and why it is this order
//!
//! | # | Refusal | Code | Source |
//! |---|---|---|---|
//! | 1 | the bytes are not a conforming note | `MalformedRequest` | INV-016: validate before reading any member |
//! | 2 | the note's `author` is not the admitted actor | `CapabilityDenied` | RFC 0027 T4, RFC 0038 W12 |
//! | 3 | a reference names nothing the graph holds a node about | `CapabilityDenied` | docs/44 "references must resolve"; RFC 0027 X2 forbids a distinguishable not-found |
//! | 4 | a decision's only support would be the decision itself | `InsufficientEvidence` | `evidence.link`'s self-edge refusal, over the same graph |
//! | 5 | no well-formed identity could be derived | `PublicationAborted` | the identity seam answered nothing |
//!
//! One runs before two because a document the daemon has not yet decided is a note is not
//! a document it may read an `author` out of. Two runs before three because an
//! authorization failure MUST NOT be distinguishable from any other one (RFC 0027 T4), and
//! a caller told "your references do not resolve" before being told "you are not this
//! author" would have learned something about the graph it was never admitted to.
//!
//! # Refusal is whole-note
//!
//! Every refusal above is raised *before* anything is appended: the nodes, the edges, and
//! the task proposals are computed in full and only then written. A half-compiled note
//! would make "what did the note say" depend on which entries happened to resolve, and
//! INV-017's per-artifact atomicity read at the note's granularity (RFC 0038 W8) forbids
//! it. There is no merge, dedup, or reconciliation step either — two entries in tension
//! are two proposals, and resolving one by authoring order is what plan §11.7 forbids.
//!
//! # No clock, no ambient anything
//!
//! The note supplies its own `created_at` and it becomes every proposal's
//! `provenance.created_at` (RFC 0038 W9), so this module reads no clock — unlike
//! `observe.ingest` and `evidence.link`, which must and therefore refuse a daemon built
//! without one. Nothing here is published to the [`ReferenceStore`]: a note is not one of
//! plan §4.4's content-addressed classes (W9), the subjects it names need not resolve
//! (W4), and there are no bytes for this operation to hold.

use continuum_evidence::claim_status::ClaimStatus;
use continuum_evidence::node::{ClaimId, NodeKind};
use continuum_evidence::provenance::{ArtifactRef, Timestamp as NoteTimestamp};
use continuum_evidence::whiteboard::{
    Decision, Entry, Experiment, NoteText, Section, WhiteboardNote,
};
use continuum_intent::canonical_json::Json;
use continuum_workspace::artifact_path::ArtifactClass;

use super::evidence::edge_identity;
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::{DaemonState, EvidenceEdge, EvidenceNode, StatusWrite};
use super::{Services, evidence as evidence_family};
use crate::protocol::envelope::{StructuralVerdictValue, Verdict};
use crate::protocol::operations::whiteboard::{
    WhiteboardCompileRequest, WhiteboardCompileResponse,
};
use crate::protocol::scalar::{Commitment, EvidenceHandle, Timestamp};
use crate::protocol::shared::WhiteboardTaskProposal;
use crate::protocol::spec::{Nullable, Optional, ProtocolEnum};
use crate::protocol::task::EvidenceEvent;
use crate::protocol::vocabulary::{ErrorCode, EvidenceEventKind, StructuralOutcome};

/// The `whiteboard` namespace's one operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct WhiteboardFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// A data table rather than a comment, so `tests/daemon_whiteboard.rs` can hold every one
/// of them to `rule errors.common` ∪ the operation's `errors` clause instead of trusting
/// that the handler stayed inside it.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("whiteboard.compile", ErrorCode::MalformedRequest),
    ("whiteboard.compile", ErrorCode::CapabilityDenied),
    ("whiteboard.compile", ErrorCode::InsufficientEvidence),
    ("whiteboard.compile", ErrorCode::PublicationAborted),
];

/// The domain tag that separates a whiteboard proposal's preimage from every other
/// evidence preimage (`rule whiteboard.compilation`, clause 4).
///
/// It carries a `/`, which is outside the artifact-handle character class every
/// `Commitment` this daemon derives belongs to, so no `observe.ingest` node preimage —
/// whose first part is such a commitment — can spell one of these. It is also distinct
/// from [`EDGE_IDENTITY_DOMAIN`](super::evidence::EDGE_IDENTITY_DOMAIN), so the three
/// derivations are pairwise disjoint rather than merely unlikely to collide.
pub const NOTE_IDENTITY_DOMAIN: &str = "whiteboard-note/v1";

impl OperationFamily for WhiteboardFamily {
    fn namespace(&self) -> &'static str {
        "whiteboard"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        match arguments {
            // A note names no snapshot and no intent, and every artifact it can reach is
            // an evidence-graph one: the class is the whole scope claim, exactly as it is
            // for `observe.ingest` and `evidence.link`.
            Arguments::WhiteboardCompile(_) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![ArtifactClass::Evidence.token()],
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        _store: &continuum_workspace::publication::ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::WhiteboardCompile(request) => compile(call, request, state, services),
            // Unreachable: the dispatcher checked shape agreement against the registry
            // before routing. A typed refusal rather than an `unreachable!`, because a
            // daemon does not abort on its own invariant.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

// --- whiteboard.compile -------------------------------------------------------------

fn compile(
    call: &Call<'_>,
    request: &WhiteboardCompileRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    // 1. The note is untrusted source (INV-016). It is validated against
    //    `whiteboard-note.schema.json` *in full*, before any member of it is read for
    //    meaning — a daemon that read `author` first would be acting on a document it had
    //    not yet decided was one.
    let note = note::read(request.note.as_bytes())?;

    // 2. The author is the note's, and it MUST be the admitted actor. Not trusted (every
    //    proposal takes it as `provenance.actor`) and not overwritten (the caller holds
    //    the document): agreement is the only remaining reading, and the refusal is RFC
    //    0027 T4's own code.
    if note.author().as_str() != call.grant.actor.as_str() {
        return Err(Fault::denied());
    }

    // 3. docs/44 "references must resolve", over every section including experiments, and
    //    before anything is built. A reference the graph holds no node about is a denial
    //    and never a distinguishable not-found (RFC 0027 X2): a note is otherwise an
    //    existence oracle over the whole graph, one handle per line.
    let held: Vec<Commitment> = state
        .evidence_nodes()
        .map(|(_, node)| node.artifact.clone())
        .collect();
    let resolves =
        |reference: &ArtifactRef| held.iter().any(|it| it.as_str() == reference.as_str());
    for section in Section::ALL {
        for entry in entries(&note, section) {
            if entry.references().any(|it| !resolves(it)) {
                return Err(Fault::denied());
            }
        }
    }
    for experiment in note.experiments() {
        if experiment.references().any(|it| !resolves(it)) {
            return Err(Fault::denied());
        }
    }

    let key = idempotency_key(call);
    let created_at = Timestamp::new(note.created_at().as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the note's `created_at` is not a well-formed timestamp",
        )
    })?;

    // 4. The proposals. Built in `Section::ALL` order — plan §11.5's own order — and, per
    //    section, in the note's line order, so the response lists what the note said in
    //    the order it said it.
    let mut nodes: Vec<(EvidenceHandle, EvidenceNode)> = Vec::new();
    let mut decisions: Vec<(usize, EvidenceHandle)> = Vec::new();
    for section in Section::ALL {
        let Some(kind) = section.node_kind() else {
            // `Experiment`, the one section that proposes no node (W3).
            continue;
        };
        for (index, entry) in entries(&note, section).iter().enumerate() {
            let (handle, node) =
                proposal(&note, section, kind, entry, &created_at, &key, services)?;
            if section == Section::Decision {
                decisions.push((index, handle.clone()));
            }
            nodes.push((handle, node));
        }
    }

    // 5. A decision's supporting edges (W6): one `SUPPORTS` edge from every node the graph
    //    holds *about* each cited artifact. Where several nodes are about one artifact,
    //    one edge is drawn from each — choosing between them is a semantic judgement this
    //    compiler cannot make, and citing an artifact is the only way the format lets an
    //    author name support.
    let mut edges: Vec<(EvidenceHandle, EvidenceEdge)> = Vec::new();
    for (index, decision) in &decisions {
        let entry = note.decisions()[*index].entry();
        for reference in entry.references() {
            let supporting: Vec<EvidenceHandle> = state
                .evidence_nodes()
                .filter(|(_, node)| node.artifact.as_str() == reference.as_str())
                .map(|(handle, _)| handle.clone())
                .collect();
            for from in supporting {
                if &from == decision {
                    return Err(Fault::new(
                        ErrorCode::InsufficientEvidence,
                        "a decision cites an artifact the decision node itself is about, so \
                         one of its supporting edges would run from the conclusion to \
                         itself; a conclusion that supports itself supports nothing",
                    ));
                }
                let relation = continuum_evidence::edge::EdgeRelation::Supports;
                let handle = edge_identity(services, &relation, &from, decision)
                    .map_err(|_| identity_unavailable())?;
                edges.push((
                    handle,
                    EvidenceEdge {
                        relation,
                        from,
                        to: decision.clone(),
                        producer: call.grant.actor.clone(),
                        tool: tool_of(&note),
                        created_at: created_at.clone(),
                        inputs: vec![
                            note.note_id().as_str().to_owned(),
                            reference.as_str().to_owned(),
                        ],
                        idempotency_key: key.clone(),
                    },
                ));
            }
        }
    }

    // 6. Experiments (W3). A task proposal is not a graph artifact: nothing is appended
    //    for one, and the response names the line and what it would be run against.
    let tasks: Vec<WhiteboardTaskProposal> = note
        .experiments()
        .iter()
        .enumerate()
        .map(|(index, experiment)| WhiteboardTaskProposal {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            references: experiment
                .references()
                .map(|it| Commitment::new(it.as_str()))
                .collect(),
        })
        .collect();

    // 7. Nothing was appended until here, so every refusal above left the graph exactly as
    //    it found it (W8).
    let mut node_handles = Vec::with_capacity(nodes.len());
    for (handle, node) in nodes {
        let claim_id = node.claim_id.clone();
        let (_, appended) = state.append_evidence(handle.clone(), node);
        if appended {
            state.record_evidence_event(EvidenceEvent {
                at: created_at.clone(),
                kind: EvidenceEventKind::NodePublished,
                node: Optional::Present(handle.clone()),
                edge: Optional::Absent,
                status: Optional::Absent,
                claim_id: Optional::Present(claim_id),
            });
        }
        node_handles.push(handle);
    }
    let mut edge_handles = Vec::with_capacity(edges.len());
    for (handle, edge) in edges {
        let (_, appended) = state.append_edge(handle.clone(), edge);
        if appended {
            state.record_evidence_event(EvidenceEvent {
                at: created_at.clone(),
                kind: EvidenceEventKind::EdgePublished,
                node: Optional::Absent,
                edge: Optional::Present(handle.clone()),
                status: Optional::Absent,
                claim_id: Optional::Absent,
            });
        }
        edge_handles.push(handle);
    }

    let artifacts = node_handles
        .iter()
        .chain(edge_handles.iter())
        .map(artifact)
        .collect::<Result<Vec<_>, Fault>>()?;

    Ok(Effect::new(
        Payload::WhiteboardCompile(WhiteboardCompileResponse {
            nodes: node_handles,
            edges: edge_handles,
            tasks,
        }),
        Nullable::Value(Verdict::Structural(StructuralVerdictValue {
            outcome: StructuralOutcome::Created,
        })),
    )
    .with_artifacts(artifacts))
}

/// The entry-bearing lines of one section, with `Decision`'s unwrapped.
///
/// [`WhiteboardNote::entries`] yields nothing for `Decision` because its element type
/// differs; the mapping from a decision to its entry is [`Decision::entry`], and this is
/// the one place the daemon needs both spellings under one name.
fn entries(note: &WhiteboardNote, section: Section) -> Vec<&Entry> {
    match section {
        Section::Decision => note.decisions().iter().map(Decision::entry).collect(),
        _ => note.entries(section).iter().collect(),
    }
}

/// One proposed node, and the identity it is filed under.
///
/// Every field is a function of the note and of `continuum-evidence`'s own decisions.
/// There is no status parameter, so `proposed` is not a value this function chose — it is
/// the only status it can write.
fn proposal(
    note: &WhiteboardNote,
    section: Section,
    kind: NodeKind,
    entry: &Entry,
    created_at: &Timestamp,
    key: &str,
    services: &Services,
) -> Result<(EvidenceHandle, EvidenceNode), Fault> {
    let wire_kind = evidence_family::wire_node_kind(kind).ok_or_else(|| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the node kind this section proposes is outside the wire's closed vocabulary",
        )
    })?;
    let subject = Commitment::new(entry.subject().as_str());
    let handle = node_identity(services, wire_kind, &subject, entry.claim().as_str())?;
    let mut inputs = vec![note.note_id().as_str().to_owned()];
    inputs.extend(entry.references().map(|it| it.as_str().to_owned()));
    let node = EvidenceNode {
        kind: wire_kind,
        // A proposal offers no class of evidence toward its claim: it sits at `proposed`
        // and asserts nothing (W5). `EvidenceKind`'s thirteen members are each *a class of
        // evidence offered*, so naming one would be an overstatement — RFC 0026 F21.
        evidence_kind: None,
        claim_id: entry.claim().as_str().to_owned(),
        artifact: subject,
        // From the admitted grant by way of the note's own `author`, which
        // `rule whiteboard.compilation` requires to be the same identity.
        producer: crate::protocol::scalar::ActorId::new(note.author().as_str()).map_err(|_| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "the note's `author` is not a well-formed actor identity",
            )
        })?,
        // docs/44's "credit and provenance: actor / tool / model version". The note's own
        // tool, and nothing else: which *section* proposed the node is already recoverable
        // from its `kind` (six proposing sections, six distinct kinds — the same fact
        // `rule whiteboard.compilation` clause 4 rests a node's identity on), while the
        // model version that wrote the note is recoverable from nowhere else, because a
        // note is provisional and the graph does not hold it (W9). Empty when the note
        // named none, which `node_record` writes as an omitted member rather than as a
        // name.
        tool: tool_of(note),
        created_at: created_at.clone(),
        inputs,
        // `continuum-evidence`'s own spelling, `whiteboard:<section key>`, in the node
        // schema's own member for it. The compiler's label is not restated here: it is
        // read off [`Section::label`], so which section proposed a node is one decision
        // and not two.
        labels: vec![section.label().as_str().to_owned()],
        // "The key the ledger filed this append under" — the *envelope's*, as
        // `observe.ingest` and `evidence.link` both record it. `rule idempotency.replay`
        // is stated against the envelope's key and a library compiling a note off the
        // wire has no envelope, which is why `continuum-evidence` derives a per-line key
        // instead; neither is inside the node's identity (plan §11.7), so the two
        // spellings cannot fork the graph.
        idempotency_key: key.to_owned(),
        history: vec![StatusWrite {
            // The unpromoted entry status. No field of the note reaches it: the format has
            // no status member at all (W5).
            status: ClaimStatus::BOTTOM,
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }],
        redaction: None,
    };
    Ok((handle, node))
}

/// The identity of a node a whiteboard note proposed (`rule whiteboard.compilation`).
///
/// Four length-framed parts through the same [`ContentIdentifier`] seam and the same
/// artifact class that name every other evidence artifact: the domain tag, the node's
/// kind, the subject it is about, and the claim identity — everything the node *asserts*.
///
/// The note handle, the author, and the time are deliberately outside it, exactly as
/// `provenance` is outside a node's and an edge's (RFC 0038 D4): a replayed write MUST
/// return the original node identity "across processes and producers", and two agents who
/// write the same line about the same claim are making one assertion. Which section
/// proposed the node is recoverable from its kind, because the six proposing sections name
/// six distinct kinds (W2).
///
/// # Errors
///
/// [`ErrorCode::PublicationAborted`] when the identity seam cannot name the preimage, or
/// when the derived token is not a well-formed `ev_` handle.
///
/// [`ContentIdentifier`]: continuum_workspace::publication::ContentIdentifier
fn node_identity(
    services: &Services,
    kind: crate::protocol::vocabulary::EvidenceNodeKind,
    subject: &Commitment,
    claim: &str,
) -> Result<EvidenceHandle, Fault> {
    let mut preimage = Vec::new();
    for part in [
        NOTE_IDENTITY_DOMAIN.as_bytes(),
        kind.as_wire().as_bytes(),
        subject.as_str().as_bytes(),
        claim.as_bytes(),
    ] {
        preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
        preimage.extend_from_slice(part);
    }
    let stored = services
        .identifier()
        .identify(ArtifactClass::Evidence, &preimage)
        .map_err(|_| identity_unavailable())?;
    EvidenceHandle::new(&stored.to_string()).map_err(|_| identity_unavailable())
}

fn identity_unavailable() -> Fault {
    Fault::new(
        ErrorCode::PublicationAborted,
        "no well-formed content identity could be derived for the proposed evidence artifact",
    )
    // Deterministic: the identity is a function of the request, so a retry fails the same way.
    .not_retryable()
}

/// The note's `tool`, or the empty string when it named none.
///
/// `provenance.tool` is a required member of the edge record, so an edge a note drew names
/// the note's tool when there is one; the per-node spelling is the section label instead,
/// because a node's derivation is which line of which section proposed it.
fn tool_of(note: &WhiteboardNote) -> String {
    note.tool()
        .map_or_else(String::new, |it| it.as_str().to_owned())
}

fn idempotency_key(call: &Call<'_>) -> String {
    call.envelope
        .idempotency_key
        .value()
        .cloned()
        .unwrap_or_default()
}

fn artifact(handle: &EvidenceHandle) -> Result<crate::protocol::envelope::ArtifactRef, Fault> {
    Ok(crate::protocol::envelope::ArtifactRef {
        kind: ArtifactClass::Evidence.token().to_owned(),
        handle: crate::protocol::scalar::ArtifactHandle::new(handle.as_str())
            .map_err(|_| identity_unavailable())?,
        commitment: Optional::Present(Commitment::new(handle.as_str())),
        redacted: Optional::Absent,
    })
}

/// Reading a whiteboard note out of the wire's `Opaque`.
///
/// This is the schema's *reader*, and it is the daemon's for the reason
/// [`daemon::evidence`](super::evidence)'s `node_record` is the daemon's *writer*: the
/// library speaks `continuum_value::Value` and the wire speaks canonical JSON, and neither
/// is entitled to the other's spelling. What the reader does not decide is what a note
/// *means* — every member it accepts goes straight into a `continuum-evidence` type whose
/// constructor carries the schema's own constraints, so `minLength: 1` on `text`,
/// `minItems: 1` on a decision's `references`, the handle pattern, and the reserved
/// note-class exclusion are all refused by the library rather than re-checked here.
///
/// The whole document is validated before anything is read for meaning (INV-016), and
/// `additionalProperties: false` is enforced by rejecting any member the schema does not
/// declare — at every level. A member the schema does not admit is as invalid as one
/// missing where it does.
mod note {
    use super::{
        ArtifactRef, ClaimId, Decision, Entry, ErrorCode, Experiment, Fault, Json, NoteText,
        NoteTimestamp, WhiteboardNote,
    };
    use std::collections::BTreeMap;

    use continuum_evidence::actor::ActorId;
    use continuum_evidence::provenance::Tool;
    use continuum_evidence::whiteboard::{SCHEMA_EPOCH, SCHEMA_ID};

    /// The twelve required members plus the one optional member, in the schema's order.
    const MEMBERS: [&str; 13] = [
        "author",
        "candidate_invariants",
        "counterexamples",
        "created_at",
        "decisions",
        "experiments",
        "goal",
        "known_facts",
        "note_id",
        "schema_epoch",
        "schema_id",
        "tool",
        "unresolved_obligations",
    ];

    /// An `entry`'s and a `decision`'s four members.
    const ENTRY_MEMBERS: [&str; 4] = ["claim", "references", "subject", "text"];

    /// An `experiment`'s two members.
    const EXPERIMENT_MEMBERS: [&str; 2] = ["references", "text"];

    fn malformed(detail: &'static str) -> Fault {
        Fault::new(ErrorCode::MalformedRequest, detail)
    }

    /// Read a note, or refuse the whole document.
    pub(super) fn read(bytes: &[u8]) -> Result<WhiteboardNote, Fault> {
        let document = Json::parse(bytes)
            .map_err(|_| malformed("the note is not a canonical JSON document"))?;
        let fields = object(&document, &MEMBERS)?;

        // The class identity and the epoch are `const`-pinned by the schema, so a document
        // claiming another class or another epoch is not this artifact class at all — and
        // is refused before any of its content is read (docs/09 T13: a typed rejection is
        // recoverable, a misread artifact is not).
        if text(fields, "schema_id")? != SCHEMA_ID {
            return Err(malformed(
                "the note does not name `whiteboard-note.schema.json` as its schema class",
            ));
        }
        let epoch = match fields.get("schema_epoch") {
            Some(Json::Integer(value)) => *value,
            _ => return Err(malformed("the note declares no integer `schema_epoch`")),
        };
        if u128::try_from(epoch).ok() != Some(SCHEMA_EPOCH) {
            return Err(malformed(
                "the note declares a schema epoch this daemon does not read",
            ));
        }

        let note_id = handle(text(fields, "note_id")?)?;
        let author = ActorId::new(text(fields, "author")?)
            .map_err(|_| malformed("the note's `author` is not one of the four actor schemes"))?;
        let created_at = NoteTimestamp::new(text(fields, "created_at")?)
            .map_err(|_| malformed("the note's `created_at` is not a well-formed timestamp"))?;

        let mut note = WhiteboardNote::new(note_id, author, created_at, entry(fields, "goal")?)
            .map_err(|_| {
                malformed("a whiteboard note may not name itself in the `ev_` or `cap_` class")
            })?;
        if let Some(value) = fields.get("tool") {
            let named = match value {
                Json::String(it) => it.as_str(),
                _ => return Err(malformed("the note's `tool` is not a string")),
            };
            note = note.with_tool(
                Tool::new(named).map_err(|_| malformed("the note's `tool` is the empty string"))?,
            );
        }

        note = note
            .with_known_facts(entry_list(fields, "known_facts")?)
            .with_candidate_invariants(entry_list(fields, "candidate_invariants")?)
            .with_counterexamples(entry_list(fields, "counterexamples")?)
            .with_unresolved_obligations(entry_list(fields, "unresolved_obligations")?)
            .with_experiments(experiments(fields)?)
            .with_decisions(decisions(fields)?);
        Ok(note)
    }

    /// The object's fields, with `additionalProperties: false` enforced against `declared`.
    fn object<'a>(value: &'a Json, declared: &[&str]) -> Result<&'a BTreeMap<String, Json>, Fault> {
        let Json::Object(fields) = value else {
            return Err(malformed(
                "the note, or one of its entries, is not an object",
            ));
        };
        if fields.keys().any(|key| !declared.contains(&key.as_str())) {
            return Err(malformed(
                "the note carries a member its schema does not declare; \
                 `additionalProperties` is false",
            ));
        }
        Ok(fields)
    }

    fn text<'a>(fields: &'a BTreeMap<String, Json>, key: &str) -> Result<&'a str, Fault> {
        match fields.get(key) {
            Some(Json::String(value)) => Ok(value.as_str()),
            _ => Err(malformed(
                "a member the note schema requires is missing or is not a string",
            )),
        }
    }

    fn handle(value: &str) -> Result<ArtifactRef, Fault> {
        ArtifactRef::new(value)
            .map_err(|_| malformed("a handle the note names is not a well-formed artifact handle"))
    }

    fn references(fields: &BTreeMap<String, Json>) -> Result<Vec<ArtifactRef>, Fault> {
        let Some(Json::Array(items)) = fields.get("references") else {
            return Err(malformed(
                "an entry's `references` is missing or is not an array",
            ));
        };
        items
            .iter()
            .map(|item| match item {
                Json::String(value) => handle(value),
                _ => Err(malformed("an entry's `references` holds a non-string")),
            })
            .collect()
    }

    fn entry(fields: &BTreeMap<String, Json>, key: &str) -> Result<Entry, Fault> {
        let value = fields
            .get(key)
            .ok_or_else(|| malformed("a section the note schema requires is missing"))?;
        entry_of(value)
    }

    fn entry_of(value: &Json) -> Result<Entry, Fault> {
        let fields = object(value, &ENTRY_MEMBERS)?;
        Ok(Entry::new(
            ClaimId::new(text(fields, "claim")?)
                .map_err(|_| malformed("an entry's `claim` is the empty string"))?,
            handle(text(fields, "subject")?)?,
            NoteText::new(text(fields, "text")?)
                .map_err(|_| malformed("an entry's `text` is the empty string"))?,
            references(fields)?,
        ))
    }

    fn array<'a>(fields: &'a BTreeMap<String, Json>, key: &str) -> Result<&'a Vec<Json>, Fault> {
        match fields.get(key) {
            Some(Json::Array(items)) => Ok(items),
            // W1: the six list sections MAY be empty but MUST be present. An omitted
            // section and an empty one would otherwise be two spellings of one note.
            _ => Err(malformed(
                "a section the note schema requires is missing or is not an array",
            )),
        }
    }

    fn entry_list(fields: &BTreeMap<String, Json>, key: &str) -> Result<Vec<Entry>, Fault> {
        array(fields, key)?.iter().map(entry_of).collect()
    }

    fn experiments(fields: &BTreeMap<String, Json>) -> Result<Vec<Experiment>, Fault> {
        array(fields, "experiments")?
            .iter()
            .map(|value| {
                let members = object(value, &EXPERIMENT_MEMBERS)?;
                Ok(Experiment::new(
                    NoteText::new(text(members, "text")?)
                        .map_err(|_| malformed("an experiment's `text` is the empty string"))?,
                    references(members)?,
                ))
            })
            .collect()
    }

    fn decisions(fields: &BTreeMap<String, Json>) -> Result<Vec<Decision>, Fault> {
        array(fields, "decisions")?
            .iter()
            .map(|value| {
                Decision::new(entry_of(value)?).map_err(|_| {
                    malformed(
                        "a decision cites no supporting artifact; docs/44 requires a \
                         conclusion to have one",
                    )
                })
            })
            .collect()
    }
}
