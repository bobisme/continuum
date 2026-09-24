//! PR-16/IMPL-03: the replicated register as a Rust program on asupersync 0.5.0, run
//! under Continuum's lab binding, and held step by step to the operational durable
//! register (bn-2rsp, IMPL-02) and through it to the abstract register (bn-3e3v,
//! IMPL-01). START_HERE PR 16, third bullet ("asupersync implementation"); bn-131yp.
//!
//! # Where the program lives, and why
//!
//! Here, as tests of `continuum-asupersync`, with the program, its projection and the
//! oracle in `tests/support/replicated_register.rs`. Plan §20 has no crate for example
//! programs: `continuum-corpus` is the TLA+ corpus and Tribunal inputs (plan §9.5) and
//! depends on nothing, so the program there would need two new dependency edges. This
//! crate's tests already have the binding, the lift, and the independent A7 model
//! (`tests/support/primitive_conformance_model.rs`), and they add no edge at all.
//!
//! The IMPL-02 oracle (`correspondence` and `durability_violations` in
//! `crates/continuum-cml-elab/tests/pr16_impl02_durable_register.rs`) runs the reference
//! engine over the lowered model. Calling it from here would add dev-dependency edges
//! to `continuum-cml-elab`, `continuum-model-core` and `continuum-engine-reference`. So
//! the oracle is a port with a drift check instead: the slot protocol `Spec`, which
//! that file's differential shows is exactly the engine's reachable states and
//! labelled transitions, is copied verbatim and compared item by item
//! ([`the_port_is_the_impl02_slot_protocol_verbatim`]), and the port recomputes that
//! file's golden step counts at both of its scopes
//! ([`the_port_reproduces_the_impl02_golden_at_both_scopes`]). The model texts and the
//! IMPL-02 oracle functions the port follows are pinned by digest in the golden.
//!
//! # The mapping
//!
//! The program (`support/replicated_register.rs`, part 1) runs three replicas, each an
//! incarnation region with one writer task per epoch, and one coordinator task per
//! `(epoch, value)` with a bounded channel. Its storage phases are asupersync
//! obligations, and the projection (part 2) maps them to the durable register's slot
//! machine:
//!
//! | runtime event (journal) | durable-register step |
//! |---|---|
//! | writer's `Transaction` reserved | `Reserve(n, e)`: the write permit |
//! | writer's `IoOp` opened | `Submit(n, e, v)`: bytes in the volatile log |
//! | that `IoOp` committed | `Sync(n, e)`: the bytes are durable |
//! | permit aborted `explicit`, or committed, before any bytes | `Abort(n, e)` |
//! | replica region cancelled: `IoOp` aborted / permit aborted `cancel` before any bytes | `Lose(n, e, v)`: a crash, as graceful region cancellation, loses the in-flight write |
//! | coordinator's `Transaction` committed after a majority of confirmations | `Ack(e, v)`: the quorum ack |
//! | every other event: spawns, polls, the permit's release, confirmations on the channel, receives, the ack's reserve, cancellation phases, region drains | stutter |
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr16_impl03_replicated_register.evidence.txt`.
//! Regenerate with `PR16_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test
//! pr16_impl03_replicated_register`, and review the diff.
//!
//! - **positive** `pr16-impl03-pos-01-two-writers` (every admissible log, 6,732),
//!   `pos-02-quorum-split`, `pos-03-crash-restart`, `pos-04-abort-retry`,
//!   `pos-05-two-epochs` (claim A's nodes, values and epochs; seeded samples of 400
//!   admissible logs each), and `pos-06-port` (the port against IMPL-02). For each
//!   plan: every journal lifts as `Conforms`, the A7 model accepts it, every observed
//!   step is a durable-register step or a stutter and every `Ack` an enabled `Choose`,
//!   no step violates durability, and every projected state is reachable;
//! - **negative** `neg-01…03`: journal edits the oracle must refute. They test the
//!   oracle, not the program: the program mutants are bn-28oa's;
//! - **boundary** `bnd-01…04`: a log that commands a parked coordinator is a typed
//!   refusal and no journal; a journal with another plan's roles is a typed projection
//!   refusal and no steps; a value without a quorum is never acknowledged; a sampled
//!   plan is recorded as a sample, not as complete.
//!
//! # Scope, stated so no one takes more from it
//!
//! The refinement holds over the explored runs: every admissible log of `pos-01`, and
//! seeded samples of the other plans. The plan, not the log, fixes which value each
//! replica writes and where in its own script it crashes; the log fixes every
//! interleaving across replicas and coordinators. This is a test-level check, not the
//! PR-17 refinement checker, and it is not the correct-version exit (bn-5fpl) or the
//! mutants (bn-28oa).
//!
//! A crash is modelled as graceful region cancellation (`register::CRASH_SEMANTICS`,
//! bn-20d8u): the crashed incarnation's writers run their cancellation cleanup, which
//! aborts their permits and unsynced bytes, and the projection reads those aborts as
//! `Lose`. The process pack's profile `process/crash-restart-v0` calls that graceful
//! cancellation. Its fail-stop crash runs no handler and leaves pending operations
//! pending, fenced by `(node, epoch)`. So `pos-03-crash-restart` and `pos-05-two-epochs`
//! show refinement for crashes as graceful region cancellation, and not for a fail-stop
//! crash, which the binding cannot express. `neg-03-crash-keeps-bytes` shows that the
//! projection needs the cleanup's abort to read a crash as `Lose`, so a fail-stop
//! journal would need a different map.
//! [`a_crash_is_a_graceful_region_cancellation_and_not_a_fail_stop_crash`] holds the
//! program to that reading.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use continuum_asupersync::binding::{BindingConfig, BindingRefusal, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::effect::EffectEvent;
use continuum_asupersync::family::{EventBody, Family};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_value::identity::{Blake3Hasher, ContentHasher};

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

use model::{Alphabet, FamilyTag, judge};
use register::{
    Act, Built, Correspondence, Expect, Mismatch, Plan, ProjectionRefusal, Role, build, check,
    observe, reachable, sample_logs, write,
};

