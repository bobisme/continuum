//! PR-16/IMPL-05: the replicated register's required mutants, each with its expected
//! intent and a deterministic campaign derived from the correct version's
//! `pr16-correct-baseline`. START_HERE PR 16, fifth bullet ("ack-before-sync, lost-abort,
//! stale-epoch, and orphan mutants") and the PR 16 exit ("each mutant has an expected
//! intent/property and deterministic campaign"); bn-28oa.
//!
//! `tests/support/register_mutants.rs` defines the ten mutants of
//! `replicated_register.md`, how each is written as a change to the correct program, and
//! what its campaign is expected to show ([`Expected`]). This file runs the nine campaigns
//! (the baseline and the eight mutants that change the program) in parallel, once, and
//! holds each to its expectation.
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr16_impl05_mutants.evidence.txt`. Regenerate with
//! `PR16_IMPL05_BLESS=1 cargo test -p continuum-asupersync --test pr16_impl05_mutants`,
//! and review the diff. `pr16-impl05-mut-00-baseline` is the baseline campaign, which
//! stays at zero findings with IMPL-04's identity; `pr16-impl05-mut-NN-<slug>` is mutant
//! `MNN`: the defect, the mutation, the expected result, the campaign and its identity,
//! the result, and the replayed witness.
//!
//! # Scope
//!
//! As the baseline's: the explored runs only, most logs sampled, not DPOR. A witness is
//! the shallowest failing run the campaign found, replayed from its plan and log, and not
//! minimized. Two mutants (M03, M08) are excluded by the substrate rather than detected,
//! and two (M09, M10) are not program mutants and are deferred to their owners.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

#[path = "support/register_baseline.rs"]
#[allow(dead_code)]
mod baseline;

#[path = "support/register_mutants.rs"]
#[allow(dead_code)]
mod mutants;

use baseline::{Campaign, Finding, Outcome, Property, render_plan};
use mutants::{Exclusion, Expected, Id, Mutant, Refusal, Symptom, Witness};

struct All {
    base: Campaign,
    baseline: Outcome,
    mutants: BTreeMap<Id, (Mutant, Outcome)>,
}

/// The baseline and every mutant campaign, run once, in parallel.
fn all() -> &'static All {
    static CELL: OnceLock<All> = OnceLock::new();
    CELL.get_or_init(|| {
        let base = baseline::baseline();
        let ms: Vec<Mutant> = Id::ALL
            .iter()
            .filter_map(|id| mutants::mutant(*id, &base))
            .collect();
        std::thread::scope(|s| {
            let b = s.spawn(|| baseline::execute(&base));
            let hs: Vec<_> = ms
                .iter()
                .map(|m| s.spawn(move || mutants::execute(m)))
                .collect();
            let mutants = ms
                .iter()
                .cloned()
                .zip(hs)
                .map(|(m, h)| (m.id, (m, h.join().expect("a campaign"))))
                .collect();
            All {
                baseline: b.join().expect("the baseline"),
                base: base.clone(),
                mutants,
            }
        })
    })
}

fn get(id: Id) -> &'static (Mutant, Outcome) {
    &all().mutants[&id]
}

fn repo(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Per plan of a mutant campaign: `(group, plan, changed, runs, failing runs)`.
fn per_plan(m: &Mutant, o: &Outcome) -> Vec<(usize, usize, bool, usize, usize)> {
    let base = &all().base;
    o.plans
        .iter()
        .map(|p| {
            let gi = base
                .groups
                .iter()
                .position(|g| g.id == p.group)
                .expect("a group");
            (
                gi,
                p.index,
                mutants::changed(m, base, gi, p.index),
                p.runs,
                p.findings.len(),
            )
        })
        .collect()
}

/// What one mutant campaign shows, for the tests and the evidence.
#[derive(Debug)]
struct Summary {
    plans: usize,
    changed: usize,
    failing: usize,
    runs: usize,
    changed_runs: usize,
    failing_runs: usize,
    refused_runs: usize,
    detecting_plans: usize,
    detecting_runs: usize,
    witness: Option<Witness>,
    also: Option<Witness>,
}

fn summary(id: Id) -> Summary {
    let (m, o) = get(id);
    let rows = per_plan(m, o);
    let (symptom, also) = match mutants::expected(id) {
        Expected::Detected { symptom, also, .. } => (Some(symptom), also),
        _ => (None, None),
    };
    let refused_runs = o
        .plans
        .iter()
        .flat_map(|p| &p.findings)
        .filter(|(_, f)| f.iter().any(|f| matches!(f, Finding::Refused(_))))
        .count();
    // Runs that refute a scenario property, not counting a run that is only a refusal.
    let detects = |f: &Vec<Finding>| f.iter().any(|f| f.property() != Property::Run);
    let detecting_runs = o
        .plans
        .iter()
        .flat_map(|p| &p.findings)
        .filter(|(_, f)| detects(f))
        .count();
    let detecting_plans = o
        .plans
        .iter()
        .filter(|p| p.findings.iter().any(|(_, f)| detects(f)))
        .count();
    let witness = match (symptom, mutants::expected(id)) {
        (Some(s), _) => mutants::witness(m, o, |f| s.matches(f), |_, _| true),
        (
            None,
            Expected::Excluded {
                exclusion: Exclusion::EpochFenced,
                ..
            },
        ) => mutants::witness(
            m,
            o,
            |f| matches!(f, Finding::Refused(r) if Refusal::TaskEnded.matches(r)),
            |_, _| true,
        ),
        _ => None,
    };
    // M01's second witness is on the scenario plan and follows `replicated_register.md`'s
    // causal core ([`causal_core`]); any other's is the shallowest.
    let also = also.and_then(|s| {
        mutants::witness_where(
            m,
            o,
            |f| s.matches(f),
            |g, p| id != Id::M01 || (g, p) == (0, 0),
            |w| id != Id::M01 || causal_core(&mutants::story(m, w)),
        )
    });
    Summary {
        plans: rows.len(),
        changed: rows.iter().filter(|r| r.2).count(),
        failing: rows.iter().filter(|r| r.4 > 0).count(),
        runs: o.runs,
        changed_runs: rows.iter().filter(|r| r.2).map(|r| r.3).sum(),
        failing_runs: rows.iter().map(|r| r.4).sum(),
        refused_runs,
        detecting_plans,
        detecting_runs,
        witness,
        also,
    }
}

/// Whether a run's durable-register steps follow M01's expected causal core in
/// `replicated_register.md`: `v0` is acknowledged, then the crash loses `a`'s `v0`
/// bytes, then `v1` is acknowledged for the same epoch. It checks this order only, not
/// the spec's cancellation request before the first ack.
fn causal_core(story: &[String]) -> bool {
    let at = |l: &str| story.iter().position(|s| s == l);
    match (
        at("Ack(epoch=0,value=v0)"),
        at("Lose(n=a,epoch=0,value=v0)"),
        at("Ack(epoch=0,value=v1)"),
    ) {
        (Some(first), Some(lost), Some(second)) => first < lost && lost < second,
        _ => false,
    }
}

fn summaries() -> &'static BTreeMap<Id, Summary> {
    static CELL: OnceLock<BTreeMap<Id, Summary>> = OnceLock::new();
    CELL.get_or_init(|| {
        Id::ALL
            .iter()
            .filter(|id| all().mutants.contains_key(id))
            .map(|id| (*id, summary(*id)))
            .collect()
    })
}

// ---------------------------------------------------------------------------
// the baseline and the campaigns' derivation
// ---------------------------------------------------------------------------

/// The `campaign identity` line of IMPL-04's golden.
fn impl04_identity() -> String {
    repo("crates/continuum-asupersync/tests/golden/pr16_impl04_correct_version.evidence.txt")
        .lines()
        .find_map(|l| l.strip_prefix("campaign identity "))
        .expect("IMPL-04's golden names its identity")
        .to_owned()
}

/// mut-00: the baseline the mutants are run beside has no finding, and it is IMPL-04's
/// campaign, by identity.
#[test]
fn mut_00_the_baseline_stays_at_zero_findings_with_impl04s_identity() {
    let a = all();
    assert_eq!(a.baseline.by_property, BTreeMap::new());
    assert!(a.baseline.plans.iter().all(|p| p.findings.is_empty()));
    assert_eq!(a.baseline.breaches, BTreeMap::new());
    assert_eq!(a.baseline.digest, impl04_identity());
    assert_eq!(
        a.baseline.tally.timers, [0; 4],
        "the correct program sets no timer"
    );
}

/// Every mutant campaign is the baseline's under a new name: same seed, bounds, groups and
/// log counts, and the same plans before the mutation.
#[test]
fn every_mutant_campaign_is_derived_from_the_baseline() {
    let a = all();
    assert_eq!(a.base.name, baseline::BASELINE);
    for (id, (m, o)) in &a.mutants {
        let c = &m.campaign;
        assert_eq!(c.name, id.name());
        assert_eq!(c.seed, a.base.seed);
        assert_eq!(c.bounds, a.base.bounds);
        let shape = |c: &Campaign| -> Vec<(&str, usize, usize)> {
            c.groups
                .iter()
                .map(|g| (g.id, g.logs, g.plans.len()))
                .collect()
        };
        assert_eq!(shape(c), shape(&a.base), "{id:?}");
        if !m.plan_mutant {
            for (g, h) in c.groups.iter().zip(&a.base.groups) {
                for (p, q) in g.plans.iter().zip(&h.plans) {
                    assert_eq!(render_plan(p), render_plan(q), "{id:?}");
                }
            }
        }
        assert_eq!(o.plans.len(), a.baseline.plans.len());
        assert_ne!(o.digest, a.baseline.digest, "{id:?}: its own identity");
    }
    // The deferred mutants have no campaign.
    for id in [Id::M09, Id::M10] {
        assert!(mutants::mutant(id, &a.base).is_none());
        assert!(matches!(mutants::expected(id), Expected::Deferred { .. }));
    }
}

/// A mutant campaign is deterministic and its seed is load-bearing: the same campaign
/// twice has the same identity, and another seed another one. (The full campaigns'
/// identities are pinned by the golden.)
#[test]
fn a_mutant_campaign_is_deterministic_and_its_seed_is_load_bearing() {
    let a = all();
    let mut m = mutants::mutant(Id::M01, &a.base).expect("M01");
    m.campaign.groups.truncate(1);
    m.campaign.groups[0].plans.truncate(1);
    m.campaign.groups[0].logs = 20;
    let once = mutants::execute(&m);
    assert!(once.by_property.contains_key(&Property::Agreement));
    assert_eq!(mutants::execute(&m).digest, once.digest);
    m.campaign.seed ^= 1;
    assert_ne!(mutants::execute(&m).digest, once.digest);
}

/// The mutant table is `replicated_register.md`'s, row for row, and START_HERE's four
/// names are the slugs of M01, M02, M03 and M06.
#[test]
fn the_mutants_are_the_specs() {
    let md = repo("notes/plan/examples/replicated_register.md");
    let rows: Vec<(String, String)> = md
        .lines()
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            (cells.len() == 4 && cells[1].starts_with('M') && cells[1].len() == 3)
                .then(|| (cells[1].to_owned(), cells[2].to_owned()))
        })
        .collect();
    let ours: Vec<(String, String)> = Id::ALL
        .iter()
        .map(|id| (format!("{id:?}"), id.defect().to_owned()))
        .collect();
    assert_eq!(rows, ours);
    for (id, n) in Id::ALL.iter().zip(1..) {
        assert_eq!(id.number(), n);
        assert!(id.name().starts_with(&format!("pr16-impl05-mut-{n:02}-")));
    }
    let start = repo("notes/plan/notes/START_HERE_IMPLEMENTATION.md");
    assert!(start.contains("ack-before-sync, lost-abort, stale-epoch, and orphan mutants"));
    for (id, slug) in [
        (Id::M01, "ack-before-sync"),
        (Id::M02, "lost-abort"),
        (Id::M03, "stale-epoch"),
        (Id::M06, "orphan"),
    ] {
        assert!(id.name().ends_with(slug), "{id:?}");
    }
}

// ---------------------------------------------------------------------------
// the detected mutants
// ---------------------------------------------------------------------------

/// Hold a detected mutant's campaign to its expectation.
fn check_detected(id: Id) {
    let Expected::Detected {
        breach,
        symptom,
        also,
        refusal,
    } = mutants::expected(id)
    else {
        panic!("{id:?} is expected to be detected");
    };
    let (m, o) = get(id);
    let s = &summaries()[&id];
    // The mutant changes some plans, and each changed plan of a plan mutant breaks the
    // named protocol rule first. A program mutant's plans are the correct ones.
    assert!(s.changed > 0, "{id:?} changes something");
    match breach {
        Some(b) => assert_eq!(o.breaches, BTreeMap::from([(b, s.changed)]), "{id:?}"),
        None => assert_eq!(o.breaches, BTreeMap::new(), "{id:?}"),
    }
    // Findings come only from changed plans: the mutation, not the campaign, is what
    // fails.
    for (g, p, changed, _, failing) in per_plan(m, o) {
        assert!(
            changed || failing == 0,
            "{id:?}: unchanged plan {g}/{p} fails"
        );
    }
    // The properties refuted are exactly the expected ones, and the only runs that are
    // not runs are the expected typed refusal.
    let mut want = BTreeSet::from([symptom.property()]);
    want.extend(also.map(Symptom::property));
    if refusal.is_some() {
        want.insert(Property::Run);
    }
    assert_eq!(
        o.by_property.keys().copied().collect::<BTreeSet<_>>(),
        want,
        "{id:?}"
    );
    for (_, fs) in o.plans.iter().flat_map(|p| &p.findings) {
        for f in fs {
            if f.property() == Property::Run {
                let Finding::Refused(r) = f else {
                    panic!("{id:?}: {f:?}")
                };
                assert!(refusal.is_some_and(|x| x.matches(r)), "{id:?}: {r}");
            }
        }
    }
    assert!(s.detecting_runs > 0 && s.detecting_plans > 0, "{id:?}");
    // The witness: a failing run with the symptom, replayed from its plan and log alone
    // to the findings the campaign recorded for it; two replays give one journal.
    let w = s.witness.as_ref().expect("a witness");
    assert!(symptom.matches(&w.finding));
    let (r1, raw1) = mutants::replay(m, w);
    let (r2, raw2) = mutants::replay(m, w);
    assert_eq!(r1.findings, w.findings, "{id:?}: replay reproduces the run");
    assert_eq!((r1.digest, raw1), (r2.digest, raw2));
    if let Some(also) = also {
        let w = s.also.as_ref().expect("a second witness");
        assert!(also.matches(&w.finding));
        assert_eq!(mutants::replay(m, w).0.findings, w.findings);
    }
}

/// mut-01, ack-before-sync: RuntimeToAbstract (an ack with no durable majority), and on
/// the scenario plan Agreement, through `replicated_register.md`'s causal core.
#[test]
fn mut_01_ack_before_sync_is_detected_through_the_causal_core() {
    check_detected(Id::M01);
    let (m, _) = get(Id::M01);
    let w = summaries()[&Id::M01]
        .also
        .as_ref()
        .expect("an Agreement witness");
    assert_eq!((w.group.1, w.plan), ("scenario", 0));
    // The causal core: `v0` is acknowledged over `a`'s volatile bytes, the crash loses
    // them, and `v1` is acknowledged for the same epoch.
    assert!(causal_core(&mutants::story(m, w)));
    assert!(!causal_core(&[
        "Lose(n=a,epoch=0,value=v0)".to_owned(),
        "Ack(epoch=0,value=v0)".to_owned(),
        "Ack(epoch=0,value=v1)".to_owned()
    ]));
    // This is the baseline's scenario plan, mutated; IMPL-04's pos-03 shows the correct
    // program never acknowledges v0 on it.
    let correct = baseline::scenario_plan();
    assert_eq!(
        render_plan(&all().base.groups[0].plans[0]),
        render_plan(&correct)
    );
}

/// mut-02, lost-abort: RuntimeToAbstract (the lost permit still holds the slot when the
/// writer retries). A run whose writer ends holding the lost permit is the binding's typed
/// `ReservationDropped` refusal instead, which is not counted as a detection.
#[test]
fn mut_02_lost_abort_is_detected_as_a_slot_still_reserved() {
    check_detected(Id::M02);
    let s = &summaries()[&Id::M02];
    assert!(s.refused_runs > 0 && s.refused_runs < s.failing_runs);
    // Every changed plan fails: each run either shows the held slot or is refused. The
    // detection proper is the runs that are not refused; the golden pins their count.
    assert_eq!(s.failing, s.changed);
    assert_eq!(s.detecting_runs + s.refused_runs, s.failing_runs);
}

/// mut-03, stale-epoch: excluded. Every run of every changed plan is the binding's typed
/// `TaskEnded` refusal: the crash cancelled the old epoch's region, so its tasks cannot
/// act for the new process.
#[test]
fn mut_03_stale_epoch_is_excluded_by_the_region_fence() {
    let (m, o) = get(Id::M03);
    assert!(matches!(
        mutants::expected(Id::M03),
        Expected::Excluded {
            exclusion: Exclusion::EpochFenced,
            ..
        }
    ));
    assert_eq!(o.breaches, BTreeMap::new());
    let s = &summaries()[&Id::M03];
    assert!(s.changed > 0);
    for (g, p, changed, runs, failing) in per_plan(m, o) {
        assert_eq!(failing, if changed { runs } else { 0 }, "{g}/{p}");
    }
    for (_, fs) in o.plans.iter().flat_map(|p| &p.findings) {
        assert!(
            matches!(fs.as_slice(), [Finding::Refused(r)] if Refusal::TaskEnded.matches(r)),
            "{fs:?}"
        );
    }
    assert_eq!(s.refused_runs, s.changed_runs);
    // Typed, from the binding itself.
    let w = s.witness.as_ref().expect("a refused run");
    let (_, raw) = mutants::replay(m, w);
    assert!(raw.is_err_and(|r| Refusal::TaskEnded.is(&r)));
}

/// mut-04, duplicate-commit: RuntimeToAbstract (a second write on a durable slot).
#[test]
fn mut_04_duplicate_commit_is_detected_as_a_rewrite() {
    check_detected(Id::M04);
}

/// mut-05, double-count: RuntimeToAbstract (an ack over one durable replica counted
/// twice), and Agreement.
#[test]
fn mut_05_double_count_is_detected() {
    check_detected(Id::M05);
}

/// mut-06, orphan: Quiescence (the losing coordinator never ends). The first finding of
/// every failing run is a loser, and exactly the plans with a loser fail.
#[test]
fn mut_06_orphan_is_detected_by_quiescence() {
    check_detected(Id::M06);
    let (m, o) = get(Id::M06);
    let base = &all().base;
    for p in &o.plans {
        let gi = base.groups.iter().position(|g| g.id == p.group).expect("g");
        let plan = &base.groups[gi].plans[p.index];
        let built = (m.builder)(plan).expect("built");
        let losers = mutants::losers(plan, &built);
        for (_, fs) in &p.findings {
            let Finding::Quiescence(baseline::Unsettled::TaskLive(t)) = fs[0] else {
                panic!("{fs:?}")
            };
            assert!(losers.contains(&(t as usize)), "t{t} is a loser");
        }
        assert_eq!(p.findings.is_empty(), losers.is_empty());
    }
}

/// mut-07, torn-tail: RuntimeToAbstract (the recovered record has no durable majority
/// under its ack).
#[test]
fn mut_07_torn_tail_is_detected() {
    check_detected(Id::M07);
}

/// mut-08, stale-timer: excluded. The old epoch's timer is scheduled before each crash
/// and cancelled by it, none fires, and no run has a finding.
#[test]
fn mut_08_stale_timer_is_excluded_by_the_region_cancel() {
    let (m, o) = get(Id::M08);
    assert!(matches!(
        mutants::expected(Id::M08),
        Expected::Excluded {
            exclusion: Exclusion::TimerDropped,
            ..
        }
    ));
    let s = &summaries()[&Id::M08];
    assert!(s.changed > 0);
    assert_eq!(o.by_property, BTreeMap::new());
    assert_eq!(o.breaches, BTreeMap::new());
    let [scheduled, fired, cancelled, stale] = o.tally.timers;
    assert_eq!(scheduled, crash_runs(m), "one timer per crash per run");
    assert_eq!(stale, 0, "no timer fires after its epoch is cancelled");
    assert_eq!(fired + cancelled, scheduled, "every timer resolves");
    assert!(cancelled > 0 && cancelled > fired);
}

/// The stale-fire count is not vacuous: an M08 journal whose first timer cancellation is
/// edited into a fire, after the crash, counts one stale fire.
#[test]
fn a_timer_fired_after_its_epoch_is_cancelled_is_counted() {
    use continuum_asupersync::family::EventBody;
    use continuum_asupersync::family::time::TimeEvent;
    let (m, _) = get(Id::M08);
    let plan = baseline::scenario_plan();
    let built = (m.builder)(&plan).expect("built");
    let (_, logs) = baseline::logs_for(&built, 1, m.campaign.plan_seed(0, 0));
    let journal =
        continuum_asupersync::binding::run(&built.programs, &logs[0], &baseline::config())
            .expect("a run");
    let tally = |j: &continuum_asupersync::journal::Journal| {
        baseline::settle(j, &continuum_asupersync::lift::lift(j))
            .2
            .timers
    };
    let [_, _, cancelled, stale] = tally(&journal);
    assert_eq!((cancelled, stale), (1, 0));
    let mut edited = continuum_asupersync::journal::Journal::new();
    for ev in journal.events() {
        let body = match ev.body() {
            EventBody::Time(TimeEvent::Cancelled { timer, at }) => {
                EventBody::Time(TimeEvent::Fired {
                    timer: *timer,
                    at: *at,
                })
            }
            other => other.clone(),
        };
        edited.append(body).expect("appends");
    }
    assert_eq!(tally(&edited)[3], 1);
}

/// The crashes over every run of a campaign: what M08 arms timers for.
fn crash_runs(m: &Mutant) -> usize {
    let (_, o) = get(m.id);
    o.plans
        .iter()
        .map(|p| {
            let g = m
                .campaign
                .groups
                .iter()
                .find(|g| g.id == p.group)
                .expect("g");
            let plan = &g.plans[p.index];
            let crashes = plan
                .replicas
                .iter()
                .flatten()
                .filter(|a| matches!(a, register::Act::Crash | register::Act::CrashRepropose(..)))
                .count();
            crashes * p.runs
        })
        .sum()
}

/// mut-09 and mut-10: deferred, with owners.
#[test]
fn mut_09_and_mut_10_are_deferred_to_their_owners() {
    for id in [Id::M09, Id::M10] {
        let Expected::Deferred { owner, .. } = mutants::expected(id) else {
            panic!("{id:?}")
        };
        assert!(owner.starts_with("bn-"));
    }
}

// ---------------------------------------------------------------------------
// the evidence
// ---------------------------------------------------------------------------

fn counts<K: std::fmt::Display>(m: impl IntoIterator<Item = (K, usize)>) -> String {
    let parts: Vec<String> = m.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(" ")
    }
}

fn by_property(o: &Outcome) -> String {
    counts(
        o.by_property
            .iter()
            .map(|(p, n)| (p.scenario_name().unwrap_or("run").to_owned(), *n)),
    )
}

fn expected_line(id: Id) -> String {
    match mutants::expected(id) {
        Expected::Detected {
            breach,
            symptom,
            also,
            refusal,
        } => {
            let mut s = format!(
                "detected; protocol breach {}; property {} with {symptom:?}",
                breach.map_or_else(|| "none, a program mutant".to_owned(), |b| format!("{b:?}")),
                symptom.property().scenario_name().unwrap_or("run")
            );
            if let Some(a) = also {
                let _ = write!(
                    s,
                    "; also {} with {a:?}",
                    a.property().scenario_name().unwrap_or("run")
                );
            }
            if let Some(r) = refusal {
                let _ = write!(s, "; some runs are the typed refusal {r:?}, not counted");
            }
            s
        }
        Expected::Excluded {
            exclusion,
            residual,
        } => format!(
            "excluded by the substrate: {}, on every changed plan; residual, not covered here: {residual}",
            match exclusion {
                Exclusion::EpochFenced =>
                    "typed refusal TaskEnded on every run, the binding's check for a command to an ended task, with no INV-008 reason",
                Exclusion::TimerDropped =>
                    "typed TimeEvent::Cancelled for every armed timer the crash overtakes, and no timer fired after its epoch is cancelled, by the binding's bookkeeping over the substrate's trace",
            }
        ),
        Expected::Deferred { owner, why } => format!("deferred to {owner}: {why}"),
    }
}

fn witness_line(label: &str, m: &Mutant, w: &Witness) -> String {
    let (report, raw) = mutants::replay(m, w);
    let at = match (mutants::seq_of(&w.finding), &raw) {
        (Some(s), Ok(n)) => format!("at event {s} of {n}"),
        (None, Ok(n)) => format!("at the end, {n} events"),
        (_, Err(_)) => "the binding refuses the log".to_owned(),
    };
    format!(
        "{label}: {} plan {} log {} ({} choices) {}\n  plan {}\n  log {}\n  finding {:?} {at}; replayed: {} findings, as recorded\n",
        w.group.1,
        w.plan,
        w.index,
        w.log.len(),
        if report.findings == w.findings {
            "reproduces"
        } else {
            "DIFFERS"
        },
        render_plan(&m.campaign.groups[w.group.0].plans[w.plan]),
        w.log,
        w.finding,
        report.findings.len()
    )
}

fn evidence() -> String {
    let a = all();
    let mut out = String::from(
        "# PR-16/IMPL-05 mutants of the replicated register (bn-28oa): each required mutant of\n\
         # notes/plan/examples/replicated_register.md with its expected result and a deterministic\n\
         # campaign derived from pr16-correct-baseline; tests/support/register_mutants.rs.\n\
         # Regenerate: PR16_IMPL05_BLESS=1 cargo test -p continuum-asupersync --test pr16_impl05_mutants\n\n",
    );
    let _ = writeln!(out, "[pr16-impl05-mut-00-baseline]");
    let _ = writeln!(
        out,
        "campaign {} seed {}: {} plans, {} runs; identity {} (IMPL-04's); findings {}\n",
        a.base.name,
        a.base.seed,
        a.baseline.plans.len(),
        a.baseline.runs,
        a.baseline.digest,
        by_property(&a.baseline)
    );
    for id in Id::ALL {
        let _ = writeln!(out, "[{}]", id.name());
        let _ = writeln!(out, "defect {id:?}: {}", id.defect());
        let _ = writeln!(out, "mutation: {}", id.mutation());
        let _ = writeln!(out, "expected: {}", expected_line(id));
        let Some((m, o)) = a.mutants.get(&id) else {
            let _ = writeln!(out, "campaign: none\nresult: deferred\n");
            continue;
        };
        let s = &summaries()[&id];
        let _ = writeln!(
            out,
            "campaign {} from {}: seed {}, {} plans, {} runs; identity {}",
            m.campaign.name, a.base.name, m.campaign.seed, s.plans, s.runs, o.digest
        );
        let _ = writeln!(
            out,
            "plans changed {} (runs {}), failing {}, detecting {}; runs failing {}, refused {}, detecting a scenario property {}; protocol breaches {}; findings {}",
            s.changed,
            s.changed_runs,
            s.failing,
            s.detecting_plans,
            s.failing_runs,
            s.refused_runs,
            s.detecting_runs,
            counts(o.breaches.iter().map(|(b, n)| (format!("{b:?}"), *n))),
            by_property(o)
        );
        let result = match mutants::expected(id) {
            Expected::Detected { symptom, also, .. } => {
                let mut r = format!(
                    "result: detected, {} with {symptom:?}",
                    symptom.property().scenario_name().unwrap_or("run")
                );
                if let Some(x) = also {
                    let _ = write!(
                        r,
                        ", and {} with {x:?}",
                        x.property().scenario_name().unwrap_or("run")
                    );
                }
                r
            }
            Expected::Excluded {
                exclusion: Exclusion::EpochFenced,
                ..
            } => format!(
                "result: excluded, every run of the {} changed plans is the typed refusal TaskEnded, the binding's check with no INV-008 reason; no detection claimed",
                s.changed
            ),
            Expected::Excluded {
                exclusion: Exclusion::TimerDropped,
                ..
            } => {
                let [sch, fired, can, stale] = o.tally.timers;
                format!(
                    "result: excluded, timers scheduled {sch}, cancelled {can}, fired {fired}, fired after their task's region was cancelled {stale}; no finding; no detection claimed"
                )
            }
            Expected::Deferred { .. } => "result: deferred".to_owned(),
        };
        let _ = writeln!(out, "{result}");
        if let Some(w) = &s.witness {
            out.push_str(&witness_line("witness", m, w));
        }
        if let Some(w) = &s.also {
            out.push_str(&witness_line("second witness", m, w));
            if id == Id::M01 {
                let story = mutants::story(m, w);
                let _ = writeln!(out, "  causal story: {}", story.join(" "));
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn the_evidence_matches_its_golden() {
    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/pr16_impl05_mutants.evidence.txt"
    );
    let got = evidence();
    if std::env::var_os("PR16_IMPL05_BLESS").is_some() {
        std::fs::write(PATH, &got).expect("writes the golden");
        return;
    }
    let want = std::fs::read_to_string(PATH).expect("the golden exists");
    let first = got
        .lines()
        .zip(want.lines())
        .position(|(g, w)| g != w)
        .map_or_else(String::new, |i| {
            format!(
                "line {}: `{}`",
                i + 1,
                got.lines().nth(i).unwrap_or_default()
            )
        });
    assert!(
        got == want,
        "the evidence drifted at {first}; regenerate with PR16_IMPL05_BLESS=1 and review"
    );
    assert!(!got.contains("DIFFERS"));
    let ids: Vec<&str> = got
        .lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "artifact IDs are unique");
    assert_eq!(ids.len(), 11);
    assert!(ids.iter().all(|i| i.starts_with("pr16-impl05-mut-")));
}
