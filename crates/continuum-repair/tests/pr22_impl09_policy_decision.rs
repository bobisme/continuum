//! PR-22 / IMPL-09 (bn-1plr): the receipt's policy decision.
//!
//! RFC 0032 makes `policy_decision` "the server-recomputed `allow`" ("The promotion
//! receipt"), recomputed at promotion together with "the semantic and intent
//! classification" and never taken from a client or a cache ("Promotion" step 2), and
//! required on a receipt (correction 11). RFC 0031 has no `unknown` decision. This suite
//! checks, on real exact-replay versions of the replicated register's ack-after-sync
//! repair (M01), that the decision is computed inside verification from the named
//! version's recorded gates and the classification of the diff the receipt recomputes
//! from the stores (PR-22 / IMPL-03), that the caller supplies only gate reasons, and
//! that a forged, blocked, unclassified or inconclusive claim is refused typed and with
//! its reasons. The eligible path needs a record with gates 2, 3 and 5–11 passed and a
//! classified candidate, which no evaluation writes yet; it is the crate's unit tests'
//! (`receipt::policy::tests::pr22_impl09_*`).
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is | Checked by |
//! |---|---|---|
//! | `pr22-impl09-ack-after-sync-policy-decision` | `tests/fixtures/pr22-impl09-ack-after-sync-policy-decision.json`: the M01 version's IMPL-09 fragment, `repair_transaction` only: gate 4 is inconclusive under `Exact` and the diff's program layer is unclassified, so the receipt renders no decision | this suite (byte equality) and `validate_dossier.py` (each field against the receipt schema's own property) |
//! | `pr22-impl09-gate-1-failure-policy-decision` | `tests/fixtures/pr22-impl09-gate-1-failure-policy-decision.json`: a version whose base does not reproduce the failure, gate 1 `failed`: `policy_decision` `block` | as above |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite the fixtures from the library. Stable test
//! artifact ids are in each test's doc comment: `pr22-impl09-{pos,neg,bnd,met,diff}-NN`.

mod common;

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::PolicyDecision;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::diff::{
    self, DiffOutcome, DiffScope, ImpactScope, IntentClassification as DiffClassification,
    ScopeAnswer,
};
use continuum_repair::handle::{CrashpackId, RepairId, SnapshotId};
use continuum_repair::hypothesis::{Hypothesis, Proposal};
use continuum_repair::policy::{
    ClaimRefusal as VerdictClaimRefusal, GateReasons, NoReasons, PolicyVerdict, ReplayRecords,
    Standing, Undecided, Verdict, VerdictClaim,
};
use continuum_repair::receipt::{
    CandidateEvidence, CertificateRecord, ClaimRefusal, ClaimedReceipt, DecisionRefusal,
    DecisionStanding, DiffGap, IntentRegistry, IntentStanding, PackCase, ReceiptClassification,
    ReceiptDecision, ReceiptDiff, ReceiptField, ReceiptSkeleton, RegisteredIntent, SealedSnapshots,
    TransactionStore, UndiffableCause, UnenforcedGate, VerifyRefusal, verify_for_promotion,
    verify_skeleton,
};
use continuum_repair::replay::{
    self, EvidenceRecord, RecordedRun, ReplayOutcome, ReplayRun, Replayer,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, GateStatus, RepairTransaction, Resolution,
};
use continuum_semantic_diff::impact::{
    DependencyEdge, DependencyReason, EvidenceId, EvidenceRecord as ScopeRecord, Independence,
    ReuseEdgeClass,
};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_workspace::snapshot::WorkspaceContent;

use common::{ack_after_sync_change, base_content, base_id, repaired_content, snapshot_id};

type Tx = RepairTransaction<Blake3Hasher>;

const REGISTER_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const SCHEDULE: &[u8] = b"r0/i0/Reserve(0)#0\nr0/i0/Submit(0)#0\nr0/i0/Confirm(0)#0\n";
const JOURNAL: &[u8] = b"journal: ack v0; lose v0; ack v1";
const FAILURE: &[u8] = b"Agreement(41)";

const M01_FIXTURE: &str = "tests/fixtures/pr22-impl09-ack-after-sync-policy-decision.json";
const BLOCKED_FIXTURE: &str = "tests/fixtures/pr22-impl09-gate-1-failure-policy-decision.json";

// --- the replicated register's M01 transaction ---------------------------------------

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the contract decodes")
}