const SEED: u64 = 0;
const SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];
/// The explicit seed of the sampled schedules.
const SAMPLE_SEED: u64 = 0x1603;
/// Logs per sampled plan.
const SAMPLE: usize = 400;
/// The admissible logs of `pos-01`, all of them.
const TWO_WRITERS_LOGS: usize = 6_732;
/// A cap past which a plan's logs are sampled, not enumerated.
const ENUMERATION_CAP: usize = 50_000;

/// Every run observes all six families.
fn config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

fn alphabet() -> Alphabet {
    Alphabet::new([
        FamilyTag::Lifecycle,
        FamilyTag::Effect,
        FamilyTag::Cancellation,
        FamilyTag::Obligation,
        FamilyTag::Time,
        FamilyTag::Channel,
    ])
}

// ---------------------------------------------------------------------------
// the correct plans
// ---------------------------------------------------------------------------

fn then(mut first: Vec<Act>, rest: Vec<Act>) -> Vec<Act> {
    first.extend(rest);
    first
}

/// `a` and `b` write `v0`; `c` is idle. Small enough to enumerate every log.
fn two_writers() -> Plan {
    Plan {
        name: "two-writers",
        epochs: 1,
        values: [[0, 0], [0, 0], [1, 0]],
        replicas: [write(0), write(0), vec![]],
    }
}

/// `a` and `b` write `v0`, `c` writes `v1`: `v0` has a quorum, `v1` never has one.
fn quorum_split() -> Plan {
    Plan {
        name: "quorum-split",
        epochs: 1,
        values: [[0, 0], [0, 0], [1, 0]],
        replicas: [write(0), write(0), write(0)],
    }
}

/// Every replica writes `v0` and crashes once: `a` with bytes in the volatile log,
/// `b` with only a permit, `c` after `sync` and before its confirmation. Each restarts
/// and finishes: `a` and `b` write again, `c` confirms its durable record.
fn crash_restart() -> Plan {
    Plan {
        name: "crash-restart",
        epochs: 1,
        values: [[0, 0], [0, 0], [0, 0]],
        replicas: [
            then(vec![Act::Reserve(0), Act::Submit(0), Act::Crash], write(0)),
            then(vec![Act::Reserve(0), Act::Crash], write(0)),
            vec![
                Act::Reserve(0),
                Act::Submit(0),
                Act::Sync(0),
                Act::Crash,
                Act::Confirm(0),
            ],
        ],
    }
}

/// `a`'s first writer is cancelled before it submits and releases its permit, then
/// writes; `a` and `b` write `v1`, `c` writes `v0`.
fn abort_retry() -> Plan {
    Plan {
        name: "abort-retry",
        epochs: 1,
        values: [[1, 0], [1, 0], [0, 0]],
        replicas: [
            then(vec![Act::Reserve(0), Act::Abort(0)], write(0)),
            write(0),
            write(0),
        ],
    }
}

/// Claim A's scope: three nodes, two values, two epochs. Epoch 0 splits `v0 v0 v1`,
/// epoch 1 is unanimous `v1`; `b` writes its epochs in the other order, and `c` crashes
/// mid-write in epoch 1 after its epoch-0 record is durable.
fn two_epochs() -> Plan {
    Plan {
        name: "two-epochs",
        epochs: 2,
        values: [[0, 1], [0, 1], [1, 1]],
        replicas: [
            then(write(0), write(1)),
            then(write(1), write(0)),
            then(
                then(write(0), vec![Act::Reserve(1), Act::Submit(1), Act::Crash]),
                write(1),
            ),
        ],
    }
}

// ---------------------------------------------------------------------------
// the corpus, run once
// ---------------------------------------------------------------------------

/// How a plan's logs were chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Every admissible log.
    Exhaustive,
    /// A seeded sample; the plan has more admissible logs than the cap.
    Sampled { seed: u64 },
}

struct Entry {
    id: &'static str,
    plan: Plan,
    built: Built,
    scope: Scope,
    logs: Vec<ChoiceLog>,
}

fn entry(id: &'static str, plan: Plan, sampled: bool) -> Entry {
    let built = build(&plan);
    let (scope, logs) = if sampled {
        (
            Scope::Sampled { seed: SAMPLE_SEED },
            sample_logs(&built, SAMPLE, SAMPLE_SEED),
        )
    } else {
        (
            Scope::Exhaustive,
            register::admissible_logs(&built, ENUMERATION_CAP).expect("enumerable"),
        )
    };
    Entry {
        id,
        plan,
        built,
        scope,
        logs,
    }
}

fn corpus() -> &'static [Entry] {
    static CELL: OnceLock<Vec<Entry>> = OnceLock::new();
    CELL.get_or_init(|| {
        vec![
            entry("pr16-impl03-pos-01-two-writers", two_writers(), false),
            entry("pr16-impl03-pos-02-quorum-split", quorum_split(), true),
            entry("pr16-impl03-pos-03-crash-restart", crash_restart(), true),
            entry("pr16-impl03-pos-04-abort-retry", abort_retry(), true),
            entry("pr16-impl03-pos-05-two-epochs", two_epochs(), true),
        ]
    })
}

/// One plan's results over all of its logs.
struct Outcome {
    journals: Vec<Journal>,
    conforms: usize,
    accepted: usize,
    distinct: usize,
    refusals: usize,
    correspondence: Correspondence,
}

