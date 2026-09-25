//! PR-22 evidence: refinement and certificate status (IMPL-05, bn-9r5e) and `unknowns`
//! (IMPL-06, bn-yzu1), composed for the replicated register's ack-after-sync repair.
//!
//! The normative sources are `notes/plan/schemas/promotion-receipt.schema.json` (shape,
//! INV-003) and RFC 0032, "The promotion receipt" and "Gate 8 is a computed delta".
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is | Checked by |
//! |---|---|---|
//! | `pr22-impl05-ack-after-sync-certificate-status` | `tests/fixtures/pr22-impl05-ack-after-sync-certificate-status.json`: the `unknowns` entries the status contributes — refinement not computed, and each certificate that is not model-bound to the candidate — with `repair_transaction` | this suite (byte equality) and `validate_dossier.py` (each field against the receipt schema's own property) |
//! | `pr22-impl06-ack-after-sync-unknowns` | `tests/fixtures/pr22-impl06-ack-after-sync-unknowns.json`: the whole derived `unknowns` of the applied transaction under `phase-b` | as above |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite the fixtures from the library.
//!
//! The verification side (a forged coverage group, an omitted certificate unknown, an
//! invented or reordered entry) needs a transaction record with in-profile gates
//! `passed`, which only the crate's unit tests can build until PR-20 `evaluate` lands:
//! `receipt::tests::pr22_impl05_*` and `pr22_impl06_*`, and the completeness test
//! `receipt::unknowns::tests::pr22_impl06_every_absent_or_inconclusive_input_appears_exactly_once`.

mod common;

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::handle::{CrashpackId, RepairId, SnapshotId};
use continuum_repair::hypothesis::{Hypothesis, Proposal};
use continuum_repair::receipt::status::{
    CertificateStatus, InsufficientReason, MAX_CANDIDATE_CERTIFICATES, RefinementAbsence,
    RefinementStatus, UnverifiedReason,
};
use continuum_repair::receipt::unknowns::MAX_PACK_CASES;
use continuum_repair::receipt::{
    CandidateEvidence, CertificateNode, CertificateRecord, CertificateVerdict, ClaimedReceipt,
    ComposeRefusal, EvidenceKind, IntentRegistry, IntentStanding, PackCase, ReceiptSkeleton,
    RegisteredIntent, SealedSnapshots, Store, TransactionStore, Unknown, UnknownKind,
    VerifyRefusal, verify_skeleton,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, RepairTransaction, Resolution,
};
use continuum_value::identity::Blake3Hasher;

use common::{ack_after_sync_change, base_content, base_id};

type Tx = RepairTransaction<Blake3Hasher>;

const REGISTER_CONTRACT: &[u8] =
    include_bytes!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const CRASH: &str = "crash_pr20_impl01_ack_before_sync";
const ACK_AFTER_SYNC: &str = "move the ack after the storage sync: the replica publishes its \
    reply only once the write is durable, so an acknowledged write survives a crash";

const STATUS_FIXTURE: &str = "tests/fixtures/pr22-impl05-ack-after-sync-certificate-status.json";
const UNKNOWNS_FIXTURE: &str = "tests/fixtures/pr22-impl06-ack-after-sync-unknowns.json";

fn register_contract() -> IntentContract {
    IntentContract::decode(REGISTER_CONTRACT).expect("the replicated register contract decodes")
}

