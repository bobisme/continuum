//! Dedicated exit evidence for `PR-14-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-14-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14's Exit line).
//!
//! > **Exit:** identical controlled choice logs produce canonical identical semantic
//! > events.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 14
//!
//! This file is an independent exit layer over the six PR-14-IMPL bones' own per-family
//! suites (`pr14_impl01_binding.rs` .. `pr14_impl06_channels.rs`, `pr14_deadline_cancellation.rs`,
//! `pr14_calculus_ledger.rs`), the way `pr11_exit_evidence.rs` sits over `dx01_falsification.rs`
//! and `pr13_exit_evidence.rs` sits over its four CLI bones: one named witness that walks the
//! whole sentence in one place, over a fixed corpus of its own, touching none of the
//! per-family tests and none of `src/`. Where those suites each hold one or two families to
//! the substrate, this file runs [`binding::run`] with a [`BindingConfig`] observing **all
//! six** families at once — lifecycle, reserve/commit/abort, cancellation (region and
//! deadline), obligations, virtual time and channels — over every log of a corpus built for
//! exactly that combination.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical choice logs give byte-identical journals, every log of the corpus, all six families observed | [`identical_choice_logs_give_byte_identical_journals`] |
//! | different choice logs give different journals (anti-vacuity of the identity) | [`identical_choice_logs_give_byte_identical_journals`] |
//! | the lab seed does not reach the journal, on a sample, across 7 seeds | [`the_lab_seed_does_not_reach_the_journal`] |
//! | every journal of the corpus lifts as `Conforms`, never accepted on `Violates` or `Inconclusive` | [`every_journal_lifts_as_conforms`] |
//! | the A7 model, independently written, accepts every journal of the corpus under the full six-family alphabet | [`the_a7_model_accepts_every_journal`] |
//! | negative control: a perturbed choice log (a different, still-legal schedule) changes the journal | [`a_perturbed_choice_log_changes_the_journal`] |
//! | negative control: a mutated journal — one event of a conforming run deleted — fails the lift | [`a_mutated_journal_fails_the_lift`] |
//! | the retained exit-evidence artifact — corpus digest, per-scenario journal-digest counts, family coverage — is byte-stable | [`the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden`] |
//!
//! # The corpus
//!
//! Two named scenarios, each a fixed setup (run first, in program order) plus a set of
//! actors whose operations [`ChoiceLog::enumerate`] interleaves exhaustively, exactly the
//! convention `pr14_deadline_cancellation.rs` and `pr14_impl06_channels.rs` use:
//!
//! - **`deadline-and-effect`** (7,560 logs): `r1` has two sibling child regions, `r2`
//!   (`t1`, deadline 50, holding a lease and a reservation) and `r3` (`t2`, deadline 80).
//!   The clock advances past both deadlines while `r3` is cancelled explicitly before its
//!   task's deadline is reached. One scenario, both cancellation causes (deadline and
//!   region), reserve/commit/abort's auto-abort on cancellation, an obligation's
//!   auto-abort/leak on cancellation, and virtual time's advances and deadlines — five of
//!   the six families in one substrate run, the same shape `pr14_deadline_cancellation.rs`
//!   already runs and lifts.
//! - **`channels`** (720 logs): three root producers race one message each into a
//!   capacity-1 channel whose receiving task finishes mid-race, and a second channel is
//!   closed from the sender's side while its receiver receives — the sixth family,
//!   channel communication, plus lifecycle.
//!
//! Every run in both scenarios uses one [`BindingConfig`] that observes all six families
//! (`config`), so "all six families observed" is the run's own configuration, not merely
//! the union of what a given log happens to produce; the two scenarios' union of *events*
//! spans all six families, which [`the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden`]'s
//! golden pins as a count.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use continuum_asupersync::binding::{BindingConfig, Program, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::Family;
use continuum_asupersync::family::channel::ChannelLabel;
use continuum_asupersync::family::effect::ReservationLabel;
use continuum_asupersync::family::lifecycle::{RegionLabel, TaskLabel};
use continuum_asupersync::family::obligation::ObligationKind;
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, Lifted, lift};
use continuum_task::region::worker::Resumability;
use continuum_value::identity::{Blake3Hasher, ContentHasher, Digest256};

// This exit layer uses only the model's top-level judge/alphabet surface (the
// per-family per-transition machinery is `a7_primitive_conformance.rs`'s to exercise),
// so most of the shared model file is unused from this binary's own vantage point.
#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

