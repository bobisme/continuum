//! PR-20 / IMPL-04 (bn-1b69): the semantic and protected-intent diff of a repair
//! transaction.
//!
//! The diff is computed by `continuum_repair::diff::compute` from the stores the
//! transaction names: the base and candidate snapshots' intent bindings, the intent
//! registry, and the evidence store. No test passes a contract, a snapshot, or a diff to
//! it directly.
//!
//! - Positive: the ack-after-sync repair keeps the base intent. Its diff is an RFC 0031
//!   artifact with no intent change and decision `allow`, gate 3 passes, and the bytes are
//!   the retained artifact `pr20-impl04-ack-after-sync-diff`.
//! - Negative: a candidate bound to a weakened contract is a privileged intent revision,
//!   one test per weakening kind of INV-001 (weakened property, strengthened assumption,
//!   shrunk bound, hidden event, removed fault, downgraded assurance). A rebinding the
//!   verb table would allow is still a revision. A claimed diff or handle that is not the
//!   computed one is refused.
//! - Boundary: every undiffable input is a typed inconclusive with an INV-008 reason,
//!   never "no change"; at `draft` gate 3 stays `pending`.

mod common;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyDecision, PolicyField, Relation};
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::diff::{
    self, ChangeClass, DiffClaimRefusal, DiffOutcome, DiffScope, DiffStore, ImpactScope,
    IntentClassification, IntentRevision, ScopeAnswer, TransactionDiff, Undiffable, Weakening,
};
use continuum_repair::handle::{CrashpackId, SnapshotId};
use continuum_repair::hypothesis::{Hypothesis, Proposal};
use continuum_repair::receipt::{
    IntentRegistry, IntentStanding, RegisteredIntent, SealedSnapshots, SnapshotRole,
};
use continuum_repair::transaction::{
    FailureBinding, GateProfile, GateStatus, RepairTransaction, Resolution,
};
use continuum_semantic_diff::artifact::{self as oracle, DiffId, DiffRequest};
use continuum_semantic_diff::impact::{
    DependencyEdge, DependencyReason, EvidenceId, EvidenceRecord, Independence, ProgramLayer,
    ReuseEdgeClass,
};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

const CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const RETAINED_REPAIR: &str = include_str!("fixtures/pr20-impl04-ack-after-sync-diff.json");
const RETAINED_WEAKENED: &str = include_str!("fixtures/pr20-impl04-weakened-property-diff.json");
const CRASH: &str = "crash_m01_ack_before_sync";
const BASE_INTENT: &str = "in_replicated_register_v1";

/// `v1 == v2`, the Agreement claim's disjunct; `v1 != v2` appended guts it.
const AGREEMENT_EQ: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"eq","right":{"kind":"var","name":"v2"}}"#;
const AGREEMENT_NE: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"ne","right":{"kind":"var","name":"v2"}}"#;
/// A favorable assumption in sorted position.
const OPERATOR_HONESTY: &str = r#"{"classification":"trust","expression":{"ast":{"args":[],"kind":"predicate","name":"operator_is_honest"},"fragment":"Finite","normal_form":"cpnf-1","source":"operator_is_honest"},"id":"OperatorHonesty"}"#;

// --- the world -------------------------------------------------------------------

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the contract decodes")
}

fn replace_once(document: &str, needle: &str, replacement: &str) -> String {
    assert_eq!(document.matches(needle).count(), 1, "one needle: {needle}");
    document.replacen(needle, replacement, 1)
}

/// The base contract rewritten by `edit` and filed under `intent`.
fn revised(intent: &str, edit: impl Fn(&str) -> String) -> IntentContract {
    let document = replace_once(
        &edit(CONTRACT),
        &format!("\"intent_id\":\"{BASE_INTENT}\""),
        &format!("\"intent_id\":\"{intent}\""),
    );
    decode(&document)
}

fn intent(handle: &str) -> IntentId {
    IntentId::new(handle).expect("an in_ handle")
}

/// The stores: every answer the diff reads, and nothing else.
struct World {
    /// The intent the candidate snapshot is bound to; `None` leaves it unsealed.
    candidate_binding: Resolution<IntentId>,
    /// The intent the base snapshot is bound to.
    base_binding: Resolution<IntentId>,
    /// The registry's records, by handle.
    registry: Vec<(IntentId, IntentContract)>,
    registry_available: bool,
    /// The candidate snapshot the evidence scope must name.
    candidate: SnapshotId,
    /// The base intent's registry standing.
    base_standing: IntentStanding,
    evidence: ScopeAnswer,
}

impl World {
    fn repair() -> Self {
        Self {
            candidate_binding: Resolution::Found(intent(BASE_INTENT)),
            base_binding: Resolution::Found(intent(BASE_INTENT)),
            registry: vec![(intent(BASE_INTENT), decode(CONTRACT))],
            registry_available: true,
            candidate: common::snapshot_id(&common::repaired_content()),
            base_standing: IntentStanding::Accepted,
            evidence: ScopeAnswer::Complete(Vec::new()),
        }
    }

    /// The candidate bound to `contract`, filed under its own declared handle.
    fn rebound_to(contract: IntentContract) -> Self {
        let mut world = Self::repair();
        world.candidate_binding = Resolution::Found(contract.intent_id().clone());
        world
            .registry
            .push((contract.intent_id().clone(), contract));
        world
    }
}

