//! PR-20 / IMPL-06 (bn-b6u4): the policy verdict.
//!
//! RFC 0032 makes the verdict a recomputation over the version's recorded gates under its
//! profile: a failed gate blocks, failure dominates inconclusiveness, a gate that is not
//! `passed` never supports promotion, gate 12 is the promotion step, and a protected
//! change routes through a privileged intent revision (INV-001, INV-011). This suite
//! checks the pure core ([`policy::decide`]) over every profile, the verdict bound to a
//! real exact-replay version, the typed INV-008 reasons its evidence records, the stale
//! and forged refusals, gate-order independence, and a property check against a
//! reference reading of RFC 0032 transcribed below from the RFC's tables and rules. The
//! reference shares the RFC's rule order with the crate, so it checks the transcription;
//! the per-gate negative tests pin each rule on its own.
//!
//! Stable artifact IDs are in each test's doc comment: `pr20-impl06-{pos,neg,bnd,met,prop}-NN`.

mod common;

use std::collections::BTreeMap;

use continuum_intent::change_policy::PolicyDecision;
use continuum_intent::contract::IntentId;
use continuum_repair::handle::{CrashpackId, EvidenceRef, SnapshotId};
use continuum_repair::hypothesis::{Hypothesis, Proposal};
use continuum_repair::policy::{
    self, ClaimRefusal, GateListRefusal, GateReading, GateReasons, IntentClassification, NoReasons,
    PolicyVerdict, ReplayRecords, StaleVerdict, Standing, Undecided, Verdict, VerdictClaim,
};
use continuum_repair::replay::{
    self, AuthorizationAbsence, Divergence, EvidenceRecord, RecordedRun, ReplayOutcome, ReplayRun,
    Replayer,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, GateStatus, RepairTransaction, Resolution,
};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_workspace::snapshot::WorkspaceContent;

use common::{ack_after_sync_change, base_content, base_id, repaired_content, snapshot_id};

type Tx = RepairTransaction<Blake3Hasher>;

const SCHEDULE: &[u8] = b"r0/i0/Reserve(0)#0\nr0/i0/Submit(0)#0\nr0/i0/Confirm(0)#0\n";
const JOURNAL: &[u8] = b"journal: ack v0; lose v0; ack v1";
const FAILURE: &[u8] = b"Agreement(41)";

// --- the register transaction ------------------------------------------------------------

fn intent() -> IntentId {
    IntentId::new("in_pr20_impl06_register").unwrap()
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

struct Binding;

impl FailureBinding for Binding {
    fn failure_base(&self, _: &CrashpackId) -> Resolution<SnapshotId> {
        Resolution::Found(base_id())
    }

    fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
        Resolution::Found(intent())
    }
}

fn applied(profile: GateProfile) -> Tx {
    let failure = CrashpackId::new(&format!(
        "crash_{}",
        Blake3Hasher::hash(&crashpack()).to_token()
    ))
    .unwrap();
    let draft = Tx::begin(failure, profile, &Binding).unwrap();
    let proposal = Proposal::new(
        Hypothesis::new("move the ack after the storage sync"),
        vec![ack_after_sync_change()],
    );
    draft
        .apply(&proposal, &base_content())
        .unwrap()
        .into_parts()
        .0
}

struct Scripted(BTreeMap<SnapshotId, ReplayOutcome>);

impl Scripted {
    fn new(base: ReplayOutcome, candidate: ReplayOutcome) -> Self {
        Self(BTreeMap::from([
            (base_id(), base),
            (snapshot_id(&repaired_content()), candidate),
        ]))
    }
}

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

/// Evaluate gates 1 and 4 of a fresh phase-b transaction.
fn evaluated(base: ReplayOutcome, candidate: ReplayOutcome) -> (Tx, Vec<EvidenceRecord>) {
    replay::evaluate(
        &applied(GateProfile::PhaseB),
        &crashpack(),
        &base_content(),
        &repaired_content(),
        &Scripted::new(base, candidate),
        None,
    )
    .unwrap()
    .into_parts()
}

fn pending(gates: &[GateName]) -> Vec<Undecided> {
    gates
        .iter()
        .map(|gate| Undecided {
            gate: *gate,
            standing: Standing::Pending,
        })
        .collect()
}

/// Phase-b gates 1–11 other than `except`, all pending.
fn phase_b_pending_except(except: &[GateName]) -> Vec<Undecided> {
    let gates: Vec<GateName> = GateName::ALL
        .into_iter()
        .filter(|gate| {
            GateProfile::PhaseB.enforces(*gate)
                && *gate != GateName::ReceiptGeneration
                && !except.contains(gate)
        })
        .collect();
    pending(&gates)
}

// --- pure-core readings ----------------------------------------------------------------

fn reading(gate: GateName, status: GateStatus) -> GateReading {
    GateReading {
        gate,
        status,
        reason: Resolution::Unknown,
    }
}

