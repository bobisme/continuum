//! PR-22 evidence: the promotion-receipt skeleton — intent identity (IMPL-01, bn-1ebx),
//! before/after snapshots (IMPL-02, bn-cps6), `gate_profile` and the `NotYetEnforced`
//! list (IMPL-07 and IMPL-08, bn-1cec).
//!
//! The normative sources are `notes/plan/schemas/promotion-receipt.schema.json` (shape,
//! INV-003) and RFC 0032, "The promotion receipt" and "Gate profiles".
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is | Checked by |
//! |---|---|---|
//! | `pr22-impl01-ack-after-sync-intent` | `tests/fixtures/pr22-impl01-ack-after-sync-intent.json`: the `intent` field of the replicated register's ack-after-sync repair, with its `repair_transaction` | this suite (byte equality) and `validate_dossier.py` (each field against the receipt schema's own property) |
//! | `pr22-impl02-ack-after-sync-snapshots` | `tests/fixtures/pr22-impl02-ack-after-sync-snapshots.json`: `base_snapshot` and `result_snapshot` | as above |
//! | `pr22-impl07-ack-after-sync-gate-profile` | `tests/fixtures/pr22-impl07-ack-after-sync-gate-profile.json`: `gate_profile` | as above |
//! | `pr22-impl07-phase-b-not-yet-enforced` | `tests/fixtures/pr22-impl07-phase-b-not-yet-enforced.json`: the `not_yet_enforced` entries of `gates` under `phase-b` | as above, each entry against the schema's `gates.items` |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite the fixtures from the library.
//!
//! # Tests
//!
//! - positive: `pr22_impl0N_positive_*` — the ack-after-sync repair's skeleton, and a
//!   claim matching it verifies its owned fields and nothing more.
//! - negative: `pr22_impl0N_negative_*` — a client-supplied receipt, a swapped intent,
//!   a foreign snapshot, an unenforced gate claimed passed, a profile other than the
//!   transaction's; unregistered, unprotected, unsealed, unavailable.
//! - boundary: `pr22_boundary_*` — canonical bytes and identity, the size bound, and the
//!   schema's closed sets.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::diff::{DiffScope, ImpactScope, ScopeAnswer};
use continuum_repair::handle::{CrashpackId, RepairId, SnapshotId};
use continuum_repair::hypothesis::{ChangeKind, Hypothesis, Proposal};
use continuum_repair::patch::{DeclaredChange, FileEdit};
use continuum_repair::receipt::{
    CandidateEvidence, CertificateRecord, ClaimRefusal, ClaimedGateStatus, ClaimedReceipt,
    ComposeRefusal, IntentRegistry, IntentStanding, MAX_CLAIMED_RECEIPT_BYTES, NotYetEnforced,
    PackCase, RECEIPT_PROPERTIES, RECEIPT_SCHEMA_ID, ReceiptField, ReceiptSeam, ReceiptSkeleton,
    RegisteredIntent, SealedSnapshots, SnapshotRole, Store, TransactionStore, UnenforcedGate,
    VerifyRefusal, verify_for_promotion, verify_skeleton,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, RepairTransaction, Resolution,
};
use continuum_value::identity::Blake3Hasher;

use common::{ack_after_sync_change, base_content, base_id, path};

type Tx = RepairTransaction<Blake3Hasher>;

const SCHEMA: &str = include_str!("../../../notes/plan/schemas/promotion-receipt.schema.json");
const EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/promotion-receipt.example.json");
const REGISTER_CONTRACT: &[u8] =
    include_bytes!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const PR20_APPLIED: &[u8] = include_bytes!("fixtures/pr20-impl01-ack-after-sync-applied.json");

const INTENT_FIXTURE: &str = "tests/fixtures/pr22-impl01-ack-after-sync-intent.json";
const SNAPSHOTS_FIXTURE: &str = "tests/fixtures/pr22-impl02-ack-after-sync-snapshots.json";
const PROFILE_FIXTURE: &str = "tests/fixtures/pr22-impl07-ack-after-sync-gate-profile.json";
const NYE_FIXTURE: &str = "tests/fixtures/pr22-impl07-phase-b-not-yet-enforced.json";

// The PR-20 / IMPL-01 transaction, reproduced exactly: the receipt cites that version.
const CRASH: &str = "crash_pr20_impl01_ack_before_sync";
const ACK_AFTER_SYNC: &str = "move the ack after the storage sync: the replica publishes its \
    reply only once the write is durable, so an acknowledged write survives a crash";

/// A snapshot sealed under another contract.
const FOREIGN: &str = "ws_pr22_foreign_workspace";
const FOREIGN_INTENT: &str = "in_some_other_contract";

// --- the world: stores the daemon would hold ---------------------------------------

fn register_contract() -> IntentContract {
    IntentContract::decode(REGISTER_CONTRACT).expect("the replicated register contract decodes")
}

fn register_intent() -> IntentId {
    register_contract().intent_id().clone()
}

/// Which repair a transaction applies. Both are sealed through `SealedCandidate::seal`
/// on the register base (bn-195b): the candidate's `ws_` identity is derived from its
/// content, never chosen.
#[derive(Clone, Copy)]
enum Repair {
    /// The ack-after-sync repair of PR-20 / IMPL-01.
    AckAfterSync,
    /// Some other repair of the same base: a second sealed snapshot of the register.
    Other,
}

/// The base snapshot's handle.
fn base() -> String {
    base_id().as_str().to_owned()
}

