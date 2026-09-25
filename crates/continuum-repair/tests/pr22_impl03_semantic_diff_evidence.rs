//! PR-22 / IMPL-03 evidence (bn-19gw): the receipt's `semantic_diff` and `intent_diff`
//! are the diff the system recomputes from the stores for the exact transaction version
//! the receipt names, never the agent's claim.
//!
//! The normative sources are `notes/plan/schemas/promotion-receipt.schema.json` (shape,
//! INV-003), RFC 0032 "The promotion receipt" and correction 10, and RFC 0031 "Wire
//! surface" and "Fail-closed rule".
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is | Checked by |
//! |---|---|---|
//! | `pr22-impl03-ack-after-sync-semantic-diff` | `tests/fixtures/pr22-impl03-ack-after-sync-semantic-diff.json`: `semantic_diff` and `intent_diff` of the replicated register's ack-after-sync repair (M01), both the recomputed `diff_` handle, the `unknowns` entry the diff contributes (its program side is not classified), and `repair_transaction` | this suite (byte equality) and `validate_dossier.py` (each field against the receipt schema's own property) |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite the fixture from the library.
//!
//! # Tests
//!
//! - positive: `pr22_impl03_pos_*` — the derived reference is the recomputed diff; M01
//!   carries its honest unclassified state.
//! - negative: `pr22_impl03_neg_*` — a forged handle, another version's diff, a diff
//!   under the other program layer, a diff with its intent changes emptied, two
//!   different diff fields.
//! - boundary: `pr22_impl03_bnd_*` — an undiffable input is an unknown, never omitted; a
//!   silent store is a refusal; a published receipt survives a later intent revision.
//! - metamorphic: `pr22_impl03_met_*` — store answer order moves no byte.
//! - differential: `pr22_impl03_dif_*` — the receipt's handle is the semantic-diff
//!   assembler's output under the layer tag, built independently.
//!
//! No transaction records an in-profile gate `passed` until PR-20 `evaluate` lands, so
//! a claim that passes the diff check is refused next at the gate record
//! (`GateNotOnRecord(base_replay)`). That refusal is the witness that the diff check
//! passed. The gate-3 check on a record listing gate 3 `passed` is in the crate's unit
//! tests (`receipt::tests::pr22_impl03_*`), which can build such a record.

mod common;

use std::collections::BTreeMap;

use continuum_intent::assurance_policy::AssuranceLevel;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::AcceptancePath;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::diff::{
    self, DiffClaimRefusal, DiffOutcome, DiffScope, ImpactScope, IntentClassification, ScopeAnswer,
    mint_handle,
};
use continuum_repair::handle::{CrashpackId, RepairId, SnapshotId};
use continuum_repair::hypothesis::{ChangeKind, Hypothesis, Proposal};
use continuum_repair::patch::{DeclaredChange, FileEdit};
use continuum_repair::policy::{NoReasons, Verdict};
use continuum_repair::receipt::DecisionRefusal;
use continuum_repair::receipt::{
    CandidateEvidence, CertificateRecord, ClaimRefusal, ClaimedReceipt, ComposeRefusal,
    IntentRegistry, IntentStanding, PackCase, ReceiptDiff, ReceiptField, ReceiptSkeleton,
    RegisteredIntent, SealedSnapshots, SnapshotRole, Store, TransactionStore, UndiffableCause,
    VerificationStage, VerifyRefusal, verify_for_promotion, verify_referenced_diff,
    verify_skeleton,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, GateStatus, RepairTransaction, Resolution,
};
use continuum_semantic_diff::artifact::{self as oracle, DiffId, DiffRequest};
use continuum_semantic_diff::impact::{
    DependencyEdge, DependencyReason, EvidenceId, EvidenceRecord, Independence, ProgramLayer,
    ReuseEdgeClass,
};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

use common::{ack_after_sync_change, base_content, base_id, path};

type Tx = RepairTransaction<Blake3Hasher>;

const CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
/// The PR-20 / IMPL-04 retained diff of the same base and candidate with no evidence.
const RETAINED_PR20_DIFF: &str = include_str!("fixtures/pr20-impl04-ack-after-sync-diff.json");
const DIFF_FIXTURE: &str = "tests/fixtures/pr22-impl03-ack-after-sync-semantic-diff.json";

const CRASH: &str = "crash_pr20_impl01_ack_before_sync";
const ACK_AFTER_SYNC: &str = "move the ack after the storage sync: the replica publishes its \
    reply only once the write is durable, so an acknowledged write survives a crash";