fn register_intent() -> IntentId {
    register_contract().intent_id().clone()
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

fn applied(profile: GateProfile) -> Tx {
    Tx::begin(CrashpackId::new(CRASH).unwrap(), profile, &Binding)
        .unwrap()
        .apply(
            &Proposal::new(
                Hypothesis::new(ACK_AFTER_SYNC),
                vec![ack_after_sync_change()],
            ),
            &base_content(),
        )
        .unwrap()
        .into_parts()
        .0
}

/// A certificate the daemon holds for the candidate.
#[derive(Clone)]
enum Cert {
    /// Model-bound to the candidate the seam is asked about.
    Bound,
    /// Another verdict.
    Verdict(CertificateVerdict),
}

/// The daemon's view. Every snapshot is sealed under the register contract.
struct World {
    transactions: Vec<Tx>,
    certificates: Resolution<Vec<(String, Cert)>>,
    packs: Resolution<Vec<PackCase>>,
}

impl World {
    fn new(tx: &Tx, certificates: Vec<(&str, Cert)>, packs: Vec<PackCase>) -> Self {
        Self {
            transactions: vec![tx.clone()],
            certificates: Resolution::Found(
                certificates
                    .into_iter()
                    .map(|(handle, cert)| (handle.to_owned(), cert))
                    .collect(),
            ),
            packs: Resolution::Found(packs),
        }
    }
}

impl IntentRegistry for World {
    fn registered(&self, intent: &IntentId) -> Resolution<RegisteredIntent> {
        if intent == &register_intent() {
            Resolution::Found(RegisteredIntent::new(
                register_contract(),
                IntentStanding::Accepted,
            ))
        } else {
            Resolution::Unknown
        }
    }
}

impl SealedSnapshots for World {
    fn sealed_binding(&self, _: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(register_intent())
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
    fn certificates(
        &self,
        _: &RepairId,
        candidate: &SnapshotId,
    ) -> Resolution<Vec<CertificateRecord>> {
        match &self.certificates {
            Resolution::Found(list) => Resolution::Found(
                list.iter()
                    .map(|(handle, cert)| {
                        let verdict = match cert {
                            Cert::Bound => CertificateVerdict::ModelBound {
                                snapshot: candidate.clone(),
                            },
                            Cert::Verdict(verdict) => verdict.clone(),
                        };
                        CertificateRecord::new(CertificateNode::new(handle).unwrap(), verdict)
                    })
                    .collect(),
            ),
            Resolution::Unknown => Resolution::Unknown,
            Resolution::Unavailable => Resolution::Unavailable,
        }
    }

    fn unsupported_pack_cases(&self, _: &RepairId, _: &SnapshotId) -> Resolution<Vec<PackCase>> {
        self.packs.clone()
    }
}

fn flush_dishonesty() -> PackCase {
    PackCase::new("storage/append-log-v1", "flush-dishonesty").unwrap()
}

/// The register's evidence: one certificate model-bound to the candidate, one from wire
/// epoch 1 (it trusts its model correspondence), one bound to the base snapshot's model
/// rather than the candidate's, and one attached and never verified; the storage pack's
/// flush-dishonesty case.
fn register_world(tx: &Tx) -> World {
    World::new(
        tx,
        vec![
            ("ev_register_closure_epoch2", Cert::Bound),
            (
                "ev_register_closure_epoch1",
                Cert::Verdict(CertificateVerdict::InsufficientEvidence(
                    InsufficientReason::TrustsModelCorrespondence,
                )),
            ),
            (
                "ev_register_base_closure",
                Cert::Verdict(CertificateVerdict::ModelBound {
                    snapshot: base_id(),
                }),
            ),
            (
                "ev_register_agent_attached",
                Cert::Verdict(CertificateVerdict::Unchecked),
            ),
        ],
        vec![flush_dishonesty()],
    )
}

fn compose(tx: &Tx, world: &World) -> Result<ReceiptSkeleton, ComposeRefusal> {
    ReceiptSkeleton::compose(tx, world, world, world)
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

fn tokens(skeleton: &ReceiptSkeleton) -> Vec<String> {
    skeleton.unknowns().tokens()
}

// --- positive ----------------------------------------------------------------------

/// `pr22-impl05-ack-after-sync-certificate-status`: only the certificate model-bound to
/// the candidate counts; the epoch-1, other-snapshot and unchecked ones are unverified
/// with their reasons; refinement coverage is absent, typed, never a number.
#[test]
fn pr22_impl05_positive_only_the_model_bound_candidate_certificate_counts() {
    let tx = applied(GateProfile::PhaseB);
    let skeleton = compose(&tx, &register_world(&tx)).unwrap();
    let status = skeleton.status();
    assert_eq!(
        status.refinement(),
        RefinementStatus::Absent(RefinementAbsence::CheckerNotDeployed)
    );
    let by_node: BTreeMap<&str, CertificateStatus> = status
        .certificates()
        .iter()
        .map(|entry| (entry.node().as_str(), entry.status()))
        .collect();
    assert_eq!(
        by_node,
        BTreeMap::from([
            (
                "ev_register_agent_attached",
                CertificateStatus::Unverified(UnverifiedReason::Unchecked)
            ),
            (
                "ev_register_base_closure",
                CertificateStatus::Unverified(UnverifiedReason::BoundToOtherSnapshot)
            ),
            (
                "ev_register_closure_epoch1",
                CertificateStatus::Unverified(UnverifiedReason::TrustsModelCorrespondence)
            ),
            (
                "ev_register_closure_epoch2",
                CertificateStatus::VerifiedModelBound
            ),
        ])
    );
    let verified: Vec<&str> = status.verified().map(CertificateNode::as_str).collect();
    assert_eq!(verified, ["ev_register_closure_epoch2"]);
    fixture_matches(
        STATUS_FIXTURE,
        &skeleton.status_fields().to_canonical_bytes(),
    );
}

/// `pr22-impl06-ack-after-sync-unknowns`: the applied transaction's unknowns under
/// `phase-b`, in byte order — gates 9 and 10 not yet enforced, gates 1–8 and 11 pending (gate 12 is the
/// receipt step), refinement not computed, the three certificates that do not count, and
/// the storage pack's case — each exactly once, in canonical order.
#[test]
fn pr22_impl06_positive_the_applied_transactions_unknowns_are_listed_exactly_once() {
    let tx = applied(GateProfile::PhaseB);
    let skeleton = compose(&tx, &register_world(&tx)).unwrap();
    let expected = [
        "certificate_unverified:ev_register_agent_attached:unchecked",
        "certificate_unverified:ev_register_base_closure:bound_to_other_snapshot",
        "certificate_unverified:ev_register_closure_epoch1:trusts_model_correspondence",
        "gate_not_yet_enforced:certificate_rebuild",
        "gate_not_yet_enforced:incremental_parity",
        "gate_pending:base_replay",
        "gate_pending:code_and_security",
        "gate_pending:defect_mutants",
        "gate_pending:exact_regression",
        "gate_pending:intent_integrity",
        "gate_pending:neighborhood",
        "gate_pending:patch_application",
        "gate_pending:property_mutation",
        "gate_pending:refinement_coverage",
        "refinement_not_computed:checker_not_deployed",
        "unsupported_pack_case:storage/append-log-v1:flush-dishonesty",
    ];
    assert_eq!(tokens(&skeleton), expected);
    for entry in skeleton.unknowns().entries() {
        assert_eq!(Unknown::from_token(&entry.token()).as_ref(), Some(entry));
    }
    fixture_matches(
        UNKNOWNS_FIXTURE,
        &skeleton.unknown_fields().to_canonical_bytes(),
    );
}

// --- negative ----------------------------------------------------------------------

/// A missing certificate is an unknown: with no certificate at all, and with only
/// certificates that do not count, the receipt lists `certificate_none_verified`. With
/// one model-bound certificate it does not.
#[test]
fn pr22_impl05_negative_a_missing_certificate_appears_as_an_unknown() {
    let tx = applied(GateProfile::PhaseB);
    let none = "certificate_none_verified".to_owned();

    let empty = compose(&tx, &World::new(&tx, Vec::new(), Vec::new())).unwrap();
    assert!(tokens(&empty).contains(&none));
    assert_eq!(empty.status().verified().count(), 0);

    let content_missing = World::new(
        &tx,
        vec![(
            "ev_register_closure_epoch2",
            Cert::Verdict(CertificateVerdict::InsufficientEvidence(
                InsufficientReason::ContentMissing,
            )),
        )],
        Vec::new(),
    );
    let missing = tokens(&compose(&tx, &content_missing).unwrap());
    assert!(missing.contains(&none));
    assert!(
        missing.contains(
            &"certificate_unverified:ev_register_closure_epoch2:content_missing".to_owned()
        )
    );

    let bound = World::new(
        &tx,
        vec![("ev_register_closure_epoch2", Cert::Bound)],
        Vec::new(),
    );
    let with_one = tokens(&compose(&tx, &bound).unwrap());
    assert!(!with_one.contains(&none));
    assert!(
        !with_one
            .iter()
            .any(|entry| entry.starts_with("certificate_unverified"))
    );
}

/// A certificate model-bound to another snapshot — the base, or another repair's
/// candidate — never counts for this candidate, whatever else the daemon says.
#[test]
fn pr22_impl05_negative_a_certificate_bound_to_another_snapshot_is_not_verified() {
    let tx = applied(GateProfile::PhaseB);
    let world = World::new(
        &tx,
        vec![(
            "ev_other",
            Cert::Verdict(CertificateVerdict::ModelBound {
                snapshot: SnapshotId::new("ws_another_repairs_candidate").unwrap(),
            }),
        )],
        Vec::new(),
    );
    let skeleton = compose(&tx, &world).unwrap();
    assert_eq!(skeleton.status().verified().count(), 0);
    assert_eq!(
        skeleton.status().certificates()[0].status(),
        CertificateStatus::Unverified(UnverifiedReason::BoundToOtherSnapshot)
    );
}

/// The evidence store's answers are typed: no answer is not an empty answer, and an
/// over-bound or duplicated answer is a store defect.
#[test]
fn pr22_impl05_negative_an_unusable_evidence_answer_composes_nothing() {
    let tx = applied(GateProfile::PhaseB);
    let mut unavailable = register_world(&tx);
    unavailable.certificates = Resolution::Unavailable;
    assert_eq!(
        compose(&tx, &unavailable),
        Err(ComposeRefusal::StoreUnavailable(Store::Evidence))
    );
    let mut unknown = register_world(&tx);
    unknown.packs = Resolution::Unknown;
    assert_eq!(compose(&tx, &unknown), Err(ComposeRefusal::EvidenceUnknown));

    let handles: Vec<String> = (0..=MAX_CANDIDATE_CERTIFICATES)
        .map(|i| format!("ev_{i}"))
        .collect();
    let mut over = register_world(&tx);
    over.certificates = Resolution::Found(
        handles
            .iter()
            .map(|handle| (handle.clone(), Cert::Bound))
            .collect(),
    );
    assert_eq!(
        compose(&tx, &over),
        Err(ComposeRefusal::EvidenceOverBound(
            EvidenceKind::Certificates
        ))
    );

    let twice = World::new(
        &tx,
        vec![("ev_a", Cert::Bound), ("ev_a", Cert::Bound)],
        Vec::new(),
    );
    assert_eq!(
        compose(&tx, &twice),
        Err(ComposeRefusal::EvidenceDuplicate(
            EvidenceKind::Certificates
        ))
    );

    let cases: Vec<PackCase> = (0..=MAX_PACK_CASES)
        .map(|i| PackCase::new("storage/append-log-v1", &format!("case-{i}")).unwrap())
        .collect();
    let packs_over = World::new(&tx, Vec::new(), cases);
    assert_eq!(
        compose(&tx, &packs_over),
        Err(ComposeRefusal::EvidenceOverBound(EvidenceKind::PackCases))
    );
    let packs_twice = World::new(
        &tx,
        Vec::new(),
        vec![flush_dishonesty(), flush_dishonesty()],
    );
    assert_eq!(
        compose(&tx, &packs_twice),
        Err(ComposeRefusal::EvidenceDuplicate(EvidenceKind::PackCases))
    );
}

/// A client-supplied receipt whose `unknowns` is the honest list, and one that drops
/// every unknown, are both refuted first by the transaction's `pending` record: a claim
/// is never a source, and the unknowns check comes after the record in RFC order.
#[test]
fn pr22_impl06_negative_a_claim_is_checked_against_the_record_first() {
    let tx = applied(GateProfile::PhaseB);
    let world = register_world(&tx);
    let skeleton = compose(&tx, &world).unwrap();
    let mut fields = match skeleton.fields_json() {
        Json::Object(fields) => fields,
        _ => unreachable!(),
    };
    let gates = GateName::ALL
        .into_iter()
        .map(|gate| {
            let status = if GateProfile::PhaseB.enforces(gate) {
                "passed"
            } else {
                "not_yet_enforced"
            };
            Json::Object(BTreeMap::from([
                ("gate".to_owned(), Json::String(gate.token().to_owned())),
                ("status".to_owned(), Json::String(status.to_owned())),
            ]))
        })
        .collect();
    fields.insert("gates".to_owned(), Json::Array(gates));
    for unknowns in [skeleton.unknowns().entries_json(), Json::Array(Vec::new())] {
        let mut claim = fields.clone();
        claim.insert("unknowns".to_owned(), unknowns);
        let parsed = ClaimedReceipt::parse(&Json::Object(claim).to_canonical_bytes()).unwrap();
        assert_eq!(
            verify_skeleton(&parsed, &world, &world, &world, &world).map(|_| ()),
            Err(VerifyRefusal::GateNotOnRecord(GateName::BaseReplay))
        );
    }
}

// --- boundary ----------------------------------------------------------------------

/// Metamorphic relations over the evidence. Set/map insertion order: the order in which
/// the store lists its records does not change the status or the unknowns; adding an unverified certificate
/// adds exactly its one entry; adding a model-bound one removes only
/// `certificate_none_verified`.
#[test]
fn pr22_impl06_boundary_store_order_is_invisible_and_each_input_adds_one_entry() {
    let tx = applied(GateProfile::PhaseB);
    let forward = register_world(&tx);
    let mut reversed = register_world(&tx);
    if let Resolution::Found(list) = &mut reversed.certificates {
        list.reverse();
    }
    assert_eq!(
        compose(&tx, &forward).unwrap(),
        compose(&tx, &reversed).unwrap()
    );

    let bare = tokens(&compose(&tx, &World::new(&tx, Vec::new(), Vec::new())).unwrap());
    let one_unverified = tokens(
        &compose(
            &tx,
            &World::new(
                &tx,
                vec![("ev_x", Cert::Verdict(CertificateVerdict::Rejected))],
                Vec::new(),
            ),
        )
        .unwrap(),
    );
    assert_eq!(one_unverified.len(), bare.len() + 1);
    assert!(one_unverified.contains(&"certificate_unverified:ev_x:rejected".to_owned()));

    let one_bound = tokens(
        &compose(
            &tx,
            &World::new(&tx, vec![("ev_x", Cert::Bound)], Vec::new()),
        )
        .unwrap(),
    );
    let mut expected = bare.clone();
    expected.retain(|entry| entry != "certificate_none_verified");
    assert_eq!(one_bound, expected);
}

/// Every profile lists its unenforced gates once, as unknowns and nowhere else; gate 9's
/// rebuild absence is its own entry only where gate 9 is enforced.
#[test]
fn pr22_impl06_boundary_every_profile_lists_its_gates_once() {
    for profile in GateProfile::ALL {
        let tx = applied(profile);
        let skeleton = compose(&tx, &World::new(&tx, Vec::new(), Vec::new())).unwrap();
        let kinds: Vec<UnknownKind> = skeleton
            .unknowns()
            .entries()
            .iter()
            .map(Unknown::kind)
            .collect();
        let unenforced = GateName::ALL
            .into_iter()
            .filter(|gate| !profile.enforces(*gate))
            .count();
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == UnknownKind::GateNotYetEnforced)
                .count(),
            unenforced
        );
        assert_eq!(
            kinds.contains(&UnknownKind::CertificateRebuildNotComputed),
            profile.enforces(GateName::CertificateRebuild),
            "{}",
            profile.token()
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == UnknownKind::RefinementNotComputed)
                .count(),
            1
        );
    }
}

