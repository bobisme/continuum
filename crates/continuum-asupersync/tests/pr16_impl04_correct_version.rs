//! PR-16/IMPL-04: the correct version of the replicated register, and the baseline
//! campaign the IMPL-05 mutants (bn-28oa) are run beside. START_HERE PR 16, fourth bullet
//! ("correct version"); bn-5fpl.
//!
//! `tests/support/register_baseline.rs` says what "correct version" means, from
//! `replicated_register.md` (acceptance claim B), the scenario file's property list and
//! bounds, research/09 ("zero false alarms on the correct implementation") and PR 16's
//! exit sentence. In short: a rule on programs (the correct protocol, [`discipline`]),
//! the program with a shutdown so that a run ends quiescent
//! (`register::build_with_shutdown`), and one named, seeded campaign
//! (`pr16-correct-baseline`) over which the scenario's four properties hold on every run.
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr16_impl04_correct_version.evidence.txt`.
//! Regenerate with `PR16_IMPL04_BLESS=1 cargo test -p continuum-asupersync --test
//! pr16_impl04_correct_version`, and review the diff.
//!
//! - **positive** `pr16-impl04-pos-01-baseline` (the campaign: its identity, its runs,
//!   zero findings for each of the four properties), `pos-02-coverage` (what the runs
//!   reach: every durable-register step kind, both values chosen in each epoch, both
//!   scenario targets, every obligation kind, cancelled tasks), `pos-03-scenario` (the
//!   scenario's own plan, M01's causal core run by the correct program), `pos-04-bounds`
//!   (the campaign's bounds, seed, name and property list are the scenario file's), and
//!   `pos-05-sweep` (the one-epoch sweep has one plan per symmetry orbit, and every orbit);
//! - **negative** `neg-01…05`: the checks fire. Journal edits the quiescence and
//!   conservation checks must refute, crafted states the agreement and durability checks
//!   must refute, and one crafted plan per rule of the protocol. They test the checks,
//!   not the program: the program mutants are bn-28oa's;
//! - **boundary** `bnd-01…04`: the IMPL-03 program without the shutdown is refuted by
//!   quiescence alone; the plans run exhaustively and the sampled ones; Agreement's
//!   scope (no correct plan sends a majority of confirmations for two values of one
//!   epoch); and the crash bound.
//!
//! # Scope
//!
//! Refinement over the explored runs only. The plan fixes values and crash points, and
//! the log fixes interleavings. A crashed writer's value is not corroborated by the
//! journal. The shutdown runs after the protocol, the coordinators do not crash, and
//! virtual time is not used. Not DPOR and not the PR-17 checker.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use continuum_asupersync::binding::run;
use continuum_asupersync::family::EventBody;
use continuum_asupersync::family::channel::ChannelEvent;
use continuum_asupersync::family::obligation::{
    ObligationEvent, ObligationKind, ObligationOrdinal, ObligationSet,
};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::lift;

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

#[path = "support/register_baseline.rs"]
#[allow(dead_code)]
mod baseline;

use baseline::{
    Breach, Campaign, Fate, Outcome, Property, SCENARIO, Scope, Unbalanced, Unsettled,
    baseline as baseline_campaign, discipline, execute, one_epoch_options, one_epoch_replica,
    plan_of, render_plan, settle,
};
use register::{Act, Plan, raw_of, write};

fn outcome() -> &'static Outcome {
    static CELL: OnceLock<Outcome> = OnceLock::new();
    CELL.get_or_init(|| execute(&baseline_campaign()))
}

fn campaign() -> &'static Campaign {
    static CELL: OnceLock<Campaign> = OnceLock::new();
    CELL.get_or_init(baseline_campaign)
}

