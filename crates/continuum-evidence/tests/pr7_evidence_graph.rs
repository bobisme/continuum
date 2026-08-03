//! PR 7 — Evidence Graph v0: bullet-by-bullet evidence, through the public API only.
//!
//! Every module in `continuum-evidence` carries its own clause→test map; this file is the
//! other half. It is an **integration** test, so it compiles as a separate crate and can
//! reach exactly what a client, an adapter, or `continuumd` can reach. That is load-bearing
//! for the PR's exit sentence: several of the guarantees below are claims about what is
//! *unreachable* from outside the crate, and a unit test — which sees `pub(crate)` — could
//! not make them.
//!
//! | Section | Bullet | Bone |
//! |---|---|---|
//! | 1 | support/refute/depend/refine/explain/repair/check edges | bn-3cyf |
//! | 2 | provenance | bn-21iq |
//! | 3 | trusted status transitions | bn-2c8l |
//! | 4 | conflict nodes | bn-2acj |
//! | 5 | determinism and anti-vacuity, over all four | — |
//!
//! The scenario is docs/44's own: two abstraction maps that cannot both satisfy an observed
//! correspondence, the run that was supposed to distinguish them, and the checker that
//! eventually validates one of them.

use continuum_evidence::actor::{ActorId, ServiceIdentity};
use continuum_evidence::authority::{
    CreatableKinds, CreationCapability, ServiceRole, TrustedService,
};
use continuum_evidence::claim_status::{ClaimStatus, verify_promotion_history};
use continuum_evidence::conflict::{
    Conflict, ConflictGround, ConflictSubject, RequiredExperiment, Resolution, ResolutionOutcome,
};
use continuum_evidence::edge::{
    CHECKED_BY_TARGET_CANDIDATES, CheckTargetRule, CheckerBinding, EdgeKind, EdgeRef, EdgeRelation,
    EvidenceEdge, PR7_BULLET_WORDS,
};
use continuum_evidence::graph::{EvidenceGraph, GraphRefusal, PromotionRefusal, ResolutionRefusal};
use continuum_evidence::identity::{DefaultNaming, EvidenceIdentity, EvidenceNaming};
use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, Label, NodeKind, NodeRef};
use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp, Tool};
use continuum_value::assurance::{InconclusiveReason, ValidationBasis};
use continuum_value::epoch::{EpochKind, EpochSet, EvidenceEpoch, ProtocolEpoch};

// --- fixtures ---------------------------------------------------------------------------

fn at() -> Timestamp {
    Timestamp::new("2026-08-01T12:00:00.000Z").expect("well formed")
}

fn artifact(text: &str) -> ArtifactRef {
    ArtifactRef::new(text).expect("well formed")
}

fn provenance(actor: &str, inputs: &[&str]) -> Provenance {
    Provenance::new(
        ActorId::new(actor).expect("well formed"),
        at(),
        inputs.iter().map(|text| artifact(text)),
    )
}

fn node(kind: NodeKind, handle: &str, claim: &str, actor: &str) -> EvidenceNode {
    EvidenceNode::propose(
        kind,
        artifact(handle),
        ClaimId::new(claim).expect("non-empty"),
        IdempotencyKey::new("key-1").expect("non-empty"),
        provenance(actor, &[]),
    )
}

fn edge(relation: EdgeRelation, from: &EvidenceNode, to: &EvidenceNode) -> EvidenceEdge {
    EvidenceEdge::new(
        relation,
        NodeRef::of(from),
        NodeRef::of(to),
        [],
        provenance("agent:integrator", &[]),
    )
    .expect("distinct endpoints")
}

fn checker(name: &str) -> CheckerBinding {
    CheckerBinding::new(ServiceIdentity::parse(name).expect("a service"))
}

/// docs/44's conflict scenario, as a populated graph.
///
/// Two abstraction maps for one claim, a run that observed the system, a certificate the
/// run produced, the receipt an independent kernel emitted over that certificate, and a
/// review decision waiting to be used.
///
/// The receipt is here because RFC 0038 D1 decided what a `CHECKED_BY` edge points at, and
/// the canonical sentence is `certificate CHECKED_BY receipt` — the certificate crosses the
/// checker, and the receipt is the record that it did.
struct Scenario {
    graph: EvidenceGraph,
    map_a: EvidenceNode,
    map_b: EvidenceNode,
    run: EvidenceNode,
    certificate: EvidenceNode,
    receipt: EvidenceNode,
    decision: EvidenceNode,
}