/// Every gate of `profile` passed (gate 12 `pending`, as on a `ready` transaction), and
/// every gate outside it `not_yet_enforced`.
fn all_passed(profile: GateProfile) -> Vec<GateReading> {
    GateName::ALL
        .into_iter()
        .map(|gate| {
            let status = if !profile.enforces(gate) {
                GateStatus::NotYetEnforced
            } else if gate == GateName::ReceiptGeneration {
                GateStatus::Pending
            } else {
                GateStatus::Passed
            };
            reading(gate, status)
        })
        .collect()
}

fn with(mut gates: Vec<GateReading>, gate: GateName, status: GateStatus) -> Vec<GateReading> {
    for entry in &mut gates {
        if entry.gate == gate {
            entry.status = status;
        }
    }
    gates
}

/// Gates 1–11 of `profile`: the gates whose status decides eligibility.
fn deciding(profile: GateProfile) -> Vec<GateName> {
    GateName::ALL
        .into_iter()
        .filter(|gate| profile.enforces(*gate) && *gate != GateName::ReceiptGeneration)
        .collect()
}

// --- positive ----------------------------------------------------------------------------

/// `pr20-impl06-pos-01`: every required gate passed is promote-eligible, `allow`, under
/// every profile, with gate 12 `pending` (a `ready` transaction) or `passed`.
#[test]
fn pr20_impl06_positive_all_required_gates_passed_is_promote_eligible() {
    for profile in GateProfile::ALL {
        let ready = all_passed(profile);
        let verdict = policy::decide(profile, &ready, None).unwrap();
        assert_eq!(verdict, Verdict::PromoteEligible, "{profile:?}");
        assert_eq!(verdict.policy_decision(), Some(PolicyDecision::Allow));

        let composed = with(ready, GateName::ReceiptGeneration, GateStatus::Passed);
        assert_eq!(
            policy::decide(profile, &composed, None),
            Ok(Verdict::PromoteEligible),
            "{profile:?}: gate 12 passed"
        );
        // An `allow` classification changes nothing.
        assert_eq!(
            policy::decide(profile, &composed, Some(PolicyDecision::Allow)),
            Ok(Verdict::PromoteEligible)
        );
    }
}

/// `pr20-impl06-pos-02`: on a real version whose gates 1 and 4 passed exact replay, the
/// verdict is bound to that version and is not eligible: the rest of the campaign is
/// pending, and each pending gate is named.
#[test]
fn pr20_impl06_positive_a_real_exact_replay_version_is_bound_and_names_its_pending_gates() {
    let (tx, records) = evaluated(exact(), clean(Vec::new(), 0));
    let verdict =
        PolicyVerdict::compute(&tx, &ReplayRecords::<Blake3Hasher>::new(&records), None).unwrap();
    assert_eq!(verdict.repair_id(), tx.repair_id());
    assert_eq!(verdict.gate_profile(), GateProfile::PhaseB);
    verdict.check_current(&tx).unwrap();
    assert_eq!(
        verdict.verdict(),
        &Verdict::Inconclusive {
            undecided: phase_b_pending_except(&[GateName::BaseReplay, GateName::ExactRegression]),
        }
    );
    assert_eq!(verdict.verdict().policy_decision(), None);
}

// --- negative ----------------------------------------------------------------------------

/// `pr20-impl06-neg-01`: under every profile, each failed in-profile gate blocks with that
/// gate named; a failed gate 3 is a privileged intent revision.
#[test]
fn pr20_impl06_negative_each_failed_gate_blocks() {
    for profile in GateProfile::ALL {
        for gate in GateName::ALL
            .into_iter()
            .filter(|gate| profile.enforces(*gate))
        {
            let gates = with(all_passed(profile), gate, GateStatus::Failed);
            let verdict = policy::decide(profile, &gates, None).unwrap();
            let expected = if gate == GateName::IntentIntegrity {
                Verdict::PrivilegedIntentRevisionRequired {
                    classification: None,
                    failed: vec![gate],
                    undecided: Vec::new(),
                }
            } else {
                Verdict::Blocked {
                    failed: vec![gate],
                    undecided: Vec::new(),
                }
            };
            assert_eq!(verdict, expected, "{profile:?} {gate:?}");
            assert_eq!(verdict.policy_decision(), Some(PolicyDecision::Block));
        }
    }
}

/// `pr20-impl06-neg-02`: each pending gate 1–11 of the profile is never eligible and is
/// named; gate 12 `pending` alone is the promotion step, not a blocker.
#[test]
fn pr20_impl06_negative_each_pending_gate_blocks_eligibility() {
    for profile in GateProfile::ALL {
        for gate in deciding(profile) {
            let gates = with(all_passed(profile), gate, GateStatus::Pending);
            assert_eq!(
                policy::decide(profile, &gates, None),
                Ok(Verdict::Inconclusive {
                    undecided: pending(&[gate]),
                }),
                "{profile:?} {gate:?}"
            );
        }
    }
}