fn repo(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

// ---------------------------------------------------------------------------
// positive
// ---------------------------------------------------------------------------

/// pos-01: every plan of the baseline is correct, and every run of it satisfies the
/// four scenario properties and is a run.
#[test]
fn pos_01_the_baseline_campaign_has_no_finding_on_any_run() {
    let o = outcome();
    assert_eq!(o.breaches, BTreeMap::new(), "every plan obeys the protocol");
    for p in &o.plans {
        assert!(
            p.findings.is_empty(),
            "{} plan {}: {:?}",
            p.group,
            p.index,
            p.findings.first()
        );
    }
    assert_eq!(o.by_property, BTreeMap::new());
    let plans: usize = campaign().groups.iter().map(|g| g.plans.len()).sum();
    assert_eq!(o.plans.len(), plans);
    assert!(o.runs >= plans * 2, "every plan runs");
    assert!(o.correspondence.failures.is_empty());
}

/// pos-02, anti-vacuity: the runs reach every kind of durable-register step, choose each
/// value in each epoch in some run, hit both scenario targets, open every obligation
/// kind the program uses, abort some, and cancel tasks.
#[test]
fn pos_02_the_campaign_reaches_every_step_kind_both_values_and_both_targets() {
    let o = outcome();
    let kinds: BTreeSet<&str> = o.correspondence.kinds.keys().map(String::as_str).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "Abort",
            "Ack",
            "Lose/reserved",
            "Lose/volatile",
            "Reserve",
            "Submit",
            "Sync"
        ])
    );
    let chooses: BTreeSet<&str> = o
        .correspondence
        .chooses
        .keys()
        .map(String::as_str)
        .collect();
    for e in 0..2 {
        for v in register::VALUES {
            assert!(
                chooses.contains(format!("Choose(epoch={e},value={v})").as_str()),
                "epoch {e} value {v} is chosen in some run"
            );
        }
    }
    assert!(o.cancel_after_submit > 0, "cancellation after Submitted");
    assert!(o.crash_after_ack > 0, "crash after the reply");
    assert!(o.contested > 0, "both values durable in one epoch");
    for kind in ["Transaction", "IoOp", "SendPermit"] {
        assert!(o.tally.opened.get(kind).is_some_and(|n| *n > 0), "{kind}");
    }
    assert!(o.tally.aborted > 0 && o.tally.cancelled > 0);
    let opened: usize = o.tally.opened.values().sum();
    assert_eq!(
        opened,
        o.tally.committed + o.tally.aborted,
        "conserved in sum"
    );
    assert_eq!(
        o.tally.completed + o.tally.cancelled,
        o.tally.tasks,
        "every task ends by completing or by cancellation, none by failing"
    );
    // Every region the runs open is finalized, the root included, once per run.
    assert!(o.tally.finalized > o.runs);
    // Every reachable durable-register state of one epoch is visited up to the sweep's
    // symmetries: renaming the replicas and swapping the values.
    let (reached, all) = one_epoch_orbits();
    assert_eq!(reached, all, "every one-epoch state's orbit");
}

/// A projected state's least image under the twelve symmetries of one epoch.
fn state_orbit(raw: &register::Raw) -> register::Raw {
    let (log, pending, acks) = register::parts_of(raw);
    let mut best: Option<register::Raw> = None;
    for perm in PERMS {
        for swapped in [false, true] {
            let v = |x: u8| if swapped { 1 - x } else { x };
            let n = |x: u8| u8::try_from(perm[usize::from(x)]).expect("three");
            let l: Vec<_> = log.iter().map(|&(a, e, x)| (n(a), e, v(x))).collect();
            let p: Vec<_> = pending.iter().map(|&(a, e)| (n(a), e)).collect();
            let k: Vec<_> = acks.iter().map(|&(e, x)| (e, v(x))).collect();
            let image = raw_of(&l, &p, &k);
            if best.as_ref().is_none_or(|b| image < *b) {
                best = Some(image);
            }
        }
    }
    best.expect("twelve images")
}

/// The orbits of the one-epoch reachable states the campaign visits, and all of them.
fn one_epoch_orbits() -> (usize, usize) {
    let one = register::reachable(1);
    let all: BTreeSet<_> = one.iter().map(state_orbit).collect();
    let reached: BTreeSet<_> = outcome()
        .correspondence
        .states
        .iter()
        .filter(|s| one.contains(*s))
        .map(state_orbit)
        .collect();
    (reached.len(), all.len())
}

/// pos-03: the scenario's plan, M01's causal core with the correct program. `a`'s
/// submitted `v0` is lost to its cancellation in every run, `a` confirms nothing before
/// its crash, and only `v1` is ever chosen.
#[test]
fn pos_03_the_scenario_plan_chooses_only_v1_and_loses_as_bytes_every_run() {
    let o = outcome();
    let p = &o.plans[0];
    assert_eq!((p.group, p.index), ("scenario", 0));
    assert!(p.findings.is_empty());
    let mut only = campaign().clone();
    only.groups.truncate(1);
    only.groups[0].plans.truncate(1);
    let s = execute(&only);
    assert_eq!(
        s.correspondence.chooses.keys().cloned().collect::<Vec<_>>(),
        ["Choose(epoch=0,value=v1)"]
    );
    assert_eq!(s.correspondence.kinds["Lose/volatile"], s.runs);
    assert_eq!(s.cancel_after_submit, s.runs);
    assert_eq!(
        s.correspondence.durable.get("Ack(epoch=0,value=v0)"),
        None,
        "v0 never acknowledged"
    );
    // Determinism: the same campaign twice is the same identity, and the seed is
    // load-bearing.
    assert_eq!(execute(&only).digest, s.digest);
    let mut reseeded = only.clone();
    reseeded.seed ^= 1;
    assert_ne!(execute(&reseeded).digest, s.digest);
}