fn scenario() -> Scenario {
    let map_a = node(
        NodeKind::AbstractionMap,
        "map_a",
        "claim-ack",
        "agent:modeler-a",
    );
    let map_b = node(
        NodeKind::AbstractionMap,
        "map_b",
        "claim-ack",
        "agent:modeler-b",
    );
    let run = node(NodeKind::Run, "trace_9f", "claim-ack", "agent:runner");
    let certificate = node(
        NodeKind::Certificate,
        "cert_9f",
        "claim-ack",
        "agent:runner",
    );
    let receipt = node(
        NodeKind::Receipt,
        "receipt_9f",
        "claim-ack",
        "service:kernel-core",
    );
    let decision = node(NodeKind::Decision, "decision_9f", "claim-ack", "human:ada");
    let mut graph = EvidenceGraph::new();
    for member in [&map_a, &map_b, &run, &certificate, &receipt, &decision] {
        graph.add_node(member.clone());
    }
    Scenario {
        graph,
        map_a,
        map_b,
        run,
        certificate,
        receipt,
        decision,
    }
}

// =========================================================================================
// 1. IMPL-01 — the seven edge kinds, as a typed closed vocabulary (bn-3cyf)
// =========================================================================================

#[test]
fn the_seven_bullet_edges_are_typed_kinds_a_graph_accepts() {
    let mut scene = scenario();
    // Each of the PR bullet's seven words, built as a relation and appended. Nothing here
    // parses a string: the relation is the value.
    let built: Vec<(EdgeKind, EdgeRelation)> = vec![
        (EdgeKind::Supports, EdgeRelation::Supports),
        (EdgeKind::Refutes, EdgeRelation::Refutes),
        (EdgeKind::DependsOn, EdgeRelation::DependsOn),
        (EdgeKind::Refines, EdgeRelation::Refines),
        (EdgeKind::Explains, EdgeRelation::Explains),
        (EdgeKind::Repairs, EdgeRelation::Repairs),
        (
            EdgeKind::CheckedBy,
            EdgeRelation::CheckedBy(checker("service:kernel-core")),
        ),
    ];
    assert_eq!(built.len(), PR7_BULLET_WORDS.len());
    for ((expected, relation), (word, named)) in built.into_iter().zip(PR7_BULLET_WORDS) {
        assert_eq!(expected, named, "the bullet word `{word}`");
        assert_eq!(relation.kind(), expected);
        // Twelve relations may point anywhere; a check edge points at a receipt and only a
        // receipt (RFC 0038 D1), which is the graph's own rule and not this test's.
        let target = if expected == EdgeKind::CheckedBy {
            &scene.receipt
        } else {
            &scene.map_a
        };
        let appended = scene
            .graph
            .add_edge(edge(relation, &scene.run, target))
            .unwrap_or_else(|refusal| panic!("{word}: {refusal}"));
        assert!(appended.is_fresh(), "{word}");
    }
    assert_eq!(scene.graph.edge_count(), 7);
    for (_, kind) in PR7_BULLET_WORDS {
        assert_eq!(scene.graph.edges_of_kind(kind).count(), 1, "{kind}");
    }
}

#[test]
fn the_vocabulary_is_closed_and_carries_no_strings() {
    // All thirteen round-trip; nothing outside them parses.
    assert_eq!(EdgeKind::ALL.len(), 13);
    for kind in EdgeKind::ALL {
        assert_eq!(EdgeKind::from_token(kind.as_str()), Some(kind));
    }
    for invented in ["supports", "CHECKS", "SUPPORTS ", "", "CONFLICTS"] {
        assert_eq!(EdgeKind::from_token(invented), None, "{invented}");
    }
    // Exactly one kind is obliged to name a checker, and exactly one relation can.
    let obliged: Vec<EdgeKind> = EdgeKind::ALL
        .into_iter()
        .filter(|kind| kind.requires_checker())
        .collect();
    assert_eq!(obliged, [EdgeKind::CheckedBy]);
}