const BASE_INTENT: &str = "in_replicated_register_v1";
const WEAK_INTENT: &str = "in_replicated_register_weak_bound";

// --- the world -------------------------------------------------------------------

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the contract decodes")
}

fn register_contract() -> IntentContract {
    decode(CONTRACT)
}

fn register_intent() -> IntentId {
    IntentId::new(BASE_INTENT).unwrap()
}

/// The register contract with its bound shrunk from three nodes to two, filed under
/// its own handle: an intent weakening.
fn weakened_contract() -> IntentContract {
    let document = CONTRACT
        .trim_end()
        .replacen(r#""nodes":3"#, r#""nodes":2"#, 1)
        .replacen(
            &format!("\"intent_id\":\"{BASE_INTENT}\""),
            &format!("\"intent_id\":\"{WEAK_INTENT}\""),
            1,
        );
    decode(&document)
}

struct Binding;

impl FailureBinding for Binding {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        if failure.as_str() == CRASH {
            Resolution::Found(base_id())
        } else {
            Resolution::Unknown
        }
    }

    fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(register_intent())
    }
}

/// The daemon's stores. Every snapshot this suite seals is bound to the register
/// contract unless `bindings` says otherwise; the evidence scope is answered per
/// transaction version, as of that version.
struct World {
    transactions: Vec<Tx>,
    standing: IntentStanding,
    bindings: BTreeMap<String, IntentId>,
    extra_intents: Vec<IntentContract>,
    /// Evidence per `rt_` version; a version not listed has none.
    scope: BTreeMap<String, Vec<EvidenceRecord>>,
    scope_available: bool,
    /// Answer every evidence list in reverse order.
    reversed: bool,
}

impl World {
    fn of(transactions: &[&Tx]) -> Self {
        Self {
            transactions: transactions.iter().map(|tx| (*tx).clone()).collect(),
            standing: IntentStanding::Accepted,
            bindings: BTreeMap::new(),
            extra_intents: Vec::new(),
            scope: BTreeMap::new(),
            scope_available: true,
            reversed: false,
        }
    }

    fn with_scope(mut self, tx: &Tx, records: Vec<EvidenceRecord>) -> Self {
        self.scope
            .insert(tx.repair_id().as_str().to_owned(), records);
        self
    }
}

impl IntentRegistry for World {
    fn registered(&self, intent: &IntentId) -> Resolution<RegisteredIntent> {
        if intent == &register_intent() {
            return Resolution::Found(RegisteredIntent::new(register_contract(), self.standing));
        }
        self.extra_intents
            .iter()
            .find(|contract| contract.intent_id() == intent)
            .map_or(Resolution::Unknown, |contract| {
                Resolution::Found(RegisteredIntent::new(
                    contract.clone(),
                    IntentStanding::Accepted,
                ))
            })
    }
}

impl SealedSnapshots for World {
    fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(
            self.bindings
                .get(snapshot.as_str())
                .cloned()
                .unwrap_or_else(register_intent),
        )
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
    fn evidence(&self, scope: DiffScope<'_>) -> ScopeAnswer {
        if !self.scope_available {
            return ScopeAnswer::Unavailable;
        }
        let mut records = self
            .scope
            .get(scope.repair.as_str())
            .cloned()
            .unwrap_or_default();
        if self.reversed {
            records.reverse();
        }
        ScopeAnswer::Complete(records)
    }
}

fn applied_with(hypothesis: &str, change: DeclaredChange) -> Tx {
    Tx::begin(
        CrashpackId::new(CRASH).unwrap(),
        GateProfile::PhaseB,
        &Binding,
    )
    .unwrap()
    .apply(
        &Proposal::new(Hypothesis::new(hypothesis), vec![change]),
        &base_content(),
    )
    .expect("the proposal applies")
    .into_parts()
    .0
}

/// The ack-after-sync repair of M01, as PR-20 / IMPL-01 retains it.
fn applied() -> Tx {
    applied_with(ACK_AFTER_SYNC, ack_after_sync_change())
}

fn compose(tx: &Tx, world: &World) -> Result<ReceiptSkeleton, ComposeRefusal> {
    ReceiptSkeleton::compose(tx, world, world, world, world)
}