/// The scenario file's value for `key` in `[section]`.
fn scenario_value(section: &str, key: &str) -> String {
    let text = repo("notes/plan/examples/replicated_register.scenario.toml");
    let body = text
        .split(&format!("[{section}]\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("[{section}]"));
    let body = body.split("\n[").next().unwrap_or_default();
    let line = body
        .lines()
        .find(|l| l.split('=').next().map(str::trim) == Some(key))
        .unwrap_or_else(|| panic!("{section}.{key}"));
    line.split_once('=')
        .expect("key = value")
        .1
        .trim()
        .to_owned()
}

/// pos-04: the campaign is the scenario file's: bounds, seed, name, and the properties
/// it checks, in the file's order.
#[test]
fn pos_04_the_campaign_bounds_seed_and_properties_are_the_scenario_files() {
    let n = |s: &str, k: &str| scenario_value(s, k).parse::<u64>().expect("a number");
    assert_eq!(u64::from(SCENARIO.nodes), n("domains", "nodes"));
    assert_eq!(u64::from(SCENARIO.values), n("domains", "values"));
    assert_eq!(u64::from(SCENARIO.epochs), n("domains", "epochs"));
    assert_eq!(SCENARIO.max_crashes as u64, n("faults", "max_crashes"));
    assert_eq!(
        SCENARIO.max_cancellations as u64,
        n("faults", "max_cancellations")
    );
    assert_eq!(baseline::SCENARIO_SEED, n("scenario", "seed"));
    assert_eq!(campaign().seed, baseline::SCENARIO_SEED);
    assert_eq!(campaign().bounds, SCENARIO);
    assert_eq!(
        scenario_value("scenario", "name"),
        format!("\"{}\"", baseline::SCENARIO_NAME)
    );
    let text = repo("notes/plan/examples/replicated_register.scenario.toml");
    let check = text.split("check = [").nth(1).expect("a check list");
    let listed: Vec<&str> = check
        .split(']')
        .next()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    let ours: Vec<&str> = Property::CHECKED
        .iter()
        .filter_map(|p| p.scenario_name())
        .collect();
    assert_eq!(listed, ours);
    // Every campaign plan is within the domains: three replicas, two values, at most
    // two epochs.
    for g in &campaign().groups {
        for p in &g.plans {
            assert!(p.epochs <= SCENARIO.epochs);
        }
    }
}

/// A one-epoch plan's key under renaming the replicas and swapping the values: the
/// least rendering over the twelve symmetries.
const PERMS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn orbit_key(triple: [(u8, Fate); 3]) -> String {
    let swap = |(v, f): (u8, Fate)| match f {
        Fate::Idle => (0, Fate::Idle),
        Fate::CrashReserved { retry } => (1 - v, Fate::CrashReserved { retry: 1 - retry }),
        Fate::CrashSubmitted { retry } => (1 - v, Fate::CrashSubmitted { retry: 1 - retry }),
        other => (1 - v, other),
    };
    let mut keys = Vec::new();
    for perm in PERMS {
        for swapped in [false, true] {
            let t = perm.map(|i| if swapped { swap(triple[i]) } else { triple[i] });
            keys.push(render_plan(&plan_of("k", 1, t.map(one_epoch_replica))));
        }
    }
    keys.into_iter().min().expect("twelve keys")
}

/// The one-epoch sweep's plans as `(value, fate)` triples, read back from the fates'
/// scripts by matching against every option.
fn sweep_triples() -> Vec<[(u8, Fate); 3]> {
    let options = one_epoch_options();
    let group = &campaign().groups[1];
    assert_eq!(group.id, "sweep-one-epoch");
    group
        .plans
        .iter()
        .map(|p| {
            [0, 1, 2].map(|n| {
                *options
                    .iter()
                    .find(|o| {
                        let r = plan_of("k", 1, [one_epoch_replica(**o); 3]);
                        r.replicas[0] == p.replicas[n]
                            && register::incarnation_values(&r, 0)
                                == register::incarnation_values(p, n)
                    })
                    .expect("an option")
            })
        })
        .collect()
}

/// pos-05: the one-epoch sweep has exactly one plan per orbit of every configuration
/// within the crash bound, by an independent enumeration of all ordered triples.
#[test]
fn pos_05_the_one_epoch_sweep_is_one_plan_per_orbit_and_every_orbit() {
    let options = one_epoch_options();
    let mut all = BTreeSet::new();
    for a in &options {
        for b in &options {
            for c in &options {
                let t = [*a, *b, *c];
                if t.iter().filter(|(_, f)| f.crashes()).count() <= SCENARIO.max_crashes {
                    all.insert(orbit_key(t));
                }
            }
        }
    }
    let ours: Vec<String> = sweep_triples().into_iter().map(orbit_key).collect();
    let distinct: BTreeSet<&String> = ours.iter().collect();
    assert_eq!(distinct.len(), ours.len(), "one plan per orbit");
    assert_eq!(
        ours.into_iter().collect::<BTreeSet<_>>(),
        all,
        "every orbit"
    );
}

// ---------------------------------------------------------------------------
// negative: the checks fire
// ---------------------------------------------------------------------------

/// The scenario plan with the shutdown, and the journal of its first sampled log.
fn scenario_journal() -> (Plan, register::Built, Journal) {
    let plan = baseline::scenario_plan();
    let built = register::build_with_shutdown(&plan);
    let (_, logs) = baseline::logs_for(&built, 1, campaign().plan_seed(0, 0));
    let journal = run(&built.programs, &logs[0], &baseline::config()).expect("a run");
    (plan, built, journal)
}

fn rebuild(journal: &Journal, keep: impl Fn(usize, &EventBody) -> bool) -> Journal {
    let mut out = Journal::new();
    for (i, ev) in journal.events().iter().enumerate() {
        if keep(i, ev.body()) {
            out.append(ev.body().clone()).expect("appends");
        }
    }
    out
}

fn findings_of(journal: &Journal) -> (Vec<Unsettled>, Vec<Unbalanced>) {
    let (u, b, _) = settle(journal, &lift(journal));
    (u, b)
}

/// neg-01: the journal without the root's close: the root is not finalized.
fn neg_01() -> (Vec<Unsettled>, Vec<Unbalanced>) {
    let (_, _, journal) = scenario_journal();
    let n = journal.events().len();
    let (u, b) = findings_of(&journal);
    assert_eq!((u, b), (vec![], vec![]), "the whole journal settles");
    // The last four events are the root's close request, drain, finalize and settle.
    findings_of(&rebuild(&journal, |i, _| i + 4 < n))
}

#[test]
fn neg_01_a_journal_that_stops_before_the_root_closes_is_not_quiescent() {
    let (u, b) = neg_01();
    assert_eq!(u, [Unsettled::RegionUnfinalized(0)]);
    assert_eq!(b, []);
}

/// neg-02: the journal without the discharge of the last `IoOp` it opens.
fn neg_02() -> (Vec<Unsettled>, Vec<Unbalanced>, u32) {
    let (_, _, journal) = scenario_journal();
    let io: Vec<u32> = journal
        .events()
        .iter()
        .filter_map(|e| match e.body() {
            EventBody::Obligation(ObligationEvent::Opened {
                obligation,
                kind: ObligationKind::IoOp,
                ..
            }) => Some(obligation.0),
            _ => None,
        })
        .collect();
    let target = *io.last().expect("an IoOp");
    let drop = journal
        .events()
        .iter()
        .position(|e| {
            matches!(e.body(), EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) if obligation.0 == target)
        })
        .expect("its discharge");
    let (u, b) = findings_of(&rebuild(&journal, |i, _| i != drop));
    (u, b, target)
}

#[test]
fn neg_02_an_obligation_never_discharged_is_not_conserved() {
    let (u, b, target) = neg_02();
    assert_eq!(b, [Unbalanced::ObligationOpen(target)]);
    // The lift refuses the edited journal too, so quiescence cannot read it.
    assert_eq!(u, [Unsettled::NoLift]);
}

/// neg-02, the other conservation findings: one discharge journaled twice, and a region
/// settle that reports an open obligation.
fn neg_02_more() -> (Vec<Unbalanced>, Vec<Unbalanced>, u32) {
    let (_, _, journal) = scenario_journal();
    let (at, target) = journal
        .events()
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e.body() {
            EventBody::Obligation(ObligationEvent::Discharged { obligation, .. }) => {
                Some((i, obligation.0))
            }
            _ => None,
        })
        .expect("a discharge");
    let mut twice = Journal::new();
    for (i, ev) in journal.events().iter().enumerate() {
        twice.append(ev.body().clone()).expect("appends");
        if i == at {
            twice.append(ev.body().clone()).expect("appends");
        }
    }
    let mut unsettled = Journal::new();
    let mut region = None;
    for ev in journal.events() {
        let body = match ev.body() {
            EventBody::Obligation(ObligationEvent::RegionSettled {
                region: r, leaked, ..
            }) if region.is_none() => {
                region = Some(r.0);
                EventBody::Obligation(ObligationEvent::RegionSettled {
                    region: *r,
                    open: ObligationSet::new([ObligationOrdinal(target)]),
                    leaked: leaked.clone(),
                })
            }
            other => other.clone(),
        };
        unsettled.append(body).expect("appends");
    }
    (
        findings_of(&twice).1,
        findings_of(&unsettled).1,
        region.expect("a settle"),
    )
}