/// `pr20-impl06-neg-03`: each inconclusive gate is never eligible, and the verdict carries
/// the exact typed reason read, including the typed absence of one.
#[test]
fn pr20_impl06_negative_each_inconclusive_gate_blocks_with_its_reason() {
    let reasons = InconclusiveReason::ALL
        .into_iter()
        .map(Resolution::Found)
        .chain([Resolution::Unknown, Resolution::Unavailable]);
    for profile in GateProfile::ALL {
        for gate in GateName::ALL
            .into_iter()
            .filter(|gate| profile.enforces(*gate))
        {
            for reason in reasons.clone() {
                let mut gates = with(all_passed(profile), gate, GateStatus::Inconclusive);
                gates[GateName::ALL.iter().position(|g| *g == gate).unwrap()].reason =
                    reason.clone();
                assert_eq!(
                    policy::decide(profile, &gates, None),
                    Ok(Verdict::Inconclusive {
                        undecided: vec![Undecided {
                            gate,
                            standing: Standing::Inconclusive(reason.clone()),
                        }],
                    }),
                    "{profile:?} {gate:?} {reason:?}"
                );
            }
        }
    }
}

/// `pr20-impl06-neg-04`: failure dominates inconclusiveness (RFC 0032 rules 6, 7), and the
/// undecided gates are still disclosed beside the failure.
#[test]
fn pr20_impl06_negative_failure_dominates_and_undecided_gates_are_disclosed() {
    let profile = GateProfile::Default;
    let mut gates = with(
        all_passed(profile),
        GateName::Neighborhood,
        GateStatus::Failed,
    );
    gates = with(gates, GateName::ExactRegression, GateStatus::Inconclusive);
    gates[3].reason = Resolution::Found(InconclusiveReason::AbstractionAmbiguity);
    gates = with(gates, GateName::CodeAndSecurity, GateStatus::Pending);
    gates = with(gates, GateName::BaseReplay, GateStatus::Failed);
    assert_eq!(
        policy::decide(profile, &gates, None),
        Ok(Verdict::Blocked {
            failed: vec![GateName::BaseReplay, GateName::Neighborhood],
            undecided: vec![
                Undecided {
                    gate: GateName::ExactRegression,
                    standing: Standing::Inconclusive(Resolution::Found(
                        InconclusiveReason::AbstractionAmbiguity
                    )),
                },
                Undecided {
                    gate: GateName::CodeAndSecurity,
                    standing: Standing::Pending,
                },
            ],
        })
    );
}

/// `pr20-impl06-neg-05`: a protected change forces the privileged path whatever the other
/// gates say: a failed gate 3, or a bound RFC 0031 `review` or `block`, even with every
/// gate passed. An `allow` classification overrides no recorded status.
#[test]
fn pr20_impl06_negative_a_protected_change_forces_the_privileged_path() {
    let profile = GateProfile::PhaseB;
    for decision in [PolicyDecision::Review, PolicyDecision::Block] {
        let verdict = policy::decide(profile, &all_passed(profile), Some(decision)).unwrap();
        assert_eq!(
            verdict,
            Verdict::PrivilegedIntentRevisionRequired {
                classification: Some(decision),
                failed: Vec::new(),
                undecided: Vec::new(),
            }
        );
        assert_eq!(verdict.policy_decision(), Some(decision));
    }
    // Gate 3 failed, gate 5 failed, gate 6 pending: privileged, with everything named.
    let mut gates = with(
        all_passed(profile),
        GateName::IntentIntegrity,
        GateStatus::Failed,
    );
    gates = with(gates, GateName::Neighborhood, GateStatus::Failed);
    gates = with(gates, GateName::PropertyMutation, GateStatus::Pending);
    for classification in [None, Some(PolicyDecision::Allow)] {
        assert_eq!(
            policy::decide(profile, &gates, classification),
            Ok(Verdict::PrivilegedIntentRevisionRequired {
                classification: None,
                failed: vec![GateName::IntentIntegrity, GateName::Neighborhood],
                undecided: pending(&[GateName::PropertyMutation]),
            }),
            "{classification:?}"
        );
    }
    // `allow` does not pass a pending gate 3.
    let gates = with(
        all_passed(profile),
        GateName::IntentIntegrity,
        GateStatus::Pending,
    );
    assert_eq!(
        policy::decide(profile, &gates, Some(PolicyDecision::Allow)),
        Ok(Verdict::Inconclusive {
            undecided: pending(&[GateName::IntentIntegrity]),
        })
    );

    // On a real version: a bound `block` classification of the ack-after-sync candidate.
    let (tx, records) = evaluated(exact(), clean(Vec::new(), 0));
    let classification = IntentClassification::new(
        tx.base_snapshot().clone(),
        tx.candidate_snapshot().unwrap().clone(),
        tx.base_intent().clone(),
        PolicyDecision::Block,
    );
    let verdict = PolicyVerdict::compute(
        &tx,
        &ReplayRecords::<Blake3Hasher>::new(&records),
        Some(&classification),
    )
    .unwrap();
    assert!(matches!(
        verdict.verdict(),
        Verdict::PrivilegedIntentRevisionRequired {
            classification: Some(PolicyDecision::Block),
            ..
        }
    ));
}