fn outcome(e: &Entry) -> Outcome {
    let reach = reachable(e.plan.epochs);
    let alphabet = alphabet();
    let mut journals = Vec::new();
    let mut digests = BTreeSet::new();
    let (mut conforms, mut accepted, mut refusals) = (0, 0, 0);
    let mut correspondence = Correspondence::default();
    for log in &e.logs {
        let journal = run(&e.built.programs, log, &config(SEED))
            .unwrap_or_else(|r| panic!("{} log {log}: an admissible log is a run: {r}", e.id));
        if matches!(lift(&journal), LiftVerdict::Conforms(_)) {
            conforms += 1;
        }
        if judge(&alphabet, &journal.encode().expect("encodes")).is_accepted() {
            accepted += 1;
        }
        digests.insert(journal.digest().expect("digests"));
        match observe(&e.built.roles, &journal) {
            Ok(steps) => correspondence.absorb(check(&steps, e.plan.epochs, &reach)),
            Err(_) => refusals += 1,
        }
        journals.push(journal);
    }
    Outcome {
        journals,
        conforms,
        accepted,
        distinct: digests.len(),
        refusals,
        correspondence,
    }
}

fn outcomes() -> &'static [Outcome] {
    static CELL: OnceLock<Vec<Outcome>> = OnceLock::new();
    CELL.get_or_init(|| corpus().iter().map(outcome).collect())
}

fn by_name(name: &str) -> (&'static Entry, &'static Outcome) {
    let i = corpus()
        .iter()
        .position(|e| e.plan.name == name)
        .expect("a plan of the corpus");
    (&corpus()[i], &outcomes()[i])
}

// ---------------------------------------------------------------------------
// (2) the program runs under controlled choice logs, and conforms
// ---------------------------------------------------------------------------

#[test]
fn the_two_writer_plan_is_enumerated_completely() {
    let (e, _) = by_name("two-writers");
    assert_eq!(e.scope, Scope::Exhaustive);
    assert_eq!(e.logs.len(), TWO_WRITERS_LOGS);
    let distinct: BTreeSet<&ChoiceLog> = e.logs.iter().collect();
    assert_eq!(distinct.len(), e.logs.len(), "no log twice");
}

#[test]
fn every_journal_lifts_as_conforms_and_the_a7_model_accepts_it() {
    for (e, o) in corpus().iter().zip(outcomes()) {
        assert_eq!(o.journals.len(), e.logs.len());
        assert_eq!(o.conforms, e.logs.len(), "{}: every lift conforms", e.id);
        assert_eq!(o.accepted, e.logs.len(), "{}: A7 accepts every one", e.id);
    }
}

/// The program is observed in all six families' vocabulary it uses: lifecycle,
/// reserve/commit/abort, cancellation (the crash plans), obligations, and channels.
#[test]
fn the_program_exercises_regions_tasks_effects_obligations_cancellation_and_channels() {
    let mut families = BTreeSet::new();
    for o in outcomes() {
        for j in &o.journals {
            families.extend(j.events().iter().map(|ev| ev.family()));
        }
    }
    for f in [
        Family::Lifecycle,
        Family::Effect,
        Family::Cancellation,
        Family::Obligation,
        Family::Channel,
    ] {
        assert!(families.contains(&f), "{f:?} observed");
    }
    assert!(
        !families.contains(&Family::Time),
        "the program uses no timer"
    );
}

/// Whether `op` stops a task without its cancellation cleanup: what a fail-stop crash of
/// `process/crash-restart-v0` needs. The match has no wildcard, so an operation added to
/// the binding must be classed here before this file compiles again (bn-20d8u).
const fn stops_a_task_without_cleanup(op: &SubstrateOp) -> bool {
    match op {
        // `Cancel` runs each task's cancellation cleanup; `Finish` returns through the
        // task's own body; `Close` waits for the region's tasks.
        SubstrateOp::OpenRegion { .. }
        | SubstrateOp::Spawn { .. }
        | SubstrateOp::SpawnWithDeadline { .. }
        | SubstrateOp::Begin { .. }
        | SubstrateOp::Continue { .. }
        | SubstrateOp::Finish { .. }
        | SubstrateOp::Close { .. }
        | SubstrateOp::Cancel { .. }
        | SubstrateOp::Reserve { .. }
        | SubstrateOp::Commit { .. }
        | SubstrateOp::Abort { .. }
        | SubstrateOp::Acquire { .. }
        | SubstrateOp::Sleep { .. }
        | SubstrateOp::OpenChannel { .. }
        | SubstrateOp::Send { .. }
        | SubstrateOp::Recv { .. }
        | SubstrateOp::CloseSenders { .. }
        | SubstrateOp::Advance { .. }
        | SubstrateOp::Transfer { .. } => false,
        // bn-20d8u path (a): the binding's fail-stop crash.
        SubstrateOp::Crash { .. } => true,
    }
}

/// A crash is graceful region cancellation, not a fail-stop crash (bn-20d8u,
/// `register::CRASH_SEMANTICS`). Each crash act is one `Cancel` of the crashing
/// incarnation's region, never of the root or the coordinators' region; in every crash
/// plan's journals the crashed writers acknowledge the cancellation, which is their
/// cleanup starting. That the
/// binding has no operation that stops a task without its cleanup is a compile-time
/// check: `stops_a_task_without_cleanup` matches every `SubstrateOp` with no wildcard.
#[test]
fn a_crash_is_a_graceful_region_cancellation_and_not_a_fail_stop_crash() {
    use continuum_asupersync::family::cancellation::CancellationEvent;
    use continuum_asupersync::family::lifecycle::LifecycleEvent;
    for (e, o) in corpus().iter().zip(outcomes()) {
        let setup = &e.built.programs[0];
        // The setup opens every incarnation's region, then the coordinators' region.
        let mut replica_regions: Vec<_> = setup
            .iter()
            .filter_map(|op| match op {
                SubstrateOp::OpenRegion { child, .. } => Some(*child),
                _ => None,
            })
            .collect();
        replica_regions.pop();
        let mut crashes = 0;
        for n in 0..3 {
            let script = &e.plan.replicas[n];
            let ops = &e.built.programs[1 + n];
            assert_eq!(script.len(), ops.len(), "one operation per act");
            for (act, op) in script.iter().zip(ops) {
                if matches!(act, Act::Crash | Act::CrashRepropose(..)) {
                    crashes += 1;
                    let SubstrateOp::Cancel { region } = op else {
                        panic!("{}: a crash is {op:?}", e.id)
                    };
                    assert!(replica_regions.contains(region), "{}", e.id);
                }
            }
        }
        for j in &o.journals {
            let cancels = j
                .events()
                .iter()
                .filter(|ev| {
                    matches!(
                        ev.body(),
                        EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { .. })
                    )
                })
                .count();
            let acks = j
                .events()
                .iter()
                .filter(|ev| {
                    matches!(
                        ev.body(),
                        EventBody::Cancellation(CancellationEvent::Acknowledged { .. })
                    )
                })
                .count();
            assert_eq!(cancels, crashes, "{}", e.id);
            // Each crash cancels a region of `epochs` writers, and each acknowledges.
            assert_eq!(acks, crashes * usize::from(e.plan.epochs), "{}", e.id);
        }
    }
    // The exhaustive match is compiled and holds: only the binding's `Crash` stops a
    // task without its cleanup, and this graceful build never uses it.
    assert!(stops_a_task_without_cleanup(&SubstrateOp::Crash {
        region: continuum_asupersync::family::lifecycle::RegionLabel(1),
    }));
    for e in corpus() {
        for program in &e.built.programs {
            assert!(
                !program.iter().any(stops_a_task_without_cleanup),
                "{}",
                e.id
            );
        }
    }
    assert!(
        register::CRASH_SEMANTICS
            .starts_with("a crash is modelled as graceful region cancellation")
    );
}