#[test]
fn neg_02_a_double_discharge_and_an_unbalanced_settle_are_not_conserved() {
    let (twice, unsettled, region) = neg_02_more();
    assert!(
        matches!(twice.as_slice(), [Unbalanced::DischargedTwice(_)]),
        "{twice:?}"
    );
    assert_eq!(unsettled, [Unbalanced::SettledWithObligations(region)]);
}

/// neg-03: the journal without the coordinator's last receive: a confirmation sent and
/// never received.
fn neg_03() -> Vec<Unbalanced> {
    let (_, _, journal) = scenario_journal();
    let drop = journal
        .events()
        .iter()
        .rposition(|e| matches!(e.body(), EventBody::Channel(ChannelEvent::Received { .. })))
        .expect("a receive");
    findings_of(&rebuild(&journal, |i, _| i != drop)).1
}

#[test]
fn neg_03_a_confirmation_never_received_is_not_conserved() {
    let b = neg_03();
    assert!(
        matches!(b.as_slice(), [Unbalanced::MessageUnreceived(..)]),
        "{b:?}"
    );
}

/// neg-04: crafted states: two acks for one epoch, and an ack with one durable record.
#[test]
fn neg_04_agreement_and_acked_is_durable_fire_on_crafted_states() {
    let quorum = raw_of(&[(0, 0, 0), (1, 0, 0)], &[], &[(0, 0)]);
    assert!(!baseline::disagrees(&quorum));
    assert!(!baseline::ack_without_majority(&quorum));
    let two = raw_of(&[(0, 0, 0), (1, 0, 0)], &[], &[(0, 0), (0, 1)]);
    assert!(baseline::disagrees(&two));
    let one = raw_of(&[(0, 0, 0)], &[], &[(0, 0)]);
    assert!(baseline::ack_without_majority(&one));
    let volatile = raw_of(&[(0, 0, 0), (1, 0, 0)], &[(1, 0)], &[(0, 0)]);
    assert!(
        baseline::ack_without_majority(&volatile),
        "volatile is not durable"
    );
}