// --- differential ------------------------------------------------------------------

/// Differential against the storage pack's own declaration (`continuum_effects_storage`,
/// PR-15 / IMPL-04): every declared unsupported case of `storage/append-log-v1` is a
/// well-formed [`PackCase`] under the pack's own tokens, the receipt lists each one
/// exactly once, and each entry reads back as the pack's case. The oracle is the pack
/// crate's `UNSUPPORTED` table, which this crate does not restate.
#[test]
fn pr22_impl06_differential_the_storage_packs_declared_cases_are_each_one_unknown() {
    use continuum_effects_storage::profile::{PROFILE_NAME, UNSUPPORTED};

    let declared: Vec<PackCase> = UNSUPPORTED
        .iter()
        .map(|case| {
            PackCase::new(PROFILE_NAME, case.semantic.token())
                .expect("the pack's own tokens fit the pack-case grammar")
        })
        .collect();
    let tx = applied(GateProfile::PhaseB);
    let with_pack = compose(&tx, &World::new(&tx, Vec::new(), declared.clone())).unwrap();
    let without = compose(&tx, &World::new(&tx, Vec::new(), Vec::new())).unwrap();

    let pack_entries: Vec<&Unknown> = with_pack
        .unknowns()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == UnknownKind::UnsupportedPackCase)
        .collect();
    assert_eq!(pack_entries.len(), UNSUPPORTED.len());
    assert_eq!(
        with_pack.unknowns().entries().len(),
        without.unknowns().entries().len() + UNSUPPORTED.len()
    );
    for case in &UNSUPPORTED {
        let token = format!(
            "unsupported_pack_case:{PROFILE_NAME}:{}",
            case.semantic.token()
        );
        let matches = with_pack
            .unknowns()
            .tokens()
            .iter()
            .filter(|entry| **entry == token)
            .count();
        assert_eq!(matches, 1, "{token}");
        match Unknown::from_token(&token) {
            Some(Unknown::UnsupportedPackCase(read)) => {
                assert_eq!(read.pack(), PROFILE_NAME);
                assert_eq!(read.case(), case.semantic.token());
            }
            other => panic!("{token} read back as {other:?}"),
        }
    }
}
