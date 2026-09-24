//! Positive, negative, and boundary tests of the reuse-edge classes, the
//! downgrade rules, `explain_invalidation`, and the resource bounds (bn-31vf).

mod support;

use continuum_incremental::audit::{self, AuditVerdict, Blame, Parity};
use continuum_incremental::engine::{
    Cause, Config, Engine, Heuristic, Limits, Memo, Outcome, Refusal, Revision, WitnessRefusal,
};
use continuum_incremental::explain::{self, Cause as Why, SourceChange, Status};
use continuum_incremental::query::{Label, Stage, Triple};
use continuum_incremental::vocab::{EvidenceForm, Lane, ReuseClass};

use support::{CORPUS, diehard};

fn edit(id: &str) -> String {
    CORPUS
        .iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("no corpus edit {id}"))
        .apply(&diehard())
}

fn warm(lane: Lane) -> (Engine, Revision) {
    let mut engine = Engine::new(Config::default());
    let base = engine.revise(&diehard(), lane).expect("the base fits");
    (engine, base)
}

fn outcome<'a>(revision: &'a Revision, label: &Label) -> &'a Outcome {
    &revision.get(label).expect("demanded").outcome
}

fn licence(revision: &Revision, label: &Label) -> Option<ReuseClass> {
    match outcome(revision, label) {
        Outcome::Reused { licence, .. } => Some(*licence),
        Outcome::Recomputed { .. } => None,
    }
}

fn explore() -> Label {
    Label::of(Stage::Explore)
}

fn clean_parity(revision: &Revision, source: &str) -> AuditVerdict {
    audit::parity(
        revision,
        &audit::clean(
            source,
            &Limits::default(),
            continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
        ),
    )
    .verdict()
}

// --- positive -------------------------------------------------------------------

#[test]
fn an_unchanged_source_reuses_every_query_exactly() {
    let (mut engine, _) = warm(Lane::Promotion);
    let again = engine.revise(&diehard(), Lane::Promotion).expect("fits");
    for record in again.records.values() {
        assert_eq!(
            record.outcome,
            Outcome::Reused {
                licence: ReuseClass::Exact,
                evidence: Some(EvidenceForm::ContentIdentity),
                triple: Triple {
                    class: ReuseClass::Exact,
                    function_id: record.definition.function_id,
                    function_version: record.definition.function_version,
                },
                assumptions: Vec::new(),
            },
            "{}",
            record.label
        );
    }
    assert_eq!(again.explored_states, 0, "nothing is explored twice");
}

#[test]
fn a_whitespace_edit_reparses_and_cuts_off_early_with_exact_reuse() {
    let (mut engine, base) = warm(Lane::Promotion);
    let source = edit("ws-reindent");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    assert!(matches!(
        outcome(&revision, &Label::of(Stage::Parse)),
        Outcome::Recomputed { .. }
    ));
    for record in revision
        .records
        .values()
        .filter(|r| r.label.stage > Stage::Parse)
    {
        assert_eq!(licence(&revision, &record.label), Some(ReuseClass::Exact));
        assert_eq!(record.class, ReuseClass::Exact);
    }
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);
    let why = explain::explain_invalidation(&base, &revision);
    assert_eq!(why.source, SourceChange::Changed);
    assert_eq!(why.invalidated.len(), 1);
    assert!(why.unknown.is_empty());
}

#[test]
fn a_property_edit_reuses_the_exploration_through_a_conservative_edge() {
    let (mut engine, base) = warm(Lane::Promotion);
    let source = edit("prop-edit");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    assert_eq!(
        licence(&revision, &explore()),
        Some(ReuseClass::Conservative)
    );
    assert_eq!(revision.explored_states, 0);
    assert_eq!(
        licence(&revision, &Label::named(Stage::CheckInvariant, "TypeOK")),
        Some(ReuseClass::Conservative)
    );
    assert_eq!(
        licence(&revision, &Label::named(Stage::CheckInvariant, "NotSolved")),
        None,
        "the edited invariant is rechecked"
    );
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);

    // The explanation names the edit's path into the rechecked invariant, and the
    // assumption the reused exploration rests on.
    let why = explain::explain_invalidation(&base, &revision);
    let checked = why
        .invalidated
        .iter()
        .find(|e| e.label == Label::named(Stage::CheckInvariant, "NotSolved"))
        .expect("invalidated");
    let Why::Input(chain) = &checked.cause else {
        panic!("an input change caused it: {:?}", checked.cause);
    };
    let path: Vec<Stage> = chain.iter().map(|step| step.to.stage).collect();
    assert_eq!(
        path,
        [
            Stage::Parse,
            Stage::Elaborate,
            Stage::ProjectInvariant,
            Stage::CheckInvariant
        ]
    );
    assert_eq!(chain[0].from.stage, Stage::Source);
    let reused = why
        .reused
        .iter()
        .find(|e| e.label == explore())
        .expect("explore reused");
    assert_eq!(reused.evidence, EvidenceForm::ConservativeDependencyTheorem);
    assert!(reused.assumptions.iter().all(|a| a.starts_with("A1:")));
    assert_eq!(reused.assumptions.len(), 1);
}

#[test]
fn a_renamed_invariant_reuses_its_check_by_content() {
    let (mut engine, base) = warm(Lane::Promotion);
    let source = edit("rename-invariant");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let renamed = Label::named(Stage::CheckInvariant, "BigNotFour");
    assert!(
        licence(&revision, &renamed).is_some(),
        "content-addressed reuse"
    );
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);
    let why = explain::explain_invalidation(&base, &revision);
    let removed = why
        .invalidated
        .iter()
        .find(|e| e.label == Label::named(Stage::CheckInvariant, "NotSolved"))
        .expect("the old label is unaddressable");
    assert_eq!(removed.status, Status::Removed);
}

#[test]
fn a_validated_reuse_is_licensed_by_a_kernel_checked_certificate() {
    let (mut engine, _) = warm(Lane::Promotion);
    let source = edit("sem-domain-widen");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let safety = Label::of(Stage::DomainSafety);
    let Outcome::Reused {
        licence: class,
        evidence,
        ..
    } = outcome(&revision, &safety)
    else {
        panic!("domain safety is reused: {:?}", outcome(&revision, &safety));
    };
    assert_eq!(*class, ReuseClass::Validated);
    assert_eq!(*evidence, Some(EvidenceForm::CheckedCertificate));
    let Outcome::Reused { assumptions, .. } = outcome(&revision, &safety) else {
        unreachable!("matched above");
    };
    assert_eq!(
        assumptions,
        &vec![continuum_incremental::query::ASSUMPTION_V1],
        "the engine-side definedness scan is recorded, not credited to the certificate"
    );
    assert!(matches!(
        outcome(&revision, &explore()),
        Outcome::Recomputed { .. }
    ));
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);

    // Fewer reachable states is still Validated: the verified set is a closed
    // superset, and the output carries no count.
    let (mut engine, _) = warm(Lane::Promotion);
    let source = edit("sem-emptybig-noop");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    assert_eq!(licence(&revision, &safety), Some(ReuseClass::Validated));
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);
}