fn evidence(id: &str) -> EvidenceRecord {
    EvidenceRecord {
        id: EvidenceId::new(id).expect("an evidence id"),
        keyed_to_before_intent: true,
        key_change_reuse_witness: false,
        edges: vec![DependencyEdge {
            reason: DependencyReason::ReliesOnAssumptionFairnessBound,
            class: ReuseEdgeClass::Exact,
            independence: Independence::Unknown,
        }],
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

// --- claims ----------------------------------------------------------------------

fn s(value: &str) -> Json {
    Json::String(value.to_owned())
}

/// The honest claim for `tx` under `world`: the derived fields, the in-profile gates
/// `passed`, and `diff` in both diff fields.
fn claim_with(tx: &Tx, world: &World, diff: &str) -> BTreeMap<String, Json> {
    let skeleton = compose(tx, world).expect("the claim's transaction composes");
    let Json::Object(mut fields) = skeleton.fields_json() else {
        unreachable!()
    };
    let gates = GateName::ALL
        .into_iter()
        .map(|gate| {
            let status = if tx.gate_profile().enforces(gate) {
                "passed"
            } else {
                "not_yet_enforced"
            };
            Json::Object(BTreeMap::from([
                ("gate".to_owned(), s(gate.token())),
                ("status".to_owned(), s(status)),
            ]))
        })
        .collect();
    fields.insert("gates".to_owned(), Json::Array(gates));
    fields.insert("semantic_diff".to_owned(), s(diff));
    fields.insert("intent_diff".to_owned(), s(diff));
    fields.insert("policy_decision".to_owned(), s("allow"));
    fields
}

fn parse(fields: BTreeMap<String, Json>) -> Result<ClaimedReceipt, ClaimRefusal> {
    ClaimedReceipt::parse(&Json::Object(fields).to_canonical_bytes())
}

/// Verify at both stages; the two must agree on everything this suite checks.
fn verify(fields: BTreeMap<String, Json>, world: &World) -> Result<(), VerifyRefusal> {
    let claim = parse(fields)?;
    let published =
        verify_skeleton(&claim, world, world, world, world, world, &NoReasons).map(|_| ());
    let promotion =
        verify_for_promotion(&claim, world, world, world, world, world, &NoReasons).map(|_| ());
    assert_eq!(published, promotion, "both stages decide the diff alike");
    published
}

/// The refusal a claim gets once its diff check passes, on a pending record: since
/// IMPL-09 (bn-1plr) the policy step's, whose recomputed verdict is inconclusive (before
/// IMPL-09, `GateNotOnRecord(BaseReplay)` at the record comparison that follows it).
fn assert_past_the_diff(result: Result<(), VerifyRefusal>) {
    assert!(
        matches!(
            &result,
            Err(VerifyRefusal::PolicyDecision(DecisionRefusal::Disagrees { recomputed, .. }))
                if matches!(recomputed.verdict().verdict(), Verdict::Inconclusive { .. })
        ),
        "the claim passes the diff check and stops at the policy step: {result:?}"
    );
}

fn handle_of(skeleton: &ReceiptSkeleton) -> String {
    skeleton
        .semantic_diff()
        .handle()
        .expect("the diff is computed")
        .to_owned()
}

/// `diff_` plus the digest of `layer`'s tag, a NUL and `artifact`'s canonical bytes
/// without `diff_id`, computed here from the published preimage rule.
fn remint(artifact: &Json, layer: &str) -> String {
    let Json::Object(mut fields) = artifact.clone() else {
        panic!("an object")
    };
    fields
        .remove("diff_id")
        .expect("the artifact carries diff_id");
    let mut tagged = format!("semantic-diff/program-layer/{layer}/v1\0").into_bytes();
    tagged.extend_from_slice(&Json::Object(fields).to_canonical_bytes());
    format!("diff_{}", Blake3Hasher::hash(&tagged).to_token())
}

fn with_diff_id(artifact: &Json, handle: &str) -> Json {
    let Json::Object(mut fields) = artifact.clone() else {
        panic!("an object")
    };
    fields.insert("diff_id".to_owned(), s(handle));
    Json::Object(fields)
}

// --- positive --------------------------------------------------------------------

/// The skeleton's diff reference is the diff `diff::compute` derives from the stores
/// for this version, in both fields; the honest claim passes the diff check, and the
/// recomputed artifact bytes verify by reference.
#[test]
fn pr22_impl03_pos_01_the_derived_diff_reference_matches_the_recompute() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let skeleton = compose(&tx, &world).unwrap();
    let DiffOutcome::Computed(recomputed) = diff::compute(&tx, &world, &world, &world) else {
        panic!("the diff is computed");
    };
    let handle = handle_of(&skeleton);
    assert_eq!(handle, recomputed.handle());
    assert_eq!(skeleton.semantic_diff().diff(), Some(&*recomputed));
    assert_eq!(recomputed.repair(), tx.repair_id());

    let Json::Object(fields) = skeleton.fields_json() else {
        unreachable!()
    };
    assert_eq!(fields.get("semantic_diff"), Some(&s(&handle)));
    assert_eq!(fields.get("intent_diff"), Some(&s(&handle)));

    assert_past_the_diff(verify(claim_with(&tx, &world, &handle), &world));
    let claim = parse(claim_with(&tx, &world, &handle)).unwrap();
    assert_eq!(claim.semantic_diff(), handle);
    let verified = verify_referenced_diff(
        &claim,
        &recomputed.to_artifact_bytes(),
        VerificationStage::Promotion,
        &world,
        &world,
        &world,
        &world,
    )
    .expect("the recomputed bytes verify by reference");
    assert_eq!(verified.handle(), handle);
    // The stage is borne out by the record: gate 12 is `pending`, so not published.
    assert_eq!(
        verify_referenced_diff(
            &claim,
            &recomputed.to_artifact_bytes(),
            VerificationStage::Published,
            &world,
            &world,
            &world,
            &world,
        ),
        Err(VerifyRefusal::GateNotOnRecord(GateName::ReceiptGeneration))
    );
}

/// `pr22-impl03-ack-after-sync-semantic-diff`: M01's ack-after-sync receipt carries
/// its honest state. The intent is preserved (a repair: no intent change, `allow`), and
/// the `rust` change is not classified on the program side, so the diff is under
/// `Unclassified`, gate 3 as the diff decides it is `inconclusive` (`Unsupported`), and
/// `unknowns` says so. No semantic preservation is claimed. The diff is byte-identical to
/// PR-20 / IMPL-04's retained artifact for the same base and candidate.
#[test]
fn pr22_impl03_pos_02_m01_carries_the_honest_unclassified_state() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let skeleton = compose(&tx, &world).unwrap();
    let receipt_diff = skeleton.semantic_diff();
    let diff = receipt_diff.diff().expect("computed");
    assert_eq!(diff.classification(), &IntentClassification::Repair);
    assert!(diff.artifact().intent_changes().is_empty());
    assert_eq!(
        receipt_diff.program_layer(),
        Some(ProgramLayer::Unclassified)
    );
    assert_eq!(receipt_diff.gate_status(), GateStatus::Inconclusive);
    assert_eq!(
        receipt_diff.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert!(
        skeleton
            .unknowns()
            .tokens()
            .contains(&"semantic_diff_unclassified:program_layer".to_owned())
    );
    assert_eq!(
        String::from_utf8(diff.to_artifact_bytes()).unwrap(),
        RETAINED_PR20_DIFF.trim_end()
    );
    fixture_matches(DIFF_FIXTURE, &skeleton.diff_fields().to_canonical_bytes());
}

// --- negative --------------------------------------------------------------------

/// A handle nothing computed, however well formed, is refused.
#[test]
fn pr22_impl03_neg_01_a_forged_diff_handle_is_refused() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let honest = handle_of(&compose(&tx, &world).unwrap());
    let mut flipped = honest.clone().into_bytes();
    let last = flipped.len() - 1;
    flipped[last] = if flipped[last] == b'0' { b'1' } else { b'0' };
    for forged in [
        "diff_0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        "diff_agent_says_no_change".to_owned(),
        String::from_utf8(flipped).unwrap(),
    ] {
        assert_eq!(
            verify(claim_with(&tx, &world, &forged), &world),
            Err(VerifyRefusal::SemanticDiff(
                DiffClaimRefusal::HandleMismatch
            )),
            "{forged}"
        );
    }
}