// ---------------------------------------------------------------------------
// (3) the projection refines IMPL-02, and through it the abstract register
// ---------------------------------------------------------------------------

#[test]
fn every_observed_step_is_a_durable_register_step_or_a_stutter() {
    for (e, o) in corpus().iter().zip(outcomes()) {
        let c = &o.correspondence;
        assert_eq!(o.refusals, 0, "{}: every journal projects", e.id);
        assert_eq!(c.failures, [], "{}", e.id);
        assert!(c.steps > c.stutters && c.stutters > 0, "{}", e.id);
        // A named step that stutters is a repeated `Ack`; every other named step moves.
        let named: usize = c.durable.values().sum();
        assert!(named >= c.steps - c.stutters && named > 0, "{}", e.id);
    }
}

/// Anti-vacuity: across the corpus, every kind of durable-register step is realized
/// by the program, including both kinds of loss, and every plan realizes the steps
/// its scripts say it takes.
#[test]
fn every_kind_of_durable_register_step_is_realized() {
    let mut all = BTreeMap::new();
    for o in outcomes() {
        for (k, v) in &o.correspondence.kinds {
            *all.entry(k.as_str()).or_insert(0) += v;
        }
    }
    let kinds: BTreeSet<&str> = all.keys().copied().collect();
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
    let kinds_of = |name: &str| -> BTreeSet<String> {
        by_name(name)
            .1
            .correspondence
            .kinds
            .keys()
            .cloned()
            .collect()
    };
    let base = ["Ack", "Reserve", "Submit", "Sync"].map(str::to_owned);
    assert_eq!(kinds_of("two-writers"), BTreeSet::from(base.clone()));
    assert_eq!(kinds_of("quorum-split"), BTreeSet::from(base.clone()));
    let mut crash: BTreeSet<String> = BTreeSet::from(base.clone());
    crash.extend(["Lose/reserved".to_owned(), "Lose/volatile".to_owned()]);
    assert_eq!(kinds_of("crash-restart"), crash);
    let mut abort: BTreeSet<String> = BTreeSet::from(base.clone());
    abort.insert("Abort".to_owned());
    assert_eq!(kinds_of("abort-retry"), abort);
    let mut two: BTreeSet<String> = BTreeSet::from(base);
    two.insert("Lose/volatile".to_owned());
    assert_eq!(kinds_of("two-epochs"), two);
}

#[test]
fn through_the_durable_register_every_ack_is_an_enabled_choose() {
    let want = |pairs: &[(u8, &str)]| -> BTreeSet<String> {
        pairs
            .iter()
            .map(|(e, v)| format!("Choose(epoch={e},value={v})"))
            .collect()
    };
    for (name, chosen) in [
        ("two-writers", want(&[(0, "v0")])),
        ("quorum-split", want(&[(0, "v0")])),
        ("crash-restart", want(&[(0, "v0")])),
        ("abort-retry", want(&[(0, "v1")])),
        ("two-epochs", want(&[(0, "v0"), (1, "v1")])),
    ] {
        let (e, o) = by_name(name);
        let c = &o.correspondence;
        let got: BTreeSet<String> = c.chooses.keys().cloned().collect();
        assert_eq!(got, chosen, "{name}");
        // Every run acknowledges each quorum value exactly once: one `Choose` per
        // epoch acknowledged, per log.
        let per_log: usize = c.chooses.values().sum();
        assert_eq!(per_log, e.logs.len() * chosen.len(), "{name}");
    }
}

#[test]
fn every_projected_state_is_a_reachable_durable_register_state() {
    for (e, o) in corpus().iter().zip(outcomes()) {
        let reach = reachable(e.plan.epochs);
        assert!(
            o.correspondence.states.iter().all(|s| reach.contains(s)),
            "{}",
            e.id
        );
        assert!(o.correspondence.states.len() > 1, "{}", e.id);
    }
}

// ---------------------------------------------------------------------------
// (4) determinism
// ---------------------------------------------------------------------------

#[test]
fn identical_choice_logs_give_identical_journals() {
    for (e, o) in corpus().iter().zip(outcomes()) {
        for (log, first) in e.logs.iter().zip(&o.journals) {
            let again = run(&e.built.programs, log, &config(SEED)).expect("runs again");
            assert_eq!(
                again.encode().expect("encodes"),
                first.encode().expect("encodes"),
                "{} log {log}",
                e.id
            );
        }
        assert!(
            o.distinct > 1,
            "{}: the logs reach different journals",
            e.id
        );
    }
}

