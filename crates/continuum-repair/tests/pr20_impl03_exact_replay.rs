//! PR-20 / IMPL-03 evidence at the library seam: exact replay, gates 1 and 4 (bn-2pla).
//!
//! The normative sources are RFC 0032 ("The twelve gates", gates 1 and 4; "Gate status
//! semantics"; "Status machine") and `repair-transaction.schema.json`.
//!
//! This suite drives [`continuum_repair::replay::evaluate`] with a scripted [`Replayer`]
//! whose answers are fixed per program, so every outcome the gates can record is reached
//! on purpose, and it checks the crashpack's identity and grammar against untrusted
//! bytes. The real replicated register, with M01's recorded failure replayed through the
//! asupersync binding, is `crates/continuum-asupersync/tests/pr20_impl03_exact_replay.rs`.
//!
//! # Tests
//!
//! - positive `pr20_impl03_positive_*`: an exact base replay and a clean candidate pass
//!   both gates, cite the crashpack and an `ev_` run, and record a new version.
//! - negative `pr20_impl03_negative_*`: a base without the recorded failure (an empty
//!   failure identity included) fails gate 1 and leaves gate 4 pending without running
//!   the candidate; a
//!   recurring or different failure fails gate 4; a tampered, malformed or foreign
//!   crashpack, a wrong base or candidate, a draft and a second evaluation are refused
//!   and record nothing.
//! - boundary `pr20_impl03_boundary_*`: a divergence is inconclusive, never passed (on
//!   the base with a `defect_`); a base that reproduces the failure but not the run is
//!   an engine condition with a `defect_`; an unsupported program is inconclusive; the record
//!   grammar refuses every truncation and trailing byte; the result is deterministic.
//! - metamorphic `pr20_impl03_metamorphic_*`: serialization round trip.
//! - differential `pr20_impl03_differential_*`: the crashpack identity and the evidence
//!   pattern against independent constructions.

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;

use continuum_intent::contract::IntentId;
use continuum_repair::handle::{CrashpackId, EvidenceRef, SnapshotId};
use continuum_repair::hypothesis::{Hypothesis, Proposal};
use continuum_repair::replay::{
    self, BaseReplay, Divergence, EvaluationPolicy, ExactRegression, ExactReplay,
    ExactReplayRefusal, FailureMatch, MalformedRecord, RecordedRun, ReplayOutcome, ReplayRun,
    Replayer,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, GateStatus, RepairTransaction, Resolution,
    TransactionStatus,
};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use continuum_workspace::snapshot::WorkspaceContent;

use common::{
    MANIFEST, MANIFEST_PATH, MODEL, MODEL_PATH, REPLICA, REPLICA_AFTER, ack_after_sync_change,
    base_content, base_id, content, repaired_content, snapshot_id,
};

type Tx = RepairTransaction<Blake3Hasher>;

const SCHEDULE: &[u8] = b"r0/i0/Reserve(0)#0\nr0/i0/Submit(0)#0\nr0/i0/Confirm(0)#0\n";
const JOURNAL: &[u8] = b"journal: ack v0; lose v0; ack v1";
const FAILURE: &[u8] = b"Agreement(41)";

fn intent() -> IntentId {
    IntentId::new("in_pr20_impl03_register").unwrap()
}

/// The register's recorded failure: on the base, under [`SCHEDULE`].
fn recorded() -> RecordedRun {
    RecordedRun::new(
        base_id(),
        SCHEDULE.to_vec(),
        JOURNAL.to_vec(),
        FAILURE.to_vec(),
    )
    .unwrap()
}

/// Every crashpack a binding here resolves resolves to the register base.
struct Binding(Vec<CrashpackId>);

impl FailureBinding for Binding {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        if self.0.contains(failure) {
            Resolution::Found(base_id())
        } else {
            Resolution::Unknown
        }
    }

    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if *snapshot == base_id() {
            Resolution::Found(intent())
        } else {
            Resolution::Unknown
        }
    }
}