/// The diff is the named version's: two transaction versions (two `rt_` identities,
/// here from two `begin`→`apply` lineages; one lineage's `ready` and `promoted` versions
/// need PR-20 promote) with the same candidate whose
/// evidence scopes differ as of each version have different diffs, and each receipt
/// accepts only its own. Another repair's diff is refused too.
#[test]
fn pr22_impl03_neg_02_another_versions_diff_is_refused() {
    let first = applied();
    let second = applied_with("the same edit, another hypothesis", ack_after_sync_change());
    assert_ne!(first.repair_id(), second.repair_id());
    assert_eq!(first.candidate_snapshot(), second.candidate_snapshot());
    let other_repair = applied_with(
        ACK_AFTER_SYNC,
        DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path("src/other_repair.rs"),
                FileEdit::Create {
                    content: b"// some other repair\n".to_vec(),
                },
            )],
        )
        .unwrap(),
    );
    let world = World::of(&[&first, &second, &other_repair])
        .with_scope(&first, vec![evidence("ev_register_agreement_run")]);

    let first_diff = handle_of(&compose(&first, &world).unwrap());
    let second_diff = handle_of(&compose(&second, &world).unwrap());
    let other_diff = handle_of(&compose(&other_repair, &world).unwrap());
    assert_ne!(
        first_diff, second_diff,
        "the evidence scope is the version's"
    );
    assert_ne!(second_diff, other_diff);

    assert_past_the_diff(verify(claim_with(&first, &world, &first_diff), &world));
    assert_past_the_diff(verify(claim_with(&second, &world, &second_diff), &world));
    for (tx, foreign) in [
        (&second, &first_diff),
        (&first, &second_diff),
        (&second, &other_diff),
    ] {
        assert_eq!(
            verify(claim_with(tx, &world, foreign), &world),
            Err(VerifyRefusal::SemanticDiff(
                DiffClaimRefusal::HandleMismatch
            ))
        );
    }
}