/// The ack-after-sync candidate's handle.
fn candidate() -> String {
    applied().candidate_snapshot().unwrap().as_str().to_owned()
}

/// The other repair's candidate handle.
fn other_candidate() -> String {
    applied_to(GateProfile::PhaseB, Repair::Other)
        .candidate_snapshot()
        .unwrap()
        .as_str()
        .to_owned()
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

    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if snapshot == &base_id() {
            Resolution::Found(register_intent())
        } else {
            Resolution::Unknown
        }
    }
}

/// The daemon's view: the registry, the sealed snapshots, the transactions.
struct World {
    intents: BTreeMap<String, Resolution<RegisteredIntent>>,
    snapshots: BTreeMap<String, Resolution<IntentId>>,
    transactions: BTreeMap<String, Resolution<Tx>>,
}

impl World {
    fn register(transactions: &[&Tx]) -> Self {
        let mut world = Self {
            intents: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            transactions: BTreeMap::new(),
        };
        world.intents.insert(
            register_intent().as_str().to_owned(),
            Resolution::Found(RegisteredIntent::new(
                register_contract(),
                IntentStanding::Accepted,
            )),
        );
        for handle in [base(), candidate(), other_candidate()] {
            world
                .snapshots
                .insert(handle, Resolution::Found(register_intent()));
        }
        world.snapshots.insert(
            FOREIGN.to_owned(),
            Resolution::Found(IntentId::new(FOREIGN_INTENT).unwrap()),
        );
        for tx in transactions {
            world.transactions.insert(
                tx.repair_id().as_str().to_owned(),
                Resolution::Found((*tx).clone()),
            );
        }
        world
    }

    fn with_standing(mut self, standing: IntentStanding) -> Self {
        self.intents.insert(
            register_intent().as_str().to_owned(),
            Resolution::Found(RegisteredIntent::new(register_contract(), standing)),
        );
        self
    }
}

impl IntentRegistry for World {
    fn registered(&self, intent: &IntentId) -> Resolution<RegisteredIntent> {
        self.intents
            .get(intent.as_str())
            .cloned()
            .unwrap_or(Resolution::Unknown)
    }
}

impl SealedSnapshots for World {
    fn sealed_binding(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        self.snapshots
            .get(snapshot.as_str())
            .cloned()
            .unwrap_or(Resolution::Unknown)
    }
}

/// The skeleton suite's evidence: the daemon holds no certificate and no pack case for
/// any candidate. The IMPL-05 and IMPL-06 suite varies it.
impl CandidateEvidence for World {
    fn certificates(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<CertificateRecord>> {
        Resolution::Found(Vec::new())
    }

    fn unsupported_pack_cases(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<PackCase>> {
        Resolution::Found(Vec::new())
    }
}

/// The skeleton suite's diff scope: the daemon holds no evidence naming either side.
/// The IMPL-03 suite varies it.
impl ImpactScope for World {
    fn evidence(&self, _: DiffScope<'_>) -> ScopeAnswer {
        ScopeAnswer::Complete(Vec::new())
    }
}

impl TransactionStore<Blake3Hasher> for World {
    fn transaction(&self, repair: &RepairId) -> Resolution<Tx> {
        self.transactions
            .get(repair.as_str())
            .cloned()
            .unwrap_or(Resolution::Unknown)
    }
}

fn draft(profile: GateProfile) -> Tx {
    Tx::begin(CrashpackId::new(CRASH).unwrap(), profile, &Binding)
        .expect("the ack-before-sync crashpack resolves")
}

fn applied_to(profile: GateProfile, repair: Repair) -> Tx {
    let change = match repair {
        Repair::AckAfterSync => ack_after_sync_change(),
        Repair::Other => DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path("src/other_repair.rs"),
                FileEdit::Create {
                    content: b"// some other repair\n".to_vec(),
                },
            )],
        )
        .expect("a well-formed change"),
    };
    draft(profile)
        .apply(
            &Proposal::new(Hypothesis::new(ACK_AFTER_SYNC), vec![change]),
            &base_content(),
        )
        .expect("a rust-only proposal applies")
        .into_parts()
        .0
}

/// The ack-after-sync repair, under the Phase B profile.
fn applied() -> Tx {
    applied_to(GateProfile::PhaseB, Repair::AckAfterSync)
}