#[test]
fn explanations_are_deterministic_and_partition_every_query_once() {
    let run = || {
        let (mut engine, base) = warm(Lane::Interactive);
        let revision = engine
            .revise(&edit("prop-add"), Lane::Interactive)
            .expect("fits");
        (
            explain::explain_invalidation(&base, &revision),
            base,
            revision,
        )
    };
    let (first, base, revision) = run();
    assert_eq!(first, run().0);
    // Every query of either revision lands in exactly one of the three sets.
    let mut seen: Vec<&Label> = first
        .invalidated
        .iter()
        .map(|e| &e.label)
        .chain(first.unknown.iter().map(|e| &e.label))
        .chain(first.reused.iter().map(|e| &e.label))
        .collect();
    let total = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), total, "disjoint");
    let all: std::collections::BTreeSet<&Label> =
        base.records.keys().chain(revision.records.keys()).collect();
    assert_eq!(seen.len(), all.len(), "complete");
}

// --- negative -------------------------------------------------------------------

#[test]
fn quarantining_an_exact_triple_forces_a_recompute_in_the_promotion_lane() {
    let (mut engine, _) = warm(Lane::Promotion);
    let triple = Triple {
        class: ReuseClass::Exact,
        function_id: "explore",
        function_version: "1",
    };
    engine.quarantine(triple);
    let revision = engine.revise(&diehard(), Lane::Promotion).expect("fits");
    assert_eq!(
        outcome(&revision, &explore()),
        &Outcome::Recomputed {
            cause: Cause::Quarantined(triple),
            memo: Memo::Stored,
        }
    );
    assert!(revision.explored_states > 0, "the exploration ran again");
    // Only the triple is quarantined, not the class: other Exact reuse stands.
    assert_eq!(
        licence(&revision, &Label::of(Stage::Elaborate)),
        Some(ReuseClass::Exact)
    );
    assert_eq!(clean_parity(&revision, &diehard()), AuditVerdict::Parity);

    // The interactive lane may still serve it, as Experimental, never promotable.
    let revision = engine.revise(&diehard(), Lane::Interactive).expect("fits");
    assert_eq!(
        licence(&revision, &explore()),
        Some(ReuseClass::Experimental)
    );
    assert_eq!(
        revision
            .get(&explore())
            .expect("demanded")
            .class
            .promotion(),
        continuum_incremental::vocab::Promotion::NotPromotable
    );
}

#[test]
fn quarantining_a_conservative_triple_forces_a_recompute_and_explains_the_downgrade() {
    let (mut engine, base) = warm(Lane::Promotion);
    let triple = Triple {
        class: ReuseClass::Conservative,
        function_id: "explore",
        function_version: "1",
    };
    engine.quarantine(triple);
    let revision = engine
        .revise(&edit("prop-edit"), Lane::Promotion)
        .expect("fits");
    assert_eq!(
        outcome(&revision, &explore()),
        &Outcome::Recomputed {
            cause: Cause::Quarantined(triple),
            memo: Memo::Stored,
        }
    );
    let why = explain::explain_invalidation(&base, &revision);
    let entry = why
        .invalidated
        .iter()
        .find(|e| e.label == explore())
        .expect("explore is invalidated");
    assert_eq!(entry.cause, Why::Downgrade(Cause::Quarantined(triple)));
}

#[test]
fn the_promotion_lane_never_uses_the_experimental_heuristic() {
    let (mut engine, _) = warm(Lane::Promotion);
    let revision = engine
        .revise(&edit("comment-add"), Lane::Promotion)
        .expect("fits");
    for record in revision.records.values() {
        assert_ne!(record.class, ReuseClass::Experimental, "{}", record.label);
    }
    // Disabled heuristic: the interactive lane recomputes the parse too.
    let mut engine = Engine::new(Config {
        heuristic: Heuristic::Disabled,
        ..Config::default()
    });
    engine.revise(&diehard(), Lane::Interactive).expect("fits");
    let revision = engine
        .revise(&edit("comment-add"), Lane::Interactive)
        .expect("fits");
    assert_eq!(
        outcome(&revision, &Label::of(Stage::Parse)),
        &Outcome::Recomputed {
            cause: Cause::KeyMiss,
            memo: Memo::Stored
        }
    );
}

#[test]
fn an_experimental_reuse_is_unknown_never_reused_and_the_audit_catches_its_miss() {
    let (mut engine, base) = warm(Lane::Interactive);
    let source = edit("trap-join-lines");
    let revision = engine.revise(&source, Lane::Interactive).expect("fits");
    assert_eq!(
        licence(&revision, &Label::of(Stage::Parse)),
        Some(ReuseClass::Experimental)
    );
    let why = explain::explain_invalidation(&base, &revision);
    assert!(
        why.reused.is_empty(),
        "no Experimental derivation is reused"
    );
    assert_eq!(why.unknown.len(), revision.records.len());

    let clean = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let report = audit::parity(&revision, &clean);
    assert_eq!(report.verdict(), AuditVerdict::Mismatch);
    let triple = Triple {
        class: ReuseClass::Experimental,
        function_id: "parse",
        function_version: "1",
    };
    assert_eq!(
        report.first,
        Some((Label::of(Stage::Parse), Blame::Reuse(triple)))
    );
    let cone = audit::cone(
        &audit::clean(
            &diehard(),
            &Limits::default(),
            continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
        ),
        &clean,
        &base,
        &revision,
    );
    assert!(cone.under_invalidated.is_empty());
    assert!(!cone.experimental_misses.is_empty());

    engine.quarantine(triple);
    let revision = engine.revise(&source, Lane::Interactive).expect("fits");
    assert_eq!(
        outcome(&revision, &Label::of(Stage::Parse)),
        &Outcome::Recomputed {
            cause: Cause::AlternateRefused(Box::new(Cause::Quarantined(triple))),
            memo: Memo::Stored
        }
    );
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);
}

#[test]
fn a_witness_the_kernel_rejects_licenses_nothing() {
    let (mut engine, _) = warm(Lane::Promotion);
    let source = edit("sem-fillbig-4");
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let Outcome::Recomputed { cause, .. } = outcome(&revision, &Label::of(Stage::DomainSafety))
    else {
        panic!("recomputed");
    };
    assert!(
        matches!(
            cause,
            Cause::WitnessRefused(WitnessRefusal::NotVerified(_) | WitnessRefusal::Emission(_))
        ),
        "{cause:?}"
    );
    assert_eq!(clean_parity(&revision, &source), AuditVerdict::Parity);
}

#[test]
fn the_audit_catches_a_stale_conservative_reuse_it_did_not_compute() {
    // Anti-vacuity: an intentionally wrong dependency. Take the property edit's
    // revision and plant the base's verdict for the edited invariant as if the
    // engine had reused it through its conservative edge.
    let (mut engine, base) = warm(Lane::Promotion);
    let source = edit("prop-edit");
    let mut revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let label = Label::named(Stage::CheckInvariant, "NotSolved");
    let triple = Triple {
        class: ReuseClass::Conservative,
        function_id: "check_invariant",
        function_version: "2",
    };
    let stale = base.get(&label).expect("in base").output.clone();
    let record = revision.records.get_mut(&label).expect("demanded");
    record.output = stale;
    record.outcome = Outcome::Reused {
        licence: ReuseClass::Conservative,
        evidence: Some(EvidenceForm::ConservativeDependencyTheorem),
        triple,
        assumptions: vec![continuum_incremental::query::ASSUMPTION_A2],
    };
    record.class = ReuseClass::Conservative;

    let clean_after = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let report = audit::parity(&revision, &clean_after);
    assert_eq!(report.labels.get(&label), Some(&Parity::Diverges));
    let cone = audit::cone(
        &audit::clean(
            &diehard(),
            &Limits::default(),
            continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
        ),
        &clean_after,
        &base,
        &revision,
    );
    assert_eq!(cone.under_invalidated, vec![label.clone()]);
    // The first divergence is the planted one, and its own licence is blamed, not
    // the agreeing Conservative exploration above it.
    assert_eq!(report.first, Some((label, Blame::Reuse(triple))));
}