/// The applied ack-after-sync transaction on `crashpack`'s identity.
fn applied_on(crashpack: &[u8]) -> Tx {
    let failure = CrashpackId::new(&format!(
        "crash_{}",
        Blake3Hasher::hash(crashpack).to_token()
    ))
    .unwrap();
    let draft = Tx::begin(
        failure.clone(),
        GateProfile::PhaseB,
        &Binding(vec![failure]),
    )
    .unwrap();
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

fn applied() -> Tx {
    applied_on(&recorded().canonical_bytes())
}

/// A replayer whose answer is fixed per program (keyed by the program's snapshot
/// identity), which checks it is handed the recorded schedule and counts its calls.
struct Scripted {
    answers: BTreeMap<SnapshotId, ReplayOutcome>,
    calls: RefCell<Vec<SnapshotId>>,
}

impl Scripted {
    fn new(base: ReplayOutcome, candidate: ReplayOutcome) -> Self {
        Self {
            answers: BTreeMap::from([
                (base_id(), base),
                (snapshot_id(&repaired_content()), candidate),
            ]),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl Replayer for Scripted {
    fn identity(&self) -> &str {
        "scripted/1"
    }

    fn replay(&self, program: &WorkspaceContent, schedule: &[u8]) -> ReplayOutcome {
        assert_eq!(
            schedule, SCHEDULE,
            "the replayer is handed the recorded schedule"
        );
        let id = snapshot_id(program);
        self.calls.borrow_mut().push(id.clone());
        self.answers[&id].clone()
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

fn clean(eliminated: u64) -> ReplayOutcome {
    ReplayOutcome::Ran(ReplayRun::new(
        b"journal: ack v1".to_vec(),
        None,
        (0..eliminated).map(|i| 2 * i).collect(),
        u64::from(eliminated > 0),
    ))
}

fn run(
    transaction: &Tx,
    replayer: &Scripted,
) -> Result<ExactReplay<Blake3Hasher>, ExactReplayRefusal> {
    replay::evaluate(
        transaction,
        &recorded().canonical_bytes(),
        &base_content(),
        &repaired_content(),
        replayer,
        None,
    )
}

/// Whether `haystack` contains `needle` as a contiguous byte run.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn gate(transaction: &Tx, name: GateName) -> (GateStatus, Vec<String>) {
    let gate = transaction
        .gates()
        .iter()
        .find(|gate| gate.name() == name)
        .unwrap();
    (
        gate.status(),
        gate.evidence()
            .iter()
            .map(|e| e.as_str().to_owned())
            .collect(),
    )
}

fn crash_ref(transaction: &Tx) -> String {
    transaction.failure().as_str().to_owned()
}

// --- positive ------------------------------------------------------------------------

#[test]
fn pr20_impl03_positive_exact_base_and_clean_candidate_pass_both_gates() {
    let before = applied();
    let replayer = Scripted::new(exact(), clean(0));
    let result = run(&before, &replayer).unwrap();
    let after = result.transaction();
    assert_eq!(
        after.evaluation_policy(),
        Some("exact-replay/exact/1"),
        "with no authorization the policy is exact, and it is part of the version"
    );
    assert_eq!(before.evaluation_policy(), None);

    let BaseReplay::Reproduced { run: base_run } = result.base_replay() else {
        panic!("{:?}", result.base_replay());
    };
    let ExactRegression::NoLongerFails {
        run: candidate_run,
        eliminated,
        reordered,
    } = result.exact_regression()
    else {
        panic!("{:?}", result.exact_regression());
    };
    assert_eq!(
        (*eliminated, *reordered),
        (0, 0),
        "under exact, only a run that followed the recording passes"
    );
    assert_eq!(
        gate(after, GateName::BaseReplay),
        (
            GateStatus::Passed,
            vec![crash_ref(after), base_run.as_str().to_owned()]
        )
    );
    assert_eq!(
        gate(after, GateName::ExactRegression),
        (
            GateStatus::Passed,
            vec![crash_ref(after), candidate_run.as_str().to_owned()]
        )
    );
    assert!(base_run.as_str().starts_with("ev_") && candidate_run.as_str().starts_with("ev_"));
    assert_ne!(base_run, candidate_run);
    let records: Vec<&EvidenceRef> = result.records().iter().map(|r| r.handle()).collect();
    assert_eq!(records, [base_run, candidate_run]);
    for record in result.records() {
        assert_eq!(
            record.handle().as_str(),
            format!("ev_{}", Blake3Hasher::hash(record.bytes()).to_token()),
            "each record is named by its bytes"
        );
    }

    // The other gates are untouched; the version advances and supersedes.
    for gate in after.gates() {
        if !matches!(
            gate.name(),
            GateName::BaseReplay | GateName::ExactRegression
        ) {
            let prior = before
                .gates()
                .iter()
                .find(|g| g.name() == gate.name())
                .unwrap();
            assert_eq!(gate, prior);
        }
    }
    assert_eq!(after.version(), before.version() + 1);
    assert_eq!(after.supersedes(), Some(before.repair_id()));
    assert_eq!(after.status(), TransactionStatus::Evaluating);
    assert_eq!(after.failure(), before.failure());
    assert_eq!(after.candidate_snapshot(), before.candidate_snapshot());
    assert_eq!(after.changes(), before.changes());
    assert_ne!(after.repair_id(), before.repair_id());
    assert_eq!(
        *replayer.calls.borrow(),
        [base_id(), snapshot_id(&repaired_content())]
    );

    // The artifact carries the evidence and the derived status.
    let artifact = String::from_utf8(after.to_artifact_bytes()).unwrap();
    assert!(artifact.contains(&format!(
        "{{\"evidence\":[\"{}\",\"{}\"],\"name\":\"base_replay\",\"status\":\"passed\"}}",
        crash_ref(after),
        base_run
    )));
    assert!(artifact.contains("\"status\":\"evaluating\""));
}

// --- negative: gate 1 ----------------------------------------------------------------

#[test]
fn pr20_impl03_negative_a_base_that_does_not_replay_the_exact_failure_fails_gate_1() {
    let cases = [
        (
            "no failure",
            ReplayRun::new(JOURNAL.to_vec(), None, Vec::new(), 0),
            FailureMatch::Absent,
        ),
        (
            "another failure",
            ReplayRun::new(
                JOURNAL.to_vec(),
                Some(b"AckedNotDurable(7)".to_vec()),
                Vec::new(),
                0,
            ),
            FailureMatch::Different,
        ),
        (
            "an empty failure identity is a failure, never no failure",
            ReplayRun::new(JOURNAL.to_vec(), Some(Vec::new()), Vec::new(), 0),
            FailureMatch::Different,
        ),
    ];
    for (why, base, failure) in cases {
        let replayer = Scripted::new(ReplayOutcome::Ran(base), clean(0));
        let result = run(&applied(), &replayer).unwrap();
        match result.base_replay() {
            BaseReplay::NotReproduced {
                journal_differs,
                failure: found,
                ..
            } => assert_eq!((*journal_differs, *found), (false, failure), "{why}"),
            other => panic!("{why}: {other:?}"),
        }
        assert_eq!(result.base_replay().status(), GateStatus::Failed, "{why}");
        assert_eq!(result.base_replay().inconclusive_reason(), None);
        assert_eq!(*result.exact_regression(), ExactRegression::NotRun, "{why}");
        let after = result.transaction();
        assert_eq!(gate(after, GateName::BaseReplay).0, GateStatus::Failed);
        assert_eq!(
            gate(after, GateName::ExactRegression),
            (GateStatus::Pending, Vec::new()),
            "{why}: gate 4 was not evaluated, so it stays pending with no evidence"
        );
        assert_eq!(
            *replayer.calls.borrow(),
            [base_id()],
            "{why}: the candidate is not run against a failure that did not reproduce"
        );
    }
}

/// The failure reproduces but the run does not: an engine condition on a verified base,
/// inconclusive with a `defect_`, never `failed` and never `passed`.
#[test]
fn pr20_impl03_boundary_an_inconsistent_base_replay_is_an_engine_condition() {
    let cases = [
        (
            "another journal",
            ReplayRun::new(
                b"journal: other".to_vec(),
                Some(FAILURE.to_vec()),
                Vec::new(),
                0,
            ),
            (true, 0, 0),
        ),
        (
            "a recorded step eliminated",
            ReplayRun::new(JOURNAL.to_vec(), Some(FAILURE.to_vec()), vec![1], 0),
            (false, 1, 0),
        ),
        (
            "a recorded step reordered",
            ReplayRun::new(JOURNAL.to_vec(), Some(FAILURE.to_vec()), Vec::new(), 2),
            (false, 0, 2),
        ),
    ];
    for (why, base, expected) in cases {
        let replayer = Scripted::new(ReplayOutcome::Ran(base), clean(0));
        let result = run(&applied(), &replayer).unwrap();
        let BaseReplay::Inconsistent {
            defect,
            journal_differs,
            eliminated,
            reordered,
        } = result.base_replay()
        else {
            panic!("{why}: {:?}", result.base_replay());
        };
        assert_eq!(
            (*journal_differs, *eliminated, *reordered),
            expected,
            "{why}"
        );
        assert!(defect.as_str().starts_with("defect_"), "{why}");
        assert_eq!(
            result.base_replay().inconclusive_reason(),
            Some(InconclusiveReason::EngineError)
        );
        let after = result.transaction();
        assert_eq!(
            gate(after, GateName::BaseReplay),
            (
                GateStatus::Inconclusive,
                vec![crash_ref(after), defect.as_str().to_owned()]
            ),
            "{why}"
        );
        assert_eq!(*result.exact_regression(), ExactRegression::NotRun);
        assert_eq!(
            gate(after, GateName::ExactRegression).0,
            GateStatus::Pending
        );
    }
}

// --- negative: gate 4 ----------------------------------------------------------------

#[test]
fn pr20_impl03_negative_a_repair_whose_failure_recurs_fails_gate_4() {
    let replayer = Scripted::new(exact(), exact());
    let result = run(&applied(), &replayer).unwrap();
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::Recurs { .. }
    ));
    assert_eq!(result.base_replay().status(), GateStatus::Passed);
    assert_eq!(
        gate(result.transaction(), GateName::ExactRegression).0,
        GateStatus::Failed
    );

    let other = ReplayOutcome::Ran(ReplayRun::new(
        b"journal: other".to_vec(),
        Some(b"Quiescence(3)".to_vec()),
        Vec::new(),
        0,
    ));
    let result = run(&applied(), &Scripted::new(exact(), other)).unwrap();
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::FailsDifferently { .. }
    ));
    assert_eq!(
        gate(result.transaction(), GateName::ExactRegression).0,
        GateStatus::Failed
    );
    // A failed gate does not make the transaction ready: the campaign is unfinished.
    assert_eq!(result.transaction().status(), TransactionStatus::Evaluating);
}

/// Under the fail-closed [`EvaluationPolicy::Exact`], a candidate that does not fail
/// but eliminated or reordered recorded steps does not pass gate 4: the run is not
/// evidence that the failure is gone. Only a run that followed the recording exactly
/// passes. `ContinueAndDisclose` needs a transaction-bound `PolicyAuthorization`, which
/// no production path issues (bn-2vanm, bn-b6u4); its tests are the crate's own unit
/// tests.
#[test]
fn pr20_impl03_negative_an_eliminated_or_reordered_candidate_run_is_refused_under_exact() {
    let cases = [
        (
            "eliminated",
            ReplayRun::new(b"journal: ack v1".to_vec(), None, vec![2, 5], 0),
            (2, 0),
        ),
        (
            "reordered only",
            ReplayRun::new(b"journal: ack v1".to_vec(), None, Vec::new(), 3),
            (0, 3),
        ),
        (
            "both",
            ReplayRun::new(b"journal: ack v1".to_vec(), None, vec![1], 1),
            (1, 1),
        ),
    ];
    for (why, candidate, counts) in cases {
        let result = run(
            &applied(),
            &Scripted::new(exact(), ReplayOutcome::Ran(candidate.clone())),
        )
        .unwrap();
        let ExactRegression::NotExact {
            run: record,
            eliminated,
            reordered,
        } = result.exact_regression()
        else {
            panic!("{why}: {:?}", result.exact_regression());
        };
        assert_eq!((*eliminated, *reordered), counts, "{why}");
        assert_eq!(result.exact_regression().status(), GateStatus::Inconclusive);
        assert_eq!(
            result.exact_regression().inconclusive_reason(),
            Some(InconclusiveReason::AbstractionAmbiguity)
        );
        let after = result.transaction();
        assert_eq!(
            gate(after, GateName::ExactRegression),
            (
                GateStatus::Inconclusive,
                vec![crash_ref(after), record.as_str().to_owned()]
            ),
            "{why}: never passed"
        );
        assert_eq!(after.evaluation_policy(), Some("exact-replay/exact/1"));
        let bytes = result.records()[1].bytes();
        assert!(
            contains(bytes, b"AbstractionAmbiguity") && contains(bytes, b"exact-replay/exact/1")
        );

        // The caller cannot relax the policy: the public seam issues no authorization.
        assert_eq!(
            replay::request_continue_and_disclose(&applied()),
            Err(replay::AuthorizationAbsence::NoAuthority),
            "{why}"
        );
    }
    // An exact clean run passes under Exact.
    let result = run(&applied(), &Scripted::new(exact(), clean(0))).unwrap();
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::NoLongerFails {
            eliminated: 0,
            reordered: 0,
            ..
        }
    ));
    // A recurring failure fails whatever the correspondence.
    let recurs = ReplayRun::new(JOURNAL.to_vec(), Some(FAILURE.to_vec()), vec![0], 2);
    let result = run(
        &applied(),
        &Scripted::new(exact(), ReplayOutcome::Ran(recurs)),
    )
    .unwrap();
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::Recurs { .. }
    ));
}