#[test]
fn a_check_edge_names_its_checker_and_points_at_a_receipt() {
    let mut scene = scenario();
    let relation = EdgeRelation::CheckedBy(checker("service:kernel-core"));
    // The canonical sentence: the certificate crossed the kernel, and the receipt is the
    // record that it did (INV-004, "certificates cross an independent checker").
    let checked = edge(relation, &scene.certificate, &scene.receipt);
    assert_eq!(
        checked.checker().map(ServiceIdentity::as_str),
        Some("service:kernel-core")
    );
    scene.graph.add_edge(checked).expect("endpoints held");

    // RFC 0038 D1 (bn-3sypm), enforced by a graph nobody configured: the run and the
    // certificate — the two candidates the RFC weighed and rejected — are refused as
    // targets, and the refusal names the kind.
    for rejected in [&scene.run, &scene.certificate] {
        assert_eq!(
            scene.graph.add_edge(edge(
                EdgeRelation::CheckedBy(checker("service:kernel-core")),
                &scene.map_a,
                rejected,
            )),
            Err(GraphRefusal::CheckTargetRefused {
                kind: rejected.kind()
            })
        );
    }
    // The three candidates are still recorded, and the rule is still a value a deployment
    // can set, so the decision is a default rather than a branch nobody can move.
    for candidate in CHECKED_BY_TARGET_CANDIDATES {
        let rule = CheckTargetRule::one_of([candidate]);
        for other in CHECKED_BY_TARGET_CANDIDATES {
            assert_eq!(rule.admits(other), other == candidate);
        }
    }
    assert!(
        CheckTargetRule::default().admits(NodeKind::Receipt),
        "RFC 0038 D1"
    );
    let widened = EvidenceGraph::new()
        .with_check_target_rule(CheckTargetRule::one_of([NodeKind::Certificate]));
    assert!(widened.check_target_rule().admits(NodeKind::Certificate));
    assert!(!widened.check_target_rule().admits(NodeKind::Patch));
}

#[test]
fn an_edge_cannot_dangle_or_close_on_itself() {
    let scene = scenario();
    let mut empty = EvidenceGraph::new();
    // Nothing held: the reference does not resolve (docs/44).
    assert!(
        empty
            .add_edge(edge(EdgeRelation::Supports, &scene.run, &scene.map_a))
            .is_err()
    );
    // …and no relation joins an artifact to itself.
    for relation in [
        EdgeRelation::Supports,
        EdgeRelation::DependsOn,
        EdgeRelation::CheckedBy(checker("service:kernel-core")),
    ] {
        assert!(
            EvidenceEdge::new(
                relation,
                NodeRef::of(&scene.run),
                NodeRef::of(&scene.run),
                [],
                provenance("agent:integrator", &[]),
            )
            .is_err()
        );
    }
}

// =========================================================================================
// 2. IMPL-02 — provenance (bn-21iq)
// =========================================================================================

#[test]
fn every_record_in_a_populated_graph_names_a_producer() {
    let mut scene = scenario();
    scene
        .graph
        .add_edge(edge(EdgeRelation::Supports, &scene.run, &scene.map_a))
        .expect("endpoints held");
    assert_eq!(scene.graph.node_count(), 6);
    assert_eq!(scene.graph.edge_count(), 1);

    let mut producers = Vec::new();
    for (_, node) in scene.graph.nodes() {
        let record = node.provenance();
        assert!(!record.actor().as_str().is_empty());
        assert_eq!(record.created_at().as_str(), "2026-08-01T12:00:00.000Z");
        producers.push(record.actor().as_str().to_owned());
    }
    for (_, edge) in scene.graph.edges() {
        assert!(!edge.provenance().actor().as_str().is_empty());
    }
    producers.sort();
    producers.dedup();
    // Six nodes from five distinct producers — one of them the kernel service that emitted
    // the receipt: the graph keeps credit rather than collapsing it (docs/44, "Credit and
    // provenance").
    assert_eq!(producers.len(), 5);
    assert_eq!(
        scene
            .graph
            .nodes_by_producer(&ActorId::new("agent:runner").expect("well formed"))
            .count(),
        2
    );
}