fn one_replica(acts: Vec<Act>) -> Plan {
    Plan {
        name: "crafted",
        epochs: 1,
        values: [[0, 0]; 3],
        replicas: [acts, vec![], vec![]],
    }
}

/// neg-05: one crafted plan per rule of the protocol, and the rule it breaks.
fn neg_05() -> Vec<(&'static str, Plan, Breach)> {
    use Act::{Abort, Confirm, Crash, CrashRepropose, Release, Reserve, Submit, Sync};
    let mut three = one_replica(write(0));
    three.replicas = [vec![Crash], vec![Crash], vec![Crash]];
    let mut four = one_replica(vec![Reserve(0), Abort(0), Reserve(0), Abort(0)]);
    four.replicas[1] = vec![Crash];
    four.replicas[2] = vec![Crash];
    let mut two_epochs = one_replica(vec![Reserve(1)]);
    two_epochs.epochs = 1;
    let mut value = one_replica(vec![]);
    value.values[0] = [2, 0];
    vec![
        ("epoch-out-of-range", two_epochs, Breach::EpochOutOfRange),
        ("value-out-of-range", value, Breach::ValueOutOfRange),
        (
            "reserve-held",
            one_replica(vec![Reserve(0), Reserve(0)]),
            Breach::ReserveHeld,
        ),
        (
            "rewrite-durable",
            one_replica([write(0), vec![Reserve(0)]].concat()),
            Breach::RewriteDurable,
        ),
        (
            "repropose-durable",
            one_replica([write(0), vec![CrashRepropose(0, 1)]].concat()),
            Breach::RewriteDurable,
        ),
        (
            "submit-without-permit",
            one_replica(vec![Submit(0)]),
            Breach::NoPermit,
        ),
        (
            "submit-twice",
            one_replica(vec![Reserve(0), Submit(0), Submit(0)]),
            Breach::SubmitTwice,
        ),
        (
            "sync-without-bytes",
            one_replica(vec![Reserve(0), Sync(0)]),
            Breach::SyncWithoutBytes,
        ),
        (
            "release-before-sync",
            one_replica(vec![Reserve(0), Submit(0), Release(0)]),
            Breach::ReleaseBeforeSync,
        ),
        (
            "abort-after-bytes",
            one_replica(vec![Reserve(0), Submit(0), Abort(0)]),
            Breach::AbortAfterBytes,
        ),
        (
            "confirm-before-sync",
            one_replica(vec![Reserve(0), Submit(0), Confirm(0), Sync(0), Release(0)]),
            Breach::ConfirmBeforeSync,
        ),
        (
            "confirm-twice",
            one_replica([write(0), vec![Crash, Confirm(0)]].concat()),
            Breach::ConfirmTwice,
        ),
        (
            "unfinished-at-end",
            one_replica(vec![Reserve(0), Submit(0)]),
            Breach::UnfinishedAtEnd,
        ),
        ("crash-bound", three, Breach::CrashBound),
        ("cancellation-bound", four, Breach::CancellationBound),
    ]
}

#[test]
fn neg_05_each_rule_of_the_protocol_refutes_a_crafted_plan() {
    let cases = neg_05();
    for (name, plan, want) in &cases {
        let got = discipline(plan, SCENARIO).expect_err(name);
        assert_eq!(got.breach, *want, "{name}");
    }
    // Every rule is reached by a case above.
    let reached: BTreeSet<Breach> = cases.iter().map(|c| c.2).collect();
    assert_eq!(reached.len(), 14);
    // And the correct write is correct.
    assert_eq!(discipline(&one_replica(write(0)), SCENARIO), Ok(()));
}

// ---------------------------------------------------------------------------
// boundaries
// ---------------------------------------------------------------------------