/// `pr20-impl06-neg-06`: the ack-after-sync candidate drops and reorders recorded steps,
/// so under `Exact` gate 4 is inconclusive; the verdict reads its typed reason,
/// `AbstractionAmbiguity`, from the record the gate cites. Without the records the reason
/// is the typed `Unavailable`; a record of another gate states nothing about gate 4.
#[test]
fn pr20_impl06_negative_real_inconclusive_replay_preserves_its_typed_reason() {
    let (tx, records) = evaluated(exact(), clean(vec![2, 5], 1));
    let expected = |reason| {
        let mut undecided =
            phase_b_pending_except(&[GateName::BaseReplay, GateName::ExactRegression]);
        undecided.insert(
            2,
            Undecided {
                gate: GateName::ExactRegression,
                standing: Standing::Inconclusive(reason),
            },
        );
        Verdict::Inconclusive { undecided }
    };
    let read = ReplayRecords::<Blake3Hasher>::new(&records);
    let verdict = PolicyVerdict::compute(&tx, &read, None).unwrap();
    assert_eq!(
        verdict.verdict(),
        &expected(Resolution::Found(InconclusiveReason::AbstractionAmbiguity))
    );
    assert_eq!(
        PolicyVerdict::compute(&tx, &NoReasons, None)
            .unwrap()
            .verdict(),
        &expected(Resolution::Unavailable)
    );
    assert_eq!(
        PolicyVerdict::compute(&tx, &ReplayRecords::<Blake3Hasher>::new(&[]), None)
            .unwrap()
            .verdict(),
        &expected(Resolution::Unavailable)
    );

    // Gate 1's record, cited as if it were gate 4's: it states gate 1, so nothing.
    let base_evidence: Vec<EvidenceRef> = tx.gates()[0].evidence().to_vec();
    assert_eq!(
        read.inconclusive_reason(GateName::ExactRegression, &base_evidence),
        Resolution::Unknown
    );

    // A base divergence: gate 1 inconclusive, `EngineError`, read from its defect_ record.
    let (tx, records) = evaluated(
        ReplayOutcome::Diverged(Divergence { at: 2 }),
        clean(Vec::new(), 0),
    );
    let verdict =
        PolicyVerdict::compute(&tx, &ReplayRecords::<Blake3Hasher>::new(&records), None).unwrap();
    let mut undecided = phase_b_pending_except(&[]);
    undecided[0] = Undecided {
        gate: GateName::BaseReplay,
        standing: Standing::Inconclusive(Resolution::Found(InconclusiveReason::EngineError)),
    };
    assert_eq!(verdict.verdict(), &Verdict::Inconclusive { undecided });
}

/// `pr20-impl06-neg-07`: a base on which the failure does not reproduce fails gate 1, and
/// the verdict blocks with gate 1 named and the unevaluated gates disclosed.
#[test]
fn pr20_impl06_negative_a_real_gate_1_failure_blocks() {
    let (tx, records) = evaluated(clean(Vec::new(), 0), clean(Vec::new(), 0));
    let verdict =
        PolicyVerdict::compute(&tx, &ReplayRecords::<Blake3Hasher>::new(&records), None).unwrap();
    assert_eq!(
        verdict.verdict(),
        &Verdict::Blocked {
            failed: vec![GateName::BaseReplay],
            undecided: phase_b_pending_except(&[GateName::BaseReplay]),
        }
    );
    assert_eq!(
        verdict.verdict().policy_decision(),
        Some(PolicyDecision::Block)
    );
}

/// `pr20-impl06-neg-08`: a verdict is the version's: after the next version is recorded
/// it is stale, and a claim naming the old version is refused before anything is
/// recomputed.
#[test]
fn pr20_impl06_negative_a_stale_version_is_refused() {
    let before = applied(GateProfile::PhaseB);
    let verdict = PolicyVerdict::compute(&before, &NoReasons, None).unwrap();
    let (after, _) = replay::evaluate(
        &before,
        &crashpack(),
        &base_content(),
        &repaired_content(),
        &Scripted::new(exact(), clean(Vec::new(), 0)),
        None,
    )
    .unwrap()
    .into_parts();
    assert_eq!(
        verdict.check_current(&after),
        Err(StaleVerdict {
            verdict_for: before.repair_id().clone(),
            presented: after.repair_id().clone(),
        })
    );
    verdict.check_current(&before).unwrap();
    for decision in PolicyDecision::ALL {
        let claim = VerdictClaim::new(before.repair_id().clone(), decision);
        assert!(matches!(
            PolicyVerdict::recheck(&claim, &after, &NoReasons, None),
            Err(ClaimRefusal::Stale(_))
        ));
    }
    // Another transaction's verdict is stale for this one too.
    let other = applied(GateProfile::PhaseC);
    assert!(verdict.check_current(&other).is_err());
}