// --- negative: refusals --------------------------------------------------------------

#[test]
fn pr20_impl03_negative_a_tampered_crashpack_is_refused_before_it_is_read() {
    let transaction = applied();
    let genuine = recorded().canonical_bytes();
    let refused = |bytes: &[u8]| {
        let replayer = Scripted::new(exact(), clean(0));
        let result = replay::evaluate(
            &transaction,
            bytes,
            &base_content(),
            &repaired_content(),
            &replayer,
            None,
        );
        assert!(replayer.calls.borrow().is_empty(), "nothing is replayed");
        result
    };
    let expected = Err(ExactReplayRefusal::CrashpackTampered {
        expected: transaction.failure().clone(),
    });
    // Every single-bit flip of every byte: tag, version, lengths, base, schedule,
    // journal, failure.
    for index in 0..genuine.len() {
        for bit in 0..8 {
            let mut bytes = genuine.clone();
            bytes[index] ^= 1 << bit;
            assert_eq!(
                refused(&bytes).map(|_| ()),
                expected,
                "byte {index} bit {bit}"
            );
        }
    }
    // Truncated, extended, empty, and another crashpack's genuine bytes.
    assert_eq!(refused(&genuine[..genuine.len() - 1]).map(|_| ()), expected);
    let mut longer = genuine.clone();
    longer.push(0);
    assert_eq!(refused(&longer).map(|_| ()), expected);
    assert_eq!(refused(&[]).map(|_| ()), expected);
    let another = RecordedRun::new(
        base_id(),
        SCHEDULE.to_vec(),
        JOURNAL.to_vec(),
        b"Agreement(42)".to_vec(),
    )
    .unwrap()
    .canonical_bytes();
    assert_eq!(refused(&another).map(|_| ()), expected);
    // The genuine bytes still evaluate.
    assert!(run(&transaction, &Scripted::new(exact(), clean(0))).is_ok());
}