/// bnd-01: the IMPL-03 program, without the shutdown, over the scenario plan's logs:
/// refinement, agreement and conservation hold, and quiescence alone fails.
fn bnd_01() -> BTreeMap<Property, usize> {
    let plan = baseline::scenario_plan();
    let built = register::build(&plan);
    let reach = register::reachable(1);
    let (_, logs) = baseline::logs_for(&built, 20, campaign().plan_seed(0, 0));
    let mut out = BTreeMap::new();
    for log in &logs {
        let r = baseline::run_one(&built, 1, log, &reach);
        assert!(!r.findings.is_empty(), "{log}: not quiescent");
        for f in &r.findings {
            *out.entry(f.property()).or_insert(0) += 1;
        }
    }
    out
}

#[test]
fn bnd_01_without_the_shutdown_only_quiescence_fails() {
    let got = bnd_01();
    assert_eq!(
        got.keys().copied().collect::<Vec<_>>(),
        [Property::Quiescence]
    );
}

#[test]
fn bnd_02_small_plans_run_every_log_and_larger_ones_a_seeded_sample() {
    let o = outcome();
    let sampled = o
        .plans
        .iter()
        .filter(|p| matches!(p.scope, Scope::Sampled { .. }))
        .count();
    assert_eq!(o.exhaustive + sampled, o.plans.len());
    assert!(o.exhaustive > 0 && sampled > 0);
    for p in &o.plans[..2] {
        assert!(
            matches!(p.scope, Scope::Sampled { .. }),
            "the scenario plans"
        );
        assert_eq!(p.runs, baseline::SCENARIO_LOGS);
    }
}

/// bnd-03: Agreement's scope. No campaign plan sends a majority of confirmations for two
/// values of one epoch: each replica confirms each epoch at most once, and two
/// majorities of three intersect. So Agreement on these runs follows from the protocol
/// rule, while some runs still hold both values durable in one epoch.
fn bnd_03() -> (usize, usize) {
    let mut two_value_majorities = 0;
    for g in &campaign().groups {
        for p in &g.plans {
            for e in 0..p.epochs {
                let mut per: BTreeMap<u8, usize> = BTreeMap::new();
                for n in 0..3 {
                    let values = register::incarnation_values(p, n);
                    let mut inc = 0;
                    for act in &p.replicas[n] {
                        match *act {
                            Act::Crash | Act::CrashRepropose(..) => inc += 1,
                            Act::Confirm(f) if f == e => {
                                *per.entry(values[inc][usize::from(e)]).or_default() += 1;
                            }
                            _ => {}
                        }
                    }
                }
                if per.values().filter(|c| **c >= 2).count() > 1 {
                    two_value_majorities += 1;
                }
            }
        }
    }
    (two_value_majorities, outcome().contested)
}

#[test]
fn bnd_03_no_correct_plan_confirms_two_values_of_one_epoch_to_a_majority() {
    let (two, contested) = bnd_03();
    assert_eq!(two, 0);
    assert!(contested > 0);
}

/// bnd-04: the crash bound: the sweep has plans with exactly two crashes, and none with
/// three; a third crash is the protocol's `CrashBound`.
fn bnd_04() -> BTreeMap<usize, usize> {
    let mut by_crashes = BTreeMap::new();
    for t in sweep_triples() {
        *by_crashes
            .entry(t.iter().filter(|(_, f)| f.crashes()).count())
            .or_insert(0) += 1;
    }
    by_crashes
}

#[test]
fn bnd_04_the_sweep_reaches_the_crash_bound_and_not_past_it() {
    let got = bnd_04();
    assert!(got.get(&2).is_some_and(|n| *n > 0));
    assert_eq!(got.keys().max(), Some(&2));
    let over = plan_of(
        "over",
        1,
        [
            one_epoch_replica((0, Fate::CrashSynced)),
            one_epoch_replica((0, Fate::CrashSynced)),
            one_epoch_replica((0, Fate::CrashReplied)),
        ],
    );
    assert_eq!(
        discipline(&over, SCENARIO).map_err(|v| v.breach),
        Err(Breach::CrashBound)
    );
}

// ---------------------------------------------------------------------------
// the evidence
// ---------------------------------------------------------------------------

fn plans_digest(c: &Campaign) -> String {
    use continuum_value::identity::{Blake3Hasher, ContentHasher};
    let mut s = String::new();
    for g in &c.groups {
        for p in &g.plans {
            let _ = writeln!(s, "{} {}", g.id, render_plan(p));
        }
    }
    Blake3Hasher::hash(s.as_bytes()).to_string()
}

