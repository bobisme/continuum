//! Evidence for the goal bone's acceptance criterion 2 — the G0 staging gate.
//!
//! > The Phase A benchmark subset used for DX-10 and the G2 Context Pack ablation must
//! > itself pass the §19.4 family/source-hash separation check before either result is
//! > accepted; full leakage validation remains DX-15 at G9.
//! >
//! > — `notes/plan/plan.md` §22; `docs/52_RELEASE_GATES_REV3.md`
//!
//! # Clause → test
//!
//! - **"must itself pass the §19.4 … check"** →
//!   [`the_phase_a_subset_isolates_semantic_families_and_source_hashes`].
//! - **"family … separation"**, as something that can fail →
//!   [`a_family_that_straddles_two_partitions_is_refused`].
//! - **"… source-hash separation"**, as a *distinct* condition →
//!   [`a_source_hash_that_straddles_two_partitions_is_refused_even_when_families_do_not`],
//!   which builds the case that passes the family rule and fails the source rule, so the
//!   second half of the sentence is shown to be doing work.
//! - **"before … the result is accepted"** →
//!   [`a_report_over_a_leaked_subset_is_not_accepted`]: the gate is inside
//!   `Report::over`, not beside it, so no caller can assemble an accepted report over a
//!   leaked subset.
//! - **the check is not vacuous** →
//!   [`a_split_with_an_empty_side_is_refused_rather_than_trivially_separated`].
//! - **the source hash is the daemon's own model key** →
//!   [`a_tasks_source_hash_is_the_identity_verification_start_resolves_a_model_by`], which is
//!   what makes "same source" mean one thing in this crate and in `continuumd`.

use continuum_benchmark::corpus::Source;
use continuum_benchmark::report::Report;
use continuum_benchmark::separation::{self, Leak, Verdict, source_hash_token};
use continuum_benchmark::task::{
    DIE_HARD_ALL, DIE_HARD_TYPE_OK, Family, PHILOSOPHERS_ALL, PHILOSOPHERS_OWNERSHIP, Partition,
    SUBSET,
};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::verification::model_source;
use continuumd::protocol::scalar::Commitment;

/// The subset PR 10 reports on passes the check.
#[test]
fn the_phase_a_subset_isolates_semantic_families_and_source_hashes() {
    let verdict = separation::check(SUBSET);
    assert_eq!(
        verdict,
        Verdict::Separated {
            tasks: 4,
            families: 2,
            source_hashes: 2,
        },
        "four tasks, two semantic families, two corpus sources, no straddle"
    );
    assert!(verdict.passes());

    // The partition assignment is a fact about the subset, not about the checker: every
    // finite-safety task is disclosed and every deadlock task is held out.
    for task in SUBSET {
        let expected = match task.family {
            Family::FiniteSafety => Partition::Development,
            Family::Deadlock => Partition::HeldOut,
        };
        assert_eq!(task.partition, expected, "{}", task.id);
    }
}

/// A family on both sides of the split is a leak.
#[test]
fn a_family_that_straddles_two_partitions_is_refused() {
    let mut leaked = DIE_HARD_TYPE_OK;
    leaked.partition = Partition::HeldOut;
    let subset = [
        DIE_HARD_ALL,
        leaked,
        PHILOSOPHERS_ALL,
        PHILOSOPHERS_OWNERSHIP,
    ];

    let Verdict::Leaked(leaks) = separation::check(&subset) else {
        panic!("a straddling family must be refused");
    };
    assert!(
        leaks.iter().any(|leak| matches!(
            leak,
            Leak::Family {
                family: Family::FiniteSafety,
                ..
            }
        )),
        "the finite-safety family is on both sides: {leaks:?}"
    );
}