#[test]
fn unknown_class_tokens_fail_closed() {
    assert_eq!(
        ReuseClass::from_token_fail_closed("Complete"),
        ReuseClass::Experimental
    );
}

// --- boundary: resource bounds charged before work on untrusted CML --------------

#[test]
fn an_oversized_source_is_refused_before_any_work_and_changes_nothing() {
    let source = diehard();
    let limits = Limits {
        source_bytes: source.len() - 1,
        ..Limits::default()
    };
    let mut engine = Engine::new(Config {
        limits,
        ..Config::default()
    });
    assert_eq!(
        engine.revise(&source, Lane::Promotion).map(|_| ()),
        Err(Refusal::SourceTooLarge {
            bytes: source.len(),
            max: source.len() - 1
        })
    );
    assert_eq!(engine.cached_entries(), 0);
    // Exactly at the limit is admitted.
    let mut engine = Engine::new(Config {
        limits: Limits {
            source_bytes: source.len(),
            ..Limits::default()
        },
        ..Config::default()
    });
    assert!(engine.revise(&source, Lane::Promotion).is_ok());
}

#[test]
fn a_work_budget_too_small_for_the_source_refuses_atomically() {
    let source = diehard();
    // Less than the first charge (hashing the source): nothing runs.
    let tiny = Limits {
        work: source.len() as u64 - 1,
        ..Limits::default()
    };
    let mut engine = Engine::new(Config {
        limits: tiny,
        ..Config::default()
    });
    assert!(matches!(
        engine.revise(&source, Lane::Promotion),
        Err(Refusal::WorkExhausted {
            at: "hash output",
            ..
        })
    ));
    assert_eq!(engine.cached_entries(), 0);

    // A budget that runs out at the exploration's declared bounds: refused before
    // exploring, and the staged parse and elaboration are not committed.
    let mut engine = Engine::new(Config {
        limits: Limits {
            work: 1 << 22,
            ..Limits::default()
        },
        ..Config::default()
    });
    assert!(matches!(
        engine.revise(&source, Lane::Promotion),
        Err(Refusal::WorkExhausted {
            at: "explore (declared bounds)",
            ..
        })
    ));
    assert_eq!(engine.cached_entries(), 0, "a refusal commits nothing");
}

#[test]
fn the_query_count_is_checked_before_any_per_invariant_query_exists() {
    let source = diehard();
    // Two invariants demand 6 + 2 * 2 = 10 queries.
    let at = |queries| {
        let mut engine = Engine::new(Config {
            limits: Limits {
                queries,
                ..Limits::default()
            },
            ..Config::default()
        });
        (
            engine.revise(&source, Lane::Promotion).map(|_| ()),
            engine.cached_entries(),
        )
    };
    assert_eq!(at(10).0, Ok(()));
    assert_eq!(
        at(9),
        (Err(Refusal::TooManyQueries { needed: 10, max: 9 }), 0)
    );
}

#[test]
fn a_budget_dependent_front_end_failure_is_returned_but_not_memoized() {
    let source = diehard();
    let mut engine = Engine::new(Config {
        limits: Limits {
            elab: continuum_cml_elab::Limits {
                nodes: 4,
                work: 1 << 20,
            },
            ..Limits::default()
        },
        ..Config::default()
    });
    for _ in 0..2 {
        let revision = engine.revise(&source, Lane::Promotion).expect("fits");
        assert!(matches!(
            outcome(&revision, &Label::of(Stage::Elaborate)),
            Outcome::Recomputed {
                memo: Memo::NotMemoized(_),
                ..
            }
        ));
        assert!(revision.get(&Label::of(Stage::BuildModel)).is_none());
    }
}

#[test]
fn a_changed_semantic_epoch_shares_no_key() {
    let mut engine = Engine::new(Config {
        semantic_epoch: "continuum-semantics-2".to_owned(),
        ..Config::default()
    });
    let other = engine.revise(&diehard(), Lane::Promotion).expect("fits");
    let (_, base) = warm(Lane::Promotion);
    for (label, record) in &base.records {
        assert_ne!(
            record.handle,
            other.get(label).expect("same queries").handle,
            "{label}"
        );
    }
}

// --- metamorphic ------------------------------------------------------------------

/// Metamorphic relation: alpha-renaming. Renaming a `let` binding (`next_big` to
/// `nb`) is an alpha-renaming, so every compared output is unchanged, and the
/// engine re-elaborates once and reuses everything downstream through `Exact`
/// licences.
#[test]
fn alpha_renaming_a_let_binding_changes_no_output_and_reuses_everything_downstream() {
    let (mut engine, base) = warm(Lane::Promotion);
    let renamed = edit("rename-let");
    let revision = engine.revise(&renamed, Lane::Promotion).expect("fits");
    // alpha-renaming: outputs from `elaborate` on are byte-equal to the base's.
    for (label, record) in &revision.records {
        if label.stage >= Stage::Elaborate {
            assert_eq!(
                record.output,
                base.get(label).expect("same queries").output,
                "{label}"
            );
        }
        if label.stage > Stage::Elaborate {
            assert_eq!(
                licence(&revision, label),
                Some(ReuseClass::Exact),
                "{label}"
            );
        }
    }
    assert_eq!(revision.explored_states, 0);
    assert_eq!(clean_parity(&revision, &renamed), AuditVerdict::Parity);
}

// --- sequences, shared bodies, and atomicity on a warm engine --------------------

#[test]
fn a_conservative_reuse_stays_conservative_with_its_assumption_until_recomputed() {
    let (mut engine, _) = warm(Lane::Promotion);
    let edited = edit("prop-edit");
    let first = engine.revise(&edited, Lane::Promotion).expect("fits");
    let Outcome::Reused { assumptions, .. } = outcome(&first, &explore()) else {
        panic!("explore is reused");
    };
    assert_eq!(assumptions.len(), 1);
    assert!(assumptions[0].starts_with("A1:"));
    // A whitespace edit on top: no in-place upgrade. The exploration was computed
    // under the base model, so its reuse still rests on A1.
    let spaced = edited.replace("\n  action", "\n\n  action");
    let second = engine.revise(&spaced, Lane::Promotion).expect("fits");
    for label in [explore(), Label::of(Stage::DomainSafety)] {
        assert_eq!(
            licence(&second, &label),
            Some(ReuseClass::Conservative),
            "{label}"
        );
    }
    let why = explain::explain_invalidation(&first, &second);
    let reused = why
        .reused
        .iter()
        .find(|e| e.label == explore())
        .expect("reused");
    assert_eq!(reused.assumptions.len(), 1);
    assert_eq!(clean_parity(&second, &spaced), AuditVerdict::Parity);
}

// cr-1jv75r th-8947er: a cache entry keeps the class and the licence triples it was
// produced under, so quarantine reaches it however it is presented later.
#[test]
fn quarantining_a_conservative_triple_reaches_a_result_reused_under_it_before() {
    let (mut engine, _) = warm(Lane::Promotion);
    let edited = edit("prop-edit");
    engine.revise(&edited, Lane::Promotion).expect("fits");
    let triple = Triple {
        class: ReuseClass::Conservative,
        function_id: "explore",
        function_version: "1",
    };
    engine.quarantine(triple);
    let again = engine.revise(&edited, Lane::Promotion).expect("fits");
    let Outcome::Recomputed { cause, .. } = outcome(&again, &explore()) else {
        panic!(
            "the identical rerun must recompute: {:?}",
            outcome(&again, &explore())
        );
    };
    assert!(
        matches!(cause, Cause::Quarantined(_) | Cause::KeyMiss),
        "{cause:?}"
    );
    assert!(again.explored_states > 0);
    assert_eq!(clean_parity(&again, &edited), AuditVerdict::Parity);
}