#[test]
fn pr20_impl03_negative_a_malformed_or_foreign_crashpack_is_refused() {
    // Bytes that name the transaction's crashpack but are no recorded run.
    let garbage = b"not a recorded run".to_vec();
    let transaction = applied_on(&garbage);
    let replayer = Scripted::new(exact(), clean(0));
    assert_eq!(
        replay::evaluate(
            &transaction,
            &garbage,
            &base_content(),
            &repaired_content(),
            &replayer,
            None
        )
        .map(|_| ()),
        Err(ExactReplayRefusal::CrashpackMalformed(MalformedRecord::Tag))
    );

    // A genuine recorded run captured on another snapshot, bound to this base.
    let elsewhere = snapshot_id(&repaired_content());
    let foreign = RecordedRun::new(
        elsewhere.clone(),
        SCHEDULE.to_vec(),
        JOURNAL.to_vec(),
        FAILURE.to_vec(),
    )
    .unwrap()
    .canonical_bytes();
    let transaction = applied_on(&foreign);
    assert_eq!(
        replay::evaluate(
            &transaction,
            &foreign,
            &base_content(),
            &repaired_content(),
            &replayer,
            None
        )
        .map(|_| ()),
        Err(ExactReplayRefusal::ForeignCrashpack {
            recorded_on: elsewhere
        })
    );
    assert!(replayer.calls.borrow().is_empty());
}