impl FailureBinding for World {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        if failure.as_str() == CRASH {
            Resolution::Found(common::base_id())
        } else {
            Resolution::Unknown
        }
    }
    fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(intent(BASE_INTENT))
    }
}

impl SealedSnapshots for World {
    fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if snapshot == &common::base_id() {
            self.base_binding.clone()
        } else if snapshot == &common::snapshot_id(&common::repaired_content()) {
            self.candidate_binding.clone()
        } else {
            Resolution::Unknown
        }
    }
}

impl IntentRegistry for World {
    fn registered(&self, handle: &IntentId) -> Resolution<RegisteredIntent> {
        if !self.registry_available {
            return Resolution::Unavailable;
        }
        self.registry
            .iter()
            .find(|(filed, _)| filed == handle)
            .map_or(Resolution::Unknown, |(filed, contract)| {
                let standing = if filed.as_str() == BASE_INTENT {
                    self.base_standing
                } else {
                    IntentStanding::Accepted
                };
                Resolution::Found(RegisteredIntent::new(contract.clone(), standing))
            })
    }
}

impl ImpactScope for World {
    /// The scope asked for is exactly the transaction's: its base and candidate, the
    /// base intent, and the candidate's binding, as of the version being diffed.
    fn evidence(&self, scope: DiffScope<'_>) -> ScopeAnswer {
        assert!(scope.repair.as_str().starts_with("rt_"));
        assert_eq!(scope.before_snapshot, &common::base_id());
        assert_eq!(scope.after_snapshot, &self.candidate);
        assert_eq!(scope.before_intent.as_str(), BASE_INTENT);
        assert_eq!(
            Resolution::Found(scope.after_intent.clone()),
            self.candidate_binding
        );
        self.evidence.clone()
    }
}

fn draft() -> RepairTransaction<Blake3Hasher> {
    RepairTransaction::<Blake3Hasher>::begin(
        CrashpackId::new(CRASH).expect("a crash_ handle"),
        GateProfile::PhaseB,
        &World::repair(),
    )
    .expect("the failure resolves")
}

/// The ack-after-sync repair applied: one `rust` change to the replica.
fn applied_with(hypothesis: &str) -> RepairTransaction<Blake3Hasher> {
    draft()
        .apply(
            &Proposal::new(
                Hypothesis::new(hypothesis),
                vec![common::ack_after_sync_change()],
            ),
            &common::base_content(),
        )
        .expect("the repair applies")
        .into_parts()
        .0
}

fn applied() -> RepairTransaction<Blake3Hasher> {
    applied_with("move the ack after the sync")
}

fn computed(world: &World) -> TransactionDiff {
    match diff::compute(&applied(), world, world, world) {
        DiffOutcome::Computed(diff) => *diff,
        DiffOutcome::Inconclusive(reason) => panic!("undiffable: {reason}"),
    }
}

fn revision(world: &World) -> IntentRevision {
    match computed(world).classification() {
        IntentClassification::PrivilegedIntentRevision(revision) => revision.clone(),
        IntentClassification::Repair => panic!("classified a repair"),
    }
}

fn inconclusive(world: &World) -> Undiffable {
    match diff::compute(&applied(), world, world, world) {
        DiffOutcome::Inconclusive(reason) => reason,
        DiffOutcome::Computed(_) => panic!("a diff was computed"),
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical JSON is UTF-8")
}

fn evidence_record(id: &str, edges: Vec<DependencyEdge>) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId::new(id).expect("an evidence id"),
        keyed_to_before_intent: true,
        key_change_reuse_witness: false,
        edges,
    }
}

fn bound_edge() -> DependencyEdge {
    DependencyEdge {
        reason: DependencyReason::ReliesOnAssumptionFairnessBound,
        class: ReuseEdgeClass::Exact,
        independence: Independence::Unknown,
    }
}

// --- positive --------------------------------------------------------------------