#[test]
fn quarantining_a_validated_triple_reaches_the_identical_rerun() {
    let (mut engine, _) = warm(Lane::Promotion);
    let widened = edit("sem-domain-widen");
    let first = engine.revise(&widened, Lane::Promotion).expect("fits");
    let safety = Label::of(Stage::DomainSafety);
    assert_eq!(licence(&first, &safety), Some(ReuseClass::Validated));
    let triple = Triple {
        class: ReuseClass::Validated,
        function_id: "domain_safety",
        function_version: "2",
    };
    engine.quarantine(triple);
    let again = engine.revise(&widened, Lane::Promotion).expect("fits");
    assert!(
        matches!(outcome(&again, &safety), Outcome::Recomputed { .. }),
        "{:?}",
        outcome(&again, &safety)
    );
    assert_eq!(clean_parity(&again, &widened), AuditVerdict::Parity);
}

#[test]
fn a_reused_record_carries_its_licence_in_its_provenance() {
    let (mut engine, _) = warm(Lane::Promotion);
    let revision = engine
        .revise(&edit("prop-edit"), Lane::Promotion)
        .expect("fits");
    let check = revision
        .get(&Label::named(Stage::CheckInvariant, "NotSolved"))
        .expect("checked");
    // Recomputed from the Conservative exploration: that licence is its provenance.
    assert!(check.provenance.contains(&Triple {
        class: ReuseClass::Conservative,
        function_id: "explore",
        function_version: "1",
    }));
}

#[test]
fn a_validated_witness_source_survives_consecutive_semantic_edits() {
    let (mut engine, _) = warm(Lane::Promotion);
    let widened = edit("sem-domain-widen");
    let first = engine.revise(&widened, Lane::Promotion).expect("fits");
    let safety = Label::of(Stage::DomainSafety);
    assert_eq!(licence(&first, &safety), Some(ReuseClass::Validated));
    let wider = widened.replace("big <= 6", "big <= 7");
    let second = engine.revise(&wider, Lane::Promotion).expect("fits");
    assert_eq!(licence(&second, &safety), Some(ReuseClass::Validated));
    assert_eq!(clean_parity(&second, &wider), AuditVerdict::Parity);
}

#[test]
fn every_query_has_its_own_handle_even_when_two_share_a_key() {
    // Two invariants with one body share every key component of their check.
    let twin = diehard().replace(
        "  invariant TypeOK",
        "  invariant Twin { big != 4 }\n  invariant TypeOK",
    );
    let (mut engine, base) = warm(Lane::Promotion);
    let revision = engine.revise(&twin, Lane::Promotion).expect("fits");
    let handles: std::collections::BTreeSet<&String> =
        revision.records.values().map(|r| &r.handle).collect();
    assert_eq!(handles.len(), revision.records.len());
    assert_eq!(clean_parity(&revision, &twin), AuditVerdict::Parity);

    // After a rename, no handle is both invalidated and reused.
    let renamed = engine
        .revise(&edit("rename-invariant"), Lane::Promotion)
        .expect("fits");
    let why = explain::explain_invalidation(&base, &renamed);
    for entry in &why.reused {
        assert!(
            why.invalidated.iter().all(|i| i.handle != entry.handle),
            "{}",
            entry.label
        );
    }
}

#[test]
fn a_refusal_on_a_warm_engine_leaves_every_later_answer_unchanged() {
    let source = diehard();
    let edited = edit("prop-edit");
    // Reference: warm, then the edit.
    let (mut reference, _) = warm(Lane::Promotion);
    let expected = reference.revise(&edited, Lane::Promotion).expect("fits");

    // Subject: warm, a refused revision in between, then the edit.
    let mut limits = Limits::default();
    let mut engine = Engine::new(Config::default());
    engine.revise(&source, Lane::Promotion).expect("fits");
    let entries = engine.cached_entries();
    limits.queries = 3;
    let mut starved = Engine::new(Config {
        limits,
        ..Config::default()
    });
    assert!(starved.revise(&source, Lane::Promotion).is_err());
    let oversized = "x".repeat(Limits::default().source_bytes + 1);
    assert!(matches!(
        engine.revise(&oversized, Lane::Promotion),
        Err(Refusal::SourceTooLarge { .. })
    ));
    assert_eq!(engine.cached_entries(), entries);
    let after = engine.revise(&edited, Lane::Promotion).expect("fits");
    assert_eq!(after.records, expected.records);
}

#[test]
fn an_invariant_that_cannot_be_evaluated_is_a_typed_outcome_on_both_sides() {
    // `big * 2^62` overflows i64 once big reaches 2: an evaluation error, which
    // must be its own typed verdict, not a hold, a violation, or a failure of the
    // exploration.
    let source = diehard().replace(
        "  invariant TypeOK",
        "  invariant Overflows { big * 4611686018427387904 >= 0 }\n  invariant TypeOK",
    );
    let mut engine = Engine::new(Config::default());
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let clean = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
    let record = revision
        .get(&Label::named(Stage::CheckInvariant, "Overflows"))
        .expect("the invariant elaborates and lowers");
    let prefix = b"invariant-verdict/1";
    assert_eq!(
        record.output.bytes().get(prefix.len()),
        Some(&4),
        "the Evaluation arm"
    );
    // A lowering refusal (division is outside the fragment) demands no check at
    // all, on either side.
    let source = diehard().replace(
        "  invariant TypeOK",
        "  invariant Divides { big / 2 >= 0 }\n  invariant TypeOK",
    );
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    assert!(revision.get(&Label::of(Stage::Explore)).is_none());
    let clean = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
}

// cr-1jv75r th-34rprx: a partial map read is a typed undefined outcome on both
// paths, never `Holds`.
const PARTIAL_MAP: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n}\ninit { m == {} }\naction Stay { unchanged m }\ninvariant Z { m[true] == false }\n";

fn verdict_tag(bytes: &[u8]) -> Option<u8> {
    bytes.get(b"invariant-verdict/1".len()).copied()
}