/// A receipt never presents a diff under another program layer than the recompute:
/// the same artifact minted under `classified` is refused `LayerMismatch`, as a handle
/// and as presented bytes.
#[test]
fn pr22_impl03_neg_03_a_diff_under_the_other_program_layer_is_refused() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let skeleton = compose(&tx, &world).unwrap();
    let artifact = skeleton
        .semantic_diff()
        .diff()
        .unwrap()
        .artifact()
        .artifact_json();
    assert_eq!(remint(&artifact, "unclassified"), handle_of(&skeleton));
    let classified = remint(&artifact, "classified");
    assert_ne!(classified, handle_of(&skeleton));
    let layer = VerifyRefusal::SemanticDiff(DiffClaimRefusal::LayerMismatch {
        computed: ProgramLayer::Unclassified,
    });
    assert_eq!(
        verify(claim_with(&tx, &world, &classified), &world),
        Err(layer.clone())
    );

    let honest = parse(claim_with(&tx, &world, &handle_of(&skeleton))).unwrap();
    let classified_bytes = with_diff_id(&artifact, &classified).to_canonical_bytes();
    assert_eq!(
        verify_referenced_diff(
            &honest,
            &classified_bytes,
            VerificationStage::Promotion,
            &world,
            &world,
            &world,
            &world
        ),
        Err(layer)
    );
}