#[test]
fn pr20_impl03_negative_the_wrong_base_or_candidate_content_is_refused() {
    let transaction = applied();
    let crashpack = recorded().canonical_bytes();
    let replayer = Scripted::new(exact(), clean(0));
    let other = content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
    ]);
    assert_eq!(
        replay::evaluate(
            &transaction,
            &crashpack,
            &other,
            &repaired_content(),
            &replayer,
            None
        )
        .map(|_| ()),
        Err(ExactReplayRefusal::BaseMismatch {
            expected: base_id(),
            found: snapshot_id(&other),
        })
    );
    // The base content passed as the candidate: the repair's program is not the one run.
    assert_eq!(
        replay::evaluate(
            &transaction,
            &crashpack,
            &base_content(),
            &base_content(),
            &replayer,
            None
        )
        .map(|_| ()),
        Err(ExactReplayRefusal::CandidateMismatch {
            expected: snapshot_id(&repaired_content()),
            found: base_id(),
        })
    );
    let tweaked = content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, format!("{REPLICA_AFTER} ").as_bytes()),
    ]);
    assert!(matches!(
        replay::evaluate(
            &transaction,
            &crashpack,
            &base_content(),
            &tweaked,
            &replayer,
            None
        ),
        Err(ExactReplayRefusal::CandidateMismatch { .. })
    ));
    assert!(replayer.calls.borrow().is_empty());
}