#[test]
fn an_undefined_map_read_is_undefined_in_the_engine_and_in_the_audit() {
    let z = Label::named(Stage::CheckInvariant, "Z");
    let mut engine = Engine::new(Config::default());
    let revision = engine.revise(PARTIAL_MAP, Lane::Promotion).expect("fits");
    let engine_bytes = revision.get(&z).expect("checked").output.bytes().to_vec();
    // The main predicate is true at the empty map (the absent read decodes to the
    // base value), and `Z#defined` is false: the answer is Undefined (tag 6).
    assert_eq!(verdict_tag(&engine_bytes), Some(6), "engine");
    let clean = audit::clean(
        PARTIAL_MAP,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let clean_bytes = clean.outputs().get(&z).expect("checked");
    assert_eq!(verdict_tag(clean_bytes), Some(6), "audit, independently");
    let holds = continuum_incremental::output::invariant_verdict(
        &continuum_incremental::output::InvariantVerdict::Holds { states: 1 },
    );
    assert_ne!(engine_bytes, holds);
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
}

// cr-1jv75r th-1ty5ws: the canonical graph commits to the whole successor relation.
#[test]
fn two_graphs_with_one_bfs_tree_but_different_non_tree_edges_encode_differently() {
    let model = |back: &str| {
        let src = format!(
            "model G {{\n  state {{\n    x: Nat where x <= 2\n  }}\n  init Init {{ x == 0 }}\n  action Inc {{ x < 2 && x' == x + 1 }}\n  action Back {{ x == 2 && x' == {back} }}\n}}\n"
        );
        let norm = continuum_cml_elab::elaborate_source(&src).expect("elaborates");
        (src, continuum_cml_elab::lower(&norm).expect("lowers"))
    };
    let (src_a, a) = model("0");
    let (src_b, b) = model("1");
    let bounds = continuum_engine_reference::bfs::Bounds::CERTIFIABLE;
    let ea = continuum_engine_reference::bfs::explore(&a, bounds);
    let eb = continuum_engine_reference::bfs::explore(&b, bounds);
    let (ra, rb) = (
        ea.as_ref().expect("closes").reachable(),
        eb.as_ref().expect("closes").reachable(),
    );
    // Same states, depths, first discoveries and transition count ...
    assert_eq!(ra.states(), rb.states());
    assert_eq!(ra.depths(), rb.depths());
    assert_eq!(ra.origins(), rb.origins());
    assert_eq!(ra.transitions(), rb.transitions());
    // ... different graphs, so different canonical encodings.
    assert_ne!(
        continuum_incremental::output::exploration(&ea, &a),
        continuum_incremental::output::exploration(&eb, &b)
    );
    // And the audit sees the difference through the engine.
    let mut engine = Engine::new(Config::default());
    let before = engine.revise(&src_a, Lane::Promotion).expect("fits");
    let after = engine.revise(&src_b, Lane::Promotion).expect("fits");
    assert_ne!(
        before.get(&explore()).expect("explored").output,
        after.get(&explore()).expect("explored").output
    );
    assert_eq!(clean_parity(&after, &src_b), AuditVerdict::Parity);
}

// cr-1jv75r th-3vq8th: a verified witness binds only when every envelope field is
// the expected one.
#[test]
fn a_verified_witness_for_another_envelope_licenses_nothing() {
    use continuum_engine_reference::certificate::{
        ClaimEnvelope, ClosedSet, PRODUCER, emit_finite_closure,
    };
    use continuum_incremental::engine::check_witness;
    let norm = continuum_cml_elab::elaborate_source(&diehard()).expect("elaborates");
    let model = continuum_cml_elab::lower(&norm).expect("lowers");
    let explored = continuum_engine_reference::bfs::explore(
        &model,
        continuum_engine_reference::bfs::Bounds::CERTIFIABLE,
    )
    .expect("explores");
    let closed = ClosedSet::of(&explored).expect("closes");
    let expected = ClaimEnvelope {
        model_digest: "blake3:expected-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "state-domain",
        scope_digest: "incremental-validated-reuse",
        assumptions_digest: "none",
        producer: PRODUCER,
        domain_pack_digests: &[],
    };
    let emit = |envelope: &ClaimEnvelope<'_>| {
        emit_finite_closure(&model, closed, envelope).expect("emits")
    };
    let identity = model.identity();
    assert!(check_witness(&emit(&expected), &expected, identity.as_bytes()).is_ok());
    let packs = ["blake3:some-pack"];
    let mutated: [(&str, ClaimEnvelope<'_>); 7] = [
        (
            "model_digest",
            ClaimEnvelope {
                model_digest: "blake3:other-model",
                ..expected
            },
        ),
        (
            "semantic_epoch",
            ClaimEnvelope {
                semantic_epoch: "continuum-semantics-2",
                ..expected
            },
        ),
        (
            "property_digest",
            ClaimEnvelope {
                property_digest: "other-property",
                ..expected
            },
        ),
        (
            "scope_digest",
            ClaimEnvelope {
                scope_digest: "other-scope",
                ..expected
            },
        ),
        (
            "assumptions_digest",
            ClaimEnvelope {
                assumptions_digest: "some",
                ..expected
            },
        ),
        (
            "producer",
            ClaimEnvelope {
                producer: "other-engine/9",
                ..expected
            },
        ),
        (
            "domain_pack_digests",
            ClaimEnvelope {
                domain_pack_digests: &packs,
                ..expected
            },
        ),
    ];
    for (field, envelope) in &mutated {
        assert_eq!(
            check_witness(&emit(envelope), &expected, identity.as_bytes()),
            Err(WitnessRefusal::WrongClaim(field)),
            "{field}"
        );
    }
    // Bytes no kernel verifies are refused too.
    assert!(matches!(
        check_witness(
            b"CONTCERT-not-a-certificate",
            &expected,
            identity.as_bytes()
        ),
        Err(WitnessRefusal::NotVerified(_))
    ));
}

// Delta review of cr-1jv75r: `domain_safety` reads the exploration, so its class and
// provenance come through the explore edge; a re-derivation that rests on a
// Conservative exploration is never recorded Exact, and quarantining that
// exploration's triple reaches it.
#[test]
fn domain_safety_inherits_the_exploration_class_and_its_quarantine() {
    let (mut engine, _) = warm(Lane::Promotion);
    let edited = edit("prop-edit");
    engine.revise(&edited, Lane::Promotion).expect("fits");
    engine.quarantine(Triple {
        class: ReuseClass::Conservative,
        function_id: "domain_safety",
        function_version: "2",
    });
    let again = engine.revise(&edited, Lane::Promotion).expect("fits");
    let safety = again
        .get(&Label::of(Stage::DomainSafety))
        .expect("demanded");
    let explore_triple = Triple {
        class: ReuseClass::Conservative,
        function_id: "explore",
        function_version: "1",
    };
    assert_ne!(safety.class, ReuseClass::Exact, "{:?}", safety.outcome);
    assert!(safety.provenance.contains(&explore_triple));

    engine.quarantine(explore_triple);
    let last = engine.revise(&edited, Lane::Promotion).expect("fits");
    assert!(matches!(
        outcome(&last, &explore()),
        Outcome::Recomputed { .. }
    ));
    assert!(
        matches!(
            outcome(&last, &Label::of(Stage::DomainSafety)),
            Outcome::Recomputed { .. }
        ),
        "{:?}",
        outcome(&last, &Label::of(Stage::DomainSafety))
    );
    assert_eq!(clean_parity(&last, &edited), AuditVerdict::Parity);
}

// A result recomputed from a reuse under a triple is itself evicted when that
// triple is quarantined: provenance, not only the direct licence.
#[test]
fn a_result_derived_under_a_quarantined_triple_is_rederived() {
    let (mut engine, _) = warm(Lane::Promotion);
    let edited = edit("prop-edit");
    engine.revise(&edited, Lane::Promotion).expect("fits");
    engine.quarantine(Triple {
        class: ReuseClass::Conservative,
        function_id: "explore",
        function_version: "1",
    });
    let again = engine.revise(&edited, Lane::Promotion).expect("fits");
    let check = Label::named(Stage::CheckInvariant, "NotSolved");
    assert!(
        matches!(outcome(&again, &check), Outcome::Recomputed { .. }),
        "{:?}",
        outcome(&again, &check)
    );
}

// The quarantine triple is the edge's licence class, not the entry's producing
// class: a content-identical hit is an Exact licence (with the entry's weaker
// class kept on the record), and quarantining (Exact, f) reaches it.
#[test]
fn the_triple_names_the_edge_licence_and_the_record_keeps_the_origin_class() {
    let (mut engine, _) = warm(Lane::Promotion);
    let edited = edit("prop-edit");
    engine.revise(&edited, Lane::Promotion).expect("fits");
    let again = engine.revise(&edited, Lane::Promotion).expect("fits");
    let check = Label::named(Stage::CheckInvariant, "NotSolved");
    let record = again.get(&check).expect("checked");
    let Outcome::Reused {
        licence: class,
        evidence,
        triple,
        assumptions,
    } = &record.outcome
    else {
        panic!("reused: {:?}", record.outcome);
    };
    assert_eq!(*class, ReuseClass::Exact);
    assert_eq!(*evidence, Some(EvidenceForm::ContentIdentity));
    assert_eq!(triple.class, ReuseClass::Exact);
    assert!(assumptions.is_empty());
    assert_eq!(record.class, ReuseClass::Conservative, "origin class kept");

    engine.quarantine(*triple);
    let last = engine.revise(&edited, Lane::Promotion).expect("fits");
    assert!(matches!(outcome(&last, &check), Outcome::Recomputed { .. }));
}

// An undefined read in an action's guard is a typed outcome on both paths.
const PARTIAL_ACTION: &str = "module M\nstate {\n  m: Map[Bool, Bool]\n  b: Nat where b <= 1\n}\ninit { m == {} && b == 0 }\naction Flip {\n  require m[true] == false\n  next b = 1\n  unchanged m\n}\ninvariant NeverB { b == 0 }\n";

#[test]
fn an_undefined_read_in_an_action_is_a_typed_outcome_in_the_engine_and_in_the_audit() {
    let never_b = Label::named(Stage::CheckInvariant, "NeverB");
    let safety = Label::of(Stage::DomainSafety);
    let mut engine = Engine::new(Config::default());
    let revision = engine
        .revise(PARTIAL_ACTION, Lane::Promotion)
        .expect("fits");
    let clean = audit::clean(
        PARTIAL_ACTION,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    // Invariant: UndefinedAction (tag 7), not Holds, on both sides.
    assert_eq!(
        verdict_tag(revision.get(&never_b).expect("checked").output.bytes()),
        Some(7)
    );
    assert_eq!(
        verdict_tag(clean.outputs().get(&never_b).expect("checked")),
        Some(7)
    );
    // Domain safety: UndefinedAction (tag 3), not Established, on both sides.
    let tag = |bytes: &[u8]| bytes.get(b"domain-safety/1".len()).copied();
    assert_eq!(
        tag(revision.get(&safety).expect("demanded").output.bytes()),
        Some(3)
    );
    assert_eq!(
        tag(clean.outputs().get(&safety).expect("demanded")),
        Some(3)
    );
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
}

// The Validated witness refuses a set where an action's read is undefined: without
// that scan a stale `Established` would pass the kernel (closure says nothing about
// definedness).
#[test]
fn a_validated_witness_is_refused_when_an_action_read_becomes_undefined() {
    let v1 = PARTIAL_ACTION.replace("require m[true] == false", "require b == 1");
    let mut engine = Engine::new(Config::default());
    engine.revise(&v1, Lane::Promotion).expect("fits");
    let revision = engine
        .revise(PARTIAL_ACTION, Lane::Promotion)
        .expect("fits");
    assert_eq!(
        outcome(&revision, &Label::of(Stage::DomainSafety)),
        &Outcome::Recomputed {
            cause: Cause::WitnessRefused(WitnessRefusal::UndefinedAction),
            memo: Memo::Stored,
        }
    );
    assert_eq!(
        clean_parity(&revision, PARTIAL_ACTION),
        AuditVerdict::Parity
    );
}

// Precedence: an undefined action read outranks an undefined invariant read, even
// when the invariant's is shallower, on both paths.
#[test]
fn an_undefined_action_read_outranks_a_shallower_undefined_invariant_read() {
    let source = "module M\nstate {\n  m: Map[Bool, Bool]\n  b: Nat where b <= 1\n}\ninit { m == {true -> false} && b == 0 }\naction Drop {\n  require b == 0\n  next m = {}\n  next b = 1\n}\naction Read {\n  require m[true] == false\n  unchanged m, b\n}\ninvariant Other { m[false] == false }\n";
    let other = Label::named(Stage::CheckInvariant, "Other");
    let mut engine = Engine::new(Config::default());
    let revision = engine.revise(source, Lane::Promotion).expect("fits");
    let clean = audit::clean(
        source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    // `Other` is undefined at the initial state (depth 0); `Read` only after
    // `Drop` (depth 1). UndefinedAction (tag 7) on both sides.
    assert_eq!(
        verdict_tag(revision.get(&other).expect("checked").output.bytes()),
        Some(7)
    );
    assert_eq!(
        verdict_tag(clean.outputs().get(&other).expect("checked")),
        Some(7)
    );
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
}

// A TRUE certificate about a DIFFERENT model, under the expected envelope, binds to
// nothing: the kernel verifies it (it is a real closure of its carried model), and
// the carried model's identity is not the one the engine asked about.
#[test]
fn a_true_witness_about_another_model_licenses_nothing() {
    use continuum_engine_reference::certificate::{
        ClaimEnvelope, ClosedSet, PRODUCER, emit_finite_closure, emit_invariant_closure,
    };
    use continuum_incremental::engine::check_witness;
    let lowered = |source: &str| {
        let norm = continuum_cml_elab::elaborate_source(source).expect("elaborates");
        continuum_cml_elab::lower(&norm).expect("lowers")
    };
    let asked = lowered(&diehard());
    let other = lowered(&edit("sem-domain-widen"));
    let bounds = continuum_engine_reference::bfs::Bounds::CERTIFIABLE;
    let explored = continuum_engine_reference::bfs::explore(&other, bounds).expect("explores");
    let closed = ClosedSet::of(&explored).expect("closes");
    let expected = ClaimEnvelope {
        model_digest: "blake3:expected-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "state-domain",
        scope_digest: "incremental-validated-reuse",
        assumptions_digest: "none",
        producer: PRODUCER,
        domain_pack_digests: &[],
    };
    let bytes = emit_finite_closure(&other, closed, &expected).expect("emits");
    // True of its own model ...
    assert!(check_witness(&bytes, &expected, other.identity().as_bytes()).is_ok());
    // ... and refused for the model the engine asked about.
    assert_eq!(
        check_witness(&bytes, &expected, asked.identity().as_bytes()),
        Err(WitnessRefusal::WrongClaim("model_identity"))
    );
    // A true certificate of a class this licence does not handle is refused too.
    let invariant = emit_invariant_closure(&other, closed, &expected, "TypeOK").expect("emits");
    assert_eq!(
        check_witness(&invariant, &expected, other.identity().as_bytes()),
        Err(WitnessRefusal::WrongClaim("property"))
    );
}

// cr-174gtd: an audit over a run that did not complete within its bounds is
// Inconclusive, never Parity (RFC 0030: budget exhaustion is not a parity
// outcome; INV-008).
fn audit_under(limits: Limits, source: &str) -> audit::ParityReport {
    let mut engine = Engine::new(Config {
        limits,
        ..Config::default()
    });
    let revision = engine.revise(source, Lane::Promotion).expect("fits");
    audit::parity(
        &revision,
        &audit::clean(
            source,
            &limits,
            continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
        ),
    )
}

#[test]
fn an_elaboration_budget_failure_is_inconclusive_not_parity() {
    let limits = Limits {
        elab: continuum_cml_elab::Limits {
            nodes: 4,
            work: 1 << 20,
        },
        ..Limits::default()
    };
    let report = audit_under(limits, &diehard());
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(report.first.is_none(), "nothing is blamed on a budget");
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::FrontEndBudget(
                "cml.limit.elaboration_too_large"
            ))
    );
}

#[test]
fn a_lowering_budget_failure_is_inconclusive_not_parity() {
    // Elaboration fits (about 1,221 units); lowering (about 1,313) does not.
    let limits = Limits {
        elab: continuum_cml_elab::Limits {
            nodes: 1 << 20,
            work: 1250,
        },
        ..Limits::default()
    };
    let report = audit_under(limits, &diehard());
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::FrontEndBudget(
                "cml.lower.work_limit_exceeded"
            ))
    );
}