/// `pr20-impl06-neg-09`: a forged verdict claim is refused by recomputation: `allow` on an
/// inconclusive version, `allow` or `review` on a blocked one, any decision on an
/// inconclusive one. The recomputed verdict comes back with its reasons.
#[test]
fn pr20_impl06_negative_a_forged_verdict_claim_is_refused_by_recompute() {
    let (tx, records) = evaluated(exact(), clean(vec![1], 0));
    let read = ReplayRecords::<Blake3Hasher>::new(&records);
    for decision in PolicyDecision::ALL {
        let claim = VerdictClaim::new(tx.repair_id().clone(), decision);
        let Err(ClaimRefusal::Disagrees {
            claimed,
            recomputed,
        }) = PolicyVerdict::recheck(&claim, &tx, &read, None)
        else {
            panic!("{decision:?} on an inconclusive version must be refused");
        };
        assert_eq!(claimed, decision);
        assert!(matches!(recomputed.verdict(), Verdict::Inconclusive { .. }));
        assert_eq!(recomputed.repair_id(), tx.repair_id());
    }

    let (blocked, records) = evaluated(clean(Vec::new(), 0), clean(Vec::new(), 0));
    let read = ReplayRecords::<Blake3Hasher>::new(&records);
    for decision in [PolicyDecision::Allow, PolicyDecision::Review] {
        let claim = VerdictClaim::new(blocked.repair_id().clone(), decision);
        assert!(matches!(
            PolicyVerdict::recheck(&claim, &blocked, &read, None),
            Err(ClaimRefusal::Disagrees { .. })
        ));
    }
    // The true decision is returned as the recomputed verdict, never as the claim.
    let honest = VerdictClaim::new(blocked.repair_id().clone(), PolicyDecision::Block);
    let recomputed = PolicyVerdict::recheck(&honest, &blocked, &read, None).unwrap();
    assert_eq!(
        recomputed,
        PolicyVerdict::compute(&blocked, &read, None).unwrap()
    );
}

/// `pr20-impl06-neg-10`: `ContinueAndDisclose` has no authority: the policy layer, the only
/// issuer, answers `NoAuthority`, and so does the replay seam that delegates to it.
#[test]
fn pr20_impl06_negative_no_authority_issues_continue_and_disclose() {
    let tx = applied(GateProfile::PhaseB);
    assert_eq!(
        policy::authorize_continue_and_disclose(&tx, None),
        Err(AuthorizationAbsence::NoAuthority)
    );
    assert_eq!(
        replay::request_continue_and_disclose(&tx),
        Err(AuthorizationAbsence::NoAuthority)
    );
}

// --- boundary ----------------------------------------------------------------------------

/// `pr20-impl06-bnd-01`: every gate profile. A gate outside the profile must be
/// `not_yet_enforced` and never counts as passed; a gate inside it must not be. Either
/// violation is refused, never read.
#[test]
fn pr20_impl06_boundary_every_gate_profile_admits_exactly_its_gates() {
    let unenforced = |profile: GateProfile| -> Vec<GateName> {
        GateName::ALL
            .into_iter()
            .filter(|gate| !profile.enforces(*gate))
            .collect()
    };
    assert_eq!(
        unenforced(GateProfile::PhaseB),
        [GateName::CertificateRebuild, GateName::IncrementalParity]
    );
    assert_eq!(
        unenforced(GateProfile::PhaseC),
        [GateName::CertificateRebuild]
    );
    assert!(unenforced(GateProfile::PhaseD).is_empty());
    assert!(unenforced(GateProfile::Default).is_empty());

    for profile in GateProfile::ALL {
        for gate in GateName::ALL {
            for status in GateStatus::ALL {
                let gates = with(all_passed(profile), gate, status);
                let result = policy::decide(profile, &gates, None);
                let violates = profile.enforces(gate) == (status == GateStatus::NotYetEnforced);
                if violates {
                    assert_eq!(
                        result,
                        Err(GateListRefusal::ProfileViolation { gate, status }),
                        "{profile:?} {gate:?} {status:?}"
                    );
                }
            }
        }
    }

    // The real phase-b version lists gates 9 and 10 not_yet_enforced, and they never
    // appear among the undecided gates.
    let verdict = PolicyVerdict::compute(&applied(GateProfile::PhaseB), &NoReasons, None).unwrap();
    let Verdict::Inconclusive { undecided } = verdict.verdict() else {
        panic!("{verdict:?}");
    };
    assert!(undecided.iter().all(|entry| {
        entry.gate != GateName::CertificateRebuild
            && entry.gate != GateName::IncrementalParity
            && entry.gate != GateName::ReceiptGeneration
    }));
}