use model::{Alphabet, FamilyTag, Verdict, judge};

const SEED: u64 = 0;
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];

/// The configuration every test in this file runs: all six PR-14 families observed.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

/// The A7 model's alphabet over all six families, held to the same journals.
fn all_families_alphabet() -> Alphabet {
    Alphabet::new([
        FamilyTag::Lifecycle,
        FamilyTag::Effect,
        FamilyTag::Cancellation,
        FamilyTag::Obligation,
        FamilyTag::Time,
        FamilyTag::Channel,
    ])
}

// --- scenario: deadline-and-effect ----------------------------------------------------

const ROOT: RegionLabel = RegionLabel::ROOT;
const R1: RegionLabel = RegionLabel(1);
const R2: RegionLabel = RegionLabel(2);
const R3: RegionLabel = RegionLabel(3);
const T1: TaskLabel = TaskLabel(1);
const T2: TaskLabel = TaskLabel(2);
const T3: TaskLabel = TaskLabel(3);

fn open(parent: RegionLabel, child: RegionLabel) -> SubstrateOp {
    SubstrateOp::OpenRegion { parent, child }
}

fn spawn(region: RegionLabel, task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region,
        task,
        resumability: Resumability::Resumable,
    }
}

fn spawn_by(region: RegionLabel, task: TaskLabel, deadline: u64) -> SubstrateOp {
    SubstrateOp::SpawnWithDeadline {
        region,
        task,
        resumability: Resumability::Resumable,
        deadline,
    }
}

const fn begin(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Begin { task }
}

const fn advance(nanos: u64) -> SubstrateOp {
    SubstrateOp::Advance { nanos }
}

/// `r1` has two sibling children: `r2` (`t1`, deadline 50, holding a lease and a
/// reservation) and `r3` (`t2`, deadline 80). The clock advances to 45 and then 20 more
/// (65 total); `t1`'s deadline family, lifecycle and cancellation-by-deadline all fire on
/// or before that; `r3` is cancelled explicitly before `t2`'s deadline (80) is reached, so
/// `t2` ends by the region's cancellation instead. A root task `t3` runs to completion
/// alongside. Every interleaving of the four actors below is a legal run (the same shape
/// as `pr14_deadline_cancellation.rs::gate`).
///
/// ```text
/// r1 ─┬─ r2 (t1, deadline 50: lease, reservation)
///     └─ r3 (t2, deadline 80)
/// ```
fn deadline_and_effect_scenario() -> (Program, Vec<Program>) {
    let setup = vec![
        open(ROOT, R1),
        open(R1, R2),
        open(R1, R3),
        spawn_by(R2, T1, 50),
        spawn_by(R3, T2, 80),
        begin(T1),
        begin(T2),
        SubstrateOp::Acquire {
            task: T1,
            reservation: ReservationLabel(1),
            kind: ObligationKind::Lease,
        },
        SubstrateOp::Reserve {
            task: T1,
            reservation: ReservationLabel(2),
        },
    ];
    let actors = vec![
        vec![advance(45), advance(20)],
        vec![
            SubstrateOp::Continue { task: T1 },
            SubstrateOp::Close { region: R2 },
        ],
        vec![
            SubstrateOp::Continue { task: T2 },
            SubstrateOp::Cancel { region: R3 },
        ],
        vec![spawn(ROOT, T3), begin(T3), SubstrateOp::Finish { task: T3 }],
    ];
    (setup, actors)
}

// --- scenario: channels -----------------------------------------------------------------

const CT1: TaskLabel = TaskLabel(1);
const CT2: TaskLabel = TaskLabel(2);
const CT3: TaskLabel = TaskLabel(3);
const CRX: TaskLabel = TaskLabel(4);
const CRE: TaskLabel = TaskLabel(5);
const C1: ChannelLabel = ChannelLabel(1);
const C2: ChannelLabel = ChannelLabel(2);

fn c_spawn(task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region: ROOT,
        task,
        resumability: Resumability::Resumable,
    }
}

fn send(task: TaskLabel, channel: ChannelLabel) -> SubstrateOp {
    SubstrateOp::Send { task, channel }
}