#[test]
fn a_provenance_record_is_exactly_the_schema_object_and_carries_its_epochs() {
    let epochs = EpochSet::unpinned()
        .with_protocol(ProtocolEpoch::new(1, 0))
        .with_evidence(EvidenceEpoch::new("ev/1").expect("well formed"));
    let record = provenance("agent:runner", &["trace_9f", "cert_9f"])
        .with_tool(Tool::new("continuum-observer/0.0.0").expect("non-empty"))
        .under_epochs(epochs);

    // Carried (plan §4.6: evidence is epoch-scoped).
    assert_eq!(
        record.epochs().evidence().map(EvidenceEpoch::as_str),
        Some("ev/1")
    );
    assert_eq!(record.epochs().entries().len(), EpochKind::ALL.len());
    // Never rendered: the schema closes the provenance object at four members.
    let rendered = record.to_record();
    let members = match &rendered {
        continuum_value::value::Value::Record(fields) => {
            let mut names: Vec<&str> = fields
                .keys()
                .map(continuum_value::value::Name::as_str)
                .collect();
            names.sort_unstable();
            names
        }
        other => panic!("expected a record, got {other:?}"),
    };
    assert_eq!(members, ["actor", "created_at", "inputs", "tool"]);
    // Inputs are a set in canonical order, so one derivation has one spelling.
    let inputs: Vec<&str> = record.inputs().map(ArtifactRef::as_str).collect();
    assert_eq!(inputs, ["cert_9f", "trace_9f"]);
    assert!(record.has_input(&artifact("trace_9f")));
}

#[test]
fn a_producer_cannot_append_under_another_actors_name() {
    // RFC 0038: "Actor capabilities control node creation"; docs/44: "actors may append
    // only allowed node types".
    let capability = CreationCapability::new(
        ActorId::new("agent:modeler-a").expect("well formed"),
        CreatableKinds::one_of([NodeKind::AbstractionMap]),
    );
    let scene = scenario();
    assert_eq!(capability.authorize(&scene.map_a), Ok(()));
    // Right kind, wrong producer.
    assert!(capability.authorize(&scene.map_b).is_err());
    // Right producer, wrong kind.
    let wrong_kind = node(NodeKind::Patch, "patch_9f", "claim-ack", "agent:modeler-a");
    assert!(capability.authorize(&wrong_kind).is_err());
}

// =========================================================================================
// 3. IMPL-03 — trusted status transitions (bn-2c8l)
// =========================================================================================

#[test]
fn the_pr7_exit_sentence_holds_from_outside_the_crate() {
    // "an untrusted client cannot promote a proposal to validated/proved."
    //
    // Four independent reasons, all visible from here — this file is a separate crate, so
    // "unreachable" means unreachable to a client.
    //
    // 1. A proposal lands at the bottom and no constructor says otherwise.
    let scene = scenario();
    for (_, node) in scene.graph.nodes() {
        assert_eq!(node.status(), ClaimStatus::Proposed);
        assert_eq!(node.version(), 0);
    }
    // 2/3. No untrusted actor becomes a service, so no untrusted actor mints a promotion.
    for untrusted in ["agent:modeler-a", "human:ada", "ci:nightly"] {
        let actor = ActorId::new(untrusted).expect("well formed");
        assert!(ServiceIdentity::new(actor).is_err(), "{untrusted}");
        assert!(ServiceIdentity::parse(untrusted).is_err(), "{untrusted}");
    }
    // 4. Of the five declarable roles, exactly one reaches `validated` and one `proved`.
    let mut validated = Vec::new();
    let mut proved = Vec::new();
    for role in ServiceRole::ALL {
        let service = TrustedService::new(
            ServiceIdentity::parse("service:s").expect("a service"),
            [role],
        )
        .expect("one role");
        if service
            .validate(ValidationBasis::CheckedCertificate)
            .is_ok()
        {
            validated.push(role);
        }
        if service.promote_to(ClaimStatus::Proved).is_ok() {
            proved.push(role);
        }
    }
    assert_eq!(validated, [ServiceRole::IndependentChecker]);
    assert_eq!(proved, [ServiceRole::ProofService]);
}