/// `pr20-impl06-bnd-02`: a list that is not the twelve gates by identity is refused: too
/// many entries (before any entry is read), a duplicate, a missing gate, an empty list.
#[test]
fn pr20_impl06_boundary_the_gate_list_is_the_twelve_gates_by_identity() {
    let profile = GateProfile::Default;
    let mut thirteen = all_passed(profile);
    thirteen.push(reading(GateName::BaseReplay, GateStatus::Passed));
    assert_eq!(
        policy::decide(profile, &thirteen, None),
        Err(GateListRefusal::Count { entries: 13 })
    );

    let mut duplicate = all_passed(profile);
    duplicate[11] = reading(GateName::Neighborhood, GateStatus::Passed);
    assert_eq!(
        policy::decide(profile, &duplicate, None),
        Err(GateListRefusal::Duplicate {
            gate: GateName::Neighborhood
        })
    );

    let mut missing = all_passed(profile);
    missing.remove(7);
    assert_eq!(
        policy::decide(profile, &missing, None),
        Err(GateListRefusal::Missing {
            gate: GateName::RefinementCoverage
        })
    );
    assert_eq!(
        policy::decide(profile, &[], None),
        Err(GateListRefusal::Missing {
            gate: GateName::BaseReplay
        })
    );
}

// --- metamorphic -------------------------------------------------------------------------

/// `pr20-impl06-met-01`, relation "set/map insertion order": the gate list is a set
/// keyed by gate, so its insertion order does not matter: every rotation, the reversal, and
/// seeded shuffles of one list give one verdict, whose lists are in gate-number order.
#[test]
fn pr20_impl06_metamorphic_gate_order_is_irrelevant() {
    let profile = GateProfile::PhaseC;
    let mut gates = with(
        all_passed(profile),
        GateName::DefectMutants,
        GateStatus::Failed,
    );
    gates = with(gates, GateName::BaseReplay, GateStatus::Inconclusive);
    gates[0].reason = Resolution::Found(InconclusiveReason::EngineError);
    gates = with(gates, GateName::CodeAndSecurity, GateStatus::Pending);
    gates = with(gates, GateName::PatchApplication, GateStatus::Failed);
    let expected = policy::decide(profile, &gates, None).unwrap();
    assert_eq!(
        expected,
        Verdict::Blocked {
            failed: vec![GateName::PatchApplication, GateName::DefectMutants],
            undecided: vec![
                Undecided {
                    gate: GateName::BaseReplay,
                    standing: Standing::Inconclusive(Resolution::Found(
                        InconclusiveReason::EngineError
                    )),
                },
                Undecided {
                    gate: GateName::CodeAndSecurity,
                    standing: Standing::Pending,
                },
            ],
        }
    );
    for shift in 0..gates.len() {
        let mut rotated = gates.clone();
        rotated.rotate_left(shift);
        assert_eq!(
            policy::decide(profile, &rotated, None),
            Ok(expected.clone())
        );
    }
    let mut reversed = gates.clone();
    reversed.reverse();
    assert_eq!(
        policy::decide(profile, &reversed, None),
        Ok(expected.clone())
    );
    let mut rng = SplitMix(0x5eed_0006);
    for _ in 0..500 {
        let shuffled = rng.shuffled(&gates);
        assert_eq!(
            policy::decide(profile, &shuffled, None),
            Ok(expected.clone())
        );
    }
}

// --- property: an independent reference reading of RFC 0032 ------------------------------

/// A seeded generator (SplitMix64): deterministic, no ambient entropy (INV-005).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap()).unwrap()
    }

    fn shuffled<T: Clone>(&mut self, items: &[T]) -> Vec<T> {
        let mut out = items.to_vec();
        for i in (1..out.len()).rev() {
            let j = self.below(i + 1);
            out.swap(i, j);
        }
        out
    }
}

/// RFC 0032's "Gate profiles" table, by gate number, transcribed from the RFC:
/// `phase-b` enforces 1–8, 11, 12; `phase-c` 1–8, 10, 11, 12; `phase-d` and `default` all.
fn reference_enforced(profile: GateProfile, number: usize) -> bool {
    match profile {
        GateProfile::PhaseB => !matches!(number, 9 | 10),
        GateProfile::PhaseC => number != 9,
        GateProfile::PhaseD | GateProfile::Default => true,
    }
}

/// The reference outcome: what RFC 0032 says, stated as the gate numbers in each class.
#[derive(Debug, PartialEq, Eq)]
enum Reference {
    Refused,
    Eligible,
    Privileged {
        failed: Vec<usize>,
        undecided: Vec<(usize, Option<Resolution<InconclusiveReason>>)>,
    },
    Blocked {
        failed: Vec<usize>,
        undecided: Vec<(usize, Option<Resolution<InconclusiveReason>>)>,
    },
    Inconclusive {
        undecided: Vec<(usize, Option<Resolution<InconclusiveReason>>)>,
    },
}