/// Three root producers (`ct1`, `ct2`, `ct3`) race one message each into `c1` (capacity
/// 1), whose receiving task `crx` finishes mid-race: the receiver goes, dropping what is
/// queued and waking every sender still blocked, which fail as closed. `c2` (capacity 1)
/// is closed from the sender's side while its receiver `cre` receives once. The same
/// shape as `pr14_impl06_channels.rs::closing`.
fn channel_scenario() -> (Program, Vec<Program>) {
    let mut setup = Vec::new();
    for task in [CT1, CT2, CT3, CRX, CRE] {
        setup.push(c_spawn(task));
        setup.push(begin(task));
    }
    setup.push(SubstrateOp::OpenChannel {
        channel: C1,
        capacity: 1,
        receiver: CRX,
    });
    setup.push(SubstrateOp::OpenChannel {
        channel: C2,
        capacity: 1,
        receiver: CRE,
    });
    let actors = vec![
        vec![send(CT1, C1)],
        vec![send(CT2, C1)],
        vec![send(CT3, C1)],
        vec![SubstrateOp::Finish { task: CRX }],
        vec![SubstrateOp::CloseSenders { channel: C2 }],
        vec![SubstrateOp::Recv { channel: C2 }],
    ];
    (setup, actors)
}

// --- corpus assembly ---------------------------------------------------------------------