#[test]
fn a_trusted_checker_promotes_and_the_history_is_append_only() {
    let mut scene = scenario();
    let identity = scene.graph.add_node(scene.map_a.clone()).identity().clone();
    let runner = TrustedService::new(
        ServiceIdentity::parse("service:runner").expect("a service"),
        [ServiceRole::Execution],
    )
    .expect("one role");
    let checker = TrustedService::new(
        ServiceIdentity::parse("service:checker").expect("a service"),
        [ServiceRole::IndependentChecker],
    )
    .expect("one role");

    let observed = scene
        .graph
        .promote(
            &identity,
            ClaimStatus::Proposed,
            &runner
                .promote_to(ClaimStatus::Observed)
                .expect("authorized"),
        )
        .expect("a promotion from the expected status");
    assert_eq!(observed, ClaimStatus::Observed);
    let settled = scene
        .graph
        .promote(
            &identity,
            ClaimStatus::Observed,
            &checker
                .validate(ValidationBasis::CheckedCertificate)
                .expect("authorized"),
        )
        .expect("bounded → validated is docs/44's own row");
    assert_eq!(settled, ClaimStatus::Validated);

    let held = scene.graph.node(&identity).expect("still held");
    assert_eq!(held.identity(), identity, "a promotion keeps the identity");
    assert_eq!(held.version(), 2);
    assert_eq!(
        held.status_history(),
        [
            ClaimStatus::Proposed,
            ClaimStatus::Observed,
            ClaimStatus::Validated
        ]
    );
    assert_eq!(verify_promotion_history(&held.status_history()), Ok(()));
    let last = held.history().last().expect("the validation");
    assert_eq!(last.service_identity(), Some("service:checker"));
    assert_eq!(
        last.validation_basis(),
        Some(ValidationBasis::CheckedCertificate)
    );
    // The observation names its promoting service too — but `observed` is not one of the
    // four the node schema attributes, so it does not.
    let observation = held.history().nth(1).expect("the observation");
    assert_eq!(observation.service_identity(), None);
}

#[test]
fn a_service_cannot_certify_its_own_production() {
    // INV-004's other half, from outside: a deployment that produces under the identity it
    // verifies under obeys every other rule and is still refused.
    let mut graph = EvidenceGraph::new();
    let self_produced = node(
        NodeKind::Certificate,
        "cert_9f",
        "claim-ack",
        "service:checker",
    );
    let identity = graph.add_node(self_produced).identity().clone();
    let itself = TrustedService::new(
        ServiceIdentity::parse("service:checker").expect("a service"),
        [ServiceRole::IndependentChecker],
    )
    .expect("one role");
    assert_eq!(
        graph.promote(
            &identity,
            ClaimStatus::Proposed,
            &itself
                .validate(ValidationBasis::CheckedCertificate)
                .expect("authorized"),
        ),
        Err(PromotionRefusal::SelfCertification)
    );
    assert_eq!(
        graph.node(&identity).expect("held").status(),
        ClaimStatus::Proposed
    );
}

#[test]
fn the_two_payload_bearing_statuses_cannot_be_reached_without_their_payload() {
    let checker = TrustedService::new(
        ServiceIdentity::parse("service:checker").expect("a service"),
        [ServiceRole::IndependentChecker],
    )
    .expect("one role");
    // plan §11.4: `validated` records its basis, and an `inconclusive` carries a typed
    // INV-008 reason — neither has a payload-free constructor.
    assert!(checker.promote_to(ClaimStatus::Validated).is_err());
    assert!(checker.promote_to(ClaimStatus::Inconclusive).is_err());
    assert!(checker.validate(ValidationBasis::TrustedSolver).is_ok());
    assert!(
        checker
            .inconclusive(InconclusiveReason::IncompleteProofSearch)
            .is_ok()
    );
}

// =========================================================================================
// 4. IMPL-04 — conflict nodes (bn-2acj)
// =========================================================================================

fn conflict_between(
    left: &EvidenceNode,
    right: &EvidenceNode,
    extra: Vec<ConflictSubject>,
) -> Conflict {
    let mut subjects = vec![
        ConflictSubject::Node(NodeRef::of(left)),
        ConflictSubject::Node(NodeRef::of(right)),
    ];
    subjects.extend(extra);
    Conflict::between(
        subjects,
        ConflictGround::new("both cannot satisfy observed correspondence").expect("non-empty"),
        RequiredExperiment::new("distinguish observer contract").expect("non-empty"),
    )
    .expect("two distinct parties")
}