#[test]
fn a_bounded_exploration_is_inconclusive_not_parity() {
    let limits = Limits {
        explore: continuum_engine_reference::bfs::Bounds::CERTIFIABLE.with_states(4),
        ..Limits::default()
    };
    let report = audit_under(limits, &diehard());
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::ExplorationBound)
    );
}

#[test]
fn a_clean_run_that_exhausts_its_work_budget_is_inconclusive() {
    let source = diehard();
    let mut engine = Engine::new(Config::default());
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    // Enough to parse and elaborate, far too little for the exploration charge.
    let tight = Limits {
        work: 1 << 20,
        ..Limits::default()
    };
    let clean = audit::clean(
        &source,
        &tight,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(
        *clean.completion(),
        audit::Completion::Incomplete(audit::Incompleteness::WorkExhausted { at: "explore" })
    );
    assert!(clean.work() <= 1 << 20);
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Inconclusive
    );
}

#[test]
fn a_clean_run_over_a_huge_input_is_refused_before_any_work() {
    let limits = Limits::default();
    let huge = "x".repeat(limits.source_bytes + 1);
    let clean = audit::clean(
        &huge,
        &limits,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(
        *clean.completion(),
        audit::Completion::Incomplete(audit::Incompleteness::SourceTooLarge {
            bytes: limits.source_bytes + 1,
            max: limits.source_bytes,
        })
    );
    assert_eq!(clean.work(), 0);
    assert!(clean.outputs().is_empty());
}

#[test]
fn a_clean_run_with_too_many_invariants_is_refused_before_per_invariant_work() {
    let source = diehard();
    // Two invariants define 6 + 2 * 2 = 10 queries.
    let limits = Limits {
        queries: 9,
        ..Limits::default()
    };
    let clean = audit::clean(
        &source,
        &limits,
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(
        *clean.completion(),
        audit::Completion::Incomplete(audit::Incompleteness::TooManyQueries { needed: 10, max: 9 })
    );
    assert!(
        !clean.outputs().contains_key(&explore()),
        "no exploration ran"
    );
    assert_eq!(clean.explored_states(), 0);
}

// Delta review of cr-174gtd: a state bound that cannot hold the initial states is
// the bound's answer, so the audit is Inconclusive, not Parity.
#[test]
fn initial_states_beyond_the_state_bound_are_inconclusive_not_parity() {
    let source = "model G {\n  state {\n    x: Nat where x <= 2\n  }\n  init Init { x <= 2 }\n  action Stay { x' == x }\n}\n";
    let limits = Limits {
        explore: continuum_engine_reference::bfs::Bounds::CERTIFIABLE.with_states(1),
        ..Limits::default()
    };
    let report = audit_under(limits, source);
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::ExplorationBound)
    );
}