/// RFC 0032 read directly, over statuses indexed by gate number 1–12:
///
/// - "Gate profiles": an out-of-profile gate MUST be `not_yet_enforced`, an in-profile
///   gate MUST NOT be — otherwise the list is not admissible;
/// - "Status machine" rule 6 before rule 7: any failed gate blocks, else any
///   inconclusive gate, else any pending gate 1–11 (a campaign not complete), else ready;
///   gate 12 is the promotion step and its `pending` is not a precondition;
/// - "Intent integrity and reclassification": a detected protected change (gate 3 failed,
///   or an RFC 0031 decision other than `allow`) is blocked and only a privileged
///   reclassification moves it.
fn reference(
    profile: GateProfile,
    by_number: &BTreeMap<usize, (GateStatus, Resolution<InconclusiveReason>)>,
    classification: Option<PolicyDecision>,
) -> Reference {
    if by_number.len() != 12 || (1..=12).any(|n| !by_number.contains_key(&n)) {
        return Reference::Refused;
    }
    for (number, (status, _)) in by_number {
        let unenforced = *status == GateStatus::NotYetEnforced;
        if reference_enforced(profile, *number) == unenforced {
            return Reference::Refused;
        }
    }
    let failed: Vec<usize> = by_number
        .iter()
        .filter(|(_, (status, _))| *status == GateStatus::Failed)
        .map(|(number, _)| *number)
        .collect();
    let undecided: Vec<(usize, Option<Resolution<InconclusiveReason>>)> = by_number
        .iter()
        .filter_map(|(number, (status, reason))| match status {
            GateStatus::Inconclusive => Some((*number, Some(reason.clone()))),
            GateStatus::Pending if *number != 12 => Some((*number, None)),
            _ => None,
        })
        .collect();
    let protected = failed.contains(&3)
        || matches!(
            classification,
            Some(PolicyDecision::Review | PolicyDecision::Block)
        );
    if protected {
        Reference::Privileged { failed, undecided }
    } else if !failed.is_empty() {
        Reference::Blocked { failed, undecided }
    } else if !undecided.is_empty() {
        Reference::Inconclusive { undecided }
    } else {
        Reference::Eligible
    }
}

/// The decision, stated from the RFCs rather than from the verdict's shape: RFC 0031's
/// `allow` only for a promotable transaction and no `unknown`; a refuted gate is `block`
/// (RFC 0032 rule 6); a protected change with nothing refuted takes RFC 0031's decision.
fn reference_decision(
    reference: &Reference,
    classification: Option<PolicyDecision>,
) -> Option<PolicyDecision> {
    match reference {
        Reference::Refused | Reference::Inconclusive { .. } => None,
        Reference::Eligible => Some(PolicyDecision::Allow),
        Reference::Blocked { .. } => Some(PolicyDecision::Block),
        Reference::Privileged { failed, .. } if !failed.is_empty() => Some(PolicyDecision::Block),
        Reference::Privileged { .. } => match classification {
            Some(PolicyDecision::Review) => Some(PolicyDecision::Review),
            _ => Some(PolicyDecision::Block),
        },
    }
}

fn number(gate: GateName) -> usize {
    GateName::ALL.iter().position(|g| *g == gate).unwrap() + 1
}

fn as_reference(verdict: Result<Verdict, GateListRefusal>) -> Reference {
    let undecided = |list: Vec<Undecided>| {
        list.into_iter()
            .map(|entry| {
                let reason = match entry.standing {
                    Standing::Pending => None,
                    Standing::Inconclusive(reason) => Some(reason),
                };
                (number(entry.gate), reason)
            })
            .collect()
    };
    let numbers = |list: Vec<GateName>| list.into_iter().map(number).collect();
    match verdict {
        Err(_) => Reference::Refused,
        Ok(Verdict::PromoteEligible) => Reference::Eligible,
        Ok(Verdict::PrivilegedIntentRevisionRequired {
            failed,
            undecided: u,
            ..
        }) => Reference::Privileged {
            failed: numbers(failed),
            undecided: undecided(u),
        },
        Ok(Verdict::Blocked {
            failed,
            undecided: u,
        }) => Reference::Blocked {
            failed: numbers(failed),
            undecided: undecided(u),
        },
        Ok(Verdict::Inconclusive { undecided: u }) => Reference::Inconclusive {
            undecided: undecided(u),
        },
    }
}