fn contract() -> IntentContract {
    decode(REGISTER_CONTRACT)
}

fn intent() -> IntentId {
    contract().intent_id().clone()
}

/// The register contract filed under `handle`, with `edit` applied.
fn revised(handle: &str, edit: impl Fn(&str) -> String) -> IntentContract {
    let base = format!("\"intent_id\":\"{}\"", intent().as_str());
    let edited = edit(REGISTER_CONTRACT);
    assert_eq!(edited.matches(&base).count(), 1);
    decode(&edited.replacen(&base, &format!("\"intent_id\":\"{handle}\""), 1))
}

fn crashpack() -> Vec<u8> {
    RecordedRun::new(
        base_id(),
        SCHEDULE.to_vec(),
        JOURNAL.to_vec(),
        FAILURE.to_vec(),
    )
    .unwrap()
    .canonical_bytes()
}

fn scope_record(id: &str) -> ScopeRecord {
    ScopeRecord {
        id: EvidenceId::new(id).unwrap(),
        keyed_to_before_intent: true,
        key_change_reuse_witness: false,
        edges: vec![DependencyEdge {
            reason: DependencyReason::ReliesOnAssumptionFairnessBound,
            class: ReuseEdgeClass::Exact,
            independence: Independence::Unknown,
        }],
    }
}

/// The daemon's stores: the register contract (and `rebound`, the contract the candidate
/// is bound to when it is not the base intent), the base and the candidate, the
/// transactions, no certificate and no pack case, and the evidence scope `scope`.
struct World {
    transactions: Vec<Tx>,
    scope: Vec<ScopeRecord>,
    rebound: Option<IntentContract>,
}

impl World {
    fn of(transactions: &[&Tx]) -> Self {
        Self {
            transactions: transactions.iter().map(|tx| (*tx).clone()).collect(),
            scope: Vec::new(),
            rebound: None,
        }
    }
}

impl FailureBinding for World {
    fn failure_base(&self, _: &CrashpackId) -> Resolution<SnapshotId> {
        Resolution::Found(base_id())
    }
    fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(intent())
    }
}

impl IntentRegistry for World {
    fn registered(&self, handle: &IntentId) -> Resolution<RegisteredIntent> {
        if handle == &intent() {
            return Resolution::Found(RegisteredIntent::new(contract(), IntentStanding::Accepted));
        }
        match &self.rebound {
            Some(other) if other.intent_id() == handle => Resolution::Found(RegisteredIntent::new(
                other.clone(),
                IntentStanding::Accepted,
            )),
            _ => Resolution::Unknown,
        }
    }
}

impl SealedSnapshots for World {
    fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if snapshot == &base_id() {
            Resolution::Found(intent())
        } else if snapshot == &snapshot_id(&repaired_content()) {
            Resolution::Found(
                self.rebound
                    .as_ref()
                    .map_or_else(intent, |other| other.intent_id().clone()),
            )
        } else {
            Resolution::Unknown
        }
    }
}

impl TransactionStore<Blake3Hasher> for World {
    fn transaction(&self, repair: &RepairId) -> Resolution<Tx> {
        self.transactions
            .iter()
            .find(|tx| tx.repair_id() == repair)
            .cloned()
            .map_or(Resolution::Unknown, Resolution::Found)
    }
}