// The engine side alone makes the audit inconclusive: a bounded engine exploration
// against a complete clean run, and a budget failure in the engine against a
// complete clean run.
#[test]
fn an_incomplete_engine_run_alone_makes_the_audit_inconclusive() {
    let source = diehard();
    let complete = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    assert_eq!(complete.completion(), &audit::Completion::Complete);

    let mut bounded = Engine::new(Config {
        limits: Limits {
            explore: continuum_engine_reference::bfs::Bounds::CERTIFIABLE.with_states(4),
            ..Limits::default()
        },
        ..Config::default()
    });
    let revision = bounded.revise(&source, Lane::Promotion).expect("fits");
    let report = audit::parity(&revision, &complete);
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    // The clean run is complete (on other limits, its own reason); the bound is
    // found from the engine side alone.
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::ExplorationBound)
    );

    let mut starved = Engine::new(Config {
        limits: Limits {
            elab: continuum_cml_elab::Limits {
                nodes: 4,
                work: 1 << 20,
            },
            ..Limits::default()
        },
        ..Config::default()
    });
    let revision = starved.revise(&source, Lane::Promotion).expect("fits");
    let report = audit::parity(&revision, &complete);
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::EngineBudget(Label::of(
                Stage::Elaborate
            )))
    );
}

// The cone is not judged against a clean run that stopped: no false
// under-invalidation from its missing artifacts.
#[test]
fn the_cone_is_not_judged_against_an_incomplete_clean_run() {
    let source = diehard();
    let (mut engine, _) = warm(Lane::Promotion);
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let full = audit::clean(
        &source,
        &Limits::default(),
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let stopped = audit::clean(
        &source,
        &Limits {
            work: 1 << 20,
            ..Limits::default()
        },
        continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH,
    );
    let cone = audit::cone(&full, &stopped, &revision, &revision);
    assert!(cone.under_invalidated.is_empty());
    assert!(cone.true_cone.is_empty());
    assert!(
        cone.incomplete
            .contains(&audit::Incompleteness::WorkExhausted { at: "explore" })
    );
}

// The audit's and the engine's budget-failure codes are one list, and it is exactly
// the front end's budget codes: a drift in any of the three fails here.
#[test]
fn the_budget_code_lists_agree_with_each_other_and_the_front_end() {
    let codes = |text: &str| -> std::collections::BTreeSet<String> {
        text.split('"')
            .filter(|t| {
                t.starts_with("cml.") && (t.contains("too_large") || t.contains("work_limit"))
            })
            .map(str::to_owned)
            .collect()
    };
    let engine = include_str!("../src/engine.rs");
    let engine_list = &engine[engine.find("const BUDGET_CODES").expect("declared")..];
    let engine_list = &engine_list[..engine_list.find("];").expect("closed")];
    let audit = include_str!("../src/audit.rs");
    let audit_list = &audit[audit.find("fn front_end_budget").expect("declared")..];
    let audit_list = &audit_list[..audit_list.find(".then_some").expect("closed")];
    let front_end = format!(
        "{}{}",
        include_str!("../../continuum-cml-elab/src/error.rs"),
        include_str!("../../continuum-cml-elab/src/lower.rs")
    );
    // The front end's budget codes: its elaboration limits and its lowering's
    // output and work limits (the other `too_large` codes are fixed-domain
    // refusals, the model's answer).
    let front: std::collections::BTreeSet<String> = codes(&front_end)
        .into_iter()
        .filter(|c| {
            c.starts_with("cml.limit.")
                || c == "cml.lower.output_too_large"
                || c == "cml.lower.work_limit_exceeded"
        })
        .collect();
    assert_eq!(codes(engine_list), codes(audit_list));
    assert_eq!(codes(engine_list), front);
}

// cr-174gtd round 2: the audit derives its own basis; clean evidence of another
// source, other limits, or another epoch never yields Parity.
#[test]
fn clean_evidence_on_another_basis_is_inconclusive_never_parity() {
    use continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH;
    let source = diehard();
    let mut engine = Engine::new(Config::default());
    let revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let on = |source: &str, limits: &Limits, epoch: &str| {
        audit::parity(&revision, &audit::clean(source, limits, epoch))
    };
    assert_eq!(
        on(&source, &Limits::default(), DEFAULT_SEMANTIC_EPOCH).verdict(),
        AuditVerdict::Parity
    );
    // Whitespace-equivalent source: every compared output is byte-equal, and the
    // basis still differs.
    let spaced = edit("ws-trailing");
    let report = on(&spaced, &Limits::default(), DEFAULT_SEMANTIC_EPOCH);
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::BasisMismatch("source"))
    );
    // Equal outputs under other limits.
    let wider = Limits {
        work: Limits::default().work + 1,
        ..Limits::default()
    };
    let report = on(&source, &wider, DEFAULT_SEMANTIC_EPOCH);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::BasisMismatch("limits"))
    );
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    // Another epoch.
    let report = on(&source, &Limits::default(), "continuum-semantics-9");
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::BasisMismatch("semantic_epoch"))
    );
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
}