/// The source-hash rule is a second condition, not a corollary of the family rule.
///
/// Both Die Hard tasks read the same `.ctm`. Giving them *different* families and putting
/// them on opposite sides passes the family rule — no family straddles — and must still be
/// refused, because a harness tuned on one has been tuned on the other's model.
#[test]
fn a_source_hash_that_straddles_two_partitions_is_refused_even_when_families_do_not() {
    let mut moved = DIE_HARD_TYPE_OK;
    moved.family = Family::Deadlock;
    moved.partition = Partition::HeldOut;
    let subset = [DIE_HARD_ALL, moved];

    let Verdict::Leaked(leaks) = separation::check(&subset) else {
        panic!("a straddling source hash must be refused");
    };
    assert!(
        !leaks.iter().any(|leak| matches!(leak, Leak::Family { .. })),
        "no family straddles in this construction: {leaks:?}"
    );
    assert!(
        leaks
            .iter()
            .any(|leak| matches!(leak, Leak::SourceHash { .. })),
        "the TV-009 module is on both sides: {leaks:?}"
    );
}

/// A split with nothing held out is not "separated"; it is vacuous.
#[test]
fn a_split_with_an_empty_side_is_refused_rather_than_trivially_separated() {
    let subset = [DIE_HARD_ALL, DIE_HARD_TYPE_OK];
    let Verdict::Leaked(leaks) = separation::check(&subset) else {
        panic!("an empty held-out side must be refused");
    };
    assert_eq!(
        leaks,
        vec![Leak::EmptyPartition {
            partition: Partition::HeldOut
        }],
        "the only defect is that nothing is held out"
    );
}

/// The gate is inside the constructor.
#[test]
fn a_report_over_a_leaked_subset_is_not_accepted() {
    let clean = Report::new(Vec::new());
    assert!(clean.accepted, "the shipped subset is separated");

    let mut leaked = PHILOSOPHERS_ALL;
    leaked.partition = Partition::Development;
    let subset = [
        DIE_HARD_ALL,
        DIE_HARD_TYPE_OK,
        leaked,
        PHILOSOPHERS_OWNERSHIP,
    ];
    let report = Report::over(&subset, Vec::new());
    assert!(
        !report.accepted,
        "a leaked subset cannot produce an accepted report"
    );
    assert!(
        report.render().contains("verdict: leaked"),
        "and the artifact says so, rather than reporting numbers with no caveat"
    );
    assert!(report.render().contains("accepted: false"));
}

/// One identity, two readers.
#[test]
fn a_tasks_source_hash_is_the_identity_verification_start_resolves_a_model_by() {
    for source in Source::ALL {
        let daemon_key = model_source(
            &Blake3Identity,
            [(source.module_path(), source.module().as_bytes())],
        )
        .expect("blake3 names the module set");
        assert_eq!(
            source.source_hash(),
            daemon_key,
            "the separation check and the model catalog key by the same function"
        );
    }
    let die_hard = source_hash_token(&DIE_HARD_ALL);
    let philosophers = source_hash_token(&PHILOSOPHERS_ALL);
    assert_ne!(die_hard, philosophers, "two ports, two source identities");
    assert_eq!(
        source_hash_token(&DIE_HARD_TYPE_OK),
        die_hard,
        "two targets over one module share its source identity — which is exactly why the \
         source-hash rule is not implied by the family rule"
    );
    // The commitment carries the elaborated-model artifact class it was named under, which
    // is what makes a source hash a *placed* identity rather than a bare digest: two
    // different artifact classes over the same bytes are two different identities
    // (`continuum-workspace::artifact_path`).
    let rendered = Commitment::as_str(&DIE_HARD_ALL.source.source_hash()).to_owned();
    assert!(
        !rendered.is_empty() && rendered.chars().all(|byte| byte.is_ascii_graphic()),
        "a source hash renders as printable ASCII: {rendered}"
    );
    assert_eq!(
        rendered,
        Commitment::as_str(&DIE_HARD_ALL.source.source_hash()),
        "and it is a function of the source bytes alone, so it is stable across calls"
    );
}