/// A corpus entry: name, programs (the setup as actor 0), logs. A log runs the setup
/// first, then its own choices, which index the actors that remain enabled — the
/// convention `pr14_deadline_cancellation.rs` and `pr14_impl06_channels.rs` share.
type Entry = (&'static str, Vec<Program>, Vec<ChoiceLog>);

fn entry(name: &'static str, setup: Program, actors: Vec<Program>) -> Entry {
    let lengths: Vec<usize> = actors.iter().map(Vec::len).collect();
    let logs: Vec<ChoiceLog> = ChoiceLog::enumerate(&lengths)
        .into_iter()
        .map(|log| {
            let mut choices = vec![0; setup.len()];
            choices.extend(log.choices().iter().map(|c| c.0));
            ChoiceLog::new(choices)
        })
        .collect();
    let mut programs = vec![setup];
    programs.extend(actors);
    (name, programs, logs)
}

const DEADLINE_AND_EFFECT_LOGS: usize = 7_560; // 9! / (2!*2!*2!*3!)
const CHANNEL_LOGS: usize = 720; // 6!
const ALL_LOGS: usize = DEADLINE_AND_EFFECT_LOGS + CHANNEL_LOGS;

fn corpus() -> Vec<Entry> {
    let (setup, actors) = deadline_and_effect_scenario();
    let deadline_and_effect = entry("deadline-and-effect", setup, actors);
    let (setup, actors) = channel_scenario();
    let channels = entry("channels", setup, actors);
    assert_eq!(deadline_and_effect.2.len(), DEADLINE_AND_EFFECT_LOGS);
    assert_eq!(channels.2.len(), CHANNEL_LOGS);
    vec![deadline_and_effect, channels]
}

// --- fail-closed helpers (INV-008: never "not Violates") ---------------------------------

/// The conforming lift, or a panic naming exactly why it was not one. Never treats
/// `Violates` or `Inconclusive` as a pass: the exit asserts `Conforms`, nothing weaker.
fn expect_conforms(verdict: LiftVerdict, context: &str) -> Lifted {
    match verdict {
        LiftVerdict::Conforms(lifted) => lifted,
        LiftVerdict::Violates { seq, reason } => {
            panic!("{context}: the lift reports Violates at seq {seq}: {reason:?}")
        }
        LiftVerdict::Inconclusive {
            seq,
            family,
            reason,
        } => panic!(
            "{context}: the lift reports Inconclusive at seq {seq}, family {family:?}: \
             {reason:?} (fail closed: Inconclusive is never accepted as a pass)"
        ),
    }
}

/// The A7 model's acceptance, or a panic naming exactly why it was not one.
fn expect_accepted(verdict: &Verdict, context: &str) {
    assert!(
        verdict.is_accepted(),
        "{context}: the A7 model did not accept: {verdict}"
    );
}

// --- the exit property ---------------------------------------------------------------

#[test]
fn identical_choice_logs_give_byte_identical_journals() {
    let mut compared = 0;
    let mut distinct_overall = BTreeSet::new();
    for (name, programs, logs) in corpus() {
        let mut distinct = BTreeSet::new();
        for log in &logs {
            let first =
                run(&programs, log, &config(SEED)).expect("every corpus log is a legal run");
            let second =
                run(&programs, log, &config(SEED)).expect("every corpus log is a legal run");
            assert_eq!(
                first.encode().unwrap(),
                second.encode().unwrap(),
                "{name} log {log}: two runs of the identical choice log gave different \
                 canonical bytes"
            );
            distinct.insert(first.digest().unwrap());
            compared += 1;
        }
        assert!(
            distinct.len() > 1,
            "{name}: every log in this scenario gave the same journal (vacuous corpus)"
        );
        distinct_overall.extend(distinct);
    }
    assert_eq!(compared, ALL_LOGS);
    // Anti-vacuity of the identity itself: different choice logs give different journals,
    // over the whole corpus, not merely within one scenario.
    assert!(
        distinct_overall.len() > 1,
        "the whole corpus collapsed to one journal digest"
    );
}

#[test]
fn the_lab_seed_does_not_reach_the_journal() {
    let mut checked = 0;
    for (name, programs, logs) in corpus() {
        // A sample, not the whole corpus: first, middle and last log of each scenario.
        let sample_indices = [0, logs.len() / 2, logs.len() - 1];
        for &index in &sample_indices {
            let log = &logs[index];
            let reference = run(&programs, log, &config(SEED))
                .expect("every corpus log is a legal run")
                .digest()
                .unwrap();
            for seed in SEEDS {
                let digest = run(&programs, log, &config(seed))
                    .expect("every corpus log is a legal run")
                    .digest()
                    .unwrap();
                assert_eq!(
                    digest, reference,
                    "{name} log {log}: seed {seed:#x} changed the journal"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 2 * 3 * SEEDS.len());
}

#[test]
fn every_journal_lifts_as_conforms() {
    let mut checked = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let journal =
                run(&programs, log, &config(SEED)).expect("every corpus log is a legal run");
            expect_conforms(lift(&journal), &format!("{name} log {log}"));
            checked += 1;
        }
    }
    assert_eq!(checked, ALL_LOGS);
}

#[test]
fn the_a7_model_accepts_every_journal() {
    let alphabet = all_families_alphabet();
    let mut checked = 0;
    for (name, programs, logs) in corpus() {
        for log in &logs {
            let journal =
                run(&programs, log, &config(SEED)).expect("every corpus log is a legal run");
            let bytes = journal.encode().unwrap();
            let verdict = judge(&alphabet, &bytes);
            expect_accepted(&verdict, &format!("{name} log {log}"));
            checked += 1;
        }
    }
    assert_eq!(checked, ALL_LOGS);
}

// --- negative controls -----------------------------------------------------------------

/// Anti-vacuity: a different, still-legal choice log over the same programs produces a
/// different journal. Finds the first log that diverges from the corpus's first log,
/// rather than assuming a specific pair, so the assertion does not depend on which
/// reordering happens to matter for a given scenario.
#[test]
fn a_perturbed_choice_log_changes_the_journal() {
    for (name, programs, logs) in corpus() {
        let reference_log = &logs[0];
        let reference_digest = run(&programs, reference_log, &config(SEED))
            .expect("every corpus log is a legal run")
            .digest()
            .unwrap();
        let found = logs.iter().skip(1).find(|candidate| {
            let digest = run(&programs, candidate, &config(SEED))
                .expect("every corpus log is a legal run")
                .digest()
                .unwrap();
            digest != reference_digest
        });
        assert!(
            found.is_some(),
            "{name}: no other log in the corpus gave a journal different from log {reference_log} \
             (the perturbation control is vacuous)"
        );
    }
}

/// Anti-vacuity of the lift, not just the identity: deleting one event from a real,
/// conforming journal must not still conform. The mutation is at the journal level (data,
/// not a re-run), the same technique `pr14_deadline_cancellation.rs` uses for its own
/// mutation operators (`bodies`/rebuild).
#[test]
fn a_mutated_journal_fails_the_lift() {
    let (_, programs, logs) = &corpus()[0];
    let journal = run(programs, &logs[0], &config(SEED)).expect("the corpus's own first log runs");
    let original = expect_conforms(lift(&journal), "the unmutated journal");
    let _ = original;

    let bodies: Vec<_> = journal.events().iter().map(|e| e.body().clone()).collect();
    assert!(
        bodies.len() > 1,
        "the corpus's first journal is too short to mutate meaningfully"
    );

    // Deleting the very first event removes something every later event's identity or
    // ordering depends on (an open-region or a spawn): the lift must refuse it rather than
    // silently re-deriving a conforming trace from what remains.
    let mut mutated = Journal::new();
    for body in bodies.into_iter().skip(1) {
        mutated.append(body).unwrap();
    }
    match lift(&mutated) {
        LiftVerdict::Conforms(_) => {
            panic!("deleting the journal's first event still conformed: the mutation is vacuous")
        }
        LiftVerdict::Violates { .. } | LiftVerdict::Inconclusive { .. } => {}
    }
}

// --- retained artifact -------------------------------------------------------------------

/// The corpus digest: a canonical summary of its shape (scenario names, program sizes, log
/// counts), independent of any run's output, so a corpus edit that changes what the exit
/// tests without changing a scenario's name is still visible in the golden diff.
fn corpus_digest() -> Digest256 {
    let mut buf = Vec::new();
    for (name, programs, logs) in corpus() {
        buf.extend_from_slice(name.as_bytes());
        buf.push(0);
        let op_count: u64 = programs.iter().map(|p| p.len() as u64).sum();
        buf.extend_from_slice(&(programs.len() as u64).to_le_bytes());
        buf.extend_from_slice(&op_count.to_le_bytes());
        buf.extend_from_slice(&(logs.len() as u64).to_le_bytes());
    }
    Blake3Hasher::hash(&buf)
}

fn render_exit_evidence() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "PR-14 exit evidence (bn-3qzd)");
    let _ = writeln!(
        out,
        "sentence: identical controlled choice logs produce canonical identical semantic events"
    );
    let _ = writeln!(
        out,
        "source: notes/plan/notes/START_HERE_IMPLEMENTATION.md, PR 14 Exit; matrix twin: PR-14-EXIT"
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "corpus digest: {}", corpus_digest());

    let mut total_logs = 0usize;
    let mut corpus_distinct: BTreeSet<Digest256> = BTreeSet::new();
    let mut families_seen: BTreeSet<Family> = BTreeSet::new();
    let alphabet = all_families_alphabet();

    for (name, programs, logs) in corpus() {
        let mut entry_distinct: BTreeSet<Digest256> = BTreeSet::new();
        let mut conforming = 0usize;
        let mut accepted = 0usize;
        for log in &logs {
            let journal =
                run(&programs, log, &config(SEED)).expect("every corpus log is a legal run");
            entry_distinct.insert(journal.digest().unwrap());
            for event in journal.events() {
                families_seen.insert(event.family());
            }
            if matches!(lift(&journal), LiftVerdict::Conforms(_)) {
                conforming += 1;
            }
            let bytes = journal.encode().unwrap();
            if judge(&alphabet, &bytes).is_accepted() {
                accepted += 1;
            }
        }
        let _ = writeln!(
            out,
            "scenario {name}: {} logs, {} distinct journal digests, {} conforming, {} A7-accepted",
            logs.len(),
            entry_distinct.len(),
            conforming,
            accepted
        );
        total_logs += logs.len();
        corpus_distinct.extend(entry_distinct);
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "total logs: {total_logs}");
    let _ = writeln!(
        out,
        "total distinct journal digests: {}",
        corpus_distinct.len()
    );
    let _ = writeln!(
        out,
        "families observed across the corpus: {} of 6",
        families_seen.len()
    );
    let _ = writeln!(
        out,
        "config observes all 6 families on every run (lifecycle default plus effect, \
         cancellation, obligation, time, channel)"
    );
    out
}

#[test]
fn the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden() {
    const EXIT_GOLDEN: &str = include_str!("golden/pr14_exit_evidence.txt");
    let actual = render_exit_evidence();
    if std::env::var_os("PR14_EXIT_BLESS").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr14_exit_evidence.txt"
        );
        std::fs::write(path, &actual).expect("the golden is writable");
        panic!(
            "PR14_EXIT_BLESS rewrote {path}; rerun without the variable and review the diff \
             as a contract change"
        );
    }
    assert_eq!(
        actual, EXIT_GOLDEN,
        "the exit evidence artifact drifted; to regenerate deliberately, run with \
         PR14_EXIT_BLESS=1 and review the diff as a contract change.\nactual:\n{actual}"
    );
    // Rendered twice, identical bytes: no ambient anything reaches it (INV-005).
    assert_eq!(actual, render_exit_evidence());
}