#[test]
fn pr20_impl03_negative_a_draft_and_a_second_evaluation_are_refused() {
    let crashpack = recorded().canonical_bytes();
    let failure = recorded().identity::<Blake3Hasher>();
    let draft = Tx::begin(
        failure.clone(),
        GateProfile::PhaseB,
        &Binding(vec![failure]),
    )
    .unwrap();
    let replayer = Scripted::new(exact(), clean(0));
    assert_eq!(
        replay::evaluate(
            &draft,
            &crashpack,
            &base_content(),
            &repaired_content(),
            &replayer,
            None
        )
        .map(|_| ()),
        Err(ExactReplayRefusal::NoCandidate)
    );
    let evaluated = run(&applied(), &replayer).unwrap().into_parts().0;
    assert_eq!(
        run(&evaluated, &replayer).map(|_| ()),
        Err(ExactReplayRefusal::NotPending {
            gate: GateName::BaseReplay
        }),
        "statuses do not move twice: a re-evaluation is a new apply"
    );
}

// --- boundary ------------------------------------------------------------------------

#[test]
fn pr20_impl03_boundary_a_divergence_is_inconclusive_never_passed() {
    // On the candidate: the recorded choices do not determine its run.
    let replayer = Scripted::new(exact(), ReplayOutcome::Diverged(Divergence { at: 17 }));
    let result = run(&applied(), &replayer).unwrap();
    let ExactRegression::Diverged { at, run: record } = result.exact_regression() else {
        panic!("{:?}", result.exact_regression());
    };
    assert_eq!(*at, 17);
    assert_eq!(result.exact_regression().status(), GateStatus::Inconclusive);
    assert_eq!(
        result.exact_regression().inconclusive_reason(),
        Some(InconclusiveReason::AbstractionAmbiguity)
    );
    let after = result.transaction();
    assert_eq!(
        gate(after, GateName::ExactRegression),
        (
            GateStatus::Inconclusive,
            vec![crash_ref(after), record.as_str().to_owned()]
        )
    );

    // On the base: an engine condition, with a defect_ record, never `failed`.
    let replayer = Scripted::new(ReplayOutcome::Diverged(Divergence { at: 2 }), clean(0));
    let result = run(&applied(), &replayer).unwrap();
    let BaseReplay::Diverged { at, defect } = result.base_replay() else {
        panic!("{:?}", result.base_replay());
    };
    assert_eq!(*at, 2);
    assert!(defect.as_str().starts_with("defect_"));
    assert_eq!(result.base_replay().status(), GateStatus::Inconclusive);
    assert_eq!(
        result.base_replay().inconclusive_reason(),
        Some(InconclusiveReason::EngineError)
    );
    assert_eq!(result.records().len(), 1);
    assert_eq!(result.records()[0].handle(), defect);
    assert_eq!(
        defect.as_str(),
        format!(
            "defect_{}",
            Blake3Hasher::hash(result.records()[0].bytes()).to_token()
        )
    );
    assert_eq!(*result.exact_regression(), ExactRegression::NotRun);
    assert_eq!(*replayer.calls.borrow(), [base_id()]);
}