// The cone uses the same completeness check: an engine-only incompleteness leaves
// it unjudged, with complete clean runs on the same basis.
#[test]
fn the_cone_is_not_judged_for_an_incomplete_engine_revision() {
    use continuum_incremental::engine::Memo;
    use continuum_incremental::query::DEFAULT_SEMANTIC_EPOCH;
    let source = diehard();
    let edited = edit("prop-edit");
    let (mut engine, base) = warm(Lane::Promotion);
    let mut revision = engine.revise(&edited, Lane::Promotion).expect("fits");
    let before = audit::clean(&source, &Limits::default(), DEFAULT_SEMANTIC_EPOCH);
    let after = audit::clean(&edited, &Limits::default(), DEFAULT_SEMANTIC_EPOCH);
    assert_eq!(before.completion(), &audit::Completion::Complete);
    assert_eq!(after.completion(), &audit::Completion::Complete);
    // Judged as is.
    assert!(
        audit::cone(&before, &after, &base, &revision)
            .incomplete
            .is_empty()
    );
    // An engine result that failed on a budget, alone, leaves it unjudged.
    let label = Label::named(Stage::CheckInvariant, "NotSolved");
    revision.records.get_mut(&label).expect("checked").outcome = Outcome::Recomputed {
        cause: Cause::KeyMiss,
        memo: Memo::NotMemoized("budget-dependent failure"),
    };
    let cone = audit::cone(&before, &after, &base, &revision);
    assert!(cone.under_invalidated.is_empty() && cone.true_cone.is_empty());
    assert_eq!(
        cone.incomplete,
        vec![audit::Incompleteness::EngineBudget(label)]
    );
    // A `before` run of another source is a basis mismatch, never a judged cone.
    let cone = audit::cone(&after, &after, &base, &revision);
    assert!(
        cone.incomplete
            .contains(&audit::Incompleteness::BasisMismatch("source"))
    );
}

// The audited versions are exactly the registry's compared definitions, and a
// record naming another version is a basis mismatch.
#[test]
fn the_audit_refuses_a_revision_of_another_definition_version() {
    use continuum_incremental::query::{
        Compared, DEFAULT_SEMANTIC_EPOCH, QueryDefinition, REGISTRY,
    };
    let registry: Vec<(Stage, &str)> = REGISTRY
        .iter()
        .filter(|d| d.stage.compared() == Compared::Yes)
        .map(|d| (d.stage, d.function_version))
        .collect();
    assert_eq!(registry, audit::AUDITED_VERSIONS.to_vec());

    let source = diehard();
    let mut engine = Engine::new(Config::default());
    let mut revision = engine.revise(&source, Lane::Promotion).expect("fits");
    let clean = audit::clean(&source, &Limits::default(), DEFAULT_SEMANTIC_EPOCH);
    assert_eq!(
        audit::parity(&revision, &clean).verdict(),
        AuditVerdict::Parity
    );
    let record = revision.records.get_mut(&explore()).expect("explored");
    let bumped: &'static QueryDefinition = Box::leak(Box::new(QueryDefinition {
        function_version: "9",
        ..*record.definition
    }));
    record.definition = bumped;
    let report = audit::parity(&revision, &clean);
    assert_eq!(report.verdict(), AuditVerdict::Inconclusive);
    assert!(
        report
            .incomplete
            .contains(&audit::Incompleteness::BasisMismatch("function_version"))
    );
}

// The normalized identity is charged by its stated bound before it is built: a
// tight budget refuses there, repeatedly, and the bound holds on every corpus and
// fuzz-corpus model that elaborates.
#[test]
fn the_normalized_identity_is_precharged_and_its_bound_holds() {
    let source = diehard();
    let file = continuum_cml_syntax::parse(&source).expect("parses");
    let (norm, usage) =
        continuum_cml_elab::elaborate_with(&file, continuum_cml_elab::Limits::default());
    let norm = norm.expect("elaborates");
    let bound = continuum_incremental::output::norm_identity_bound(usage.nodes, source.len());
    // A budget that covers parsing, the dump and the elaboration charge but not the
    // identity bound refuses at the identity, every time.
    let parse_charges = (source.len() as u64) * (1 + 1 + 2 * 64 + 2) + 4096;
    let mut engine = Engine::new(Config {
        limits: Limits {
            work: parse_charges + (bound as u64) / 2,
            ..Limits::default()
        },
        ..Config::default()
    });
    for _ in 0..3 {
        assert!(matches!(
            engine.revise(&source, Lane::Promotion),
            Err(Refusal::WorkExhausted {
                at: "normalized identity",
                ..
            })
        ));
        assert_eq!(engine.cached_entries(), 0);
    }
    assert!(norm.identity().as_bytes().len() <= bound);

    let mut roots = vec![
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan")),
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../continuum-cml-elab/tests"
        )),
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../continuum-cml-syntax/tests"
        )),
    ];
    let mut checked = 0;
    while let Some(dir) = roots.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                roots.push(path);
            } else if path.extension().is_some_and(|e| e == "ctm") {
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(file) = continuum_cml_syntax::parse(&text) else {
                    continue;
                };
                let (norm, usage) = continuum_cml_elab::elaborate_with(
                    &file,
                    continuum_cml_elab::Limits::default(),
                );
                if let Ok(norm) = norm {
                    let bound =
                        continuum_incremental::output::norm_identity_bound(usage.nodes, text.len());
                    assert!(
                        norm.identity().as_bytes().len() <= bound,
                        "{}: {} > {bound}",
                        path.display(),
                        norm.identity().as_bytes().len()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 10, "checked {checked} models");

    // The escape worst case: a `def` returning a string of control characters,
    // inlined many times (each byte renders as `\u{7f}`, six bytes).
    let chars = "\u{7f}".repeat(4000);
    let mut adversarial = format!(
        "module S\nstate {{\n  x: Nat where x <= 1\n}}\ninit {{ x == 0 }}\ndef s(n: Nat): String = \"{chars}\"\naction Stay {{ unchanged x }}\n"
    );
    for i in 0..100 {
        adversarial.push_str(&format!("invariant I{i} {{ s(0) == s(0) }}\n"));
    }
    let file = continuum_cml_syntax::parse(&adversarial).expect("parses");
    let (norm, usage) =
        continuum_cml_elab::elaborate_with(&file, continuum_cml_elab::Limits::default());
    let norm = norm.expect("elaborates");
    let bound = continuum_incremental::output::norm_identity_bound(usage.nodes, adversarial.len());
    assert!(
        norm.identity().as_bytes().len() <= bound,
        "escaped strings: {} > {bound}",
        norm.identity().as_bytes().len()
    );
}