impl CandidateEvidence for World {
    fn certificates(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<CertificateRecord>> {
        Resolution::Found(Vec::new())
    }
    fn unsupported_pack_cases(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<PackCase>> {
        Resolution::Found(Vec::new())
    }
}

impl ImpactScope for World {
    fn evidence(&self, _: DiffScope<'_>) -> ScopeAnswer {
        ScopeAnswer::Complete(self.scope.clone())
    }
}

fn applied() -> Tx {
    let failure = CrashpackId::new(&format!(
        "crash_{}",
        Blake3Hasher::hash(&crashpack()).to_token()
    ))
    .unwrap();
    Tx::begin(failure, GateProfile::PhaseB, &World::of(&[]))
        .unwrap()
        .apply(
            &Proposal::new(
                Hypothesis::new("move the ack after the storage sync"),
                vec![ack_after_sync_change()],
            ),
            &base_content(),
        )
        .unwrap()
        .into_parts()
        .0
}

struct Scripted(BTreeMap<SnapshotId, ReplayOutcome>);

impl Replayer for Scripted {
    fn identity(&self) -> &str {
        "scripted/1"
    }
    fn replay(&self, program: &WorkspaceContent, _: &[u8]) -> ReplayOutcome {
        self.0[&snapshot_id(program)].clone()
    }
}

fn exact() -> ReplayOutcome {
    ReplayOutcome::Ran(ReplayRun::new(
        JOURNAL.to_vec(),
        Some(FAILURE.to_vec()),
        Vec::new(),
        0,
    ))
}

fn clean(eliminated: Vec<u64>, reordered: u64) -> ReplayOutcome {
    ReplayOutcome::Ran(ReplayRun::new(
        b"journal: ack v1".to_vec(),
        None,
        eliminated,
        reordered,
    ))
}

fn evaluated(base: ReplayOutcome, candidate: ReplayOutcome) -> (Tx, Vec<EvidenceRecord>) {
    replay::evaluate(
        &applied(),
        &crashpack(),
        &base_content(),
        &repaired_content(),
        &Scripted(BTreeMap::from([
            (base_id(), base),
            (snapshot_id(&repaired_content()), candidate),
        ])),
        None,
    )
    .unwrap()
    .into_parts()
}

/// M01: the failure replays exactly on the base (gate 1 passed); the ack-after-sync
/// candidate runs clean but drops recorded steps 2 and 5 and reorders one, so under
/// `Exact` gate 4 is inconclusive, `AbstractionAmbiguity`.
fn m01() -> (Tx, Vec<EvidenceRecord>) {
    evaluated(exact(), clean(vec![2, 5], 1))
}

/// A base on which the recorded failure does not reproduce: gate 1 failed.
fn gate_1_failure() -> (Tx, Vec<EvidenceRecord>) {
    evaluated(clean(Vec::new(), 0), clean(Vec::new(), 0))
}

fn compose(tx: &Tx, world: &World) -> ReceiptSkeleton {
    ReceiptSkeleton::compose(tx, world, world, world, world).expect("the version composes")
}

/// The decision of `tx` under `world`, as verification recomputes it.
fn decide(tx: &Tx, world: &World, reasons: &impl GateReasons) -> ReceiptDecision {
    ReceiptDecision::of_skeleton(&compose(tx, world), tx, reasons).unwrap()
}

// --- claims ---------------------------------------------------------------------------

/// A client's claim for `tx`: its derived owned fields, in-profile gates `passed`, and
/// `policy_decision` set to `decision`.
fn claim_fields(tx: &Tx, world: &World, decision: PolicyDecision) -> BTreeMap<String, Json> {
    let Json::Object(mut fields) = compose(tx, world).fields_json() else {
        unreachable!("a fragment is an object")
    };
    let gates = GateName::ALL
        .into_iter()
        .map(|gate| {
            let status = if tx.gate_profile().enforces(gate) {
                "passed"
            } else {
                UnenforcedGate::STATUS
            };
            Json::Object(BTreeMap::from([
                ("gate".to_owned(), Json::String(gate.token().to_owned())),
                ("status".to_owned(), Json::String(status.to_owned())),
            ]))
        })
        .collect();
    fields.insert("gates".to_owned(), Json::Array(gates));
    fields.insert(
        "policy_decision".to_owned(),
        Json::String(decision.wire().to_owned()),
    );
    fields
}

fn claim(tx: &Tx, world: &World, decision: PolicyDecision) -> ClaimedReceipt {
    ClaimedReceipt::parse(&Json::Object(claim_fields(tx, world, decision)).to_canonical_bytes())
        .expect("a well-formed claim")
}

/// Promotion verification of `decision` for `tx`, with the policy refusal it must be.
fn promote_refusal(
    tx: &Tx,
    decision: PolicyDecision,
    reasons: &impl GateReasons,
) -> DecisionRefusal {
    let world = World::of(&[tx]);
    match verify_for_promotion(
        &claim(tx, &world, decision),
        &world,
        &world,
        &world,
        &world,
        &world,
        reasons,
    ) {
        Err(VerifyRefusal::PolicyDecision(refusal)) => refusal,
        other => panic!("the policy step refuses {decision}: {other:?}"),
    }
}

fn published_refusal(
    tx: &Tx,
    decision: PolicyDecision,
    reasons: &impl GateReasons,
) -> DecisionRefusal {
    let world = World::of(&[tx]);
    match verify_skeleton(
        &claim(tx, &world, decision),
        &world,
        &world,
        &world,
        &world,
        &world,
        reasons,
    ) {
        Err(VerifyRefusal::PolicyDecision(refusal)) => refusal,
        other => panic!("the policy step refuses {decision}: {other:?}"),
    }
}

fn fixture_matches(relative: &str, bytes: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if std::env::var_os("CONTINUUM_REPAIR_BLESS").is_some() {
        std::fs::write(&path, bytes).expect("the fixture is writable");
    }
    let retained = std::fs::read(&path).expect("the retained fixture exists");
    assert!(
        retained == bytes,
        "{relative} is stale; rerun with CONTINUUM_REPAIR_BLESS=1\nlibrary:  {}\nretained: {}",
        String::from_utf8_lossy(bytes),
        String::from_utf8_lossy(&retained),
    );
}

/// The phase-b gates 1–11 an M01 version leaves undecided, with gate 4's standing.
fn m01_undecided(gate_4: Resolution<InconclusiveReason>) -> Vec<Undecided> {
    GateName::ALL
        .into_iter()
        .filter(|gate| {
            GateProfile::PhaseB.enforces(*gate)
                && !matches!(gate, GateName::BaseReplay | GateName::ReceiptGeneration)
        })
        .map(|gate| Undecided {
            gate,
            standing: if gate == GateName::ExactRegression {
                Standing::Inconclusive(gate_4.clone())
            } else {
                Standing::Pending
            },
        })
        .collect()
}

// --- positive ---------------------------------------------------------------------------

/// `pr22-impl09-pos-01`: the decision is computed from the named version's record and
/// the classification of the diff recomputed from the stores, whatever is claimed; the
/// M01 version's fragment renders no decision and a gate-1 failure's renders `block`,
/// both retained.
#[test]
fn pr22_impl09_positive_the_decision_is_the_named_versions_recomputation() {
    let (tx, records) = m01();
    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    let world = World::of(&[&tx]);
    let decision = decide(&tx, &world, &reasons);
    assert_eq!(
        decision.verdict(),
        &PolicyVerdict::compute(&tx, &reasons, None).unwrap()
    );
    assert_eq!(
        decision.classification(),
        ReceiptClassification::Absent(DiffGap::ProgramLayerUnclassified)
    );
    assert_eq!(decision.verdict().repair_id(), tx.repair_id());
    assert_eq!(decision.field(), None);
    fixture_matches(M01_FIXTURE, &decision.fields_json().to_canonical_bytes());

    // Every claim is refused with the same recomputation: the claim is compared, never
    // read into it.
    for claimed in PolicyDecision::ALL {
        let refusal = promote_refusal(&tx, claimed, &reasons);
        assert_eq!(refusal.recomputed(), Some(&decision));
    }

    let (blocked, records) = gate_1_failure();
    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    let decision = decide(&blocked, &World::of(&[&blocked]), &reasons);
    assert_eq!(decision.field(), Some(PolicyDecision::Block));
    fixture_matches(
        BLOCKED_FIXTURE,
        &decision.fields_json().to_canonical_bytes(),
    );
}

// --- negative ---------------------------------------------------------------------------

/// `pr22-impl09-neg-01`: M01, the ack-after-sync transaction. Gate 4 is inconclusive,
/// not exact under `Exact`; the recomputed diff leaves gate 3 inconclusive
/// (`Unsupported`) because the program-side layer is unclassified, so the
/// classification is absent; no evaluation records gate 3 yet, so the record shows it
/// `pending`. The verdict is inconclusive and carries no decision. A claimed `allow` is
/// refused at promotion and on publication, and the refusal says why: every undecided
/// gate, gate 4 with its reason, and the absent classification.
#[test]
fn pr22_impl09_negative_m01_is_not_promotable_and_says_why() {
    let (tx, records) = m01();
    let world = World::of(&[&tx]);
    let diff = diff::compute(&tx, &world, &world, &world);
    assert!(matches!(diff, DiffOutcome::Computed(_)));
    assert_eq!(diff.gate_status(), GateStatus::Inconclusive);
    assert_eq!(
        diff.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert_eq!(tx.gates()[2].status(), GateStatus::Pending);
    assert_eq!(tx.gates()[3].status(), GateStatus::Inconclusive);

    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    for refusal in [
        promote_refusal(&tx, PolicyDecision::Allow, &reasons),
        published_refusal(&tx, PolicyDecision::Allow, &reasons),
    ] {
        let DecisionRefusal::Disagrees {
            claimed: PolicyDecision::Allow,
            recomputed,
        } = &refusal
        else {
            panic!("a forged allow disagrees: {refusal:?}");
        };
        assert_eq!(
            recomputed.verdict().verdict(),
            &Verdict::Inconclusive {
                undecided: m01_undecided(Resolution::Found(
                    InconclusiveReason::AbstractionAmbiguity
                )),
            }
        );
        let text = refusal.to_string();
        for why in [
            "`policy_decision` claims allow",
            "inconclusive; undecided:",
            "intent_integrity pending",
            "exact_regression inconclusive (",
            "code_and_security pending",
            "classification absent (program layer unclassified)",
        ] {
            assert!(text.contains(why), "{why:?} in {text}");
        }
        assert!(text.contains(tx.repair_id().as_str()));
    }
}

/// `pr22-impl09-neg-02`: a gate-1 failure is blocked. A claimed `allow` or `review`
/// disagrees; the honest `block` is not promotable. Each refusal names `base_replay` as
/// failed and discloses the undecided gates.
#[test]
fn pr22_impl09_negative_blocked_but_claimed_allow_is_refused() {
    let (tx, records) = gate_1_failure();
    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    for claimed in PolicyDecision::ALL {
        let refusal = promote_refusal(&tx, claimed, &reasons);
        let recomputed = refusal.recomputed().expect("a recomputation");
        assert!(matches!(
            recomputed.verdict().verdict(),
            Verdict::Blocked { failed, undecided }
                if failed == &[GateName::BaseReplay] && undecided.len() == 8
        ));
        if claimed == PolicyDecision::Block {
            assert!(matches!(refusal, DecisionRefusal::NotPromotable { .. }));
        } else {
            assert!(
                matches!(refusal, DecisionRefusal::Disagrees { claimed: c, .. } if c == claimed)
            );
        }
        let text = refusal.to_string();
        assert!(text.contains("blocked; failed: base_replay;"), "{text}");
    }
}

/// `pr22-impl09-neg-03`: the receipt reads the current stores, never an earlier
/// recomputation. With the evidence scope changed so the diff is no longer classifiable
/// (it names one artifact twice), the recomputed classification is the undiffable cause,
/// the decision's standing is undecided, and a claim built against the earlier stores
/// is refused by IMPL-03's handle check, which runs before the policy step. The
/// earlier-eligible-verdict case needs a record with gates 2–11 passed and is the unit
/// tests' (`receipt::policy::tests::pr22_impl09_negative_a_changed_store_*`).
#[test]
fn pr22_impl09_negative_a_changed_store_is_read_at_verification() {
    let (tx, records) = m01();
    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    let earlier = World::of(&[&tx]);
    let earlier_claim = claim(&tx, &earlier, PolicyDecision::Allow);

    let mut changed = World::of(&[&tx]);
    changed.scope = vec![scope_record("ev_twice"), scope_record("ev_twice")];
    let now = decide(&tx, &changed, &reasons);
    assert_eq!(
        now.classification(),
        ReceiptClassification::Absent(DiffGap::Undiffable(UndiffableCause::ImpactScope))
    );
    assert_eq!(now.standing(), DecisionStanding::Undecided);
    assert_eq!(
        verify_for_promotion(
            &earlier_claim,
            &changed,
            &changed,
            &changed,
            &changed,
            &changed,
            &reasons
        )
        .map(|_| ()),
        Err(VerifyRefusal::SemanticDiffUndiffable(
            UndiffableCause::ImpactScope
        ))
    );
}

/// `pr22-impl09-neg-04`: a candidate bound to another intent is a privileged intent
/// revision on the recomputed diff: its classification is bound to `review` or `block`
/// (the diff's decision joined with `review`, since a rebinding is a revision even when
/// its verbs allow it), and never `allow`. The receipt's composition refuses such a
/// candidate outright (its after snapshot is bound elsewhere), so no receipt exists.
#[test]
fn pr22_impl09_negative_a_privileged_intent_revision_classifies_review_or_block() {
    let (tx, _) = m01();
    for (other, expected) in [
        (
            revised("in_register_rebound", str::to_owned),
            PolicyDecision::Review,
        ),
        (
            revised("in_register_no_crash", |d| d.replacen(r#""crash","#, "", 1)),
            PolicyDecision::Block,
        ),
    ] {
        let mut world = World::of(&[&tx]);
        world.rebound = Some(other);
        let DiffOutcome::Computed(computed) = diff::compute(&tx, &world, &world, &world) else {
            panic!("the rebound diff computes");
        };
        let DiffClassification::PrivilegedIntentRevision(revision) = computed.classification()
        else {
            panic!("a rebinding is a privileged intent revision");
        };
        let joined = revision.decision().join(PolicyDecision::Review);
        assert_eq!(joined, expected);
        assert_eq!(
            ReceiptClassification::of(&ReceiptDiff::Derived(computed)),
            ReceiptClassification::Bound(expected)
        );
        assert!(ReceiptSkeleton::compose(&tx, &world, &world, &world, &world).is_err());
    }
}

/// `pr22-impl09-neg-05`: the claimed field is read strictly and before any store: absent
/// is `Missing` (RFC 0032 correction 11), and anything but the schema's three tokens,
/// including RFC 0031's rejected `unknown`, is `Malformed`.
#[test]
fn pr22_impl09_negative_a_missing_or_off_enum_decision_is_refused_by_the_reader() {
    let (tx, _) = m01();
    let world = World::of(&[&tx]);
    let mut absent = claim_fields(&tx, &world, PolicyDecision::Allow);
    absent.remove("policy_decision");
    assert_eq!(
        ClaimedReceipt::parse(&Json::Object(absent).to_canonical_bytes()),
        Err(ClaimRefusal::Missing(ReceiptField::PolicyDecision))
    );
    for forged in [
        Json::String("unknown".to_owned()),
        Json::String("ALLOW".to_owned()),
        Json::String("allow ".to_owned()),
        Json::String(String::new()),
        Json::Integer(0),
        Json::Bool(true),
        Json::Null,
        Json::Array(vec![Json::String("allow".to_owned())]),
    ] {
        let mut fields = claim_fields(&tx, &world, PolicyDecision::Allow);
        fields.insert("policy_decision".to_owned(), forged);
        assert_eq!(
            ClaimedReceipt::parse(&Json::Object(fields).to_canonical_bytes()),
            Err(ClaimRefusal::Malformed(ReceiptField::PolicyDecision))
        );
    }
}

// --- boundary ---------------------------------------------------------------------------

/// `pr22-impl09-bnd-01`: every verdict kind a real version reaches today (inconclusive,
/// blocked) against every claimed decision, at both stages: nothing gets past the
/// policy step. A claim equal to the field is `NotPromotable`; any other is
/// `Disagrees`. The eligible and gate-3-failed kinds are the unit tests'.
#[test]
fn pr22_impl09_boundary_every_reachable_verdict_kind_against_every_claim() {
    for (tx, records, field) in [
        {
            let (tx, records) = m01();
            (tx, records, None)
        },
        {
            let (tx, records) = gate_1_failure();
            (tx, records, Some(PolicyDecision::Block))
        },
    ] {
        let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
        let decision = decide(&tx, &World::of(&[&tx]), &reasons);
        assert_eq!(decision.field(), field);
        assert_ne!(decision.verdict().verdict(), &Verdict::PromoteEligible);
        for claimed in PolicyDecision::ALL {
            let refusal = promote_refusal(&tx, claimed, &reasons);
            assert_eq!(refusal, published_refusal(&tx, claimed, &reasons));
            if Some(claimed) == field {
                assert!(matches!(refusal, DecisionRefusal::NotPromotable { .. }));
            } else {
                assert!(
                    matches!(refusal, DecisionRefusal::Disagrees { claimed: c, .. } if c == claimed)
                );
            }
            assert_eq!(decision.check_claim(claimed), Err(refusal));
        }
    }
}

// --- metamorphic --------------------------------------------------------------------------

/// `pr22-impl09-met-01`: relations that must not move the decision. (1) The reasons
/// store changes only the reasons: with no store, gate 4's reason is the typed
/// `Unavailable`, and the field and the refusal kind are unchanged. (2)
/// set/map insertion order: the evidence scope listed in reverse order gives the same
/// recomputation. (3) Serialization round trip: a pretty-printed encoding of the same
/// claim with its keys inserted in reverse order parses to the same claim and gets the
/// same refusal as the canonical bytes.
#[test]
fn pr22_impl09_metamorphic_reasons_scope_order_and_encoding_do_not_move_the_decision() {
    let (tx, records) = m01();
    let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
    let world = World::of(&[&tx]);
    let base = decide(&tx, &world, &reasons);
    let bare = decide(&tx, &world, &NoReasons);
    assert_eq!(base.field(), bare.field());
    assert_eq!(base.classification(), bare.classification());
    assert_eq!(
        bare.verdict().verdict(),
        &Verdict::Inconclusive {
            undecided: m01_undecided(Resolution::Unavailable)
        }
    );

    let mut forward = World::of(&[&tx]);
    forward.scope = vec![scope_record("ev_a"), scope_record("ev_b")];
    let mut backward = World::of(&[&tx]);
    backward.scope = vec![scope_record("ev_b"), scope_record("ev_a")];
    assert_eq!(
        decide(&tx, &forward, &reasons),
        decide(&tx, &backward, &reasons)
    );

    for claimed in PolicyDecision::ALL {
        let kind = |refusal: &DecisionRefusal| std::mem::discriminant(refusal);
        let reference = promote_refusal(&tx, claimed, &reasons);
        assert_eq!(
            kind(&promote_refusal(&tx, claimed, &NoReasons)),
            kind(&reference)
        );

        let fields = claim_fields(&tx, &world, claimed);
        let mut pretty = String::from("{\n");
        let rendered: Vec<String> = fields
            .iter()
            .rev()
            .map(|(key, value)| {
                format!(
                    "  \"{key}\" : {}",
                    String::from_utf8(value.to_canonical_bytes()).unwrap()
                )
            })
            .collect();
        pretty.push_str(&rendered.join(",\n"));
        pretty.push_str("\n}\n");
        let reencoded = ClaimedReceipt::parse(pretty.as_bytes()).expect("the same claim");
        assert_eq!(reencoded, claim(&tx, &world, claimed));
        assert_eq!(
            verify_for_promotion(&reencoded, &world, &world, &world, &world, &world, &reasons),
            Err(VerifyRefusal::PolicyDecision(reference))
        );
    }
}

// --- differential -----------------------------------------------------------------------------

/// `pr22-impl09-diff-01`: the receipt's check agrees with the IMPL-06 claim checker,
/// `PolicyVerdict::recheck`, on every real version and every claimed decision: a
/// receipt disagrees exactly when the recheck disagrees, and both carry the same
/// recomputed verdict. (The receipt refuses more only for a promote-eligible verdict on
/// an unclassified diff, which no real version reaches today; the unit tests cover it.)
#[test]
fn pr22_impl09_differential_the_receipt_check_agrees_with_the_verdict_recheck() {
    for (tx, records) in [
        m01(),
        gate_1_failure(),
        evaluated(exact(), clean(Vec::new(), 0)),
    ] {
        let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
        let decision = decide(&tx, &World::of(&[&tx]), &reasons);
        for claimed in PolicyDecision::ALL {
            let recheck = PolicyVerdict::recheck(
                &VerdictClaim::new(tx.repair_id().clone(), claimed),
                &tx,
                &reasons,
                None,
            );
            match (decision.check_claim(claimed), recheck) {
                (
                    Err(DecisionRefusal::Disagrees {
                        recomputed: ours, ..
                    }),
                    Err(VerdictClaimRefusal::Disagrees {
                        recomputed: theirs, ..
                    }),
                ) => assert_eq!(ours.verdict(), &*theirs),
                (Err(DecisionRefusal::NotPromotable { recomputed }), Ok(theirs)) => {
                    assert_eq!(recomputed.verdict(), &theirs);
                }
                (ours, theirs) => panic!("{claimed}: {ours:?} against {theirs:?}"),
            }
        }
    }
}

/// `pr22-impl09-diff-02`: the receipt field against `continuum-intent`'s RFC 0031 P5 join
/// (`PolicyDecision::join`), computed independently from the recorded gates: `block` for
/// every failed gate, and no decision while a gate is undecided and none failed.
#[test]
fn pr22_impl09_differential_the_field_is_the_rfc_0031_join() {
    for (tx, records) in [m01(), gate_1_failure()] {
        let reasons = ReplayRecords::<Blake3Hasher>::new(&records);
        let decision = decide(&tx, &World::of(&[&tx]), &reasons);
        let failed = tx
            .gates()
            .iter()
            .any(|gate| gate.status() == GateStatus::Failed);
        let oracle = failed.then(|| PolicyDecision::Allow.join(PolicyDecision::Block));
        assert_eq!(decision.field(), oracle);
    }
}