fn skeleton() -> ReceiptSkeleton {
    let tx = applied();
    let world = World::register(&[&tx]);
    ReceiptSkeleton::compose(&tx, &world, &world, &world, &world)
        .expect("the ack-after-sync skeleton composes")
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

// --- claims: what a client could send ----------------------------------------------

fn s(value: &str) -> Json {
    Json::String(value.to_owned())
}

/// A client's receipt claim for `tx` built the honest way: the transaction's own fields,
/// in-profile gates `passed`, the rest `not_yet_enforced`. Tests then tamper with it.
fn claim_for(tx: &Tx) -> BTreeMap<String, Json> {
    let profile = tx.gate_profile();
    let gates = GateName::ALL
        .into_iter()
        .map(|gate| {
            let status = if profile.enforces(gate) {
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
    BTreeMap::from([
        ("repair_transaction".to_owned(), s(tx.repair_id().as_str())),
        ("intent".to_owned(), s(tx.base_intent().as_str())),
        ("base_snapshot".to_owned(), s(tx.base_snapshot().as_str())),
        (
            "result_snapshot".to_owned(),
            s(tx.candidate_snapshot().unwrap().as_str()),
        ),
        ("gate_profile".to_owned(), s(profile.token())),
        ("gates".to_owned(), Json::Array(gates)),
        ("semantic_diff".to_owned(), s(&diff_for(tx))),
        ("intent_diff".to_owned(), s(&diff_for(tx))),
        ("unknowns".to_owned(), unknowns_for(tx)),
    ])
}

/// The recomputed diff handle of `tx` under the skeleton suite's stores.
fn diff_for(tx: &Tx) -> String {
    let world = World::register(&[tx]);
    ReceiptSkeleton::compose(tx, &world, &world, &world, &world)
        .expect("the claim's transaction composes")
        .semantic_diff()
        .handle()
        .expect("the diff is computed")
        .to_owned()
}

/// The derived `unknowns` of `tx` under the skeleton suite's evidence.
fn unknowns_for(tx: &Tx) -> Json {
    let world = World::register(&[tx]);
    ReceiptSkeleton::compose(tx, &world, &world, &world, &world)
        .expect("the claim's transaction composes")
        .unknowns()
        .entries_json()
}

fn set_gate(claim: &mut BTreeMap<String, Json>, gate: GateName, status: &str) {
    let Some(Json::Array(items)) = claim.get_mut("gates") else {
        panic!("the claim lists gates")
    };
    for item in items.iter_mut() {
        if let Json::Object(entry) = item
            && entry.get("gate").and_then(Json::as_str) == Some(gate.token())
        {
            entry.insert("status".to_owned(), s(status));
        }
    }
}

fn verify_claim(claim: BTreeMap<String, Json>, world: &World) -> Result<(), VerifyRefusal> {
    let bytes = Json::Object(claim).to_canonical_bytes();
    let parsed = ClaimedReceipt::parse(&bytes)?;
    verify_skeleton(&parsed, world, world, world, world, world).map(|_| ())
}

// --- positive ----------------------------------------------------------------------

/// `pr22-impl01-ack-after-sync-intent`: the receipt's intent is the transaction's frozen
/// base intent, resolved in the registry to the protected contract, whose ADR-0013
/// content identity the skeleton carries.
#[test]
fn pr22_impl01_positive_the_intent_is_the_protected_base_intent_and_its_content_identity() {
    let tx = applied();
    let skeleton = skeleton();
    assert_eq!(skeleton.intent().handle(), tx.base_intent());
    assert_eq!(skeleton.intent().handle(), &register_intent());
    assert_eq!(
        skeleton.intent().identity().canonical_bytes(),
        register_contract().identity_preimage_bytes().as_slice(),
    );
    assert_eq!(skeleton.repair_transaction(), tx.repair_id());
    fixture_matches(
        INTENT_FIXTURE,
        &skeleton.intent_fields().to_canonical_bytes(),
    );
}

/// `pr22-impl02-ack-after-sync-snapshots`: before is the transaction's base, after is
/// its sealed candidate, and both are sealed snapshots bound to the base intent.
#[test]
fn pr22_impl02_positive_before_is_the_base_and_after_is_the_sealed_candidate() {
    let tx = applied();
    let skeleton = skeleton();
    assert_eq!(skeleton.snapshots().before(), tx.base_snapshot());
    assert_eq!(skeleton.snapshots().before().as_str(), base());
    assert_eq!(Some(skeleton.snapshots().after()), tx.candidate_snapshot());
    assert_eq!(skeleton.snapshots().after().as_str(), candidate());
    fixture_matches(
        SNAPSHOTS_FIXTURE,
        &skeleton.snapshot_fields().to_canonical_bytes(),
    );
}

/// `pr22-impl07-ack-after-sync-gate-profile` and `pr22-impl07-phase-b-not-yet-enforced`:
/// the skeleton names the transaction's profile, and lists gates 9 and 10 — and only
/// those — `not_yet_enforced`.
#[test]
fn pr22_impl07_positive_a_phase_b_skeleton_names_its_profile_and_lists_gates_9_and_10() {
    let skeleton = skeleton();
    assert_eq!(skeleton.gate_profile(), GateProfile::PhaseB);
    let unenforced: Vec<GateName> = skeleton
        .not_yet_enforced()
        .gates()
        .iter()
        .map(|gate| gate.gate())
        .collect();
    assert_eq!(
        unenforced,
        [GateName::CertificateRebuild, GateName::IncrementalParity]
    );
    fixture_matches(
        PROFILE_FIXTURE,
        &skeleton.profile_fields().to_canonical_bytes(),
    );
    fixture_matches(
        NYE_FIXTURE,
        &skeleton
            .not_yet_enforced()
            .entries_json()
            .to_canonical_bytes(),
    );
}

/// The receipt cites the exact version PR-20 / IMPL-01 retained, not a look-alike.
#[test]
fn pr22_positive_the_skeleton_cites_the_retained_pr20_transaction_version() {
    let retained = Json::parse(PR20_APPLIED).expect("the PR-20 fixture parses");
    let repair_id = retained
        .as_object()
        .and_then(|fields| fields.get("repair_id"))
        .and_then(Json::as_str)
        .expect("the PR-20 fixture names its repair_id");
    assert_eq!(skeleton().repair_transaction().as_str(), repair_id);
}

/// A claim that matches every owned field of the ack-after-sync transaction, and lists
/// the ten in-profile gates `passed`, is refused: the transaction records them `pending`,
/// so the referenced artifact refutes the claim. No transaction records an in-profile gate
/// `passed` until PR-20 `evaluate` lands, so no claim verifies today; the success path is
/// covered by the crate's unit tests over a test-only record.
#[test]
fn pr22_positive_a_claim_matching_every_owned_field_is_refuted_by_the_pending_record() {
    let tx = applied();
    let world = World::register(&[&tx]);
    assert_eq!(
        verify_claim(claim_for(&tx), &world),
        Err(VerifyRefusal::GateNotOnRecord(GateName::BaseReplay))
    );
    assert_eq!(ReceiptSeam::ALL.len(), 6);
    assert!(ReceiptSeam::ALL.contains(&ReceiptSeam::PromotionRecord));
}

/// cr-tz8fwi: promotion verification checks the claim against the record with gate 12
/// still `pending`, so on the ack-after-sync transaction the first gate it refuses is
/// gate 1 (`pending` on record), never gate 12. The record's gate 12 is exactly what
/// promotion requires; the success path over a record with gates 1–11 passed is in the
/// crate's unit tests (`promotion_verifies_against_a_record_with_gate_12_pending`).
#[test]
fn pr22_impl07_negative_promotion_verification_reaches_the_record_with_gate_12_pending() {
    let tx = applied();
    assert_eq!(
        tx.gates()[11].name(),
        GateName::ReceiptGeneration,
        "gate 12 is the last entry"
    );
    let world = World::register(&[&tx]);
    let bytes = Json::Object(claim_for(&tx)).to_canonical_bytes();
    let claim = ClaimedReceipt::parse(&bytes).unwrap();
    assert_eq!(
        verify_for_promotion(&claim, &world, &world, &world, &world, &world),
        Err(VerifyRefusal::GateNotOnRecord(GateName::BaseReplay))
    );
}

/// A superseded base intent stays admissible when a receipt is verified (RFC 0032:
/// published receipts remain verifiable) — the claim gets past re-derivation and every
/// owned field, to the gate record — but a superseded intent composes no new receipt.
#[test]
fn pr22_impl01_positive_a_published_receipt_survives_a_later_intent_revision() {
    let tx = applied();
    let world = World::register(&[&tx]).with_standing(IntentStanding::Superseded);
    assert_eq!(
        verify_claim(claim_for(&tx), &world),
        Err(VerifyRefusal::GateNotOnRecord(GateName::BaseReplay))
    );
    assert_eq!(
        ReceiptSkeleton::compose(&tx, &world, &world, &world, &world),
        Err(ComposeRefusal::IntentSuperseded)
    );
}

// --- negative: forgery -------------------------------------------------------------

/// A client-supplied receipt naming a transaction the daemon never recorded verifies
/// nothing, however well formed. The schema's own shipped example is such a receipt.
#[test]
fn pr22_negative_a_client_supplied_receipt_for_an_unrecorded_transaction_verifies_nothing() {
    let tx = applied();
    let world = World::register(&[&tx]);

    let mut forged = claim_for(&tx);
    forged.insert(
        "repair_transaction".to_owned(),
        s("rt_0000000000000000000000000000000000000000000000000000000000000000"),
    );
    assert_eq!(
        verify_claim(forged, &world),
        Err(VerifyRefusal::TransactionUnknown)
    );

    // The shipped example carries four fractional coverage numbers, which the ID5 reader
    // refuses (it admits integers only). Open question, reported: the schema types
    // refinement coverage as `number`. With them rounded, the example is a well-formed
    // claim — and it names `rt_demo1`, which nothing recorded.
    assert_eq!(
        ClaimedReceipt::parse(EXAMPLE.as_bytes()),
        Err(ClaimRefusal::OutsideId5Vocabulary)
    );
    let rounded = EXAMPLE
        .replace("1.85", "2")
        .replace("0.91", "1")
        .replace("4.32", "4");
    let example = ClaimedReceipt::parse(rounded.as_bytes()).expect("the example is well formed");
    assert_eq!(example.repair_transaction().as_str(), "rt_demo1");
    assert_eq!(
        verify_skeleton(&example, &world, &world, &world, &world, &world),
        Err(VerifyRefusal::TransactionUnknown)
    );
}

/// A store that answers a lookup with some other version is refused, not believed.
#[test]
fn pr22_negative_a_transaction_store_answering_with_another_version_is_refused() {
    let tx = applied();
    let other = applied_to(GateProfile::PhaseB, Repair::Other);
    let mut world = World::register(&[&other]);
    world.transactions.insert(
        tx.repair_id().as_str().to_owned(),
        Resolution::Found(other.clone()),
    );
    assert_eq!(
        verify_claim(claim_for(&tx), &world),
        Err(VerifyRefusal::TransactionStoreInconsistent)
    );
}

// --- negative: intent (IMPL-01) ----------------------------------------------------

/// A claim that swaps the intent id — another registered contract, or one that merely
/// matches the pattern — fails: the intent is read from the transaction, not the claim.
#[test]
fn pr22_impl01_negative_a_swapped_intent_id_fails_verification() {
    let tx = applied();
    let world = World::register(&[&tx]);
    for swapped in [FOREIGN_INTENT, "in_replicated_register_v2"] {
        let mut claim = claim_for(&tx);
        claim.insert("intent".to_owned(), s(swapped));
        assert_eq!(
            verify_claim(claim, &world),
            Err(VerifyRefusal::IntentMismatch),
            "{swapped}"
        );
    }
}

/// An intent the registry does not hold, one that was never accepted, one the registry
/// files under the wrong handle, and a registry that does not answer: no skeleton.
#[test]
fn pr22_impl01_negative_an_unregistered_unprotected_or_inconsistent_intent_composes_nothing() {
    let tx = applied();

    let mut unregistered = World::register(&[&tx]);
    unregistered.intents.clear();
    assert_eq!(
        ReceiptSkeleton::compose(
            &tx,
            &unregistered,
            &unregistered,
            &unregistered,
            &unregistered
        ),
        Err(ComposeRefusal::IntentUnregistered)
    );

    let proposed = World::register(&[&tx]).with_standing(IntentStanding::Proposed);
    assert_eq!(
        ReceiptSkeleton::compose(&tx, &proposed, &proposed, &proposed, &proposed),
        Err(ComposeRefusal::IntentNotProtected)
    );
    assert_eq!(
        verify_claim(claim_for(&tx), &proposed),
        Err(VerifyRefusal::Compose(ComposeRefusal::IntentNotProtected))
    );

    // The registry files some other contract under the register's handle.
    let mut inconsistent = World::register(&[&tx]);
    let other_text = String::from_utf8(REGISTER_CONTRACT.to_vec())
        .unwrap()
        .replace("in_replicated_register_v1", FOREIGN_INTENT);
    let other =
        IntentContract::decode(other_text.as_bytes()).expect("the renamed contract decodes");
    inconsistent.intents.insert(
        register_intent().as_str().to_owned(),
        Resolution::Found(RegisteredIntent::new(other, IntentStanding::Accepted)),
    );
    assert_eq!(
        ReceiptSkeleton::compose(
            &tx,
            &inconsistent,
            &inconsistent,
            &inconsistent,
            &inconsistent
        ),
        Err(ComposeRefusal::RegistryInconsistent)
    );

    let mut unavailable = World::register(&[&tx]);
    unavailable.intents.insert(
        register_intent().as_str().to_owned(),
        Resolution::Unavailable,
    );
    assert_eq!(
        ReceiptSkeleton::compose(&tx, &unavailable, &unavailable, &unavailable, &unavailable),
        Err(ComposeRefusal::StoreUnavailable(Store::IntentRegistry))
    );
}

// --- negative: snapshots (IMPL-02) -------------------------------------------------

/// A foreign after (another repair's sealed snapshot, or one of another contract), a
/// before that is not the base, and swapped before/after all fail.
#[test]
fn pr22_impl02_negative_a_foreign_or_swapped_snapshot_fails_verification() {
    let tx = applied();
    let world = World::register(&[&tx]);
    let cases = [
        ("result_snapshot", other_candidate(), SnapshotRole::After),
        ("result_snapshot", FOREIGN.to_owned(), SnapshotRole::After),
        ("result_snapshot", base(), SnapshotRole::After),
        ("base_snapshot", other_candidate(), SnapshotRole::Before),
        ("base_snapshot", candidate(), SnapshotRole::Before),
    ];
    for (field, value, role) in cases {
        let mut claim = claim_for(&tx);
        claim.insert(field.to_owned(), s(&value));
        assert_eq!(
            verify_claim(claim, &world),
            Err(VerifyRefusal::SnapshotMismatch(role)),
            "{field}={value}"
        );
    }
    let mut swapped = claim_for(&tx);
    swapped.insert("base_snapshot".to_owned(), s(&candidate()));
    swapped.insert("result_snapshot".to_owned(), s(&base()));
    assert_eq!(
        verify_claim(swapped, &world),
        Err(VerifyRefusal::SnapshotMismatch(SnapshotRole::Before))
    );
}

/// A draft has no after; an unsealed candidate, a candidate bound to another contract,
/// and a snapshot store that does not answer compose nothing.
#[test]
fn pr22_impl02_negative_a_missing_unsealed_or_foreign_candidate_composes_nothing() {
    let draft = draft(GateProfile::PhaseB);
    let world = World::register(&[&draft]);
    assert_eq!(
        ReceiptSkeleton::compose(&draft, &world, &world, &world, &world),
        Err(ComposeRefusal::NoCandidate)
    );

    // The candidate was sealed by `apply`, but the daemon holds no sealed snapshot
    // under its identity.
    let unsealed = applied();
    let mut world = World::register(&[&unsealed]);
    world.snapshots.remove(&candidate());
    assert_eq!(
        ReceiptSkeleton::compose(&unsealed, &world, &world, &world, &world),
        Err(ComposeRefusal::SnapshotUnresolved(SnapshotRole::After))
    );
    assert_eq!(
        verify_claim(claim_for(&unsealed), &world),
        Err(VerifyRefusal::Compose(ComposeRefusal::SnapshotUnresolved(
            SnapshotRole::After
        )))
    );

    // The daemon holds the candidate bound to another contract.
    let foreign = applied();
    let mut world = World::register(&[&foreign]);
    world.snapshots.insert(
        candidate(),
        Resolution::Found(IntentId::new(FOREIGN_INTENT).unwrap()),
    );
    assert_eq!(
        ReceiptSkeleton::compose(&foreign, &world, &world, &world, &world),
        Err(ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::After))
    );

    let tx = applied();
    let mut base_foreign = World::register(&[&tx]);
    base_foreign.snapshots.insert(
        base(),
        Resolution::Found(IntentId::new(FOREIGN_INTENT).unwrap()),
    );
    assert_eq!(
        verify_claim(claim_for(&tx), &base_foreign),
        Err(VerifyRefusal::Compose(
            ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::Before)
        ))
    );

    let mut base_unbound = World::register(&[&tx]);
    base_unbound.snapshots.remove(&base());
    assert_eq!(
        ReceiptSkeleton::compose(
            &tx,
            &base_unbound,
            &base_unbound,
            &base_unbound,
            &base_unbound
        ),
        Err(ComposeRefusal::SnapshotUnresolved(SnapshotRole::Before))
    );

    let mut unavailable = World::register(&[&tx]);
    unavailable
        .snapshots
        .insert(candidate(), Resolution::Unavailable);
    assert_eq!(
        ReceiptSkeleton::compose(&tx, &unavailable, &unavailable, &unavailable, &unavailable),
        Err(ComposeRefusal::StoreUnavailable(Store::Snapshots))
    );
}

// --- negative: gate_profile and NotYetEnforced (IMPL-07, IMPL-08) ------------------

/// Under `phase-b`, claiming gate 9 or gate 10 `passed` is refused: an unenforced gate
/// is never rendered as passed.
#[test]
fn pr22_impl07_negative_a_not_yet_enforced_gate_claimed_passed_fails() {
    let tx = applied();
    let world = World::register(&[&tx]);
    for gate in [GateName::CertificateRebuild, GateName::IncrementalParity] {
        let mut claim = claim_for(&tx);
        set_gate(&mut claim, gate, "passed");
        assert_eq!(
            verify_claim(claim, &world),
            Err(VerifyRefusal::UnenforcedGateClaimedPassed(gate)),
            "{}",
            gate.token()
        );
    }
}

/// A gate inside the profile listed `not_yet_enforced` hides reduced scope the other way.
#[test]
fn pr22_impl07_negative_an_enforced_gate_listed_not_yet_enforced_fails() {
    let tx = applied();
    let world = World::register(&[&tx]);
    for gate in GateName::ALL
        .into_iter()
        .filter(|gate| GateProfile::PhaseB.enforces(*gate))
    {
        let mut claim = claim_for(&tx);
        set_gate(&mut claim, gate, "not_yet_enforced");
        assert_eq!(
            verify_claim(claim, &world),
            Err(VerifyRefusal::EnforcedGateListedUnenforced(gate)),
            "{}",
            gate.token()
        );
    }
}

/// A Phase B transaction's receipt cannot be presented as a Phase C or Phase D one, even
/// with a gate list that is consistent with the claimed profile.
#[test]
fn pr22_impl07_negative_a_profile_other_than_the_transactions_fails() {
    let tx = applied();
    let world = World::register(&[&tx]);
    for claimed in [
        GateProfile::PhaseC,
        GateProfile::PhaseD,
        GateProfile::Default,
    ] {
        let mut claim = claim_for(&tx);
        claim.insert("gate_profile".to_owned(), s(claimed.token()));
        for gate in GateName::ALL {
            let status = if claimed.enforces(gate) {
                "passed"
            } else {
                "not_yet_enforced"
            };
            set_gate(&mut claim, gate, status);
        }
        assert_eq!(
            verify_claim(claim, &world),
            Err(VerifyRefusal::ProfileMismatch { claimed }),
            "{}",
            claimed.token()
        );
    }
    // And the converse: a Phase D transaction's receipt cannot claim Phase B.
    let phase_d = applied_to(GateProfile::PhaseD, Repair::AckAfterSync);
    let world = World::register(&[&phase_d]);
    let mut claim = claim_for(&phase_d);
    claim.insert("gate_profile".to_owned(), s("phase-b"));
    assert_eq!(
        verify_claim(claim, &world),
        Err(VerifyRefusal::ProfileMismatch {
            claimed: GateProfile::PhaseB
        })
    );
}

// --- negative: malformed claims ----------------------------------------------------

/// The claim reader is strict where the schema is, and echoes nothing it read.
#[test]
fn pr22_negative_a_malformed_claim_is_refused_by_the_reader_which_takes_no_store() {
    let tx = applied();
    let refusal = |claim: BTreeMap<String, Json>| {
        ClaimedReceipt::parse(&Json::Object(claim).to_canonical_bytes()).map(|_| ())
    };

    let mut pending = claim_for(&tx);
    set_gate(&mut pending, GateName::BaseReplay, "pending");
    assert_eq!(
        refusal(pending),
        Err(ClaimRefusal::Malformed(ReceiptField::Gates))
    );

    let mut eleven = claim_for(&tx);
    if let Some(Json::Array(items)) = eleven.get_mut("gates") {
        items.pop();
    }
    assert_eq!(refusal(eleven), Err(ClaimRefusal::GateCount));

    let mut duplicate = claim_for(&tx);
    if let Some(Json::Array(items)) = duplicate.get_mut("gates") {
        items[11] = items[0].clone();
    }
    assert_eq!(
        refusal(duplicate),
        Err(ClaimRefusal::DuplicateGate(GateName::BaseReplay))
    );

    let mut undeclared = claim_for(&tx);
    undeclared.insert("status".to_owned(), s("passed"));
    assert_eq!(refusal(undeclared), Err(ClaimRefusal::UndeclaredField));

    // Each `gates[]` entry is exactly `{gate, status}` with string tokens.
    type Tamper = fn(&mut BTreeMap<String, Json>);
    let entry_tampers: [(&str, Tamper); 4] = [
        ("an extra evidence key", |entry| {
            entry.insert("evidence".to_owned(), Json::Array(Vec::new()));
        }),
        ("no status", |entry| {
            entry.remove("status");
        }),
        ("a non-string status", |entry| {
            entry.insert("status".to_owned(), Json::Integer(1));
        }),
        ("an unknown gate", |entry| {
            entry.insert("gate".to_owned(), s("gate_thirteen"));
        }),
    ];
    for (case, tamper) in entry_tampers {
        let mut claim = claim_for(&tx);
        if let Some(Json::Array(items)) = claim.get_mut("gates")
            && let Json::Object(entry) = &mut items[0]
        {
            tamper(entry);
        }
        assert_eq!(
            refusal(claim),
            Err(ClaimRefusal::Malformed(ReceiptField::Gates)),
            "{case}"
        );
    }
    let mut not_an_object = claim_for(&tx);
    if let Some(Json::Array(items)) = not_an_object.get_mut("gates") {
        items[0] = s("base_replay");
    }
    assert_eq!(
        refusal(not_an_object),
        Err(ClaimRefusal::Malformed(ReceiptField::Gates))
    );

    for field in ReceiptField::ALL {
        let mut missing = claim_for(&tx);
        missing.remove(field.property());
        assert_eq!(refusal(missing), Err(ClaimRefusal::Missing(field)));
    }

    let mut off_pattern = claim_for(&tx);
    off_pattern.insert("result_snapshot".to_owned(), s("rt_not_a_snapshot"));
    assert_eq!(
        refusal(off_pattern),
        Err(ClaimRefusal::Malformed(ReceiptField::ResultSnapshot))
    );

    let mut off_enum = claim_for(&tx);
    off_enum.insert("gate_profile".to_owned(), s("phase-a"));
    assert_eq!(
        refusal(off_enum),
        Err(ClaimRefusal::Malformed(ReceiptField::GateProfile))
    );

    // A refusal's rendering names the field, never the value.
    let text =
        VerifyRefusal::Claim(ClaimRefusal::Malformed(ReceiptField::ResultSnapshot)).to_string();
    assert!(!text.contains("rt_not_a_snapshot"));

    let mut unavailable = World::register(&[]);
    unavailable
        .transactions
        .insert(tx.repair_id().as_str().to_owned(), Resolution::Unavailable);
    assert_eq!(
        verify_claim(claim_for(&tx), &unavailable),
        Err(VerifyRefusal::StoreUnavailable(Store::Transactions))
    );
}

// --- boundary ----------------------------------------------------------------------

/// Metamorphic relation: serialization round trip. Canonical bytes are stable, survive
/// a parse and re-encode, and the skeleton's
/// identity is its derivation: composing twice gives equal skeletons, and changing the
/// transaction's candidate changes both the anchor and the after.
#[test]
fn pr22_boundary_canonical_bytes_are_stable_and_survive_a_serialization_round_trip() {
    let first = skeleton();
    let second = skeleton();
    assert_eq!(first, second);
    for json in [
        first.fields_json(),
        first.intent_fields(),
        first.snapshot_fields(),
        first.profile_fields(),
        first.diff_fields(),
        first.not_yet_enforced().entries_json(),
    ] {
        let bytes = json.to_canonical_bytes();
        assert_eq!(bytes, second_rendering(&json));
        let reparsed = Json::parse(&bytes).expect("canonical bytes parse");
        assert_eq!(reparsed.to_canonical_bytes(), bytes);
    }
    // The combined fragment is the union of the per-bone fragments.
    let fields = first.fields_json();
    let keys: Vec<&str> = fields
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "base_snapshot",
            "gate_profile",
            "intent",
            "intent_diff",
            "repair_transaction",
            "result_snapshot",
            "semantic_diff",
            "unknowns"
        ]
    );

    let other_tx = applied_to(GateProfile::PhaseB, Repair::Other);
    let world = World::register(&[&other_tx]);
    let other = ReceiptSkeleton::compose(&other_tx, &world, &world, &world, &world).unwrap();
    assert_ne!(other, first);
    assert_ne!(other.repair_transaction(), first.repair_transaction());
    assert_ne!(other.snapshots().after(), first.snapshots().after());
    assert_eq!(other.snapshots().before(), first.snapshots().before());
    assert_eq!(other.intent(), first.intent());
}