#[test]
fn docs_44s_scenario_becomes_a_conflict_node_and_is_never_silently_chosen() {
    let mut scene = scenario();
    // Two abstraction maps, each supported by the same run: the contradiction docs/44
    // describes, made concrete as a SUPPORTS/REFUTES pair over one map.
    let supports = edge(EdgeRelation::Supports, &scene.run, &scene.map_a);
    let refutes = edge(EdgeRelation::Refutes, &scene.run, &scene.map_a);
    scene.graph.add_edge(supports.clone()).expect("held");
    scene.graph.add_edge(refutes.clone()).expect("held");
    let contradictions = scene.graph.contradictions();
    assert_eq!(contradictions.len(), 1);
    assert_eq!(contradictions[0].asserted, supports.identity());
    assert_eq!(contradictions[0].denied, refutes.identity());

    let conflict = conflict_between(
        &scene.map_a,
        &scene.map_b,
        vec![
            ConflictSubject::Edge(EdgeRef::of(&supports)),
            ConflictSubject::Edge(EdgeRef::of(&refutes)),
        ],
    );
    let naming = DefaultNaming::new();
    let materialized = conflict.materialize(
        &naming,
        artifact("conflict_9f"),
        ClaimId::new("claim-ack").expect("non-empty"),
        IdempotencyKey::new("key-c").expect("non-empty"),
        provenance("agent:integrator", &[]),
    );
    let identity = scene
        .graph
        .add_conflict(&materialized)
        .expect("every node subject is held");

    let held = scene.graph.node(&identity).expect("held");
    assert_eq!(held.kind(), NodeKind::Conflict);
    assert_eq!(held.status(), ClaimStatus::Proposed);
    // Every subject — including the two edges no edge can point at — is named as an input.
    let inputs: Vec<&str> = held
        .provenance()
        .inputs()
        .map(ArtifactRef::as_str)
        .collect();
    assert_eq!(inputs.len(), 4);
    for subject in conflict.subjects() {
        assert!(inputs.contains(&subject.handle(&naming).as_str()));
    }
    // The two maps are linked to each other and to the conflict.
    assert_eq!(
        scene.graph.edges_of_kind(EdgeKind::ConflictsWith).count(),
        1
    );
    assert_eq!(scene.graph.edges_of_kind(EdgeKind::DerivedFrom).count(), 2);
    // …and nothing was chosen: both contradicting edges are still held and still reported.
    assert!(scene.graph.edge(&supports.identity()).is_some());
    assert!(scene.graph.edge(&refutes.identity()).is_some());
    assert_eq!(scene.graph.contradictions().len(), 1);
    assert_eq!(scene.graph.unresolved_conflicts().count(), 1);
}