fn counts<K: std::fmt::Display>(m: impl IntoIterator<Item = (K, usize)>) -> String {
    m.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn evidence() -> String {
    let c = campaign();
    let o = outcome();
    let mut out = String::from(
        "# PR-16/IMPL-04 correct version of the replicated register (bn-5fpl): the protocol rule,\n\
         # the program with its shutdown, and the baseline campaign in\n\
         # crates/continuum-asupersync/tests/support/register_baseline.rs.\n\
         # Regenerate: PR16_IMPL04_BLESS=1 cargo test -p continuum-asupersync --test pr16_impl04_correct_version\n\n",
    );
    let _ = writeln!(out, "[pr16-impl04-pos-01-baseline]");
    let _ = writeln!(
        out,
        "campaign {} seed {} (scenario {}); bounds nodes={} values={} epochs<={} crashes<={} cancellations<={}",
        c.name,
        c.seed,
        baseline::SCENARIO_NAME,
        c.bounds.nodes,
        c.bounds.values,
        c.bounds.epochs,
        c.bounds.max_crashes,
        c.bounds.max_cancellations
    );
    for (g, gi) in c.groups.iter().zip(0..) {
        let plans: Vec<_> = o.plans.iter().filter(|p| p.group == g.id).collect();
        let runs: usize = plans.iter().map(|p| p.runs).sum();
        let exhaustive = plans
            .iter()
            .filter(|p| p.scope == Scope::Exhaustive)
            .count();
        let _ = writeln!(
            out,
            "group {gi} {}: {} plans, logs per plan {} (all when fewer), {} runs, {} plans exhaustive",
            g.id,
            g.plans.len(),
            g.logs,
            runs,
            exhaustive
        );
    }
    let _ = writeln!(out, "plans digest {}", plans_digest(c));
    let _ = writeln!(out, "campaign identity {}", o.digest);
    let _ = writeln!(
        out,
        "runs {}; protocol breaches {}; findings: {}",
        o.runs,
        o.breaches.len(),
        Property::CHECKED
            .iter()
            .chain([Property::Run].iter())
            .map(|p| format!(
                "{}={}",
                p.scenario_name().unwrap_or("run"),
                o.by_property.get(p).copied().unwrap_or(0)
            ))
            .collect::<Vec<_>>()
            .join(" ")
    );
    out.push('\n');
    let _ = writeln!(out, "[pr16-impl04-pos-02-coverage]");
    let _ = writeln!(
        out,
        "steps {} observed, {} stutters",
        o.correspondence.steps, o.correspondence.stutters
    );
    let _ = writeln!(
        out,
        "durable-register steps: {}",
        counts(o.correspondence.kinds.iter().map(|(k, v)| (k, *v)))
    );
    let _ = writeln!(
        out,
        "abstract chooses: {}",
        counts(o.correspondence.chooses.iter().map(|(k, v)| (k, *v)))
    );
    let one = register::reachable(1);
    let two = register::reachable(2);
    let _ = writeln!(
        out,
        "projected states: {} of {} one-epoch reachable, {} of {} two-epoch reachable",
        o.correspondence.states.intersection(&one).count(),
        one.len(),
        o.correspondence.states.intersection(&two).count(),
        two.len()
    );
    let (reached, all) = one_epoch_orbits();
    let _ = writeln!(
        out,
        "one-epoch reachable state orbits under replica renaming and value swap: {reached} of {all} visited"
    );
    let _ = writeln!(
        out,
        "targets: cancellation after Submitted in {} runs, crash after an ack in {} runs, both values durable in one epoch in {} runs",
        o.cancel_after_submit, o.crash_after_ack, o.contested
    );
    let _ = writeln!(
        out,
        "obligations opened: {}; committed={} aborted={}; tasks={} cancelled={}; regions finalized={}; confirmations sent={}",
        counts(o.tally.opened.iter().map(|(k, v)| (k, *v))),
        o.tally.committed,
        o.tally.aborted,
        o.tally.tasks,
        o.tally.cancelled,
        o.tally.finalized,
        o.tally.sent
    );
    out.push('\n');
    let _ = writeln!(out, "[pr16-impl04-pos-03-scenario]");
    let scenario = &c.groups[0].plans[0];
    let _ = writeln!(out, "plan {}", render_plan(scenario));
    let mut only = c.clone();
    only.groups.truncate(1);
    only.groups[0].plans.truncate(1);
    let s = execute(&only);
    let _ = writeln!(
        out,
        "{} runs; chooses {}; Lose/volatile={}; findings {}; identity {}",
        s.runs,
        counts(s.correspondence.chooses.iter().map(|(k, v)| (k, *v))),
        s.correspondence.kinds["Lose/volatile"],
        s.by_property.values().sum::<usize>(),
        s.digest
    );
    out.push('\n');
    let _ = writeln!(out, "[pr16-impl04-pos-04-bounds]");
    let _ = writeln!(
        out,
        "notes/plan/examples/replicated_register.scenario.toml: domains, max_crashes, max_cancellations, seed, name and check list equal the campaign's: {}; not modeled: [network] faults, max_partitions, [storage] torn tails",
        Property::CHECKED
            .iter()
            .filter_map(|p| p.scenario_name())
            .collect::<Vec<_>>()
            .join(", ")
    );
    out.push('\n');
    let _ = writeln!(out, "[pr16-impl04-pos-05-sweep]");
    let _ = writeln!(
        out,
        "one-epoch options {}, orbits under replica renaming and value swap with <= {} crashes: {}",
        one_epoch_options().len(),
        SCENARIO.max_crashes,
        c.groups[1].plans.len()
    );
    out.push('\n');
    let (u, b) = neg_01();
    let _ = writeln!(
        out,
        "[pr16-impl04-neg-01-root-open]\nscenario log 0 without the root's close\nquiescence {u:?} conservation {b:?}\n"
    );
    let (_, b, target) = neg_02();
    let _ = writeln!(
        out,
        "[pr16-impl04-neg-02-undischarged]\nscenario log 0 without the discharge of o{target}\nconservation {b:?}"
    );
    let (twice, unsettled, region) = neg_02_more();
    let _ = writeln!(
        out,
        "scenario log 0 with its first discharge twice: conservation {twice:?}\nscenario log 0 with r{region} settled holding an obligation: conservation {unsettled:?}\n"
    );
    let _ = writeln!(
        out,
        "[pr16-impl04-neg-03-unreceived]\nscenario log 0 without the last receive\nconservation {:?}\n",
        neg_03()
    );
    let _ = writeln!(
        out,
        "[pr16-impl04-neg-04-crafted-states]\ntwo acks for epoch 0: Agreement fails; an ack over one durable record or over volatile bytes: AckedIsDurable fails\n"
    );
    let _ = writeln!(out, "[pr16-impl04-neg-05-protocol-rules]");
    for (name, plan, _) in neg_05() {
        let v = discipline(&plan, SCENARIO).expect_err(name);
        let _ = writeln!(
            out,
            "{name}: {:?} at replica {:?} act {}",
            v.breach, v.replica, v.at
        );
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "[pr16-impl04-bnd-01-no-shutdown]\nIMPL-03's build of the scenario plan, 20 logs: findings by property {}\n",
        counts(bnd_01().into_iter().map(|(k, v)| (format!("{k:?}"), v)))
    );
    let sampled = o.plans.len() - o.exhaustive;
    let _ = writeln!(
        out,
        "[pr16-impl04-bnd-02-scope]\nplans exhaustive={} sampled={}\n",
        o.exhaustive, sampled
    );
    let (two, contested) = bnd_03();
    let _ = writeln!(
        out,
        "[pr16-impl04-bnd-03-agreement-scope]\nplan epochs with two values confirmed to a majority: {two}; runs with both values durable in one epoch: {contested}\n"
    );
    let _ = writeln!(
        out,
        "[pr16-impl04-bnd-04-crash-bound]\nsweep plans by crashes: {}; three crashes: CrashBound",
        counts(bnd_04())
    );
    out
}

#[test]
fn the_evidence_matches_its_golden() {
    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/pr16_impl04_correct_version.evidence.txt"
    );
    let got = evidence();
    if std::env::var_os("PR16_IMPL04_BLESS").is_some() {
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
        "the evidence drifted at {first}; regenerate with PR16_IMPL04_BLESS=1 and review"
    );
    let ids: Vec<&str> = got
        .lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "artifact IDs are unique");
    assert_eq!(ids.len(), 14);
}