/// A weakening never gets a receipt, and its diff with the intent changes emptied and
/// the verdict flipped to `allow` (it still names the weakened `after_intent`, so it
/// differs from the honest diff in more than those two fields) —
/// re-minted to a self-consistent handle, or presented under the honest receipt's
/// handle — is refused.
#[test]
fn pr22_impl03_neg_04_a_claimed_diff_with_emptied_intent_changes_is_refused() {
    let honest_tx = applied();
    let weak_tx = applied_with("shrink the bound instead", ack_after_sync_change());
    let mut world = World::of(&[&honest_tx, &weak_tx]);

    // The weakening: a candidate bound to the shrunk-bound contract.
    let mut weak_world = World::of(&[&weak_tx]);
    weak_world.extra_intents.push(weakened_contract());
    weak_world.bindings.insert(
        weak_tx.candidate_snapshot().unwrap().as_str().to_owned(),
        weakened_contract().intent_id().clone(),
    );
    assert_eq!(
        compose(&weak_tx, &weak_world),
        Err(ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::After))
    );
    let DiffOutcome::Computed(weak) =
        diff::compute(&weak_tx, &weak_world, &weak_world, &weak_world)
    else {
        panic!("the weakening's diff is computed");
    };
    let Json::Array(changes) = weak
        .artifact()
        .artifact_json()
        .as_object()
        .unwrap()
        .get("intent_changes")
        .cloned()
        .unwrap()
    else {
        panic!("intent_changes is an array");
    };
    assert!(
        !changes.is_empty(),
        "not vacuous: the weakening has intent changes"
    );

    // The forgery: intent changes emptied, the verdict flipped to allow.
    let Json::Object(mut forged) = weak.artifact().artifact_json() else {
        unreachable!()
    };
    forged.insert("intent_changes".to_owned(), Json::Array(Vec::new()));
    forged.insert(
        "policy".to_owned(),
        Json::Object(BTreeMap::from([
            ("decision".to_owned(), s("allow")),
            ("reasons".to_owned(), Json::Array(Vec::new())),
        ])),
    );
    let forged = Json::Object(forged);
    let forged_handle = remint(&forged, "unclassified");
    let forged_bytes = with_diff_id(&forged, &forged_handle).to_canonical_bytes();

    // Named by a receipt: refused as a handle.
    world.transactions = vec![honest_tx.clone()];
    assert_eq!(
        verify(claim_with(&honest_tx, &world, &forged_handle), &world),
        Err(VerifyRefusal::SemanticDiff(
            DiffClaimRefusal::HandleMismatch
        ))
    );
    // Presented as the content of the honest receipt's reference: refused as bytes.
    let honest_handle = handle_of(&compose(&honest_tx, &world).unwrap());
    let honest = parse(claim_with(&honest_tx, &world, &honest_handle)).unwrap();
    let under_honest_id = with_diff_id(&forged, &honest_handle).to_canonical_bytes();
    for bytes in [&forged_bytes, &under_honest_id] {
        assert_eq!(
            verify_referenced_diff(
                &honest,
                bytes,
                VerificationStage::Promotion,
                &world,
                &world,
                &world,
                &world
            ),
            Err(VerifyRefusal::SemanticDiff(DiffClaimRefusal::Disagrees))
        );
    }
    let forged_claim = parse(claim_with(&honest_tx, &world, &forged_handle)).unwrap();
    assert_eq!(
        verify_referenced_diff(
            &forged_claim,
            &forged_bytes,
            VerificationStage::Promotion,
            &world,
            &world,
            &world,
            &world
        ),
        Err(VerifyRefusal::SemanticDiff(
            DiffClaimRefusal::HandleMismatch
        ))
    );
}

/// RFC 0032 correction 10: the two fields name one artifact. Two different handles,
/// a handle off the `diff_` pattern and an absent field are refused before any store
/// is read.
#[test]
fn pr22_impl03_neg_05_the_two_diff_fields_must_name_one_well_formed_handle() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let honest = handle_of(&compose(&tx, &world).unwrap());

    let mut split = claim_with(&tx, &world, &honest);
    split.insert("intent_diff".to_owned(), s("diff_some_other_artifact"));
    assert_eq!(parse(split), Err(ClaimRefusal::DiffFieldsDisagree));

    for field in [ReceiptField::SemanticDiff, ReceiptField::IntentDiff] {
        let mut off_pattern = claim_with(&tx, &world, &honest);
        off_pattern.insert(field.property().to_owned(), s("rt_not_a_diff"));
        assert_eq!(parse(off_pattern), Err(ClaimRefusal::Malformed(field)));
        let mut absent = claim_with(&tx, &world, &honest);
        absent.remove(field.property());
        assert_eq!(parse(absent), Err(ClaimRefusal::Missing(field)));
    }
    // The refusal names the fields, never a value.
    assert!(
        !ClaimRefusal::DiffFieldsDisagree
            .to_string()
            .contains("diff_some")
    );
}

// --- boundary --------------------------------------------------------------------