#[test]
fn a_conflict_is_resolved_by_a_recorded_transition_and_nothing_is_deleted() {
    let mut scene = scenario();
    let conflict = conflict_between(&scene.map_a, &scene.map_b, Vec::new());
    let materialized = conflict.materialize(
        &DefaultNaming::new(),
        artifact("conflict_9f"),
        ClaimId::new("claim-ack").expect("non-empty"),
        IdempotencyKey::new("key-c").expect("non-empty"),
        provenance("agent:integrator", &[]),
    );
    let identity = scene.graph.add_conflict(&materialized).expect("held");
    let nodes_before = scene.graph.node_count();
    let edges_before = scene.graph.edge_count();

    let resolution = Resolution::new(
        NodeRef::of(&scene.decision),
        ResolutionOutcome::Retained(ConflictSubject::Node(NodeRef::of(&scene.map_a))),
        [artifact("trace_9f")],
        provenance("human:ada", &["trace_9f"]),
    )
    .expect("named evidence");
    // A resolution that names no evidence is refused — docs/44's "most confident answer".
    assert!(
        Resolution::new(
            NodeRef::of(&scene.decision),
            ResolutionOutcome::AllSuperseded,
            [],
            provenance("human:ada", &[]),
        )
        .is_err()
    );
    // …and only the policy owner can mint the promotion that retires it.
    let checker = TrustedService::new(
        ServiceIdentity::parse("service:checker").expect("a service"),
        [ServiceRole::IndependentChecker],
    )
    .expect("one role");
    assert!(checker.promote_to(ClaimStatus::Superseded).is_err());
    let owner = TrustedService::new(
        ServiceIdentity::parse("service:owner").expect("a service"),
        [ServiceRole::PolicyOwner],
    )
    .expect("one role");
    // A promotion that installs anything but `superseded` is a choice, not a resolution.
    assert!(matches!(
        scene.graph.resolve_conflict(
            &identity,
            &resolution,
            &checker
                .validate(ValidationBasis::CheckedCertificate)
                .expect("authorized"),
        ),
        Err(ResolutionRefusal::NotARetirement { .. })
    ));

    let settled = scene
        .graph
        .resolve_conflict(
            &identity,
            &resolution,
            &owner
                .promote_to(ClaimStatus::Superseded)
                .expect("authorized"),
        )
        .expect("a policy owner retires a conflict");
    assert_eq!(settled, ClaimStatus::Superseded);

    // Nothing removed; one edge added; the conflict still held, at a new version.
    assert_eq!(scene.graph.node_count(), nodes_before);
    assert_eq!(scene.graph.edge_count(), edges_before + 1);
    assert_eq!(scene.graph.edges_of_kind(EdgeKind::Supersedes).count(), 1);
    assert_eq!(
        scene.graph.edges_of_kind(EdgeKind::ConflictsWith).count(),
        1
    );
    let resolved = scene.graph.node(&identity).expect("still held");
    assert_eq!(resolved.status(), ClaimStatus::Superseded);
    assert_eq!(
        resolved.status_history(),
        [ClaimStatus::Proposed, ClaimStatus::Superseded]
    );
    assert_eq!(scene.graph.conflicts().count(), 1);
    assert_eq!(scene.graph.unresolved_conflicts().count(), 0);
    // Both parties are untouched at their own versions.
    for party in [&scene.map_a, &scene.map_b] {
        let held = scene.graph.node(&party.identity()).expect("still held");
        assert_eq!(held.status(), ClaimStatus::Proposed);
        assert_eq!(held.version(), 0);
    }
}

// =========================================================================================
// 5. Determinism and anti-vacuity, over all four bullets
// =========================================================================================

#[test]
fn two_independently_built_graphs_agree_byte_for_byte() {
    // INV-005/INV-006, GOV-1-03: the content decides, and nothing else does — not insertion
    // order, not the process.
    let build = |reverse: bool| -> Vec<Vec<u8>> {
        let mut scene = scenario();
        let mut edges = vec![
            edge(EdgeRelation::Supports, &scene.run, &scene.map_a),
            edge(EdgeRelation::Refutes, &scene.run, &scene.map_b),
            edge(
                EdgeRelation::CheckedBy(checker("service:kernel-core")),
                &scene.certificate,
                &scene.receipt,
            ),
        ];
        if reverse {
            edges.reverse();
        }
        for edge in edges {
            scene.graph.add_edge(edge).expect("endpoints held");
        }
        let naming = DefaultNaming::new();
        let mut rendered: Vec<Vec<u8>> = Vec::new();
        for (identity, node) in scene.graph.nodes() {
            rendered.push(identity.canonical_bytes().to_vec());
            rendered.push(node.to_record(&naming.name(identity)).encode());
        }
        for (identity, edge) in scene.graph.edges() {
            rendered.push(identity.canonical_bytes().to_vec());
            rendered.push(
                edge.to_record(
                    &naming.name(identity),
                    &naming.name(edge.from().identity()),
                    &naming.name(edge.to().identity()),
                )
                .encode(),
            );
        }
        rendered
    };
    let forward = build(false);
    let backward = build(true);
    assert_eq!(forward, backward);
    // Not vacuous: the rendering is non-trivial and covers every held record.
    assert_eq!(forward.len(), (6 + 3) * 2);
    assert!(forward.iter().all(|bytes| !bytes.is_empty()));
}