#[test]
fn pr20_impl04_pos_01_the_ack_after_sync_repair_yields_its_diff_and_no_intent_change() {
    let transaction = applied();
    let outcome = diff::compute(
        &transaction,
        &World::repair(),
        &World::repair(),
        &World::repair(),
    );
    // The honest outcome today: the intent is preserved, but the `rust` change is not
    // classified on the program side, so gate 3 cannot pass on it.
    assert_eq!(outcome.gate_status(), GateStatus::Inconclusive);
    assert_eq!(
        outcome.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    let DiffOutcome::Computed(diff) = outcome else {
        panic!("the repair's diff is computed");
    };
    assert_eq!(diff.classification(), &IntentClassification::Repair);
    assert_eq!(diff.artifact().program_layer(), ProgramLayer::Unclassified);
    assert_eq!(diff.repair(), transaction.repair_id());
    assert!(diff.artifact().intent_changes().is_empty());
    assert_eq!(diff.artifact().decision(), PolicyDecision::Allow);

    let Json::Object(fields) = diff.artifact().artifact_json() else {
        panic!("an object");
    };
    let field = |key: &str| match fields.get(key) {
        Some(Json::String(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    };
    assert_eq!(field("diff_id"), diff.handle());
    assert_eq!(
        field("before_snapshot"),
        transaction.base_snapshot().as_str()
    );
    assert_eq!(
        field("after_snapshot"),
        transaction.candidate_snapshot().expect("applied").as_str()
    );
    assert_eq!(field("before_intent"), BASE_INTENT);
    assert_eq!(field("after_intent"), BASE_INTENT);
    // The base contract's own minimum, not a caller's choice.
    assert_eq!(field("requested_assurance"), "validated");
}

#[test]
fn pr20_impl04_pos_02_the_retained_repair_diff_is_the_computed_bytes() {
    let diff = computed(&World::repair());
    assert_eq!(text(&diff.to_artifact_bytes()), RETAINED_REPAIR.trim_end());
}

#[test]
fn pr20_impl04_pos_03_the_handle_is_the_content_identity_of_the_artifact() {
    let diff = computed(&World::repair());
    let mut preimage = diff.artifact().artifact_json();
    if let Json::Object(fields) = &mut preimage {
        assert!(fields.remove("diff_id").is_some());
    }
    // The preimage is the versioned layer tag, a NUL, then the canonical bytes.
    let mut tagged = b"semantic-diff/program-layer/unclassified/v1\0".to_vec();
    tagged.extend_from_slice(&preimage.to_canonical_bytes());
    let expected = format!("diff_{}", Blake3Hasher::hash(&tagged).to_token());
    assert_eq!(diff.handle(), expected);
    // Deterministic: a second computation is byte-identical.
    assert_eq!(
        computed(&World::repair()).to_artifact_bytes(),
        diff.to_artifact_bytes()
    );
    // A different revision pair is a different identity.
    let weakened = computed(&World::rebound_to(revised("in_weak_bound", |d| {
        replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
    })));
    assert_ne!(weakened.handle(), diff.handle());
}

/// Differential: the transaction's diff is exactly what the RFC 0031 assembler
/// (`continuum_semantic_diff::artifact::assemble`, the oracle) produces for the same
/// revision pair, request fields and minted handle. `continuum_repair::diff` is the
/// subject.
#[test]
fn pr20_impl04_dif_01_the_transaction_diff_is_the_assembler_output() {
    let weak = revised("in_weak_fault", |d| replace_once(d, r#""crash","#, ""));
    let base = decode(CONTRACT);
    let transaction = applied();
    for (world, after) in [
        (World::repair(), base.clone()),
        (World::rebound_to(weak.clone()), weak),
    ] {
        let subject = match continuum_repair::diff::compute(&transaction, &world, &world, &world) {
            DiffOutcome::Computed(diff) => diff,
            DiffOutcome::Inconclusive(reason) => panic!("{reason}"),
        };
        let request = DiffRequest {
            diff_id: DiffId::new(subject.handle()).expect("a diff_ handle"),
            before_snapshot: oracle::SnapshotId::new(transaction.base_snapshot().as_str())
                .expect("ws_"),
            after_snapshot: oracle::SnapshotId::new(
                transaction.candidate_snapshot().expect("applied").as_str(),
            )
            .expect("ws_"),
            before_intent: intent(BASE_INTENT),
            after_intent: after.intent_id().clone(),
            before: &base,
            after: &after,
            requested_assurance: base.assurance().minimum(),
            path: continuum_intent::change_policy::AcceptancePath::AgentAccept,
            semantic_changes: Vec::new(),
            evidence: Vec::new(),
        };
        let expected = continuum_semantic_diff::artifact::assemble(&request).expect("assembles");
        assert_eq!(subject.to_artifact_bytes(), expected.to_artifact_bytes());
    }
}

/// Metamorphic, relation "set/map insertion order": the evidence store may answer in
/// any order, and the diff is byte-identical.
#[test]
fn pr20_impl04_met_01_evidence_insertion_order_does_not_move_a_byte() {
    let weak = || {
        revised("in_weak_bound", |d| {
            replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
        })
    };
    let records = vec![
        evidence_record("ev_bound_conditional", vec![bound_edge()]),
        evidence_record("ev_agreement_run", Vec::new()),
        evidence_record("proof_register_model", Vec::new()),
    ];
    let mut forward = World::rebound_to(weak());
    forward.evidence = ScopeAnswer::Complete(records.clone());
    let mut backward = World::rebound_to(weak());
    backward.evidence = ScopeAnswer::Complete(records.into_iter().rev().collect());
    let one = computed(&forward);
    let two = computed(&backward);
    assert_eq!(one.to_artifact_bytes(), two.to_artifact_bytes());
    assert_eq!(one.handle(), two.handle());
    // Not vacuous: the evidence reached the impact set, all of it to revalidate.
    assert_eq!(one.artifact().impact().unknown().count(), 3);
    assert_eq!(one.artifact().impact().reused().count(), 0);
}

/// Metamorphic, relation "serialization round trip": a registry that holds contracts
/// read back from their own artifact bytes yields the same diff.
#[test]
fn pr20_impl04_met_02_contracts_read_back_from_their_bytes_give_the_same_diff() {
    let weak = revised("in_weak_property", |d| {
        replace_once(d, AGREEMENT_EQ, &format!("{AGREEMENT_EQ},{AGREEMENT_NE}"))
    });
    let round_trip = |contract: &IntentContract| {
        IntentContract::decode(&contract.to_artifact_bytes()).expect("round trip")
    };
    let direct = World::rebound_to(weak.clone());
    let mut read_back = World::rebound_to(round_trip(&weak));
    read_back.registry[0].1 = round_trip(&decode(CONTRACT));
    assert_eq!(
        computed(&direct).to_artifact_bytes(),
        computed(&read_back).to_artifact_bytes()
    );
}

// --- negative: each weakening is a privileged intent revision ------------------------

fn assert_reclassified(
    handle: &str,
    edit: impl Fn(&str) -> String,
    field: PolicyField,
    relation: Relation,
    weakening: Weakening,
) -> IntentRevision {
    let world = World::rebound_to(revised(handle, edit));
    let outcome = diff::compute(&applied(), &world, &world, &world);
    assert_eq!(outcome.gate_status(), GateStatus::Failed, "{weakening:?}");
    let reclassified = revision(&world);
    assert!(reclassified.rebinds());
    assert_ne!(
        reclassified.decision(),
        PolicyDecision::Allow,
        "{weakening:?}"
    );
    assert!(
        reclassified
            .changes()
            .iter()
            .any(|change| change.field() == field
                && change.relation() == relation
                && change.class() == ChangeClass::Weakening(weakening)),
        "{weakening:?}: {:?}",
        reclassified.changes()
    );
    assert!(reclassified.weakenings().contains(&weakening));
    reclassified
}

#[test]
fn pr20_impl04_neg_01_a_weakened_property_is_a_privileged_intent_revision() {
    let weakened = assert_reclassified(
        "in_weak_property",
        |d| replace_once(d, AGREEMENT_EQ, &format!("{AGREEMENT_EQ},{AGREEMENT_NE}")),
        PolicyField::Properties,
        Relation::Weakened,
        Weakening::WeakenedProperty,
    );
    assert_eq!(weakened.decision(), PolicyDecision::Block);
}

#[test]
fn pr20_impl04_neg_02_a_strengthened_assumption_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_strong_assumption",
        |d| {
            replace_once(
                d,
                r#"{"classification":"environment""#,
                &format!("{OPERATOR_HONESTY},{{\"classification\":\"environment\""),
            )
        },
        PolicyField::Assumptions,
        Relation::Added,
        Weakening::StrengthenedAssumption,
    );
}

#[test]
fn pr20_impl04_neg_03_a_shrunk_bound_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_weak_bound",
        |d| replace_once(d, r#""nodes":3"#, r#""nodes":2"#),
        PolicyField::Bounds,
        Relation::Contracted,
        Weakening::ShrunkBound,
    );
}

#[test]
fn pr20_impl04_neg_04_a_hidden_event_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_hidden_event",
        |d| replace_once(d, r#""events":["Committed"]"#, r#""events":[]"#),
        PolicyField::Observers,
        Relation::Coarsened,
        Weakening::HiddenEvent,
    );
}

#[test]
fn pr20_impl04_neg_05_a_removed_fault_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_weak_fault",
        |d| replace_once(d, r#""crash","#, ""),
        PolicyField::Faults,
        Relation::Removed,
        Weakening::RemovedFault,
    );
}

