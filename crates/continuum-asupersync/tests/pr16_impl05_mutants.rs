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
//! and two (M09, M10) are not program mutants and are deferred to their owners. Every
//! crash of the nine campaigns is graceful region cancellation, not a fail-stop crash
//! (`register::CRASH_SEMANTICS`, bn-20d8u); each mutant's `crash:` line in the golden
//! states what its result rests on.
//!
//! # The fail-stop reruns (bn-20d8u)
//!
//! The four mutants whose graceful result rests on the crash's cleanup (M02, M03, M07,
//! M08, `mutants::FAIL_STOP`) are run again with each crash the binding's fail-stop
//! `Crash` (`register::FAIL_STOP_SEMANTICS`), beside a fail-stop baseline, which stays at
//! zero findings with what the crashes stopped and fenced counted apart. Each mutant's
//! section of the golden gains `fail-stop` lines next to the graceful ones: its expected
//! result, the campaign and its identity, the result and a replayed witness.
//!
//! # The carried reruns (bn-2faf1)
//!
//! M03 and M08 (`mutants::CARRIED`) are run again against a process epoch carried in data
//! (`register::CARRIER_SEMANTICS`): M03 against the late completion of the crashed
//! incarnation's pending submit, a message, and M08 against a timer armed on the node's
//! supervisor, each under both crash semantics. Beside them run the correct program with
//! each carrier, message, timer and the recovery boundary, under both semantics, which
//! must have no finding. `pr16-impl05-mut-00-baseline` gains a `carried` line for each
//! carrier baseline, and the M03 and M08 sections gain `carried` lines: the mutation, the
//! typed expected result, the campaign and its identity, the result, and two replayed
//! witnesses with the second's causal story. Against the golden before bn-2faf1, four
//! pre-existing lines change, the M03 and M08 `expected:` and `fail-stop expected:` lines,
//! whose residual now points at the carried lines; every other change is an added
//! line, and no identity line changes.

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
use register::Carrier;

struct All {
    base: Campaign,
    baseline: Outcome,
    mutants: BTreeMap<Id, (Mutant, Outcome)>,
    /// The baseline, every crash fail-stop (bn-20d8u).
    fail_stop_baseline: Outcome,
    /// The fail-stop reruns of `mutants::FAIL_STOP`.
    fail_stop: BTreeMap<Id, (Mutant, Outcome)>,
    /// The correct program with each carrier, under each crash semantics (bn-2faf1).
    carrier_baselines: BTreeMap<(Carrier, Mode), (Mutant, Outcome)>,
    /// The carried reruns of `mutants::CARRIED`, under each crash semantics.
    carried: BTreeMap<(Id, Mode), (Mutant, Outcome)>,
}

/// The carriers, each run under both crash semantics.
const CARRIERS: [Carrier; 3] = [Carrier::Message, Carrier::Timer, Carrier::Recovery];