/// The IMPL-03 program is unchanged by the shutdown: `build_with_shutdown`'s programs
/// are `build`'s plus one actor, operation for operation, for every campaign plan.
#[test]
fn the_shutdown_only_adds_an_actor() {
    for g in &campaign().groups {
        for p in &g.plans {
            let (a, b) = (register::build(p), register::build_with_shutdown(p));
            assert_eq!(b.programs.len(), a.programs.len() + 1);
            assert_eq!(&b.programs[..a.programs.len()], &a.programs[..]);
            assert_eq!(a.roles, b.roles);
        }
    }
}

/// The hooks bn-28oa uses: a program mutant through `execute_with` and
/// `register::without_op` (a shutdown with one writer `Finish` missing, an orphan) is
/// refuted by quiescence, and a plan the builder refuses is a typed finding.
#[test]
fn the_hooks_run_a_program_mutant_and_refuse_an_unbuildable_plan() {
    let mut only = campaign().mutated("hook-probe", Plan::clone);
    only.groups.truncate(1);
    only.groups[0].plans.truncate(1);
    only.groups[0].logs = 4;
    assert_eq!(only.name, "hook-probe");
    let orphan = baseline::execute_with(&only, |p| {
        let built = register::build_with_shutdown(p);
        let last = built.programs.len() - 1;
        let at = built.programs[last]
            .iter()
            .rposition(|op| {
                matches!(
                    op,
                    continuum_asupersync::binding::SubstrateOp::Finish { .. }
                )
            })
            .expect("a finish");
        Ok(register::without_op(&built, last, at))
    });
    assert_eq!(orphan.runs, 4);
    assert!(orphan.plans[0].findings.len() == 4);
    assert_eq!(
        orphan.by_property.keys().copied().collect::<Vec<_>>(),
        [Property::Quiescence]
    );
    let refused = baseline::execute_with(&only, |_| Err("no program".to_owned()));
    assert_eq!(refused.runs, 0);
    assert_eq!(
        refused.plans[0].first(),
        Some(&baseline::Finding::Unbuildable("no program".to_owned()))
    );
}