#[test]
fn pr20_impl03_boundary_an_unsupported_program_is_inconclusive() {
    let result = run(
        &applied(),
        &Scripted::new(ReplayOutcome::Unsupported, clean(0)),
    )
    .unwrap();
    let BaseReplay::Unsupported { record } = result.base_replay() else {
        panic!("{:?}", result.base_replay());
    };
    assert_eq!(
        result.base_replay().inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert_eq!(
        gate(result.transaction(), GateName::BaseReplay),
        (
            GateStatus::Inconclusive,
            vec![crash_ref(result.transaction()), record.as_str().to_owned()]
        ),
        "an unsupported gate cites a record, not the crashpack alone"
    );
    assert_unsupported_record(&result, record, "base_replay", &base_id());

    let result = run(
        &applied(),
        &Scripted::new(exact(), ReplayOutcome::Unsupported),
    )
    .unwrap();
    let ExactRegression::Unsupported { record } = result.exact_regression() else {
        panic!("{:?}", result.exact_regression());
    };
    assert_eq!(
        result.exact_regression().inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert_eq!(
        gate(result.transaction(), GateName::ExactRegression),
        (
            GateStatus::Inconclusive,
            vec![crash_ref(result.transaction()), record.as_str().to_owned()]
        )
    );
    assert_unsupported_record(
        &result,
        record,
        "exact_regression",
        &snapshot_id(&repaired_content()),
    );
}

/// The record an `Unsupported` gate cites is named by its bytes and binds the gate, the
/// crashpack, the program, the replayer, the policy, the status and the typed reason.
fn assert_unsupported_record(
    result: &ExactReplay<Blake3Hasher>,
    handle: &EvidenceRef,
    gate: &str,
    program: &SnapshotId,
) {
    let record = result
        .records()
        .iter()
        .find(|r| r.handle() == handle)
        .expect("the cited record is returned");
    let bytes = record.bytes();
    assert!(handle.as_str().starts_with("ev_"));
    assert_eq!(
        handle.as_str(),
        format!("ev_{}", Blake3Hasher::hash(bytes).to_token())
    );
    for part in [
        gate,
        result.transaction().failure().as_str(),
        program.as_str(),
        "scripted/1",
        EvaluationPolicy::Exact.token(),
        "inconclusive",
        InconclusiveReason::Unsupported.as_str(),
    ] {
        assert!(contains(bytes, part.as_bytes()), "the record binds {part}");
    }
    assert_eq!(bytes.last(), Some(&2), "the answer is Unsupported");
}

#[test]
fn pr20_impl03_boundary_the_record_grammar_is_strict() {
    let bytes = recorded().canonical_bytes();
    // Every proper prefix is refused: truncation never decodes.
    for end in 0..bytes.len() {
        let error = RecordedRun::decode(&bytes[..end]).unwrap_err();
        assert!(
            matches!(error, MalformedRecord::Truncated | MalformedRecord::Tag),
            "prefix {end}: {error:?}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        RecordedRun::decode(&trailing),
        Err(MalformedRecord::TrailingBytes)
    );
    let mut version = bytes.clone();
    version[b"continuum-repair/recorded-run".len()] = 2;
    assert_eq!(
        RecordedRun::decode(&version),
        Err(MalformedRecord::Version(2))
    );
    // A length prefix far past the input is refused before anything is allocated.
    let mut huge = b"continuum-repair/recorded-run".to_vec();
    huge.push(1);
    huge.extend_from_slice(&u64::MAX.to_be_bytes());
    assert_eq!(RecordedRun::decode(&huge), Err(MalformedRecord::Truncated));
    // An empty failure is no crashpack.
    assert_eq!(
        RecordedRun::new(base_id(), Vec::new(), Vec::new(), Vec::new()),
        Err(MalformedRecord::NoFailure)
    );
    let mut no_failure = b"continuum-repair/recorded-run".to_vec();
    no_failure.push(1);
    for part in [base_id().as_str().as_bytes(), b"", b"", b""] {
        no_failure.extend_from_slice(&(part.len() as u64).to_be_bytes());
        no_failure.extend_from_slice(part);
    }
    assert_eq!(
        RecordedRun::decode(&no_failure),
        Err(MalformedRecord::NoFailure)
    );
    let mut bad_base = b"continuum-repair/recorded-run".to_vec();
    bad_base.push(1);
    for part in [b"crash_x".as_slice(), b"", b"", b"F"] {
        bad_base.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bad_base.extend_from_slice(part);
    }
    assert!(matches!(
        RecordedRun::decode(&bad_base),
        Err(MalformedRecord::BaseSnapshot(_))
    ));
}

#[test]
fn pr20_impl03_boundary_the_evaluation_is_deterministic() {
    let first = run(&applied(), &Scripted::new(exact(), clean(3))).unwrap();
    let second = run(&applied(), &Scripted::new(exact(), clean(3))).unwrap();
    assert_eq!(
        first.transaction().to_artifact_bytes(),
        second.transaction().to_artifact_bytes()
    );
    assert_eq!(first.records(), second.records());
    // Each outcome moves the identity: a different candidate run, a different version.
    let other = run(&applied(), &Scripted::new(exact(), clean(4))).unwrap();
    assert_ne!(
        first.transaction().repair_id(),
        other.transaction().repair_id()
    );
    assert_ne!(first.records()[1], other.records()[1]);
    assert_eq!(first.records()[0], other.records()[0]);
}

// --- metamorphic ---------------------------------------------------------------------

/// Relation: serialization round trip. A crashpack decoded and re-encoded is the same
/// bytes, names the same crashpack, and evaluates to the same version.
#[test]
fn pr20_impl03_metamorphic_a_serialization_round_trip_changes_nothing() {
    let bytes = recorded().canonical_bytes();
    let decoded = RecordedRun::decode(&bytes).unwrap();
    assert_eq!(decoded, recorded());
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(
        decoded.identity::<Blake3Hasher>(),
        recorded().identity::<Blake3Hasher>()
    );
    let transaction = applied();
    let direct = run(&transaction, &Scripted::new(exact(), clean(0))).unwrap();
    let round = replay::evaluate(
        &transaction,
        &decoded.canonical_bytes(),
        &base_content(),
        &repaired_content(),
        &Scripted::new(exact(), clean(0)),
        None,
    )
    .unwrap();
    assert_eq!(
        direct.transaction().to_artifact_bytes(),
        round.transaction().to_artifact_bytes()
    );
}

// --- differential --------------------------------------------------------------------

/// The crashpack identity against an independent construction of the record's bytes,
/// hashed by `continuum_value`'s BLAKE3 directly.
#[test]
fn pr20_impl03_differential_the_crashpack_identity_matches_an_independent_construction() {
    let mut bytes = b"continuum-repair/recorded-run".to_vec();
    bytes.push(1);
    for part in [base_id().as_str().as_bytes(), SCHEDULE, JOURNAL, FAILURE] {
        bytes.extend_from_slice(&u64::try_from(part.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(part);
    }
    assert_eq!(recorded().canonical_bytes(), bytes);
    let independent = format!(
        "crash_{}",
        continuum_value::identity::Blake3Hasher::hash(&bytes).to_token()
    );
    assert_eq!(
        recorded()
            .identity::<continuum_value::identity::Blake3Hasher>()
            .as_str(),
        independent
    );
}

/// The evidence pattern `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` against a backtracking
/// reference matcher, over every string of length at most five on an alphabet with one
/// representative of each character class.
#[test]
fn pr20_impl03_differential_evidence_handles_match_the_schema_pattern() {
    fn reference(s: &[u8]) -> bool {
        let class = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
        let suffix = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
        if s.is_empty() || !s[0].is_ascii_lowercase() {
            return false;
        }
        (1..s.len()).any(|sep| {
            s[sep] == b'_'
                && s[1..sep].iter().all(|b| class(*b))
                && sep + 1 < s.len()
                && s[sep + 1..].iter().all(|b| suffix(*b))
        })
    }
    let alphabet = b"a9_-Z.";
    let mut strings: Vec<Vec<u8>> = vec![Vec::new()];
    let mut frontier = strings.clone();
    for _ in 0..5 {
        let mut next = Vec::new();
        for s in &frontier {
            for c in alphabet {
                let mut t = s.clone();
                t.push(*c);
                next.push(t);
            }
        }
        strings.extend(next.iter().cloned());
        frontier = next;
    }
    let mut accepted = 0;
    for s in &strings {
        let text = std::str::from_utf8(s).unwrap();
        let ours = EvidenceRef::new(text).is_ok();
        assert_eq!(ours, reference(s), "{text:?}");
        accepted += usize::from(ours);
    }
    assert!(accepted > 100, "the enumeration reaches accepted handles");
    for good in ["crash_x", "ev_0a", "defect_A-b", "a__", "a_b_c", "ab_-"] {
        assert!(EvidenceRef::new(good).is_ok(), "{good}");
    }
    for bad in ["_x", "A_x", "ev_", "ev", "ev_a.b", "e-v_a", "ev_a b", ""] {
        assert!(EvidenceRef::new(bad).is_err(), "{bad}");
    }
}