/// The baseline and every mutant campaign, run once, in parallel.
fn all() -> &'static All {
    static CELL: OnceLock<All> = OnceLock::new();
    CELL.get_or_init(|| {
        let base = baseline::baseline();
        let ms: Vec<Mutant> = Id::ALL
            .iter()
            .filter_map(|id| mutants::mutant(*id, &base))
            .collect();
        let fs: Vec<Mutant> = mutants::FAIL_STOP
            .iter()
            .filter_map(|id| mutants::mutant_fail_stop(*id, &base))
            .collect();
        let modes = [Mode::Graceful, Mode::FailStop];
        let cbs: Vec<((Carrier, Mode), Mutant)> = CARRIERS
            .iter()
            .flat_map(|c| modes.map(|m| ((*c, m), mutants::carrier_baseline(*c, m.crash(), &base))))
            .collect();
        let cms: Vec<((Id, Mode), Mutant)> = mutants::CARRIED
            .iter()
            .flat_map(|id| {
                modes.map(|m| {
                    (
                        (*id, m),
                        mutants::mutant_carried(*id, m.crash(), &base).expect("a carried rerun"),
                    )
                })
            })
            .collect();
        std::thread::scope(|s| {
            let b = s.spawn(|| baseline::execute(&base));
            let fb = s.spawn(|| baseline::execute_in(&base, register::CrashMode::FailStop));
            let hs: Vec<_> = ms
                .iter()
                .map(|m| s.spawn(move || mutants::execute(m)))
                .collect();
            let fhs: Vec<_> = fs
                .iter()
                .map(|m| s.spawn(move || mutants::execute(m)))
                .collect();
            let cbhs: Vec<_> = cbs
                .iter()
                .map(|(_, m)| s.spawn(move || mutants::execute(m)))
                .collect();
            let cmhs: Vec<_> = cms
                .iter()
                .map(|(_, m)| s.spawn(move || mutants::execute(m)))
                .collect();
            let mutants = ms
                .iter()
                .cloned()
                .zip(hs)
                .map(|(m, h)| (m.id, (m, h.join().expect("a campaign"))))
                .collect();
            let fail_stop = fs
                .iter()
                .cloned()
                .zip(fhs)
                .map(|(m, h)| (m.id, (m, h.join().expect("a fail-stop campaign"))))
                .collect();
            let carrier_baselines = cbs
                .iter()
                .cloned()
                .zip(cbhs)
                .map(|((k, m), h)| (k, (m, h.join().expect("a carrier baseline"))))
                .collect();
            let carried = cms
                .iter()
                .cloned()
                .zip(cmhs)
                .map(|((k, m), h)| (k, (m, h.join().expect("a carried campaign"))))
                .collect();
            All {
                baseline: b.join().expect("the baseline"),
                base: base.clone(),
                mutants,
                fail_stop_baseline: fb.join().expect("the fail-stop baseline"),
                fail_stop,
                carrier_baselines,
                carried,
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

/// Which crash semantics a campaign runs under (bn-20d8u).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    Graceful,
    FailStop,
}

impl Mode {
    const fn crash(self) -> register::CrashMode {
        match self {
            Self::Graceful => register::CrashMode::Graceful,
            Self::FailStop => register::CrashMode::FailStop,
        }
    }
}

/// Which campaign of a mutant: the graceful one, the fail-stop rerun, or a carried rerun
/// under a crash semantics (bn-2faf1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Run {
    Plain(Mode),
    Carried(Mode),
}

/// A campaign and its outcome, and its expected result, under `mode`.
fn run_of(id: Id, mode: Mode) -> (&'static Mutant, &'static Outcome, Expected) {
    run_in(id, Run::Plain(mode))
}

fn run_in(id: Id, run: Run) -> (&'static Mutant, &'static Outcome, Expected) {
    let mode = match run {
        Run::Plain(m) => m,
        Run::Carried(m) => {
            let (mu, o) = &all().carried[&(id, m)];
            return (
                mu,
                o,
                mutants::expected_carried(id).expect("a carried expectation"),
            );
        }
    };
    match mode {
        Mode::Graceful => {
            let (m, o) = get(id);
            (m, o, mutants::expected(id))
        }
        Mode::FailStop => {
            let (m, o) = &all().fail_stop[&id];
            (
                m,
                o,
                mutants::expected_fail_stop(id).expect("a fail-stop expectation"),
            )
        }
    }
}

/// Whether the mutant changed a plan, against the build of its own mode.
fn changed_in(run: Run, m: &Mutant, gi: usize, pi: usize) -> bool {
    let base = &all().base;
    match run {
        Run::Plain(Mode::Graceful) => mutants::changed(m, base, gi, pi),
        Run::Plain(Mode::FailStop) => mutants::changed_fail_stop(m, base, gi, pi),
        Run::Carried(mode) => mutants::changed_carried(m, mode.crash(), base, gi, pi),
    }
}

/// Per plan of a mutant campaign: `(group, plan, changed, runs, failing runs)`.
fn per_plan(m: &Mutant, o: &Outcome) -> Vec<(usize, usize, bool, usize, usize)> {
    per_plan_in(Run::Plain(Mode::Graceful), m, o)
}

fn per_plan_in(mode: Run, m: &Mutant, o: &Outcome) -> Vec<(usize, usize, bool, usize, usize)> {
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
                changed_in(mode, m, gi, p.index),
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
    summary_in(id, Run::Plain(Mode::Graceful))
}

fn summary_in(id: Id, mode: Run) -> Summary {
    let (m, o, expected) = run_in(id, mode);
    let rows = per_plan_in(mode, m, o);
    let (symptom, also) = match expected {
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
    let witness = match (symptom, expected) {
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
        (
            None,
            Expected::Excluded {
                exclusion: Exclusion::EpochFencedByCrash,
                ..
            },
        ) => mutants::witness(
            m,
            o,
            |f| matches!(f, Finding::Refused(r) if Refusal::TaskCrashed.matches(r)),
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

fn fail_stop_summaries() -> &'static BTreeMap<Id, Summary> {
    static CELL: OnceLock<BTreeMap<Id, Summary>> = OnceLock::new();
    CELL.get_or_init(|| {
        mutants::FAIL_STOP
            .iter()
            .map(|id| (*id, summary_in(*id, Run::Plain(Mode::FailStop))))
            .collect()
    })
}

fn carried_summaries() -> &'static BTreeMap<(Id, Mode), Summary> {
    static CELL: OnceLock<BTreeMap<(Id, Mode), Summary>> = OnceLock::new();
    CELL.get_or_init(|| {
        all()
            .carried
            .keys()
            .map(|&(id, m)| ((id, m), summary_in(id, Run::Carried(m))))
            .collect()
    })
}

fn summary_of(id: Id, run: Run) -> &'static Summary {
    match run {
        Run::Plain(Mode::Graceful) => &summaries()[&id],
        Run::Plain(Mode::FailStop) => &fail_stop_summaries()[&id],
        Run::Carried(m) => &carried_summaries()[&(id, m)],
    }
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
    check_detected_in(id, Mode::Graceful);
}

fn check_detected_in(id: Id, mode: Mode) {
    check_detected_run(id, Run::Plain(mode));
}

fn check_detected_run(id: Id, mode: Run) {
    let (m, o, expected) = run_in(id, mode);
    let Expected::Detected {
        breach,
        symptom,
        also,
        refusal,
    } = expected
    else {
        panic!("{id:?} is expected to be detected");
    };
    let s = summary_of(id, mode);
    // The mutant changes some plans, and each changed plan of a plan mutant breaks the
    // named protocol rule first. A program mutant's plans are the correct ones.
    assert!(s.changed > 0, "{id:?} changes something");
    match breach {
        Some(b) => assert_eq!(o.breaches, BTreeMap::from([(b, s.changed)]), "{id:?}"),
        None => assert_eq!(o.breaches, BTreeMap::new(), "{id:?}"),
    }
    // Findings come only from changed plans: the mutation, not the campaign, is what
    // fails.
    for (g, p, changed, _, failing) in per_plan_in(mode, m, o) {
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

/// Whether every explicit abort that M02 drops from `plan` is followed, in the same
/// replica's script, by a crash: the graceful region cancellation whose cleanup then
/// aborts the lost permit (bn-20d8u).
fn lost_permits_meet_a_crash(plan: &register::Plan) -> bool {
    plan.replicas.iter().all(|s| {
        s.iter().enumerate().all(|(i, a)| {
            !matches!(a, register::Act::Abort(_))
                || s[i + 1..]
                    .iter()
                    .any(|b| matches!(b, register::Act::Crash | register::Act::CrashRepropose(..)))
        })
    })
}

/// mut-02 and the crash semantics (bn-20d8u): a changed plan's runs are runs exactly when
/// each lost permit's incarnation later crashes, because the crash is graceful region
/// cancellation and its cleanup aborts the permit for `Cancel`. Every other changed plan
/// is refused on every run. So the detection is shown for graceful cancellation only.
#[test]
fn mut_02_detects_only_where_a_graceful_crash_aborts_the_lost_permit() {
    use continuum_asupersync::family::EventBody;
    use continuum_asupersync::family::effect::{AbortCause, EffectEvent};
    let (m, o) = get(Id::M02);
    let base = &all().base;
    let (mut admitted, mut refused) = (0, 0);
    for p in &o.plans {
        let gi = base.groups.iter().position(|g| g.id == p.group).expect("g");
        if !mutants::changed(m, base, gi, p.index) {
            continue;
        }
        let runs_refused = p
            .findings
            .iter()
            .filter(|(_, fs)| {
                fs.iter().any(
                    |f| matches!(f, Finding::Refused(r) if Refusal::ReservationDropped.matches(r)),
                )
            })
            .count();
        if lost_permits_meet_a_crash(&base.groups[gi].plans[p.index]) {
            assert_eq!(runs_refused, 0, "{}/{}", p.group, p.index);
            admitted += p.runs;
        } else {
            assert_eq!(runs_refused, p.runs, "{}/{}", p.group, p.index);
            refused += p.runs;
        }
    }
    let s = &summaries()[&Id::M02];
    assert_eq!((admitted, refused), (s.detecting_runs, s.refused_runs));
    // In the witness, the lost permit is released only by the crash's cleanup: a
    // `cancel` abort of a writer permit that never had bytes, which projects as `Lose`.
    let w = s.witness.as_ref().expect("a witness");
    let plan = &m.campaign.groups[w.group.0].plans[w.plan];
    let built = (m.builder)(plan).expect("built");
    let journal = continuum_asupersync::binding::run(&built.programs, &w.log, &baseline::config())
        .expect("a run");
    assert!(journal.events().iter().any(|e| matches!(
        e.body(),
        EventBody::Effect(EffectEvent::Aborted {
            cause: AbortCause::Cancel,
            ..
        })
    )));
    // A `Lose` of a permit with no bytes is a label prefix: every value names it. The
    // witness's lost permit is b's first epoch-0 permit.
    assert!(
        mutants::story(m, w)
            .iter()
            .any(|l| l == "Lose(n=b,epoch=0,")
    );
}

/// mut-07 and the crash semantics (bn-20d8u): the crash's cleanup aborts the recovered
/// record's bytes, a `Lose`. A fail-stop crash leaves that `IoOp` pending, and whether
/// its bytes become durable is the storage pack's to state. The detection holds where
/// they stay volatile, by argument: a pending slot is not durable, so putting the bytes
/// back as volatile changes neither the durable set nor the acks. It fails where they
/// become durable: the witness's ack then has its majority. A check on the witness's
/// projected states, not a fail-stop run.
#[test]
fn mut_07_detection_does_not_rest_on_the_crash_losing_the_bytes() {
    let (m, _) = get(Id::M07);
    let lost_and_ack = |plan: &register::Plan,
                        log: &continuum_asupersync::choice::ChoiceLog,
                        seq: u64|
     -> (Vec<(u8, u8, u8)>, register::Raw) {
        let built = (m.builder)(plan).expect("built");
        let journal = continuum_asupersync::binding::run(&built.programs, log, &baseline::config())
            .expect("a run");
        let steps = register::observe(&built.roles, &journal).expect("projects");
        let mut lost = Vec::new();
        for st in steps.iter().filter(|st| st.seq <= seq) {
            if let (register::Expect::Step(l), Some(pre), Some(post)) =
                (&st.expect, &st.pre, &st.post)
                && l.starts_with("Lose(")
            {
                let (before, _, _) = register::parts_of(pre);
                let (after, _, _) = register::parts_of(post);
                lost.extend(before.into_iter().filter(|r| !after.contains(r)));
            }
        }
        let at = steps.iter().find(|st| st.seq == seq).expect("the event");
        (lost, at.post.clone().expect("projectable"))
    };
    let restore = |ack: &register::Raw, lost: &[(u8, u8, u8)], volatile: bool| {
        let (mut log, mut pending, acks) = register::parts_of(ack);
        for &(n, e, v) in lost {
            // Only a slot the crash left free: no retry wrote it since.
            if log.iter().all(|&(m, f, _)| (m, f) != (n, e)) && !pending.contains(&(n, e)) {
                log.push((n, e, v));
                if volatile {
                    pending.push((n, e));
                }
            }
        }
        register::raw_of(&log, &pending, &acks)
    };
    // The witness: the crash lost c's bytes, and had storage made them durable, the ack
    // would have its majority.
    let w = summaries()[&Id::M07].witness.as_ref().expect("a witness");
    let Finding::AckedNotDurable(seq) = w.finding else {
        panic!("{:?}", w.finding)
    };
    let (lost, ack) = lost_and_ack(&m.campaign.groups[w.group.0].plans[w.plan], &w.log, seq);
    assert!(!lost.is_empty(), "the crash's cleanup lost the record");
    assert!(baseline::ack_without_majority(&ack));
    assert!(baseline::ack_without_majority(&restore(&ack, &lost, true)));
    assert!(!baseline::ack_without_majority(&restore(
        &ack, &lost, false
    )));
}

/// Each mutant with a campaign states what its result rests on about a crash
/// (bn-20d8u), and the evidence's header states the crash semantics.
#[test]
fn every_mutant_campaign_states_its_crash_dependence() {
    for id in Id::ALL {
        assert_eq!(
            mutants::crash_dependence(id).is_some(),
            all().mutants.contains_key(&id),
            "{id:?}"
        );
    }
    assert!(register::CRASH_SEMANTICS.contains("graceful region cancellation"));
    assert!(register::CRASH_SEMANTICS.contains("bn-20d8u"));
}

/// mut-03, stale-epoch: excluded. Every run of every changed plan is the binding's typed
/// `TaskEnded` refusal: the crash cancelled the old epoch's region, so its tasks cannot
/// act for the new process. The "fence" in the name is that refusal after graceful
/// region cancellation, not the process pack's `(node, epoch)` fence (bn-20d8u).
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
    crash_runs_in(Mode::Graceful, m)
}

fn crash_runs_in(mode: Mode, m: &Mutant) -> usize {
    let (_, o, _) = run_of(m.id, mode);
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

// ---------------------------------------------------------------------------
// the fail-stop reruns (bn-20d8u)
// ---------------------------------------------------------------------------

/// The fail-stop baseline: the correct program with every crash fail-stop has no finding,
/// and what its crashes stopped and fenced is counted apart; no crash runs a cleanup.
#[test]
fn mut_00_the_fail_stop_baseline_stays_at_zero_findings_and_counts_fences_apart() {
    let a = all();
    let o = &a.fail_stop_baseline;
    assert_eq!(o.by_property, BTreeMap::new());
    assert!(o.plans.iter().all(|p| p.findings.is_empty()));
    assert_eq!(o.breaches, BTreeMap::new());
    assert_eq!(o.runs, a.baseline.runs);
    assert_ne!(o.digest, a.baseline.digest, "its own identity");
    let [tasks, obligations, reservations, timers] = o.tally.fenced;
    assert!(
        tasks > 0 && obligations > 0 && reservations > 0,
        "{:?}",
        o.tally.fenced
    );
    assert_eq!(timers, 0, "the correct program sets no timer");
    assert_eq!(
        a.baseline.tally.fenced, [0; 4],
        "graceful crashes fence nothing"
    );
    // The graceful baseline's crash cleanup cancels tasks in drains; a fail-stop crash
    // cancels none.
    assert!(a.baseline.tally.cancelled > 0);
    assert_eq!(o.tally.cancelled, 0);
    assert!(o.crash_after_ack > 0 && o.cancel_after_submit > 0);
}

/// Every rerun is the graceful campaign's plans under the fail-stop build, with its own
/// name and identity.
#[test]
fn every_fail_stop_rerun_is_derived_from_the_baseline() {
    let a = all();
    assert_eq!(
        a.fail_stop.keys().copied().collect::<Vec<_>>(),
        mutants::FAIL_STOP.to_vec()
    );
    for (id, (m, o)) in &a.fail_stop {
        let (g, go) = get(*id);
        assert_eq!(m.campaign.name, mutants::fail_stop_name(*id));
        assert_eq!(m.campaign.seed, a.base.seed);
        assert_eq!(m.plan_mutant, g.plan_mutant, "{id:?}");
        for (x, y) in m.campaign.groups.iter().zip(&g.campaign.groups) {
            for (p, q) in x.plans.iter().zip(&y.plans) {
                assert_eq!(render_plan(p), render_plan(q), "{id:?}");
            }
        }
        assert_eq!(o.plans.len(), go.plans.len());
        assert_ne!(o.digest, go.digest, "{id:?}: its own identity");
        // Each crash act is a `Crash` in the rerun's build, and a `Cancel` in the
        // graceful one.
        let plan = &m.campaign.groups[0].plans[0];
        let crashes = |b: &register::Built| {
            b.programs
                .iter()
                .flatten()
                .filter(|op| matches!(op, continuum_asupersync::binding::SubstrateOp::Crash { .. }))
                .count()
        };
        let acts = plan
            .replicas
            .iter()
            .flatten()
            .filter(|a| matches!(a, register::Act::Crash | register::Act::CrashRepropose(..)))
            .count();
        assert_eq!(crashes(&(m.builder)(plan).expect("built")), acts, "{id:?}");
        assert_eq!(crashes(&(g.builder)(plan).expect("built")), 0, "{id:?}");
    }
}

/// mut-02 under the fail-stop crash: still detected. A changed plan's runs are runs
/// exactly when each lost permit's incarnation later crashes: the crash now fences the
/// permit (it stops its holder), where the graceful crash's cleanup aborted it. So the
/// detection does not rest on the cleanup; the refused runs are those whose writer ends
/// holding the permit, as before.
#[test]
fn fail_stop_mut_02_lost_abort_is_detected_and_the_crash_fences_the_permit() {
    use continuum_asupersync::family::EventBody;
    use continuum_asupersync::family::effect::{AbortCause, EffectEvent};
    check_detected_in(Id::M02, Mode::FailStop);
    let (m, o) = &all().fail_stop[&Id::M02];
    let base = &all().base;
    let (mut admitted, mut refused) = (0, 0);
    for p in &o.plans {
        let gi = base.groups.iter().position(|g| g.id == p.group).expect("g");
        if !mutants::changed_fail_stop(m, base, gi, p.index) {
            continue;
        }
        let runs_refused = p
            .findings
            .iter()
            .filter(|(_, fs)| {
                fs.iter().any(
                    |f| matches!(f, Finding::Refused(r) if Refusal::ReservationDropped.matches(r)),
                )
            })
            .count();
        if lost_permits_meet_a_crash(&base.groups[gi].plans[p.index]) {
            assert_eq!(runs_refused, 0, "{}/{}", p.group, p.index);
            admitted += p.runs;
        } else {
            assert_eq!(runs_refused, p.runs, "{}/{}", p.group, p.index);
            refused += p.runs;
        }
    }
    let s = &fail_stop_summaries()[&Id::M02];
    assert_eq!((admitted, refused), (s.detecting_runs, s.refused_runs));
    let g = &summaries()[&Id::M02];
    assert_eq!(
        (s.detecting_runs, s.refused_runs),
        (g.detecting_runs, g.refused_runs)
    );
    // In the witness, the lost permit is fenced by the crash, never aborted.
    let w = s.witness.as_ref().expect("a witness");
    let plan = &m.campaign.groups[w.group.0].plans[w.plan];
    let built = (m.builder)(plan).expect("built");
    let journal = continuum_asupersync::binding::run(&built.programs, &w.log, &baseline::config())
        .expect("a run");
    assert!(
        journal
            .events()
            .iter()
            .any(|e| matches!(e.body(), EventBody::Effect(EffectEvent::Fenced { .. })))
    );
    assert!(!journal.events().iter().any(|e| matches!(
        e.body(),
        EventBody::Effect(EffectEvent::Aborted {
            cause: AbortCause::Cancel,
            ..
        })
    )));
}

/// mut-03 under the fail-stop crash: excluded. Every run of every changed plan is the
/// binding's typed `TaskCrashed` refusal: the crashed incarnation's tasks take no command.
#[test]
fn fail_stop_mut_03_stale_epoch_is_excluded_by_the_crash_fence() {
    let (m, o) = &all().fail_stop[&Id::M03];
    assert_eq!(o.breaches, BTreeMap::new());
    let s = &fail_stop_summaries()[&Id::M03];
    assert!(s.changed > 0);
    for (g, p, changed, runs, failing) in per_plan_in(Run::Plain(Mode::FailStop), m, o) {
        assert_eq!(failing, if changed { runs } else { 0 }, "{g}/{p}");
    }
    for (_, fs) in o.plans.iter().flat_map(|p| &p.findings) {
        assert!(
            matches!(fs.as_slice(), [Finding::Refused(r)] if Refusal::TaskCrashed.matches(r)),
            "{fs:?}"
        );
    }
    assert_eq!(s.refused_runs, s.changed_runs);
    let w = s.witness.as_ref().expect("a refused run");
    let (_, raw) = mutants::replay(m, w);
    assert!(raw.is_err_and(|r| Refusal::TaskCrashed.is(&r)));
}

/// mut-07 under the fail-stop crash: detected. The crash leaves the recovered record's
/// `IoOp` pending and fences it; the projection reads its `Lose` from the crash event,
/// so the ack has no durable majority, as under the graceful crash.
#[test]
fn fail_stop_mut_07_torn_tail_is_detected_with_the_loss_read_from_the_crash() {
    check_detected_in(Id::M07, Mode::FailStop);
    let (m, _) = &all().fail_stop[&Id::M07];
    let w = fail_stop_summaries()[&Id::M07]
        .witness
        .as_ref()
        .expect("a witness");
    let plan = &m.campaign.groups[w.group.0].plans[w.plan];
    let built = (m.builder)(plan).expect("built");
    let journal = continuum_asupersync::binding::run(&built.programs, &w.log, &baseline::config())
        .expect("a run");
    let steps = register::observe(&built.roles, &journal).expect("projects");
    let losses: Vec<&register::Observed> = steps
        .iter()
        .filter(|st| matches!(&st.expect, register::Expect::Step(l) if l.starts_with("Lose(")))
        .collect();
    assert!(!losses.is_empty());
    assert!(losses.iter().all(|st| st.event.contains("region-crashed")));
}

/// mut-08 under the fail-stop crash: excluded. Every timer the old incarnation armed and
/// had not fired before the crash is fenced at it, none is cancelled, and none fires
/// after it.
#[test]
fn fail_stop_mut_08_stale_timer_is_excluded_by_the_fence() {
    let (m, o) = &all().fail_stop[&Id::M08];
    let s = &fail_stop_summaries()[&Id::M08];
    assert!(s.changed > 0);
    assert_eq!(o.by_property, BTreeMap::new());
    assert_eq!(o.breaches, BTreeMap::new());
    let [scheduled, fired, cancelled, stale] = o.tally.timers;
    let fenced = o.tally.fenced[3];
    assert_eq!(
        scheduled,
        crash_runs_in(Mode::FailStop, m),
        "one timer per crash per run"
    );
    assert_eq!(stale, 0, "no timer fires after its incarnation crashed");
    assert_eq!(cancelled, 0, "no crash cancels a timer");
    assert_eq!(
        fired + fenced,
        scheduled,
        "every timer fires before the crash or is fenced"
    );
    assert!(fenced > fired);
}

// ---------------------------------------------------------------------------
// the carried reruns (bn-2faf1)
// ---------------------------------------------------------------------------

const MODES: [Mode; 2] = [Mode::Graceful, Mode::FailStop];

/// The payloads a campaign's build puts on its mailboxes over its runs, and how many the
/// build makes the receiver act on: its sites, by plan, times the plan's runs. A static
/// count, held in the tests to the journals' own count (`Outcome::carried`).
fn deliveries(m: &Mutant, o: &Outcome, carrier: Carrier, fence: register::Fence) -> (usize, usize) {
    let (mut delivered, mut accepted) = (0, 0);
    for p in &o.plans {
        let g = m
            .campaign
            .groups
            .iter()
            .find(|g| g.id == p.group)
            .expect("g");
        let sites = register::carrier_sites(&g.plans[p.index], carrier, fence);
        delivered += sites.len() * p.runs;
        accepted += sites.iter().filter(|s| s.accepted).count() * p.runs;
    }
    (delivered, accepted)
}

/// The journal of one run of `m`'s plan `(group, plan)` under `log`.
fn journal_of(
    m: &Mutant,
    group: usize,
    plan: usize,
    log: &continuum_asupersync::choice::ChoiceLog,
) -> (register::Built, continuum_asupersync::journal::Journal) {
    let built = (m.builder)(&m.campaign.groups[group].plans[plan]).expect("built");
    let journal = continuum_asupersync::binding::run(&built.programs, log, &baseline::config())
        .expect("a run");
    (built, journal)
}

/// The receiver's epoch check, alone: the correct check accepts exactly a payload of the
/// receiver's own process epoch, for a message and a timer alike; M03's reused epoch makes
/// the old epoch the receiver's own; M08's unchecked timer accepts any timer payload and
/// still refuses a stale message.
#[test]
fn carried_the_epoch_check_accepts_exactly_its_own_epoch() {
    use register::{CORRECT_FENCE, accepts, process_epoch};
    for via in [Carrier::Message, Carrier::Timer, Carrier::Recovery] {
        assert!(!accepts(CORRECT_FENCE, via, 1, 0), "{via:?}: stale");
        assert!(accepts(CORRECT_FENCE, via, 1, 1), "{via:?}: current");
        assert!(accepts(CORRECT_FENCE, via, 0, 0), "{via:?}: first");
    }
    let m03 = mutants::carried_fence(Id::M03);
    assert_eq!(process_epoch(1, m03), process_epoch(0, m03));
    assert_eq!(process_epoch(2, CORRECT_FENCE), 2);
    let m08 = mutants::carried_fence(Id::M08);
    assert!(accepts(m08, Carrier::Timer, 1, 0));
    assert!(!accepts(m08, Carrier::Message, 1, 0));
    // M08's defect is the timer's: with the message carrier, its fence builds the correct
    // program on every plan.
    for g in &all().base.groups {
        for plan in &g.plans {
            for mode in MODES {
                assert_eq!(
                    register::build_carried(plan, mode.crash(), Carrier::Message, m08).programs,
                    register::build_carried(plan, mode.crash(), Carrier::Message, CORRECT_FENCE)
                        .programs
                );
            }
        }
    }
    // The scenario plan: `a` crashes with its v0 bytes submitted and not synced, so the
    // message carrier has one site, at `a`'s crash, naming process epoch 0; the new
    // incarnation, epoch 1, refuses it; under M03 it is epoch 0 again and acts on it.
    let plan = baseline::scenario_plan();
    let sites = register::carrier_sites(&plan, Carrier::Message, CORRECT_FENCE);
    assert_eq!(sites.len(), 1);
    let s = sites[0];
    assert_eq!((s.node, s.at, s.crashed), (0, 2, 0));
    assert_eq!(
        (
            s.payload.process,
            s.payload.epoch,
            s.payload.value,
            s.accepted
        ),
        (0, 0, 0, false)
    );
    let s = register::carrier_sites(&plan, Carrier::Message, m03)[0];
    assert!(s.accepted);
    // A crash with nothing in flight has no pending completion, so no message or timer
    // site: the crash-after-reply plan has none, and one recovery-free plan stays as built.
    let reply = baseline::crash_after_reply_plan();
    for c in [Carrier::Message, Carrier::Timer, Carrier::Recovery] {
        assert!(
            register::carrier_sites(&reply, c, CORRECT_FENCE).is_empty(),
            "{c:?}"
        );
        assert_eq!(
            register::build_carried(&reply, register::CrashMode::FailStop, c, CORRECT_FENCE)
                .programs,
            register::build_with_shutdown_in(&reply, register::CrashMode::FailStop).programs
        );
    }
}

/// The negative: the correct program with each carrier, under each crash semantics, has no
/// finding. Every stale payload is delivered and refused; every recovery report, which
/// names the receiver's own epoch, is delivered and acted on; plans with no site build
/// exactly as the baseline; the timer carrier's timers all fire, none is cancelled or
/// fenced (the supervisor never crashes).
#[test]
fn carried_mut_00_the_correct_program_with_each_carrier_has_zero_findings() {
    let a = all();
    for c in CARRIERS {
        for mode in MODES {
            let (m, o) = &a.carrier_baselines[&(c, mode)];
            let what = format!("{c:?} {mode:?}");
            assert_eq!(o.by_property, BTreeMap::new(), "{what}");
            assert!(o.plans.iter().all(|p| p.findings.is_empty()), "{what}");
            assert_eq!(o.breaches, BTreeMap::new(), "{what}");
            assert_eq!(o.runs, a.baseline.runs, "{what}");
            assert_ne!(o.digest, a.baseline.digest, "{what}: its own identity");
            let (delivered, accepted) = deliveries(m, o, c, register::CORRECT_FENCE);
            assert!(delivered > 0, "{what}");
            // The journals receive every payload the build sends, and the receivers act on
            // exactly the ones the build accepts.
            assert_eq!(o.carried, [delivered, accepted], "{what}");
            if c == Carrier::Recovery {
                assert_eq!(accepted, delivered, "{what}: the current epoch is accepted");
            } else {
                assert_eq!(accepted, 0, "{what}: every stale payload is refused");
            }
            for (gi, g) in m.campaign.groups.iter().enumerate() {
                for plan in &g.plans {
                    let sites = register::carrier_sites(plan, c, register::CORRECT_FENCE);
                    let plain = register::build_with_shutdown_in(plan, mode.crash()).programs;
                    let built = (m.builder)(plan).expect("built").programs;
                    assert_eq!(sites.is_empty(), built == plain, "{what} group {gi}");
                }
            }
            let [scheduled, fired, cancelled, stale] = o.tally.timers;
            if c == Carrier::Timer {
                assert_eq!((scheduled, fired), (delivered, delivered), "{what}");
            } else {
                assert_eq!((scheduled, fired), (0, 0), "{what}");
            }
            assert_eq!((cancelled, stale, o.tally.fenced[3]), (0, 0, 0), "{what}");
        }
    }
    // The same crashes are graceful in one and fail-stop in the other.
    for c in CARRIERS {
        let g = &a.carrier_baselines[&(c, Mode::Graceful)].1;
        let f = &a.carrier_baselines[&(c, Mode::FailStop)].1;
        assert_eq!(g.tally.fenced, [0; 4]);
        assert!(f.tally.fenced[0] > 0);
        assert_eq!(f.tally.cancelled, 0);
    }
}

/// A carried rerun is the carrier baseline's plans with one field of the fence changed:
/// the same seed, groups and plans, its own name and identity, and it changes exactly
/// the plans whose site it makes the receiver act on.
fn check_carried_derivation(id: Id, mode: Mode) {
    let a = all();
    let (m, o) = &a.carried[&(id, mode)];
    let c = mutants::carrier_of(id);
    assert_eq!(m.campaign.name, mutants::carried_name(id, mode.crash()));
    assert_eq!(m.campaign.seed, a.base.seed);
    for (x, y) in m.campaign.groups.iter().zip(&a.base.groups) {
        assert_eq!((x.id, x.logs, x.plans.len()), (y.id, y.logs, y.plans.len()));
        for (p, q) in x.plans.iter().zip(&y.plans) {
            assert_eq!(render_plan(p), render_plan(q), "{id:?}");
        }
    }
    let (_, co) = &a.carrier_baselines[&(c, mode)];
    assert_ne!(o.digest, co.digest, "{id:?} {mode:?}");
    for (gi, g) in a.base.groups.iter().enumerate() {
        for (pi, plan) in g.plans.iter().enumerate() {
            let acted = register::carrier_sites(plan, c, mutants::carried_fence(id))
                .iter()
                .any(|s| s.accepted);
            assert_eq!(
                mutants::changed_carried(m, mode.crash(), &a.base, gi, pi),
                acted,
                "{id:?} {mode:?} {gi}/{pi}"
            );
        }
    }
}

/// In a carried witness, each payload names the crashed incarnation's process epoch under
/// the mutant's fence, is received by the last incarnation's writer after the crash, and
/// the receiver's first own step after the receipt is the confirmation of the payload's
/// slot and value (`register::carried_in_journal`): the new process acts on the stale
/// payload. On the scenario plan, only the payload's delivery corroborates that
/// confirmation: with the payload's value flipped in the roles, or with the journal's
/// receipt of it deleted, the projection refuses the journal.
fn check_carried_witness(id: Id, mode: Mode) {
    use continuum_asupersync::family::EventBody;
    use continuum_asupersync::family::channel::ChannelEvent;
    use continuum_asupersync::family::lifecycle::LifecycleEvent;
    let (m, _) = &all().carried[&(id, mode)];
    let s = &carried_summaries()[&(id, mode)];
    let w = s.witness.as_ref().expect("a witness");
    let (built, journal) = journal_of(m, w.group.0, w.plan, &w.log);
    let fence = mutants::carried_fence(id);
    let sites = register::carrier_sites(
        &m.campaign.groups[w.group.0].plans[w.plan],
        mutants::carrier_of(id),
        fence,
    );
    assert!(!sites.is_empty());
    assert_eq!(sites.len(), built.roles.mailboxes.len());
    for (s, mb) in sites.iter().zip(&built.roles.mailboxes) {
        assert_eq!(s.payload, mb.payload);
        assert_eq!(s.payload.process, register::process_epoch(s.crashed, fence));
        assert!(mb.accepted);
    }
    let crash_at = journal
        .events()
        .iter()
        .position(|e| {
            matches!(
                e.body(),
                EventBody::Lifecycle(
                    LifecycleEvent::RegionCancelRequested { .. }
                        | LifecycleEvent::RegionCrashed { .. }
                )
            )
        })
        .expect("a crash");
    for mb in &built.roles.mailboxes {
        let received = journal
            .events()
            .iter()
            .position(|e| {
                matches!(e.body(), EventBody::Channel(ChannelEvent::Received { channel, .. }) if channel.0 == mb.channel)
            })
            .expect("received");
        assert!(received > crash_at);
    }
    assert!(
        register::carried_in_journal(&built.roles, &journal)
            .iter()
            .all(|d| *d == Some(true)),
        "each receiver's next step confirms the payload"
    );
    let plan = &m.campaign.groups[0].plans[0];
    assert_eq!(render_plan(plan), render_plan(&baseline::scenario_plan()));
    let built = (m.builder)(plan).expect("built");
    let (_, logs) = baseline::logs_for(&built, 1, m.campaign.plan_seed(0, 0));
    let (_, journal) = journal_of(m, 0, 0, &logs[0]);
    assert!(register::observe(&built.roles, &journal).is_ok());
    let mut roles = built.roles.clone();
    for mb in &mut roles.mailboxes {
        mb.payload.value = 1 - mb.payload.value;
    }
    assert!(matches!(
        register::observe(&roles, &journal),
        Err(register::ProjectionRefusal::RolesDisagree { .. })
    ));
    let channel = built.roles.mailboxes[0].channel;
    let mut edited = continuum_asupersync::journal::Journal::new();
    for ev in journal.events() {
        if !matches!(ev.body(), EventBody::Channel(ChannelEvent::Received { channel: c, .. }) if c.0 == channel)
        {
            edited.append(ev.body().clone()).expect("appends");
        }
    }
    assert!(matches!(
        register::observe(&built.roles, &edited),
        Err(register::ProjectionRefusal::RolesDisagree { .. })
    ));
}

/// M03 against the message carrier, graceful and fail-stop: detected. A restart that
/// reuses the old process epoch acts on the late completion of the crashed incarnation's
/// pending submit, and confirms the lost write: an ack with no durable majority, and on
/// the scenario plan two values acked for one epoch.
#[test]
fn carried_mut_03_stale_epoch_in_a_message_is_detected() {
    for mode in MODES {
        check_carried_derivation(Id::M03, mode);
        check_detected_run(Id::M03, Run::Carried(mode));
        check_carried_witness(Id::M03, mode);
    }
}

/// M08 against the timer carrier, graceful and fail-stop: detected. The crashed
/// incarnation's timer, armed on its node's supervisor outside the incarnation's region,
/// comes due after the crash, and its unchecked delivery confirms the lost write.
#[test]
fn carried_mut_08_stale_timer_payload_is_detected() {
    for mode in MODES {
        check_carried_derivation(Id::M08, mode);
        check_detected_run(Id::M08, Run::Carried(mode));
        check_carried_witness(Id::M08, mode);
        let (m, o) = &all().carried[&(Id::M08, mode)];
        let (delivered, accepted) =
            deliveries(m, o, Carrier::Timer, mutants::carried_fence(Id::M08));
        assert_eq!(delivered, accepted, "every stale timer payload is acted on");
        assert_eq!(
            o.carried,
            [delivered, accepted],
            "read back from the journals"
        );
        assert_eq!(o.tally.timers[0], delivered);
        assert_eq!(o.tally.timers[1], delivered, "every carried timer fires");
    }
}

/// The boundary: a payload that names the receiver's current epoch, delivered right after
/// the restart (the recovery report of a synced, unconfirmed write), is accepted and acted
/// on, and the campaign is clean (`carried_mut_00`). In a run, the receiver takes no step
/// of its own between the crash and the receipt, and its first step after the receipt is
/// the confirmation. "Right after" is program order in the replica's actor: other actors
/// interleave. The same report naming the crashed incarnation's epoch is refused: built
/// with that stale payload, the receiver receives it and never confirms.
#[test]
fn carried_boundary_a_current_epoch_right_after_restart_is_accepted() {
    use continuum_asupersync::family::EventBody;
    use continuum_asupersync::family::channel::ChannelEvent;
    use continuum_asupersync::family::effect::EffectEvent;
    use continuum_asupersync::family::lifecycle::LifecycleEvent;
    use continuum_asupersync::family::obligation::ObligationEvent;
    let a = all();
    for mode in MODES {
        let (m, o) = &a.carrier_baselines[&(Carrier::Recovery, mode)];
        let mut checked = 0;
        for p in &o.plans {
            let gi = m
                .campaign
                .groups
                .iter()
                .position(|g| g.id == p.group)
                .expect("g");
            let plan = &m.campaign.groups[gi].plans[p.index];
            let sites = register::carrier_sites(plan, Carrier::Recovery, register::CORRECT_FENCE);
            for s in &sites {
                assert_eq!(s.payload.process, s.crashed + 1, "the current epoch");
                assert!(s.accepted);
            }
            if sites.is_empty() || checked >= 3 {
                continue;
            }
            let built = (m.builder)(plan).expect("built");
            let (_, logs) = baseline::logs_for(&built, 3, m.campaign.plan_seed(gi, p.index));
            let reach = register::reachable(plan.epochs);
            for log in &logs {
                let journal =
                    continuum_asupersync::binding::run(&built.programs, log, &baseline::config())
                        .expect("a run");
                assert!(
                    register::carried_in_journal(&built.roles, &journal)
                        .iter()
                        .all(|d| *d == Some(true))
                );
                let mb = built.roles.mailboxes[0];
                let events = journal.events();
                let crash = events
                    .iter()
                    .position(|e| {
                        matches!(
                            e.body(),
                            EventBody::Lifecycle(
                                LifecycleEvent::RegionCancelRequested { .. }
                                    | LifecycleEvent::RegionCrashed { .. }
                            )
                        )
                    })
                    .expect("a crash");
                let received = events
                    .iter()
                    .position(|e| {
                        matches!(e.body(), EventBody::Channel(ChannelEvent::Received { channel, .. }) if channel.0 == mb.channel)
                    })
                    .expect("received");
                assert!(crash < received);
                let own_step = events[crash..received].iter().any(|e| match e.body() {
                    EventBody::Effect(EffectEvent::Reserved { task, .. }) => task.0 == mb.taker,
                    EventBody::Obligation(ObligationEvent::Opened { holder, kind, .. }) => holder.0
                        == mb.taker
                        && *kind
                            != continuum_asupersync::family::obligation::ObligationKind::SendPermit,
                    EventBody::Channel(ChannelEvent::Sent { sender, .. }) => sender.0 == mb.taker,
                    _ => false,
                });
                assert!(
                    !own_step,
                    "the report is the receiver's first input after the restart"
                );
                let r = baseline::run_one(&built, plan.epochs, log, &reach);
                assert!(r.findings.is_empty(), "{:?}", r.findings);
            }
            // The same report naming the crashed incarnation's epoch: refused.
            let stale: Vec<register::Site> = sites
                .iter()
                .map(|s| {
                    let mut t = *s;
                    t.payload.process = s.crashed;
                    t.accepted = register::accepts(
                        register::CORRECT_FENCE,
                        Carrier::Recovery,
                        s.crashed + 1,
                        s.crashed,
                    );
                    t
                })
                .collect();
            assert!(stale.iter().all(|s| !s.accepted));
            let refused = register::build_with_sites(plan, mode.crash(), &stale);
            let (_, logs) = baseline::logs_for(&refused, 1, m.campaign.plan_seed(gi, p.index));
            let journal = continuum_asupersync::binding::run(
                &refused.programs,
                &logs[0],
                &baseline::config(),
            )
            .expect("a run");
            assert!(
                register::carried_in_journal(&refused.roles, &journal)
                    .iter()
                    .all(|d| *d == Some(false))
            );
            checked += 1;
        }
        assert_eq!(checked, 3, "{mode:?}: recovery sites exist");
    }
}

/// Why a carried rerun's changed plan has no finding (reviewer finding, bn-2faf1): either
/// the stale confirmation's coordinator gets fewer than two confirmations, so no ack can
/// follow ("no quorum"), or an ack can follow and the campaign's logs put two durable
/// confirmations first ("unexplored"). For the second kind, 256 fresh sampled logs of the
/// plan: `(plans, of them detected with 256 logs)`.
fn silent_plans(id: Id, mode: Mode) -> (usize, usize, usize) {
    let a = all();
    let (m, o) = &a.carried[&(id, mode)];
    let fence = mutants::carried_fence(id);
    let (mut no_quorum, mut unexplored, mut found) = (0, 0, 0);
    for p in &o.plans {
        let gi = a
            .base
            .groups
            .iter()
            .position(|g| g.id == p.group)
            .expect("g");
        if !mutants::changed_carried(m, mode.crash(), &a.base, gi, p.index)
            || !p.findings.is_empty()
        {
            continue;
        }
        let plan = &a.base.groups[gi].plans[p.index];
        let mut counts = mutants::confirmation_counts(plan);
        for s in register::carrier_sites(plan, mutants::carrier_of(id), fence) {
            if s.accepted {
                *counts
                    .entry((s.payload.epoch, s.payload.value))
                    .or_default() += 1;
            }
        }
        let quorum = register::carrier_sites(plan, mutants::carrier_of(id), fence)
            .iter()
            .any(|s| s.accepted && counts[&(s.payload.epoch, s.payload.value)] >= 2);
        if !quorum {
            no_quorum += 1;
            continue;
        }
        unexplored += 1;
        let built = (m.builder)(plan).expect("built");
        let reach = register::reachable(plan.epochs);
        let logs = register::sample_logs(&built, 256, m.campaign.plan_seed(gi, p.index) ^ 0x5eed);
        if logs.iter().any(|l| {
            baseline::run_one(&built, plan.epochs, l, &reach)
                .findings
                .iter()
                .any(|f| f.property() != Property::Run)
        }) {
            found += 1;
        }
    }
    (no_quorum, unexplored, found)
}

/// `(no quorum, unexplored, detected with more logs)` by carried rerun.
type Silent = BTreeMap<(Id, Mode), (usize, usize, usize)>;

fn silent_summaries() -> &'static Silent {
    static CELL: OnceLock<Silent> = OnceLock::new();
    CELL.get_or_init(|| {
        all()
            .carried
            .keys()
            .map(|&(id, m)| ((id, m), silent_plans(id, m)))
            .collect()
    })
}

/// Every changed plan of a carried rerun that has no finding is classified, and each
/// "unexplored" one is detected by more logs.
#[test]
fn carried_silent_plans_are_classified() {
    for (&(id, mode), &(no_quorum, unexplored, found)) in silent_summaries() {
        let s = &carried_summaries()[&(id, mode)];
        assert_eq!(
            no_quorum + unexplored,
            s.changed - s.failing,
            "{id:?} {mode:?}"
        );
        assert_eq!(found, unexplored, "{id:?} {mode:?}: more logs detect each");
    }
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
    render_expected(mutants::expected(id))
}

fn render_expected(expected: Expected) -> String {
    match expected {
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
            "excluded by the substrate: {}, on every changed plan; residual, not covered by this campaign: {residual}",
            match exclusion {
                Exclusion::EpochFenced =>
                    "typed refusal TaskEnded on every run, the binding's check for a command to an ended task, with no INV-008 reason",
                Exclusion::TimerDropped =>
                    "typed TimeEvent::Cancelled for every armed timer the crash overtakes, and no timer fired after its epoch is cancelled, by the binding's bookkeeping over the substrate's trace",
                Exclusion::EpochFencedByCrash =>
                    "typed refusal TaskCrashed on every run, the binding's check for a command to a task a fail-stop crash stopped, with no INV-008 reason",
                Exclusion::TimerFenced =>
                    "typed TimeEvent::Fenced for every armed timer the crash stops, and no timer fired after its incarnation crashed",
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
         # Regenerate: PR16_IMPL05_BLESS=1 cargo test -p continuum-asupersync --test pr16_impl05_mutants\n",
    );
    let _ = writeln!(out, "# crash: {}", register::CRASH_SEMANTICS);
    let _ = writeln!(out, "# fail-stop: {}", register::FAIL_STOP_SEMANTICS);
    let _ = writeln!(out, "# carried: {}\n", register::CARRIER_SEMANTICS);
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
    // The previous line ends the section with a blank line; the fail-stop baseline goes
    // before it.
    out.pop();
    let fb = &a.fail_stop_baseline;
    let _ = writeln!(
        out,
        "fail-stop campaign {} seed {}: {} plans, {} runs; identity {}; findings {}; fenced tasks {}, obligations {}, reservations {}, timers {}; tasks cancelled in drains {}",
        mutants::FAIL_STOP_BASELINE,
        a.base.seed,
        fb.plans.len(),
        fb.runs,
        fb.digest,
        by_property(fb),
        fb.tally.fenced[0],
        fb.tally.fenced[1],
        fb.tally.fenced[2],
        fb.tally.fenced[3],
        fb.tally.cancelled
    );
    for c in CARRIERS {
        for mode in MODES {
            let (m, o) = &a.carrier_baselines[&(c, mode)];
            let (delivered, accepted) = deliveries(m, o, c, register::CORRECT_FENCE);
            let site_plans: Vec<&baseline::PlanOutcome> = o
                .plans
                .iter()
                .filter(|p| {
                    let g = m
                        .campaign
                        .groups
                        .iter()
                        .find(|g| g.id == p.group)
                        .expect("g");
                    !register::carrier_sites(&g.plans[p.index], c, register::CORRECT_FENCE)
                        .is_empty()
                })
                .collect();
            let [sch, fired, can, _] = o.tally.timers;
            let _ = writeln!(
                out,
                "carried {} campaign {} seed {}: {} plans, {} runs; identity {}; findings {}; plans with a site {} (runs {}); payloads sent {delivered}, received {}, confirmed by the receiver {} (journals; the build accepts {accepted}); timers scheduled {sch}, fired {fired}, cancelled {can}, fenced {}",
                carrier_token(c),
                m.campaign.name,
                m.campaign.seed,
                o.plans.len(),
                o.runs,
                o.digest,
                by_property(o),
                site_plans.len(),
                site_plans.iter().map(|p| p.runs).sum::<usize>(),
                o.carried[0],
                o.carried[1],
                o.tally.fenced[3]
            );
        }
    }
    out.push('\n');
    for id in Id::ALL {
        let _ = writeln!(out, "[{}]", id.name());
        let _ = writeln!(out, "defect {id:?}: {}", id.defect());
        let _ = writeln!(out, "mutation: {}", id.mutation());
        let _ = writeln!(out, "expected: {}", expected_line(id));
        if let Some(c) = mutants::crash_dependence(id) {
            let _ = writeln!(out, "crash: {c}");
        }
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
        let result = result_line(mutants::expected(id), s, o);
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
        if let Some((fm, fo)) = a.fail_stop.get(&id) {
            out.push_str(&fail_stop_lines(id, fm, fo));
        }
        if mutants::CARRIED.contains(&id) {
            out.push_str(&carried_lines(id));
        }
        out.push('\n');
    }
    out
}

fn carrier_token(c: Carrier) -> &'static str {
    match c {
        Carrier::Message => "message",
        Carrier::Timer => "timer",
        Carrier::Recovery => "recovery",
    }
}

/// A mutant's `carried` lines, graceful then fail-stop (bn-2faf1).
fn carried_lines(id: Id) -> String {
    let expected = mutants::expected_carried(id).expect("a carried expectation");
    let c = mutants::carrier_of(id);
    let mut out = String::new();
    let _ = writeln!(out, "carried mutation: {}", mutants::carried_mutation(id));
    for mode in MODES {
        let label = match mode {
            Mode::Graceful => "carried",
            Mode::FailStop => "carried fail-stop",
        };
        let (m, o) = &all().carried[&(id, mode)];
        let (cm, _) = &all().carrier_baselines[&(c, mode)];
        let s = &carried_summaries()[&(id, mode)];
        let (delivered, accepted) = deliveries(m, o, c, mutants::carried_fence(id));
        let _ = writeln!(out, "{label} expected: {}", render_expected(expected));
        let _ = writeln!(
            out,
            "{label} campaign {} from {}: seed {}, {} plans, {} runs; identity {}",
            m.campaign.name, cm.campaign.name, m.campaign.seed, s.plans, s.runs, o.digest
        );
        let _ = writeln!(
            out,
            "{label} plans changed {} (runs {}), failing {}, detecting {}; runs failing {}, refused {}, detecting a scenario property {}; protocol breaches {}; findings {}; stale payloads sent {delivered}, received {}, confirmed by the receiver {} (journals; the build accepts {accepted})",
            s.changed,
            s.changed_runs,
            s.failing,
            s.detecting_plans,
            s.failing_runs,
            s.refused_runs,
            s.detecting_runs,
            counts(o.breaches.iter().map(|(b, n)| (format!("{b:?}"), *n))),
            by_property(o),
            o.carried[0],
            o.carried[1]
        );
        let (no_quorum, unexplored, found) = silent_summaries()[&(id, mode)];
        let _ = writeln!(
            out,
            "{label} changed plans with no finding {}: the stale confirmation's coordinator gets fewer than two confirmations {no_quorum}; an ack can follow and the campaign's logs order two durable confirmations first {unexplored}, of which 256 more sampled logs detect {found}",
            s.changed - s.failing
        );
        let _ = writeln!(out, "{label} {}", result_line(expected, s, o));
        if let Some(w) = &s.witness {
            out.push_str(&witness_line(&format!("{label} witness"), m, w));
        }
        if let Some(w) = &s.also {
            out.push_str(&witness_line(&format!("{label} second witness"), m, w));
            let _ = writeln!(out, "  causal story: {}", mutants::story(m, w).join(" "));
        }
    }
    out
}

/// A rerun's `fail-stop` lines, next to the graceful ones (bn-20d8u).
fn fail_stop_lines(id: Id, m: &Mutant, o: &Outcome) -> String {
    let expected = mutants::expected_fail_stop(id).expect("a fail-stop expectation");
    let s = &fail_stop_summaries()[&id];
    let mut out = String::new();
    let _ = writeln!(out, "fail-stop expected: {}", render_expected(expected));
    let _ = writeln!(
        out,
        "fail-stop campaign {} from {}: seed {}, {} plans, {} runs; identity {}",
        m.campaign.name,
        all().base.name,
        m.campaign.seed,
        s.plans,
        s.runs,
        o.digest
    );
    let _ = writeln!(
        out,
        "fail-stop plans changed {} (runs {}), failing {}, detecting {}; runs failing {}, refused {}, detecting a scenario property {}; protocol breaches {}; findings {}; fenced tasks {}, obligations {}, reservations {}, timers {}",
        s.changed,
        s.changed_runs,
        s.failing,
        s.detecting_plans,
        s.failing_runs,
        s.refused_runs,
        s.detecting_runs,
        counts(o.breaches.iter().map(|(b, n)| (format!("{b:?}"), *n))),
        by_property(o),
        o.tally.fenced[0],
        o.tally.fenced[1],
        o.tally.fenced[2],
        o.tally.fenced[3]
    );
    let _ = writeln!(out, "fail-stop {}", result_line(expected, s, o));
    if let Some(w) = &s.witness {
        out.push_str(&witness_line("fail-stop witness", m, w));
    }
    out
}

fn result_line(expected: Expected, s: &Summary, o: &Outcome) -> String {
    match expected {
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
        Expected::Excluded {
            exclusion: Exclusion::EpochFencedByCrash,
            ..
        } => format!(
            "result: excluded, every run of the {} changed plans is the typed refusal TaskCrashed, the binding's check with no INV-008 reason; no detection claimed",
            s.changed
        ),
        Expected::Excluded {
            exclusion: Exclusion::TimerFenced,
            ..
        } => {
            let [sch, fired, can, stale] = o.tally.timers;
            format!(
                "result: excluded, timers scheduled {sch}, fired before the crash {fired}, fenced by the crash {}, cancelled {can}, fired after their incarnation crashed {stale}; no finding; no detection claimed",
                o.tally.fenced[3]
            )
        }
        Expected::Deferred { .. } => "result: deferred".to_owned(),
    }
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