fn second_rendering(json: &Json) -> Vec<u8> {
    let mut out = Vec::new();
    json.write_canonical(&mut out);
    out
}

/// The intent identity is exactly the contract's ADR-0013 preimage bytes as
/// `continuum-intent` computes them from the fixture, not a string the skeleton holds.
/// The oracle is the owning crate's decoder, run apart from the registry path; it is not
/// an independent re-implementation of the preimage rule.
#[test]
fn pr22_impl01_differential_the_intent_identity_matches_continuum_intent_independently() {
    let independent = IntentContract::decode(REGISTER_CONTRACT).unwrap();
    let skeleton = skeleton();
    assert_eq!(
        skeleton.intent().identity(),
        independent.identity(),
        "the skeleton's identity is the contract's own"
    );
    assert_eq!(
        skeleton.intent().identity().canonical_bytes(),
        independent
            .identity_preimage_json()
            .to_canonical_bytes()
            .as_slice()
    );
}

/// Exactly `MAX_CLAIMED_RECEIPT_BYTES` is read; one byte more is refused before parsing.
#[test]
fn pr22_boundary_the_claim_size_bound_is_checked_before_parsing() {
    let tx = applied();
    let mut bytes = Json::Object(claim_for(&tx)).to_canonical_bytes();
    bytes.resize(MAX_CLAIMED_RECEIPT_BYTES, b' ');
    assert!(ClaimedReceipt::parse(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(
        ClaimedReceipt::parse(&bytes),
        Err(ClaimRefusal::TooLarge {
            limit: MAX_CLAIMED_RECEIPT_BYTES
        })
    );
    // Not JSON at all, and under the bound: refused as such, not as oversize.
    assert_eq!(
        ClaimedReceipt::parse(&[b'['; 8]),
        Err(ClaimRefusal::NotJson)
    );
}

/// Every profile's `NotYetEnforced` list is RFC 0032's table; a Phase B skeleton is
/// structurally distinct from a Phase D one; no entry renders any status but
/// `not_yet_enforced`.
#[test]
fn pr22_impl07_boundary_every_profile_lists_exactly_its_unenforced_gates() {
    let expected: [(GateProfile, &[GateName]); 4] = [
        (
            GateProfile::PhaseB,
            &[GateName::CertificateRebuild, GateName::IncrementalParity],
        ),
        (GateProfile::PhaseC, &[GateName::CertificateRebuild]),
        (GateProfile::PhaseD, &[]),
        (GateProfile::Default, &[]),
    ];
    for (profile, gates) in expected {
        let list = NotYetEnforced::of(profile);
        assert_eq!(list.profile(), profile);
        let listed: Vec<GateName> = list.gates().iter().map(|gate| gate.gate()).collect();
        assert_eq!(listed, gates, "{}", profile.token());
        for gate in GateName::ALL {
            assert_eq!(list.contains(gate), gates.contains(&gate));
        }
        for entry in list.entries_json().as_array().unwrap() {
            let status = entry.as_object().unwrap().get("status").unwrap();
            assert_eq!(status.as_str(), Some(UnenforcedGate::STATUS));
        }
    }

    let phase_b = skeleton();
    let tx = applied_to(GateProfile::PhaseD, Repair::AckAfterSync);
    let world = World::register(&[&tx]);
    let phase_d = ReceiptSkeleton::compose(&tx, &world, &world, &world, &world).unwrap();
    assert_ne!(phase_b.profile_fields(), phase_d.profile_fields());
    assert_eq!(phase_b.not_yet_enforced().gates().len(), 2);
    assert!(phase_d.not_yet_enforced().gates().is_empty());
}

fn schema() -> Json {
    Json::parse(SCHEMA.as_bytes()).expect("the receipt schema parses")
}

fn at<'a>(json: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(json, |cursor, key| {
        cursor
            .as_object()
            .and_then(|fields| fields.get(*key))
            .unwrap_or_else(|| panic!("the schema declares {key}"))
    })
}