/// `pr20-impl06-prop-01`: over 60,000 seeded random gate-status vectors — every profile,
/// every status on every gate (so most vectors are inadmissible under their profile and
/// a third are biased admissible), random reasons, random classifications, and random
/// entry order — the crate's verdict equals the reference reading of RFC 0032, and its
/// decision equals the one the reference states from the RFCs (a refuted gate is
/// `block`, whatever the classification).
#[test]
fn pr20_impl06_property_random_gate_vectors_agree_with_the_rfc_reference() {
    let reasons: Vec<Resolution<InconclusiveReason>> = InconclusiveReason::ALL
        .into_iter()
        .map(Resolution::Found)
        .chain([Resolution::Unknown, Resolution::Unavailable])
        .collect();
    let mut rng = SplitMix(0x0bad_5eed_b6a4_0006);
    let mut seen = BTreeMap::<&str, usize>::new();
    for round in 0..60_000 {
        let profile = GateProfile::ALL[rng.below(4)];
        let admissible = round % 3 != 0;
        let mut by_number = BTreeMap::new();
        let mut gates = Vec::new();
        for gate in GateName::ALL {
            let status = if admissible && !profile.enforces(gate) {
                GateStatus::NotYetEnforced
            } else if admissible {
                // Bias towards passed so that eligible vectors occur.
                [
                    GateStatus::Passed,
                    GateStatus::Passed,
                    GateStatus::Passed,
                    GateStatus::Passed,
                    GateStatus::Passed,
                    GateStatus::Passed,
                    GateStatus::Failed,
                    GateStatus::Pending,
                    GateStatus::Inconclusive,
                ][rng.below(9)]
            } else {
                GateStatus::ALL[rng.below(5)]
            };
            let reason = reasons[rng.below(reasons.len())].clone();
            by_number.insert(number(gate), (status, reason.clone()));
            gates.push(GateReading {
                gate,
                status,
                reason,
            });
        }
        let classification = [
            None,
            None,
            None,
            Some(PolicyDecision::Allow),
            Some(PolicyDecision::Review),
            Some(PolicyDecision::Block),
        ][rng.below(6)];
        let gates = rng.shuffled(&gates);
        let actual = policy::decide(profile, &gates, classification);
        let decision = actual.as_ref().ok().and_then(Verdict::policy_decision);
        let expected = reference(profile, &by_number, classification);
        assert_eq!(
            decision,
            reference_decision(&expected, classification),
            "round {round}"
        );
        let class = match &expected {
            Reference::Refused => "refused",
            Reference::Eligible => "eligible",
            Reference::Privileged { .. } => "privileged",
            Reference::Blocked { .. } => "blocked",
            Reference::Inconclusive { .. } => "inconclusive",
        };
        *seen.entry(class).or_default() += 1;
        assert_eq!(as_reference(actual), expected, "round {round}: {gates:?}");
    }
    // Every class of the reference was exercised, so the agreement is not vacuous.
    for class in [
        "refused",
        "eligible",
        "privileged",
        "blocked",
        "inconclusive",
    ] {
        assert!(
            seen.get(class).copied().unwrap_or(0) >= 100,
            "{class}: {seen:?}"
        );
    }
}

// --- differential: the decision against continuum-intent's RFC 0031 lattice join --------

/// `pr20-impl06-diff-01`: the verdict's wire decision equals the RFC 0031 P5 join, as
/// `continuum-intent`'s `PolicyDecision::join` computes it, of each decided contribution:
/// `block` for each failed gate (gate 3 included: a classification never lowers it), the
/// classification itself when it is not `allow`, and `allow` for a fully passed list. Where a
/// gate is undecided and nothing is failed or protected there is no decision.
#[test]
fn pr20_impl06_differential_the_decision_is_the_rfc_0031_join() {
    let mut rng = SplitMix(0xd1ff_0006);
    for round in 0..20_000 {
        let profile = GateProfile::ALL[rng.below(4)];
        let gates: Vec<GateReading> = GateName::ALL
            .into_iter()
            .map(|gate| {
                let status = if profile.enforces(gate) {
                    [
                        GateStatus::Passed,
                        GateStatus::Passed,
                        GateStatus::Passed,
                        GateStatus::Passed,
                        GateStatus::Failed,
                        GateStatus::Pending,
                        GateStatus::Inconclusive,
                    ][rng.below(7)]
                } else {
                    GateStatus::NotYetEnforced
                };
                reading(gate, status)
            })
            .collect();
        let classification = [
            None,
            Some(PolicyDecision::Allow),
            Some(PolicyDecision::Review),
            Some(PolicyDecision::Block),
        ][rng.below(4)];
        let verdict = policy::decide(profile, &gates, classification).unwrap();

        let mut contributions = Vec::new();
        for entry in &gates {
            if entry.status == GateStatus::Failed {
                // A failed gate is refuted, gate 3 included: `block`.
                contributions.push(PolicyDecision::Block);
            }
        }
        if let Some(decision) = classification.filter(|d| *d != PolicyDecision::Allow) {
            contributions.push(decision);
        }
        let undecided = gates.iter().any(|entry| {
            entry.status == GateStatus::Inconclusive
                || (entry.status == GateStatus::Pending
                    && entry.gate != GateName::ReceiptGeneration)
        });
        let oracle = match contributions.into_iter().reduce(PolicyDecision::join) {
            Some(joined) => Some(joined),
            None if undecided => None,
            None => Some(PolicyDecision::Allow),
        };
        assert_eq!(
            verdict.policy_decision(),
            oracle,
            "round {round}: {verdict:?}"
        );
    }
    // The cases the join decides: a `review` classification beside a failed gate, gate 3
    // or another, is `block`, never `review`.
    let profile = GateProfile::PhaseB;
    for failed in [GateName::Neighborhood, GateName::IntentIntegrity] {
        let gates = with(all_passed(profile), failed, GateStatus::Failed);
        assert_eq!(
            policy::decide(profile, &gates, Some(PolicyDecision::Review))
                .unwrap()
                .policy_decision(),
            Some(PolicyDecision::Block),
            "{failed:?}"
        );
    }
}