/// An undiffable input — here an evidence scope that names one artifact twice, which
/// the classifier refuses — is carried as an unknown, never omitted: the skeleton
/// composes, renders no handle, and lists `semantic_diff_undiffable`; every claimed
/// handle is refused typed.
#[test]
fn pr22_impl03_bnd_01_an_undiffable_diff_is_an_unknown_never_omitted() {
    let tx = applied();
    let honest_handle = handle_of(&compose(&tx, &World::of(&[&tx])).unwrap());
    let world = World::of(&[&tx]).with_scope(
        &tx,
        vec![evidence("ev_register_twice"), evidence("ev_register_twice")],
    );
    let skeleton = compose(&tx, &world).expect("an undiffable diff composes, as an unknown");
    assert_eq!(
        skeleton.semantic_diff(),
        &ReceiptDiff::Undiffable(UndiffableCause::ImpactScope)
    );
    assert_eq!(skeleton.semantic_diff().handle(), None);
    assert_eq!(
        skeleton.semantic_diff().gate_status(),
        GateStatus::Inconclusive
    );
    assert_eq!(
        skeleton.semantic_diff().inconclusive_reason(),
        Some(InconclusiveReason::EngineError)
    );
    let entry = "semantic_diff_undiffable:impact_scope_malformed".to_owned();
    assert!(skeleton.unknowns().tokens().contains(&entry));
    let Json::Object(fields) = skeleton.fields_json() else {
        unreachable!()
    };
    assert!(!fields.contains_key("semantic_diff") && !fields.contains_key("intent_diff"));
    assert_eq!(
        skeleton.diff_fields().as_object().unwrap().get("unknowns"),
        Some(&Json::Array(vec![Json::String(entry)]))
    );

    let undiffable = VerifyRefusal::SemanticDiffUndiffable(UndiffableCause::ImpactScope);
    assert_eq!(
        verify(claim_with(&tx, &world, &honest_handle), &world),
        Err(undiffable.clone())
    );
    let claim = parse(claim_with(&tx, &world, &honest_handle)).unwrap();
    assert_eq!(
        verify_referenced_diff(
            &claim,
            b"{}",
            VerificationStage::Promotion,
            &world,
            &world,
            &world,
            &world
        ),
        Err(undiffable)
    );
}

/// A silent evidence store is not a fact about the repair: it composes nothing and
/// verifies nothing, and is never an unknown baked into a receipt.
#[test]
fn pr22_impl03_bnd_02_a_silent_evidence_store_is_a_refusal_not_an_unknown() {
    let tx = applied();
    let honest_handle = handle_of(&compose(&tx, &World::of(&[&tx])).unwrap());
    let claim = claim_with(&tx, &World::of(&[&tx]), &honest_handle);
    let mut world = World::of(&[&tx]);
    world.scope_available = false;
    let silent = ComposeRefusal::StoreUnavailable(Store::ImpactScope);
    assert_eq!(compose(&tx, &world), Err(silent.clone()));
    assert_eq!(
        verify(claim.clone(), &world),
        Err(VerifyRefusal::Compose(silent.clone()))
    );
    assert_eq!(
        verify_referenced_diff(
            &parse(claim).unwrap(),
            b"{}",
            VerificationStage::Promotion,
            &world,
            &world,
            &world,
            &world
        ),
        Err(VerifyRefusal::Compose(silent))
    );
}

/// A published receipt stays verifiable after a later intent revision: the skeleton
/// check re-derives the diff against the superseded base and passes it, stopping at the
/// record. A caller cannot reach that admission by naming a stage: presented bytes are
/// checked at `Published` only for a record that shows gate 12 `passed`, and at
/// promotion a superseded base composes nothing. The success path over a promoted
/// record is `receipt::tests::pr22_impl03_boundary_a_promoted_versions_diff_re_derives_after_a_revision`.
#[test]
fn pr22_impl03_bnd_03_a_published_receipts_diff_survives_a_later_intent_revision() {
    let tx = applied();
    let accepted = World::of(&[&tx]);
    let skeleton = compose(&tx, &accepted).unwrap();
    let handle = handle_of(&skeleton);
    let bytes = skeleton.semantic_diff().diff().unwrap().to_artifact_bytes();
    let claim = parse(claim_with(&tx, &accepted, &handle)).unwrap();

    let mut superseded = World::of(&[&tx]);
    superseded.standing = IntentStanding::Superseded;
    assert_past_the_diff(
        verify_skeleton(
            &claim,
            &superseded,
            &superseded,
            &superseded,
            &superseded,
            &superseded,
            &NoReasons,
        )
        .map(|_| ()),
    );
    assert_eq!(
        verify_referenced_diff(
            &claim,
            &bytes,
            VerificationStage::Published,
            &superseded,
            &superseded,
            &superseded,
            &superseded,
        ),
        Err(VerifyRefusal::GateNotOnRecord(GateName::ReceiptGeneration))
    );
    assert_eq!(
        verify_referenced_diff(
            &claim,
            &bytes,
            VerificationStage::Promotion,
            &superseded,
            &superseded,
            &superseded,
            &superseded,
        ),
        Err(VerifyRefusal::Compose(ComposeRefusal::IntentSuperseded))
    );
}