fn tokens(json: &Json) -> BTreeSet<String> {
    json.as_array()
        .expect("an enum is an array")
        .iter()
        .map(|item| item.as_str().expect("a string token").to_owned())
        .collect()
}

/// The crate's closed sets are the schema's: properties, profiles, gate names, the two
/// receipt statuses; and the owned fields plus the seams cover every property.
#[test]
fn pr22_boundary_the_closed_sets_and_fields_are_the_receipt_schemas() {
    let schema = schema();
    assert_eq!(
        at(&schema, &["properties", "schema_id", "const"]).as_str(),
        Some(RECEIPT_SCHEMA_ID)
    );
    let properties: BTreeSet<String> = at(&schema, &["properties"])
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let ours: BTreeSet<String> = RECEIPT_PROPERTIES.iter().map(|p| (*p).to_owned()).collect();
    assert_eq!(ours, properties);

    let covered: BTreeSet<String> = ReceiptField::ALL
        .iter()
        .map(|field| field.property().to_owned())
        .chain(
            ReceiptSeam::ALL
                .iter()
                .flat_map(|seam| seam.properties().iter().map(|p| (*p).to_owned())),
        )
        .collect();
    assert_eq!(
        covered, properties,
        "every property is owned or a named seam"
    );

    assert_eq!(
        tokens(at(&schema, &["properties", "gate_profile", "enum"])),
        GateProfile::ALL
            .iter()
            .map(|p| p.token().to_owned())
            .collect()
    );
    assert_eq!(
        tokens(at(&schema, &["$defs", "gate_name", "enum"])),
        GateName::ALL.iter().map(|g| g.token().to_owned()).collect()
    );
    assert_eq!(
        tokens(at(
            &schema,
            &[
                "properties",
                "gates",
                "items",
                "properties",
                "status",
                "enum"
            ]
        )),
        BTreeSet::from(["passed".to_owned(), UnenforcedGate::STATUS.to_owned()])
    );

    // The claimed-status vocabulary is those two, and a claim can name every gate.
    let tx = applied();
    let bytes = Json::Object(claim_for(&tx)).to_canonical_bytes();
    let claim = ClaimedReceipt::parse(&bytes).unwrap();
    for gate in GateName::ALL {
        let expected = if GateProfile::PhaseB.enforces(gate) {
            ClaimedGateStatus::Passed
        } else {
            ClaimedGateStatus::NotYetEnforced
        };
        assert_eq!(claim.gate(gate), expected);
    }
}