#[test]
fn a_mutant_in_any_identifying_field_moves_the_identity() {
    // Anti-vacuity for "content-identified": each mutation is one field, and each must be
    // visible. A rule that could not tell these apart would pass every test above.
    let base = node(
        NodeKind::AbstractionMap,
        "map_a",
        "claim-ack",
        "agent:modeler-a",
    );
    let mutants = [
        node(NodeKind::Model, "map_a", "claim-ack", "agent:modeler-a"),
        node(
            NodeKind::AbstractionMap,
            "map_b",
            "claim-ack",
            "agent:modeler-a",
        ),
        node(
            NodeKind::AbstractionMap,
            "map_a",
            "claim-other",
            "agent:modeler-a",
        ),
        node(
            NodeKind::AbstractionMap,
            "map_a",
            "claim-ack",
            "agent:modeler-b",
        ),
        base.clone()
            .with_labels([Label::new("frontier").expect("non-empty")]),
        EvidenceNode::propose(
            NodeKind::AbstractionMap,
            artifact("map_a"),
            ClaimId::new("claim-ack").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:modeler-a", &["trace_9f"]),
        ),
        EvidenceNode::propose(
            NodeKind::AbstractionMap,
            artifact("map_a"),
            ClaimId::new("claim-ack").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            Provenance::new(
                ActorId::new("agent:modeler-a").expect("well formed"),
                Timestamp::new("2026-08-01T12:00:01.000Z").expect("well formed"),
                [],
            ),
        ),
        EvidenceNode::propose(
            NodeKind::AbstractionMap,
            artifact("map_a"),
            ClaimId::new("claim-ack").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:modeler-a", &[]).under_epochs(
                EpochSet::unpinned()
                    .with_evidence(EvidenceEpoch::new("ev/1").expect("well formed")),
            ),
        ),
    ];
    let mut seen = vec![base.identity()];
    for mutant in mutants {
        let identity = mutant.identity();
        assert!(
            !seen.contains(&identity),
            "a mutant collided with an earlier record"
        );
        seen.push(identity);
    }
    assert_eq!(seen.len(), 9);
    // …and the one field that is deliberately *not* identifying stays not identifying.
    let other_key = EvidenceNode::propose(
        NodeKind::AbstractionMap,
        artifact("map_a"),
        ClaimId::new("claim-ack").expect("non-empty"),
        IdempotencyKey::new("key-2").expect("non-empty"),
        provenance("agent:modeler-a", &[]),
    );
    assert_eq!(base.identity(), other_key.identity());
}

#[test]
fn a_replay_converges_and_a_second_producer_does_not() {
    let mut scene = scenario();
    let before = scene.graph.node_count();
    // A replayed write returns the original identity and changes nothing (RFC 0038).
    let replay = scene.graph.add_node(scene.map_a.clone());
    assert!(!replay.is_fresh());
    assert_eq!(replay.identity(), &scene.map_a.identity());
    assert_eq!(scene.graph.node_count(), before);

    // A second producer's identical assertion is a second edge, because credit is content.
    let first = edge(EdgeRelation::Supports, &scene.run, &scene.map_a);
    let second = EvidenceEdge::new(
        EdgeRelation::Supports,
        NodeRef::of(&scene.run),
        NodeRef::of(&scene.map_a),
        [],
        provenance("agent:modeler-b", &[]),
    )
    .expect("distinct endpoints");
    scene.graph.add_edge(first).expect("held");
    scene.graph.add_edge(second).expect("held");
    assert_eq!(scene.graph.edge_count(), 2);
}

#[test]
fn a_handle_is_a_name_and_never_the_identity() {
    // ADR-0013: the identity is the canonical encoding; a digest names it.
    let scene = scenario();
    let naming = DefaultNaming::new();
    let mut handles = Vec::new();
    let mut identities: Vec<EvidenceIdentity> = Vec::new();
    for (identity, _) in scene.graph.nodes() {
        handles.push(naming.name(identity).as_str().to_owned());
        identities.push(identity.clone());
    }
    assert_eq!(handles.len(), 6);
    let mut unique = handles.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        handles.len(),
        "distinct records, distinct names"
    );
    for handle in &handles {
        assert!(handle.starts_with("ev_"));
        // The handle is also a well-formed artifact reference, so it can appear in a
        // provenance `inputs` list.
        assert!(ArtifactRef::new(handle).is_ok());
    }
    // Naming is pure: the same identity names the same handle every time.
    for (index, identity) in identities.iter().enumerate() {
        assert_eq!(naming.name(identity).as_str(), handles[index]);
    }
}