/// A store that answers the diff's second read differently from the skeleton's first
/// is refused, not composed: here the candidate's binding moves to a weakened contract
/// between the two reads.
#[test]
fn pr22_impl03_bnd_04_a_store_that_changes_its_answer_between_reads_is_refused() {
    struct Flipping<'a> {
        world: &'a World,
        candidate: SnapshotId,
        reads: std::cell::Cell<u32>,
    }
    impl SealedSnapshots for Flipping<'_> {
        fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
            if snapshot == &self.candidate {
                self.reads.set(self.reads.get() + 1);
                if self.reads.get() > 1 {
                    return Resolution::Found(weakened_contract().intent_id().clone());
                }
            }
            self.world.sealed_binding(snapshot)
        }
    }
    let tx = applied();
    let mut world = World::of(&[&tx]);
    world.extra_intents.push(weakened_contract());
    let flipping = Flipping {
        world: &world,
        candidate: tx.candidate_snapshot().unwrap().clone(),
        reads: std::cell::Cell::new(0),
    };
    assert_eq!(
        ReceiptSkeleton::compose(&tx, &world, &flipping, &world, &world),
        Err(ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::After))
    );
    assert_eq!(flipping.reads.get(), 2, "both reads happened");
}

// --- metamorphic -----------------------------------------------------------------

/// Relation "set/map insertion order": the evidence store answering the same scope in
/// another order moves no byte of the receipt's diff, handle, or unknowns.
#[test]
fn pr22_impl03_met_01_store_answer_order_moves_no_byte() {
    let tx = applied();
    let records = vec![
        evidence("ev_register_agreement_run"),
        evidence("ev_register_bound_conditional"),
        evidence("proof_register_model"),
    ];
    let forward = World::of(&[&tx]).with_scope(&tx, records.clone());
    let mut backward = World::of(&[&tx]).with_scope(&tx, records);
    backward.reversed = true;
    let one = compose(&tx, &forward).unwrap();
    let two = compose(&tx, &backward).unwrap();
    assert_eq!(one, two);
    assert_eq!(
        one.fields_json().to_canonical_bytes(),
        two.fields_json().to_canonical_bytes()
    );
    // Not vacuous: the evidence reached the impact set, none of it reused.
    let impact = one.semantic_diff().diff().unwrap().artifact().impact();
    assert_eq!(impact.reused().count(), 0);
    assert_eq!(impact.unknown().count() + impact.invalidated().count(), 3);
    // And a claim built on one ordering verifies against the other.
    assert_past_the_diff(verify(
        claim_with(&tx, &forward, &handle_of(&one)),
        &backward,
    ));
}

// --- differential ----------------------------------------------------------------

/// Oracle `continuum-semantic-diff`, subject `continuum-repair`: the receipt's handle
/// and bytes are the assembler's output for the stored contracts and snapshots under
/// the unclassified layer, minted by the published preimage rule — built here without
/// the repair crate's diff module.
#[test]
fn pr22_impl03_dif_01_the_receipt_diff_is_the_assemblers_output() {
    let tx = applied();
    let world = World::of(&[&tx]);
    let skeleton = compose(&tx, &world).unwrap();
    let contract = register_contract();
    let mut request = DiffRequest {
        diff_id: DiffId::new("diff_unminted").unwrap(),
        before_snapshot: oracle::SnapshotId::new(tx.base_snapshot().as_str()).unwrap(),
        after_snapshot: oracle::SnapshotId::new(tx.candidate_snapshot().unwrap().as_str()).unwrap(),
        before_intent: register_intent(),
        after_intent: register_intent(),
        before: &contract,
        after: &contract,
        requested_assurance: contract
            .assurance()
            .minimum()
            .max(AssuranceLevel::Validated),
        path: AcceptancePath::AgentAccept,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    };
    let draft = oracle::assemble_under(&request, ProgramLayer::Unclassified).unwrap();
    let handle = remint(&draft.artifact_json(), "unclassified");
    assert_eq!(handle, handle_of(&skeleton));
    assert_eq!(
        handle,
        mint_handle::<Blake3Hasher>(ProgramLayer::Unclassified, &{
            let Json::Object(mut fields) = draft.artifact_json() else {
                unreachable!()
            };
            fields.remove("diff_id");
            Json::Object(fields).to_canonical_bytes()
        })
    );
    request.diff_id = DiffId::new(&handle).unwrap();
    let artifact = oracle::assemble_under(&request, ProgramLayer::Unclassified).unwrap();
    assert_eq!(
        artifact.to_artifact_bytes(),
        skeleton.semantic_diff().diff().unwrap().to_artifact_bytes()
    );
}