#[test]
fn the_lab_seed_does_not_reach_the_journal() {
    for (e, o) in corpus().iter().zip(outcomes()) {
        for i in [0, e.logs.len() / 2, e.logs.len() - 1] {
            let want = o.journals[i].digest().expect("digests");
            for seed in SEEDS {
                let got = run(&e.built.programs, &e.logs[i], &config(seed))
                    .expect("runs")
                    .digest()
                    .expect("digests");
                assert_eq!(got, want, "{} log {} seed {seed:#x}", e.id, e.logs[i]);
            }
        }
    }
}

#[test]
fn the_sampled_logs_are_a_function_of_the_seed_alone() {
    for e in corpus() {
        if let Scope::Sampled { seed } = e.scope {
            assert_eq!(sample_logs(&e.built, SAMPLE, seed), e.logs, "{}", e.id);
            assert_ne!(
                sample_logs(&e.built, SAMPLE, seed ^ 1),
                e.logs,
                "{}: the seed is load-bearing",
                e.id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// the port and its drift check
// ---------------------------------------------------------------------------

fn repo(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

const IMPL02: &str = "crates/continuum-cml-elab/tests/pr16_impl02_durable_register.rs";
const PORT: &str = "crates/continuum-asupersync/tests/support/replicated_register.rs";

/// The text of the top-level item whose first line is `header`, with its doc comments
/// and attributes, through its closing brace at column 0. Visibility is dropped, since
/// the port makes `Raw` public to its module's users.
fn item(src: &str, header: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.replace("pub ", "") == header)
        .unwrap_or_else(|| panic!("{header} is in the source"));
    let mut start = at;
    while start > 0 && (lines[start - 1].starts_with("///") || lines[start - 1].starts_with("#[")) {
        start -= 1;
    }
    let end = if header.ends_with(';') {
        at
    } else {
        at + lines[at..]
            .iter()
            .position(|l| *l == "}")
            .expect("a closing brace")
    };
    lines[start..=end]
        .iter()
        .map(|l| l.strip_prefix("pub ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

const PORTED: [&str; 9] = [
    "const NODES: [&str; 3] = [\"a\", \"b\", \"c\"];",
    "const MAJORITY: &[&[&str]] = &[&[\"a\", \"b\"], &[\"a\", \"c\"], &[\"b\", \"c\"]];",
    "struct Raw {",
    "impl Raw {",
    "enum Slot {",
    "struct Spec {",
    "struct Protocol {",
    "impl Protocol {",
    "impl Spec {",
];

#[test]
fn the_port_is_the_impl02_slot_protocol_verbatim() {
    let (theirs, ours) = (repo(IMPL02), repo(PORT));
    for header in PORTED {
        assert_eq!(
            item(&ours, header),
            item(&theirs, header),
            "{header}: the port drifted from {IMPL02}; re-copy it"
        );
    }
    // The whole port section, so nothing can be added between the items: the two
    // constants, then the seven items, each separated by one blank line.
    let marker =
        "// verbatim port of crates/continuum-cml-elab/tests/pr16_impl02_durable_register.rs";
    let section = ours.split(marker).nth(1).expect("the port section");
    let body = section
        .split_once("\n\n")
        .map(|(_, rest)| rest)
        .expect("a blank line after the section header");
    let got: Vec<&str> = body
        .lines()
        .map(|l| l.strip_prefix("pub ").unwrap_or(l))
        .collect();
    let want = format!(
        "{}\n{}\n\n{}",
        item(&theirs, PORTED[0]),
        item(&theirs, PORTED[1]),
        PORTED[2..]
            .iter()
            .map(|h| item(&theirs, h))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    assert_eq!(got.join("\n").trim_end(), want, "the port section drifted");
}

/// The IMPL-02 golden's numbers for one of its scenarios: the scope line, the
/// correspondence line, and the durability line.
fn impl02_golden(id: &str) -> (String, String, String) {
    let golden =
        repo("crates/continuum-cml-elab/tests/golden/pr16_impl02_durable_register.evidence.txt");
    let section = golden
        .split(&format!("[{id}]\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("{id} in the IMPL-02 golden"));
    let line = |prefix: &str| {
        section
            .lines()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix} in {id}"))
            .to_owned()
    };
    (
        line("scope: "),
        line("correspondence: "),
        line("durability step violations: "),
    )
}

fn port_lines(epochs: u8) -> (String, String, String) {
    let (steps, stutters, chooses, realized, covered, failures, durability) =
        register::port_correspondence(epochs);
    (
        format!("scope: Complete {{ states: {} }}", reachable(epochs).len()),
        format!(
            "correspondence: steps={steps} stutters={stutters} chooses={chooses} \
             realized={realized} covered={covered} failures={failures}"
        ),
        format!("durability step violations: {durability}"),
    )
}

/// The engine's numbers, from IMPL-02's golden, are the port's: reachable states,
/// every step at every reachable state held to the abstract register, and the
/// durability predicate, at the claim-A scope and at one epoch.
#[test]
fn the_port_reproduces_the_impl02_golden_at_both_scopes() {
    assert_eq!(port_lines(2), impl02_golden("pr16-impl02-pos-01-claim-a"));
    assert_eq!(port_lines(1), impl02_golden("pr16-impl02-pos-02-one-epoch"));
}

/// The sources the port follows, by digest: a change to any of them changes the
/// golden and asks for the port to be reviewed.
fn pinned_sources() -> Vec<(String, String)> {
    let digest = |text: &str| Blake3Hasher::hash(text.as_bytes()).to_string();
    let impl02 = repo(IMPL02);
    let mut out = Vec::new();
    for rel in [
        "notes/plan/examples/durable_register.ctm",
        "notes/plan/examples/abstract_register.ctm",
    ] {
        out.push((rel.to_owned(), digest(&repo(rel))));
    }
    for f in [
        "fn project(abs: &Model, raw: &Raw, values: &[String]) -> Option<State> {",
        "fn correspondence(model: &Model, exploration: &Exploration) -> Correspondence {",
        "fn durability_violations(model: &Model, exploration: &Exploration) -> Vec<Labelled> {",
    ] {
        let name = f
            .split('(')
            .next()
            .unwrap_or_default()
            .trim_start_matches("fn ");
        out.push((format!("{IMPL02}::{name}"), digest(&item(&impl02, f))));
    }
    out
}

// ---------------------------------------------------------------------------
// the oracle's redundant layers fire
// ---------------------------------------------------------------------------

/// On journals of this program only the step check can fail; the durability, the
/// abstract `Choose`, the reachability and the stutter checks follow from it. So each
/// is fired here on a crafted step, to show it is not dead code.
#[test]
fn every_layer_of_the_oracle_fires_on_a_crafted_step() {
    use register::{Observed, abstract_step, durability_violated, raw_of};
    // Durability: IMPL-02's release-without-sync, bytes promoted by an `Abort`.
    let volatile = raw_of(&[(0, 0, 0)], &[(0, 0)], &[]);
    let durable = raw_of(&[(0, 0, 0)], &[], &[]);
    assert!(durability_violated(
        "Abort(n=a,epoch=0)",
        &volatile,
        &durable
    ));
    assert!(!durability_violated(
        "Sync(n=a,epoch=0)",
        &volatile,
        &durable
    ));
    // Durability: an acknowledgement withdrawn.
    let acked = raw_of(&[], &[], &[(0, 0)]);
    let empty = raw_of(&[], &[], &[]);
    assert!(durability_violated("Reserve(n=a,epoch=0)", &acked, &empty));
    // Choose: a second value for an acknowledged epoch, and a non-Ack that moves.
    let both = raw_of(&[], &[], &[(0, 0), (0, 1)]);
    assert_eq!(
        abstract_step("Ack(epoch=0,value=v1)", &acked, &both),
        Err(Mismatch::NotAnEnabledChoose)
    );
    assert_eq!(
        abstract_step("Reserve(n=a,epoch=0)", &empty, &acked),
        Err(Mismatch::NotAnEnabledChoose)
    );
    assert_eq!(
        abstract_step("Ack(epoch=0,value=v0)", &empty, &acked),
        Ok(Some("Choose(epoch=0,value=v0)".to_owned()))
    );
    // NotAStutter and Unreachable, through `check`.
    let step = |expect: Expect, pre: &register::Raw, post: &register::Raw| Observed {
        seq: 0,
        event: "crafted".to_owned(),
        expect,
        pre: Some(pre.clone()),
        post: Some(post.clone()),
    };
    let reach = reachable(1);
    let c = check(&[step(Expect::Stutter, &empty, &volatile)], 1, &reach);
    assert_eq!(c.failures[0].0, Mismatch::NotAStutter);
    let c = check(&[step(Expect::Stutter, &acked, &acked)], 1, &reach);
    assert_eq!(
        c.failures[0].0,
        Mismatch::Unreachable,
        "an ack with no quorum"
    );
}

// ---------------------------------------------------------------------------
// negative: journal edits the oracle must refute
// ---------------------------------------------------------------------------

fn rebuild(journal: &Journal, keep: impl Fn(usize, &EventBody) -> bool) -> Journal {
    let mut out = Journal::new();
    for (i, ev) in journal.events().iter().enumerate() {
        if keep(i, ev.body()) {
            out.append(ev.body().clone()).expect("appends");
        }
    }
    out
}

/// The journal with the events at `from` (ascending) moved, in order, to just before
/// the event at `to`, which precedes them all.
fn moved(journal: &Journal, from: &[usize], to: usize) -> Journal {
    let mut bodies: Vec<EventBody> = journal.events().iter().map(|e| e.body().clone()).collect();
    let taken: Vec<EventBody> = from.iter().rev().map(|i| bodies.remove(*i)).collect();
    for body in taken {
        bodies.insert(to, body);
    }
    let mut out = Journal::new();
    for b in bodies {
        out.append(b).expect("appends");
    }
    out
}

/// The indices of the coordinator's ack reserve and commit: the last effect reserve
/// and commit of the journal.
fn ack_indices(journal: &Journal) -> [usize; 2] {
    let last = |f: fn(&EventBody) -> bool| {
        journal
            .events()
            .iter()
            .rposition(|e| f(e.body()))
            .expect("an ack")
    };
    [
        last(|b| matches!(b, EventBody::Effect(EffectEvent::Reserved { .. }))),
        last(|b| matches!(b, EventBody::Effect(EffectEvent::Committed { .. }))),
    ]
}

/// The first failure the oracle reports on a journal of `plan`.
fn first_failure(plan: &Plan, journal: &Journal) -> Option<(Mismatch, String)> {
    let built = build(plan);
    let steps = observe(&built.roles, journal).expect("projects");
    let c = check(&steps, plan.epochs, &reachable(plan.epochs));
    c.failures.first().map(|(m, _, ev)| (*m, ev.clone()))
}

/// The journal of the first log of `two-writers`.
fn two_writers_journal() -> (&'static Plan, &'static Journal) {
    let (e, o) = by_name("two-writers");
    (&e.plan, &o.journals[0])
}

/// The `IoOp` commits the projection reads as `Sync`, by journal index.
fn syncs(plan: &Plan, journal: &Journal) -> Vec<usize> {
    let built = build(plan);
    let steps = observe(&built.roles, journal).expect("projects");
    steps
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(&s.expect, Expect::Step(l) if l.starts_with("Sync(")))
        .map(|(i, _)| i)
        .collect()
}

/// neg-01: the journal without one of the quorum's `sync`s. The ack then has no
/// durable quorum under it.
fn neg_01() -> Option<(Mismatch, String)> {
    let (plan, journal) = two_writers_journal();
    let drop = syncs(plan, journal)[0];
    first_failure(plan, &rebuild(journal, |i, _| i != drop))
}

/// neg-02: the ack's reserve and commit moved to just before the first `sync`.
fn neg_02() -> Option<(Mismatch, String)> {
    let (plan, journal) = two_writers_journal();
    let first = syncs(plan, journal)[0];
    first_failure(plan, &moved(journal, &ack_indices(journal), first))
}

/// neg-03: a crash-restart journal without the crash's loss of `a`'s submitted bytes.
/// The restarted writer's permit then lands on a slot with a write still in flight.
fn neg_03() -> Option<(Mismatch, String)> {
    let (e, o) = by_name("crash-restart");
    let journal = &o.journals[0];
    let steps = observe(&e.built.roles, journal).expect("projects");
    let lost = steps
        .iter()
        .position(|s| matches!(&s.expect, Expect::Step(l) if l.starts_with("Lose(n=a,")))
        .expect("a loses its bytes");
    first_failure(&e.plan, &rebuild(journal, |i, _| i != lost))
}

#[test]
fn neg_01_a_missing_sync_under_the_ack_is_refuted_at_the_ack() {
    let (m, ev) = neg_01().expect("refuted");
    assert_eq!(m, Mismatch::NotAnEnabledStep);
    assert!(ev.contains("reserve-commit-abort committed"), "{ev}");
}

#[test]
fn neg_02_an_ack_before_its_quorum_is_durable_is_refuted_at_the_ack() {
    let (m, ev) = neg_02().expect("refuted");
    assert_eq!(m, Mismatch::NotAnEnabledStep);
    assert!(ev.contains("reserve-commit-abort committed"), "{ev}");
}

#[test]
fn neg_03_a_crash_that_keeps_in_flight_bytes_is_refuted_at_the_restart() {
    let (m, ev) = neg_03().expect("refuted");
    assert_eq!(m, Mismatch::Unprojectable);
    assert!(ev.contains("reserve-commit-abort reserved"), "{ev}");
}

// ---------------------------------------------------------------------------
// boundaries
// ---------------------------------------------------------------------------

/// bnd-01: a log that commands a coordinator parked on an empty channel is the
/// binding's typed refusal, and no journal.
fn bnd_01() -> (ChoiceLog, BindingRefusal) {
    let (e, _) = by_name("two-writers");
    let log = register::an_inadmissible_log(&e.built).expect("one exists");
    let refusal = run(&e.built.programs, &log, &config(SEED)).expect_err("refused");
    (log, refusal)
}

#[test]
fn bnd_01_a_log_that_commands_a_parked_coordinator_is_a_typed_refusal() {
    let (_, refusal) = bnd_01();
    assert!(
        matches!(refusal, BindingRefusal::TaskBlocked { .. }),
        "{refusal}"
    );
    assert_eq!(
        refusal.inconclusive_reason(),
        None,
        "not a run of the program"
    );
}

/// bnd-02: a journal projected with another plan's roles is refused before any step.
fn bnd_02() -> ProjectionRefusal {
    let (_, journal) = two_writers_journal();
    observe(&build(&crash_restart()).roles, journal).expect_err("refused")
}

#[test]
fn bnd_02_a_journal_with_another_plans_roles_is_refused_with_no_steps() {
    assert!(matches!(bnd_02(), ProjectionRefusal::RolesDisagree { .. }));
    // Roles that give `a` the other value: `a`'s confirmation goes to the `v0`
    // coordinator, so the journal contradicts the table.
    let (plan, journal) = two_writers_journal();
    let mut roles = build(plan).roles;
    assert_eq!(
        roles.tasks[0].0,
        Role::Writer {
            node: 0,
            epoch: 0,
            value: 0
        }
    );
    roles.tasks[0].0 = Role::Writer {
        node: 0,
        epoch: 0,
        value: 1,
    };
    assert!(matches!(
        observe(&roles, journal),
        Err(ProjectionRefusal::RolesDisagree { .. })
    ));
}

#[test]
fn bnd_03_a_value_without_a_quorum_is_never_acknowledged() {
    let (_, o) = by_name("quorum-split");
    let c = &o.correspondence;
    assert!(!c.durable.contains_key("Ack(epoch=0,value=v1)"));
    assert!(c.durable["Sync(n=c,epoch=0)"] > 0, "c's v1 is durable");
    assert!(
        c.states
            .iter()
            .all(|s| !register::acks_of(s).contains(&(0, 1)))
    );
}

#[test]
fn bnd_04_the_sampled_plans_are_larger_than_the_enumeration_cap() {
    for e in corpus() {
        let all = register::admissible_logs(&e.built, ENUMERATION_CAP);
        match e.scope {
            Scope::Exhaustive => assert_eq!(all.map(|l| l.len()), Some(e.logs.len())),
            Scope::Sampled { .. } => assert!(all.is_none(), "{}", e.id),
        }
    }
}

// ---------------------------------------------------------------------------
// the evidence
// ---------------------------------------------------------------------------

fn script(acts: &[Act]) -> String {
    let parts: Vec<String> = acts
        .iter()
        .map(|a| match a {
            Act::Reserve(e) => format!("reserve{e}"),
            Act::Submit(e) => format!("submit{e}"),
            Act::Sync(e) => format!("sync{e}"),
            Act::Release(e) => format!("release{e}"),
            Act::Abort(e) => format!("abort{e}"),
            Act::Confirm(e) => format!("confirm{e}"),
            Act::Crash => "crash".to_owned(),
            Act::CrashRepropose(e, v) => {
                format!("crash-repropose{e}={}", register::VALUES[usize::from(*v)])
            }
        })
        .collect();
    if parts.is_empty() {
        "idle".to_owned()
    } else {
        parts.join(" ")
    }
}

fn logs_digest(logs: &[ChoiceLog]) -> String {
    let mut buf = String::new();
    for log in logs {
        let _ = writeln!(buf, "{log}");
    }
    Blake3Hasher::hash(buf.as_bytes()).to_string()
}

fn render_entry(e: &Entry, o: &Outcome) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[{}]", e.id);
    let values: Vec<String> = (0..3)
        .map(|n| {
            let per: Vec<&str> = (0..e.plan.epochs)
                .map(|ep| register::VALUES[usize::from(e.plan.values[n][usize::from(ep)])])
                .collect();
            format!("{}={}", ["a", "b", "c"][n], per.join("/"))
        })
        .collect();
    let _ = writeln!(
        s,
        "plan {}: epochs={} values {}",
        e.plan.name,
        e.plan.epochs,
        values.join(" ")
    );
    for (n, acts) in e.plan.replicas.iter().enumerate() {
        let _ = writeln!(s, "  {}: {}", ["a", "b", "c"][n], script(acts));
    }
    let ops: usize = e.built.programs.iter().map(Vec::len).sum();
    let _ = writeln!(
        s,
        "program: actors={} operations={} tasks={}",
        e.built.programs.len(),
        ops,
        e.built.roles.tasks.len()
    );
    let scope = match e.scope {
        Scope::Exhaustive => format!(
            "exhaustive, every admissible log with the setup first: {}",
            e.logs.len()
        ),
        Scope::Sampled { seed } => format!(
            "sampled, {} admissible logs from seed {seed:#x} (more than {ENUMERATION_CAP} exist)",
            e.logs.len()
        ),
    };
    let _ = writeln!(s, "scope: {scope}; logs digest {}", logs_digest(&e.logs));
    let _ = writeln!(
        s,
        "journals: {} runs, {} distinct, {} conform, {} A7-accepted, {} projection refusals",
        e.logs.len(),
        o.distinct,
        o.conforms,
        o.accepted,
        o.refusals
    );
    let c = &o.correspondence;
    let _ = writeln!(s, "steps: {} observed, {} stutters", c.steps, c.stutters);
    let kinds: Vec<String> = c.kinds.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let _ = writeln!(s, "durable-register steps: {}", kinds.join(" "));
    let chooses: Vec<String> = c.chooses.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let _ = writeln!(s, "abstract chooses: {}", chooses.join(" "));
    let _ = writeln!(
        s,
        "projected states: {} of {} reachable",
        c.states.len(),
        reachable(e.plan.epochs).len()
    );
    let _ = writeln!(s, "failures: {}", c.failures.len());
    s
}

fn render_failure(id: &str, what: &str, got: Option<(Mismatch, String)>) -> String {
    let (m, ev) = got.expect("refuted");
    format!("[{id}]\n{what}\nfirst failure: {m:?} at `{ev}`\n")
}

fn evidence() -> String {
    let mut out = String::from(
        "# PR-16/IMPL-03 replicated register on asupersync 0.5.0 (bn-131yp): the program in\n\
         # crates/continuum-asupersync/tests/support/replicated_register.rs, run through the\n\
         # binding, projected onto notes/plan/examples/durable_register.ctm.\n\
         # Regenerate: PR16_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test pr16_impl03_replicated_register\n",
    );
    let _ = writeln!(out, "# crash: {}\n", register::CRASH_SEMANTICS);
    for (e, o) in corpus().iter().zip(outcomes()) {
        out.push_str(&render_entry(e, o));
        out.push('\n');
    }
    let _ = writeln!(out, "[pr16-impl03-pos-06-port]");
    let _ = writeln!(
        out,
        "slot protocol ported verbatim from {IMPL02}: {} items",
        PORTED.len()
    );
    for (epochs, id) in [(2, "claim A"), (1, "one epoch")] {
        let (scope, corr, dur) = port_lines(epochs);
        let _ = writeln!(out, "{id}: {scope}; {corr}; {dur}");
    }
    for (name, digest) in pinned_sources() {
        let _ = writeln!(out, "pinned {name} {digest}");
    }
    out.push('\n');
    out.push_str(&render_failure(
        "pr16-impl03-neg-01-missing-sync",
        "two-writers log 0 without the first sync",
        neg_01(),
    ));
    out.push('\n');
    out.push_str(&render_failure(
        "pr16-impl03-neg-02-ack-before-quorum",
        "two-writers log 0 with the ack moved before the first sync",
        neg_02(),
    ));
    out.push('\n');
    out.push_str(&render_failure(
        "pr16-impl03-neg-03-crash-keeps-bytes",
        "crash-restart log 0 without a's lost bytes",
        neg_03(),
    ));
    out.push('\n');
    let (log, refusal) = bnd_01();
    let _ = writeln!(
        out,
        "[pr16-impl03-bnd-01-parked-coordinator]\nlog {log}\nrefusal: {refusal}\n"
    );
    let _ = writeln!(
        out,
        "[pr16-impl03-bnd-02-wrong-roles]\ntwo-writers log 0 with crash-restart's roles\nrefusal: {:?}\n",
        bnd_02()
    );
    let (_, o) = by_name("quorum-split");
    let _ = writeln!(
        out,
        "[pr16-impl03-bnd-03-no-quorum-no-ack]\nquorum-split: Sync(n=c,epoch=0)={} Ack(epoch=0,value=v1)={}\n",
        o.correspondence.durable["Sync(n=c,epoch=0)"],
        o.correspondence
            .durable
            .get("Ack(epoch=0,value=v1)")
            .copied()
            .unwrap_or(0)
    );
    let sampled: Vec<&str> = corpus()
        .iter()
        .filter(|e| matches!(e.scope, Scope::Sampled { .. }))
        .map(|e| e.plan.name)
        .collect();
    let _ = writeln!(
        out,
        "[pr16-impl03-bnd-04-sampled-scope]\nsampled, each over {ENUMERATION_CAP} admissible logs: {}",
        sampled.join(" ")
    );
    out
}

#[test]
fn the_evidence_matches_its_golden() {
    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/pr16_impl03_replicated_register.evidence.txt"
    );
    let got = evidence();
    if std::env::var_os("PR16_IMPL03_BLESS").is_some() {
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
        "the evidence drifted at {first}; regenerate with PR16_IMPL03_BLESS=1 and review"
    );
    assert_eq!(got, evidence(), "rendering is deterministic");
    let ids: Vec<&str> = got
        .lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "artifact IDs are unique");
    assert_eq!(ids.len(), 13);
}