#[test]
fn pr20_impl04_neg_06_a_downgraded_assurance_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_weak_assurance",
        |d| replace_once(d, r#""minimum":"validated""#, r#""minimum":"bounded""#),
        PolicyField::Assurance,
        Relation::Downgraded,
        Weakening::DowngradedAssurance,
    );
}

#[test]
fn pr20_impl04_neg_07_the_retained_weakened_diff_is_the_computed_bytes() {
    let diff = computed(&World::rebound_to(revised("in_weak_property", |d| {
        replace_once(d, AGREEMENT_EQ, &format!("{AGREEMENT_EQ},{AGREEMENT_NE}"))
    })));
    assert_eq!(
        text(&diff.to_artifact_bytes()),
        RETAINED_WEAKENED.trim_end()
    );
}

#[test]
fn pr20_impl04_neg_08_a_rebinding_the_verbs_would_allow_is_still_not_a_repair() {
    // `bounds` is `no-decrease`, so an expansion alone is `allow` under the verdict.
    let expanded = World::rebound_to(revised("in_more_nodes", |d| {
        replace_once(d, r#""nodes":3"#, r#""nodes":4"#)
    }));
    let expansion = revision(&expanded);
    assert_eq!(expansion.decision(), PolicyDecision::Allow);
    assert!(expansion.rebinds());
    assert!(expansion.weakenings().is_empty());
    assert_eq!(expansion.changes().len(), 1);
    assert_eq!(expansion.changes()[0].class(), ChangeClass::Other);
    assert_eq!(
        diff::compute(&applied(), &expanded, &expanded, &expanded).gate_status(),
        GateStatus::Failed
    );

    // The same content under another handle: nothing classifies, and the rebinding is
    // still privileged (RFC 0037).
    let renamed = World::rebound_to(revised("in_same_content", str::to_owned));
    let rename = revision(&renamed);
    assert!(rename.rebinds());
    assert!(rename.changes().is_empty());
    assert_eq!(rename.decision(), PolicyDecision::Allow);
}

#[test]
fn pr20_impl04_neg_09_an_undecided_relation_fails_closed_to_a_revision() {
    let merged = World::rebound_to(revised("in_merged_map", |d| {
        replace_once(
            d,
            r#""map_id":"map_runtime_to_abstract_v1""#,
            r#""map_id":"map_runtime_to_abstract_v2_merged""#,
        )
    }));
    let undecided = revision(&merged);
    assert_eq!(undecided.decision(), PolicyDecision::Review);
    assert_eq!(undecided.changes().len(), 1);
    assert_eq!(undecided.changes()[0].field(), PolicyField::AbstractionMaps);
    assert_eq!(undecided.changes()[0].relation(), Relation::Unknown);
    assert_eq!(undecided.changes()[0].class(), ChangeClass::Undecided);
}

#[test]
fn pr20_impl04_neg_10_a_forged_diff_is_refused() {
    let weak = World::rebound_to(revised("in_weak_property", |d| {
        replace_once(d, AGREEMENT_EQ, &format!("{AGREEMENT_EQ},{AGREEMENT_NE}"))
    }));
    let transaction = applied();
    let honest = computed(&weak);

    // The computed bytes are accepted, and give back the computed diff.
    let accepted = diff::verify_claimed_diff(
        &transaction,
        &honest.to_artifact_bytes(),
        &weak,
        &weak,
        &weak,
    )
    .expect("the computed bytes verify");
    assert_eq!(accepted, honest);

    // The weakened diff with its decision flipped to `allow`.
    let flipped = text(&honest.to_artifact_bytes()).replacen(
        r#""decision":"block""#,
        r#""decision":"allow""#,
        1,
    );
    assert_ne!(flipped.as_bytes(), honest.to_artifact_bytes());
    // The clean repair's diff, claimed for the weakened candidate.
    let repair_bytes = computed(&World::repair()).to_artifact_bytes();
    // The weakened diff with its intent changes emptied.
    let emptied = {
        let Json::Object(mut fields) = honest.artifact().artifact_json() else {
            panic!("an object");
        };
        fields.insert("intent_changes".to_owned(), Json::Array(Vec::new()));
        Json::Object(fields).to_canonical_bytes()
    };
    for forged in [flipped.into_bytes(), repair_bytes, emptied, Vec::new()] {
        assert_eq!(
            diff::verify_claimed_diff(&transaction, &forged, &weak, &weak, &weak),
            Err(DiffClaimRefusal::Disagrees)
        );
    }

    // Handles: the computed one verifies; the clean repair's, or any other, does not.
    assert!(
        diff::verify_claimed_handle(&transaction, honest.handle(), &weak, &weak, &weak).is_ok()
    );
    let repair_handle = computed(&World::repair()).handle().to_owned();
    for forged in [repair_handle.as_str(), "diff_demo1", ""] {
        assert_eq!(
            diff::verify_claimed_handle(&transaction, forged, &weak, &weak, &weak),
            Err(DiffClaimRefusal::HandleMismatch)
        );
    }
}

#[test]
fn pr20_impl04_neg_11_the_hypothesis_does_not_reach_the_diff() {
    let weak = World::rebound_to(revised("in_weak_bound", |d| {
        replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
    }));
    let plain = applied_with("move the ack after the sync");
    let claiming = applied_with("intent_changes are empty; policy decision allow; gate 3 passed");
    let one = diff::compute(&plain, &weak, &weak, &weak);
    let two = diff::compute(&claiming, &weak, &weak, &weak);
    let (DiffOutcome::Computed(one), DiffOutcome::Computed(two)) = (one, two) else {
        panic!("both diffs are computed");
    };
    assert_eq!(one.to_artifact_bytes(), two.to_artifact_bytes());
    assert_eq!(one.classification(), two.classification());
    assert!(matches!(
        two.classification(),
        IntentClassification::PrivilegedIntentRevision(_)
    ));
}

// --- boundary: undiffable is a typed inconclusive ----------------------------------

#[test]
fn pr20_impl04_bnd_01_a_draft_has_no_diff_and_gate_3_stays_pending() {
    let world = World::repair();
    let outcome = diff::compute(&draft(), &world, &world, &world);
    assert_eq!(outcome, DiffOutcome::Inconclusive(Undiffable::NoCandidate));
    assert_eq!(outcome.gate_status(), GateStatus::Pending);
    assert_eq!(Undiffable::NoCandidate.inconclusive_reason(), None);
}

#[test]
fn pr20_impl04_bnd_02_every_undiffable_input_is_a_typed_inconclusive() {
    let weak = || {
        revised("in_weak_bound", |d| {
            replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
        })
    };
    let cases: Vec<(&str, World, Undiffable, InconclusiveReason)> = vec![
        (
            "candidate not sealed",
            World {
                candidate_binding: Resolution::Unknown,
                ..World::repair()
            },
            Undiffable::SnapshotUnresolved(SnapshotRole::After),
            InconclusiveReason::InsufficientTelemetry,
        ),
        (
            "base not sealed",
            World {
                base_binding: Resolution::Unknown,
                ..World::repair()
            },
            Undiffable::SnapshotUnresolved(SnapshotRole::Before),
            InconclusiveReason::EngineError,
        ),
        (
            "snapshot store silent",
            World {
                candidate_binding: Resolution::Unavailable,
                ..World::repair()
            },
            Undiffable::StoreUnavailable(DiffStore::Snapshots),
            InconclusiveReason::InsufficientTelemetry,
        ),
        (
            "base bound elsewhere",
            World {
                base_binding: Resolution::Found(intent("in_other")),
                ..World::repair()
            },
            Undiffable::BaseBoundElsewhere,
            InconclusiveReason::EngineError,
        ),
        (
            "candidate intent unregistered",
            World {
                candidate_binding: Resolution::Found(intent("in_unregistered")),
                ..World::repair()
            },
            Undiffable::IntentUnregistered(SnapshotRole::After),
            InconclusiveReason::InsufficientTelemetry,
        ),
        (
            "base intent unregistered",
            World {
                registry: Vec::new(),
                ..World::repair()
            },
            Undiffable::IntentUnregistered(SnapshotRole::Before),
            InconclusiveReason::EngineError,
        ),
        (
            "registry silent",
            World {
                registry_available: false,
                ..World::rebound_to(weak())
            },
            Undiffable::StoreUnavailable(DiffStore::IntentRegistry),
            InconclusiveReason::InsufficientTelemetry,
        ),
        (
            "candidate contract filed under a foreign handle",
            {
                let mut world = World::rebound_to(weak());
                world.candidate_binding = Resolution::Found(intent("in_filed_elsewhere"));
                world.registry[1].0 = intent("in_filed_elsewhere");
                world
            },
            Undiffable::RegistryInconsistent(SnapshotRole::After),
            InconclusiveReason::EngineError,
        ),
        (
            "evidence store silent",
            World {
                evidence: ScopeAnswer::Unavailable,
                ..World::rebound_to(weak())
            },
            Undiffable::StoreUnavailable(DiffStore::Evidence),
            InconclusiveReason::InsufficientTelemetry,
        ),
    ];
    for (name, world, expected, reason) in cases {
        let outcome = diff::compute(&applied(), &world, &world, &world);
        assert_eq!(
            outcome,
            DiffOutcome::Inconclusive(expected.clone()),
            "{name}"
        );
        assert_eq!(outcome.gate_status(), GateStatus::Inconclusive, "{name}");
        assert_eq!(expected.inconclusive_reason(), Some(reason), "{name}");
        // An undiffable input verifies no claim, whatever it says.
        assert_eq!(
            diff::verify_claimed_diff(
                &applied(),
                &computed(&World::repair()).to_artifact_bytes(),
                &world,
                &world,
                &world
            ),
            Err(DiffClaimRefusal::Undiffable(expected)),
            "{name}"
        );
    }
}

#[test]
fn pr20_impl04_bnd_03_an_unclassifiable_scope_is_inconclusive_not_a_partial_diff() {
    let mut world = World::rebound_to(revised("in_weak_bound", |d| {
        replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
    }));
    world.evidence = ScopeAnswer::Complete(vec![
        evidence_record("ev_twice", Vec::new()),
        evidence_record("ev_twice", vec![bound_edge()]),
    ]);
    let reason = inconclusive(&world);
    assert!(
        matches!(reason, Undiffable::Unclassifiable(_)),
        "{reason:?}"
    );
    assert_eq!(
        reason.inconclusive_reason(),
        Some(InconclusiveReason::EngineError)
    );
}

#[test]
fn pr20_impl04_bnd_04_an_empty_evidence_scope_is_an_answer_and_a_silent_one_is_not() {
    let weak = || {
        revised("in_weak_bound", |d| {
            replace_once(d, r#""nodes":3"#, r#""nodes":2"#)
        })
    };
    let empty = World::rebound_to(weak());
    assert!(matches!(
        diff::compute(&applied(), &empty, &empty, &empty),
        DiffOutcome::Computed(_)
    ));
    let silent = World {
        evidence: ScopeAnswer::Unavailable,
        ..World::rebound_to(weak())
    };
    assert_eq!(
        inconclusive(&silent),
        Undiffable::StoreUnavailable(DiffStore::Evidence)
    );
}

#[test]
fn pr20_impl04_neg_12_a_loosened_evidence_class_set_is_a_privileged_intent_revision() {
    assert_reclassified(
        "in_loose_evidence",
        |d| {
            replace_once(
                d,
                r#""accepted_evidence_classes":["certificate","#,
                r#""accepted_evidence_classes":["certificate","differential","#,
            )
        },
        PolicyField::Assurance,
        Relation::Added,
        Weakening::LoosenedEvidenceClasses,
    );
}

#[test]
fn pr20_impl04_bnd_05_a_base_intent_without_current_protection_is_inconclusive() {
    for (standing, expected) in [
        (IntentStanding::Proposed, Undiffable::BaseNotProtected),
        (IntentStanding::Superseded, Undiffable::BaseSuperseded),
    ] {
        let world = World {
            base_standing: standing,
            ..World::repair()
        };
        let outcome = diff::compute(&applied(), &world, &world, &world);
        assert_eq!(outcome, DiffOutcome::Inconclusive(expected.clone()));
        assert_eq!(outcome.gate_status(), GateStatus::Inconclusive);
        assert_eq!(
            expected.inconclusive_reason(),
            Some(InconclusiveReason::Unsupported)
        );
    }
}

#[test]
fn pr20_impl04_bnd_06_the_requested_assurance_is_never_below_the_gate_3_floor() {
    // A base contract whose own minimum is `observed`, the lowest level.
    let low = CONTRACT.replacen(r#""minimum":"validated""#, r#""minimum":"observed""#, 1);
    assert_ne!(low, CONTRACT);
    let mut world = World::repair();
    world.registry = vec![(intent(BASE_INTENT), decode(&low))];
    let diff = computed(&world);
    let Json::Object(fields) = diff.artifact().artifact_json() else {
        panic!("an object");
    };
    assert_eq!(
        fields.get("requested_assurance"),
        Some(&Json::String("validated".to_owned()))
    );
}

/// Scoped evidence the intent side would reuse: no edge at all, and a witnessed
/// independence on an `Exact` edge.
fn reusable_evidence() -> Vec<EvidenceRecord> {
    vec![
        EvidenceRecord {
            id: EvidenceId::new("ev_pre_repair_run").expect("an evidence id"),
            keyed_to_before_intent: false,
            key_change_reuse_witness: false,
            edges: Vec::new(),
        },
        EvidenceRecord {
            id: EvidenceId::new("proof_witnessed_independent").expect("an evidence id"),
            keyed_to_before_intent: true,
            key_change_reuse_witness: false,
            edges: vec![DependencyEdge {
                reason: DependencyReason::ReliesOnAssumptionFairnessBound,
                class: ReuseEdgeClass::Exact,
                independence: Independence::Independent { witnessed: true },
            }],
        },
    ]
}

/// The ack-after-sync transaction's draft with a no-op change applied: the candidate
/// is byte-identical to the base.
fn applied_no_op() -> RepairTransaction<Blake3Hasher> {
    draft()
        .apply(
            &Proposal::new(
                Hypothesis::new("rewrite the replica unchanged"),
                vec![common::replace(
                    continuum_repair::hypothesis::ChangeKind::Rust,
                    common::REPLICA,
                    common::REPLICA_BEFORE.as_bytes(),
                )],
            ),
            &common::base_content(),
        )
        .expect("the no-op applies")
        .into_parts()
        .0
}

#[test]
fn pr20_impl04_neg_13_scoped_evidence_is_never_reused_while_the_program_side_is_unclassified() {
    // Same intent, a changed `rust` file, and pre-repair evidence the intent side
    // alone would reuse.
    let mut same_intent = World::repair();
    same_intent.evidence = ScopeAnswer::Complete(reusable_evidence());
    // And a rebinding whose content is equal.
    let mut rebound = World::rebound_to(revised("in_same_content", str::to_owned));
    rebound.evidence = ScopeAnswer::Complete(reusable_evidence());
    for world in [same_intent, rebound] {
        let diff = computed(&world);
        let impact = diff.artifact().impact();
        assert_eq!(impact.reused().count(), 0, "nothing is reused");
        assert_eq!(
            impact.unknown().count() + impact.invalidated().count(),
            2,
            "every scoped entry is revalidated"
        );
        let Json::Object(fields) = diff.artifact().artifact_json() else {
            panic!("an object");
        };
        let Some(Json::Object(wire)) = fields.get("impact") else {
            panic!("an impact object");
        };
        assert_eq!(wire.get("reused"), Some(&Json::Array(Vec::new())));
    }

    // Non-vacuity: with the program side classified (a candidate identical to its
    // base), the same evidence is reused.
    let mut unchanged = World::repair();
    unchanged.candidate = common::base_id();
    unchanged.evidence = ScopeAnswer::Complete(reusable_evidence());
    let transaction = applied_no_op();
    assert_eq!(transaction.candidate_snapshot(), Some(&common::base_id()));
    let DiffOutcome::Computed(diff) =
        diff::compute(&transaction, &unchanged, &unchanged, &unchanged)
    else {
        panic!("computed");
    };
    assert_eq!(diff.artifact().program_layer(), ProgramLayer::Classified);
    assert_eq!(diff.artifact().impact().reused().count(), 2);
}

#[test]
fn pr20_impl04_neg_14_an_empty_program_section_is_never_read_as_no_change() {
    let world = World::repair();
    let outcome = diff::compute(&applied(), &world, &world, &world);
    let DiffOutcome::Computed(diff) = &outcome else {
        panic!("computed");
    };
    let Json::Object(fields) = diff.artifact().artifact_json() else {
        panic!("an object");
    };
    // The wire section is empty and the decision is the intent verdict, `allow`...
    assert_eq!(
        fields.get("semantic_changes"),
        Some(&Json::Array(Vec::new()))
    );
    assert_eq!(diff.artifact().decision(), PolicyDecision::Allow);
    // ...but the empty section is typed as unclassified, and gate 3 does not pass.
    assert_eq!(diff.artifact().program_layer(), ProgramLayer::Unclassified);
    assert_ne!(outcome.gate_status(), GateStatus::Passed);
    assert_eq!(outcome.gate_status(), GateStatus::Inconclusive);

    // Only a candidate identical to its base, which has no program change to
    // classify, passes gate 3.
    let mut unchanged = World::repair();
    unchanged.candidate = common::base_id();
    let outcome = diff::compute(&applied_no_op(), &unchanged, &unchanged, &unchanged);
    assert_eq!(outcome.gate_status(), GateStatus::Passed);
    assert_eq!(outcome.inconclusive_reason(), None);
}

/// The diff of the ack-after-sync transaction's inputs, assembled by the oracle under
/// `program` and minted under that layer: what a cache written under another layer
/// would hold.
fn diff_under(program: ProgramLayer) -> (String, Vec<u8>) {
    let transaction = applied();
    let base = decode(CONTRACT);
    let mut request = DiffRequest {
        diff_id: DiffId::new("diff_unminted").expect("a diff_ handle"),
        before_snapshot: oracle::SnapshotId::new(transaction.base_snapshot().as_str())
            .expect("ws_"),
        after_snapshot: oracle::SnapshotId::new(
            transaction.candidate_snapshot().expect("applied").as_str(),
        )
        .expect("ws_"),
        before_intent: intent(BASE_INTENT),
        after_intent: intent(BASE_INTENT),
        before: &base,
        after: &base,
        requested_assurance: base.assurance().minimum(),
        path: continuum_intent::change_policy::AcceptancePath::AgentAccept,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    };
    let draft = oracle::assemble_under(&request, program).expect("assembles");
    let Json::Object(mut fields) = draft.artifact_json() else {
        panic!("an object");
    };
    fields.remove("diff_id");
    let handle =
        diff::mint_handle::<Blake3Hasher>(program, &Json::Object(fields).to_canonical_bytes());
    request.diff_id = DiffId::new(&handle).expect("a diff_ handle");
    let bytes = oracle::assemble_under(&request, program)
        .expect("assembles")
        .to_artifact_bytes();
    (handle, bytes)
}

#[test]
fn pr20_impl04_neg_15_equal_bytes_under_the_two_layers_have_distinct_handles() {
    let preimage = computed(&World::repair()).to_artifact_bytes();
    let classified = diff::mint_handle::<Blake3Hasher>(ProgramLayer::Classified, &preimage);
    let unclassified = diff::mint_handle::<Blake3Hasher>(ProgramLayer::Unclassified, &preimage);
    assert_ne!(classified, unclassified);
    // The two layers' artifacts for one transaction's inputs: the wire sections are
    // equal apart from the handle, and the handles differ.
    let (classified_handle, classified_bytes) = diff_under(ProgramLayer::Classified);
    let (unclassified_handle, unclassified_bytes) = diff_under(ProgramLayer::Unclassified);
    assert_ne!(classified_handle, unclassified_handle);
    assert_eq!(
        text(&classified_bytes).replace(&classified_handle, "diff_x"),
        text(&unclassified_bytes).replace(&unclassified_handle, "diff_x"),
        "no evidence is in scope, so only the handle carries the layer"
    );
    // The transaction's own diff is the unclassified one.
    assert_eq!(computed(&World::repair()).handle(), unclassified_handle);
}

#[test]
fn pr20_impl04_neg_16_a_classified_claim_for_an_unclassified_recompute_is_refused() {
    let world = World::repair();
    let transaction = applied();
    let (handle, bytes) = diff_under(ProgramLayer::Classified);
    let refusal = DiffClaimRefusal::LayerMismatch {
        computed: ProgramLayer::Unclassified,
    };
    assert_eq!(
        diff::verify_claimed_handle(&transaction, &handle, &world, &world, &world),
        Err(refusal.clone())
    );
    assert_eq!(
        diff::verify_claimed_diff(&transaction, &bytes, &world, &world, &world),
        Err(refusal)
    );
    // The unclassified claim, which is the computed one, verifies.
    let (handle, bytes) = diff_under(ProgramLayer::Unclassified);
    assert!(diff::verify_claimed_handle(&transaction, &handle, &world, &world, &world).is_ok());
    assert!(diff::verify_claimed_diff(&transaction, &bytes, &world, &world, &world).is_ok());
}

#[test]
fn pr20_impl04_neg_17_a_cached_artifact_cannot_yield_gate_3_passed() {
    let world = World::repair();
    let transaction = applied();
    // A cache holding the classified artifact, as if written before the layer rule.
    let (_, classified) = diff_under(ProgramLayer::Classified);
    assert_eq!(
        diff::gate_status_of_cached(&transaction, &classified, &world, &world, &world),
        Err(DiffClaimRefusal::LayerMismatch {
            computed: ProgramLayer::Unclassified
        })
    );
    // The honest cached artifact yields the recomputed status, never `passed`.
    let honest = computed(&world).to_artifact_bytes();
    assert_eq!(
        diff::gate_status_of_cached(&transaction, &honest, &world, &world, &world),
        Ok(GateStatus::Inconclusive)
    );
    // Garbage and silence yield no status.
    assert_eq!(
        diff::gate_status_of_cached(&transaction, b"{}", &world, &world, &world),
        Err(DiffClaimRefusal::Disagrees)
    );
    let silent = World {
        evidence: ScopeAnswer::Unavailable,
        ..World::repair()
    };
    assert!(matches!(
        diff::gate_status_of_cached(&transaction, &honest, &silent, &silent, &silent),
        Err(DiffClaimRefusal::Undiffable(_))
    ));
    // Non-vacuity: a candidate identical to its base passes through the same path.
    let mut unchanged = World::repair();
    unchanged.candidate = common::base_id();
    let no_op = applied_no_op();
    let DiffOutcome::Computed(diff) = diff::compute(&no_op, &unchanged, &unchanged, &unchanged)
    else {
        panic!("computed");
    };
    assert_eq!(
        diff::gate_status_of_cached(
            &no_op,
            &diff.to_artifact_bytes(),
            &unchanged,
            &unchanged,
            &unchanged
        ),
        Ok(GateStatus::Passed)
    );
}
