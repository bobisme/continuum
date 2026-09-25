//! PR 18 (bn-3km4z): deletion and causal-closure reduction over the replicated register's
//! failing journals. START_HERE PR 18 ("Implement deletion, causal-closure, …, and replay
//! validation"; exit: "ack-before-sync failure reduces to a compact core and does not
//! delete the actual causal mechanism").
//!
//! # The instantiation
//!
//! `continuum_debugger::reduce` is generic over a trace with a happens-before relation
//! and a replay oracle. Here the trace is a PR-14 semantic journal of the replicated
//! register (PR 16), the relation is `continuum_asupersync::causal::predecessors` (the
//! journal's declared footprint dependence), and the oracle is [`RegisterOracle`]: a
//! candidate configuration is restricted to a sub-journal (`causal::restrict`, ordinals
//! renamed), and that sub-journal is **replayed** through the same checks a campaign run
//! gets — the lift must conform, the A7 model must accept, the projection must accept
//! with the role table carried across the renaming — and the target property is checked
//! on the projected states. A candidate that fails any of the first three is not
//! replayable and is never kept. The oracle assumes nothing about a candidate.
//!
//! Replay here is replay of the event set under the journal's semantics (the lift, the
//! A7 model and the projection), research/26's "replay/partial execution reaches a
//! violating monitor state". It is not a re-run of the substrate: an arbitrary subset of
//! events has no choice log. The original run is replayed through the binding from its
//! plan and log before any reduction.
//!
//! # Deletion under the substrate replay
//!
//! The deletion pass runs over two candidate spaces (`reduce::Deletion`). Over
//! configurations, every candidate keeps the happens-before past of what it keeps, so
//! the core keeps the mechanism: M01's cores keep the confirmations, the submits and the
//! loss. Over arbitrary unions of atoms (classic `ddmin`) the cores are far smaller and
//! still replay to the failure, but this replay is the substrate's semantics, which does
//! not know that a coordinator acknowledges only after a majority confirms, so those
//! cores drop the mechanism. `deletion_over_atoms_loses_the_mechanism_under_a_substrate_replay`
//! pins that finding.
//!
//! # The program-level replay (bn-2z08o)
//!
//! [`ProgramOracle`] judges a candidate by the program, not only the substrate. The
//! construction, chosen because the binding has no data-dependent control flow (the
//! program's decisions are its actors' operation lists, `support/replicated_register.rs`
//! part 1), is re-execution:
//!
//! 1. the run's operations are found by re-execution ([`Operations`]): the program
//!    truncated to the first `k` operations of the run is run through the binding, and
//!    when its journal is a prefix of the run's, `k` is a boundary; the operations between
//!    two boundaries are one group, an atom of the reduction's order;
//! 2. a candidate must hold whole groups, and the operations they hold must be a prefix
//!    of each actor's program (a partial run of the program: each actor stopped
//!    somewhere);
//! 3. the program truncated to those operations is re-executed through the binding in
//!    the run's order, and its journal must be exactly the candidate's sub-journal;
//! 4. that journal gets [`RegisterOracle`]'s checks (the lift, the A7 model, the
//!    projection) and the target property.
//!
//! A binding refusal with an INV-008 reading is an inconclusive, one without is not a
//! run; nothing is guessed. The order the reduction reads adds program order (each
//! operation's events follow its actor's previous operation's) to the footprint
//! dependence. Under this oracle, the closure and then deletion over atoms keep M01's
//! mechanism on both witnesses and on all 400 corpus runs; on the two witnesses the
//! closure already is the core, and deletion removes nothing more. Atom minimality here
//! means that no single operation group can be removed and still leave a failing run of
//! the program. Every substrate-replay core, of either deletion mode, is not a run of
//! the program. The cores are not compact: on these traces most events are the
//! mechanism's own operations, and the evidence states the numbers.
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr18_impl01_reduction.evidence.txt`, and RFC
//! 0028's per-removal transcripts into `tests/golden/pr18_impl01_transcripts.txt`.
//! Regenerate with `PR18_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test
//! pr18_impl01_reduction` and review the diff.
//!
//! - `pr18-impl01-01-m01-agreement-core`: M01's scenario witness (`abstract_register::
//!   Agreement`), event counts before and after each pass, the core and its causal story,
//!   under the substrate replay (both deletion modes) and under the program replay;
//! - `pr18-impl01-02-m01-acked-not-durable-core`: M01's shallowest `RuntimeToAbstract`
//!   witness (`AckedNotDurable`), the same;
//! - `pr18-impl01-03-closure-preservation`: the corpus property over every failing run of
//!   M01's scenario plan, and the program replay's cores over the same corpus;
//! - `pr18-impl01-04-determinism`: identical inputs give byte-identical cores and
//!   transcripts;
//! - `pr18-impl01-05-symmetry-renaming`: the metamorphic relation;
//! - `pr18-impl01-06-m01-agreement-transcript` and
//!   `pr18-impl01-07-m01-acked-not-durable-transcript`: every attempt of the program
//!   replay's reductions, with its candidate, verdict and reason.

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

use continuum_asupersync::binding::run;
use continuum_asupersync::causal;
use continuum_asupersync::family::EventBody;
use continuum_asupersync::family::lifecycle::LifecycleEvent;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_debugger::reduce::{
    self, Budget, CausalOrder, Deletion, Guarantee, NotReplayable, Pass, PassEnd, Reduction,
    Replay, Replayed,
};
use register::{Expect, Role, Roles};

#[path = "support/pr18_program.rs"]
#[allow(dead_code)]
mod program;

use program::*;

fn transcript_lines(r: &Reduction) -> Vec<String> {
    let (transcript, spent) = match r {
        Reduction::Reduced {
            transcript, spent, ..
        }
        | Reduction::Inconclusive {
            transcript, spent, ..
        } => (transcript, spent),
        Reduction::Refused(why) => return vec![format!("refused: {why:?}")],
    };
    let mut out: Vec<String> = transcript
        .iter()
        .map(|p| {
            format!(
                "pass {:?}: {} -> {} events, {} replays ({} held, {} nonconforming, {} inconclusive), {:?}",
                p.pass, p.before, p.after, p.replays, p.held, p.nonconforming, p.inconclusive, p.end
            )
        })
        .collect();
    out.push(format!(
        "spent: {} replays, {} work units",
        spent.replays, spent.work
    ));
    out
}

fn core_section(id: &str, c: &Case, program: &ProgramReduction) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[{id}]");
    let _ = writeln!(s, "run: M01 {}, target {}", c.label, c.target.name());
    let _ = writeln!(
        s,
        "  log {:?}",
        c.log.choices().iter().map(|x| x.0).collect::<Vec<_>>()
    );
    let _ = writeln!(
        s,
        "journal: {} events, digest {}",
        c.journal.len(),
        c.journal.digest().expect("digests")
    );
    let mut substrate_cores = Vec::new();
    for mode in [Deletion::Configurations, Deletion::Atoms] {
        let (r, _) = minimize(c, mode);
        let core = r.core().expect("reduced");
        substrate_cores.push((mode, core.events.clone()));
        let _ = writeln!(s, "reduction with deletion over {mode:?}:");
        render_core(&mut s, c, &r);
    }
    let o = &program.ops;
    let r = &program.reduction;
    let core = r.core().expect("reduced");
    let _ = writeln!(
        s,
        "reduction with deletion over Atoms under the program replay (bn-2z08o):"
    );
    let _ = writeln!(
        s,
        "  operations: {} in the run, in {} groups; truncations the binding refused {}, truncations not a prefix of the journal {}",
        o.ops.len(),
        o.groups.len(),
        o.refused,
        o.diverged
    );
    let st = story(c, &core.events);
    let attempts = r.attempts().expect("a transcript");
    let Reduction::Reduced { transcript, .. } = r else {
        unreachable!("reduced");
    };
    let _ = writeln!(
        s,
        "  the closure under the program's order keeps {} events; deletion over atoms removes {} more; atom-minimal here means no single operation group's removal is still a failing run of the program",
        transcript[0].after,
        transcript[1].before - transcript[1].after
    );
    let _ = writeln!(
        s,
        "  core events by actor (of the run's): {}",
        by_actor(c, o, &core.events)
    );
    let _ = writeln!(
        s,
        "  core operations: {} of {}; mechanism kept: {}; campaign checker on the core's re-execution reports {}: {}",
        ops_of(o, &core.events).len(),
        o.ops.len(),
        keeps_mechanism(&st, c.target),
        c.target.name(),
        campaign_finds_target(c, o, &core.events)
    );
    let _ = writeln!(
        s,
        "  transcript: {} attempts recorded, {} omitted, result version {}; minimality checked from the transcript alone: {:?}",
        attempts.entries.len(),
        attempts.omitted,
        attempts.version,
        transcript_licenses(&program.order, core, attempts)
    );
    let mut oracle = ProgramOracle {
        case: c,
        ops: o,
        replays: 0,
    };
    for (mode, events) in substrate_cores {
        let verdict = match oracle.replay(&events) {
            Replayed::Fails { .. } => "fails".to_owned(),
            Replayed::Holds => "holds".to_owned(),
            Replayed::NotReplayable(NotReplayable::Nonconforming(why)) => {
                format!("not a run: {why}")
            }
            Replayed::NotReplayable(NotReplayable::Inconclusive(r, why)) => {
                format!("inconclusive {r:?}: {why}")
            }
        };
        let _ = writeln!(
            s,
            "  the substrate replay's {mode:?} core ({} events) under the program replay: {verdict}",
            events.len()
        );
    }
    render_core(&mut s, c, r);
    s
}

/// How many of `events`, and of the whole journal, each actor's operations emitted:
/// the setup, replicas `a b c`, the coordinators, and the shutdown.
fn by_actor(c: &Case, o: &Operations, events: &[usize]) -> String {
    let actors = c.built.programs.len();
    let name = |a: usize| match a {
        0 => "setup".to_owned(),
        1..=3 => format!("replica {}", NODES[a - 1]),
        a if a + 1 == actors => "shutdown".to_owned(),
        a => format!("coordinator {}", a - 4),
    };
    let count = |set: &mut dyn Iterator<Item = usize>| {
        let mut n = vec![0_usize; actors];
        for e in set {
            let g = o.groups[o.group_of[e]];
            // A group of several operations is counted under its first operation's actor.
            n[o.ops[g.start].0] += 1;
        }
        n
    };
    let kept = count(&mut events.iter().copied());
    let all = count(&mut (0..c.journal.len()));
    (0..actors)
        .map(|a| format!("{} {} of {}", name(a), kept[a], all[a]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A reduction's pass transcript, core, witnesses and story.
fn render_core(s: &mut String, c: &Case, r: &Reduction) {
    let core = r.core().expect("reduced");
    for line in transcript_lines(r) {
        let _ = writeln!(s, "  {line}");
    }
    let cj = core_journal(c, &core.events);
    let _ = writeln!(
        s,
        "  core: {} of {} events ({:.1}%), guarantees {:?}, digest {}",
        core.events.len(),
        c.journal.len(),
        100.0 * core.events.len() as f64 / c.journal.len() as f64,
        core.guarantees,
        cj.digest().expect("digests")
    );
    let _ = writeln!(s, "  core events (original seq): {:?}", core.events);
    let _ = writeln!(s, "  witnesses (original seq): {:?}", core.witnesses);
    let _ = writeln!(s, "  causal story: {}", story(c, &core.events).join(" "));
    for line in cj.render().lines() {
        let _ = writeln!(s, "    {line}");
    }
}

/// The corpus property's numbers, computed once.
struct CorpusFacts {
    runs: usize,
    by_target: BTreeMap<Target, usize>,
    trace_len: Vec<usize>,
    closure_len: Vec<usize>,
    /// Core lengths: deletion over configurations, then over atoms.
    core_len: [Vec<usize>; 2],
    closure_not_preserving: usize,
    dropped_checked: usize,
    on_path: usize,
    atom_mates: usize,
    /// Cores of deletion over atoms whose story has no `Submit`: the mechanism lost.
    atoms_without_submit: usize,
    replays: u64,
    /// Under the program replay (bn-2z08o): core lengths of deletion over atoms, cores
    /// that keep the mechanism, cores whose atom minimality the transcript licenses,
    /// cores the campaign's checker confirms on their re-execution, substrate-replay
    /// cores (both deletion modes) that are not runs of the program, and replays.
    program_core_len: Vec<usize>,
    program_mechanism: usize,
    program_licensed: usize,
    program_campaign: usize,
    substrate_not_runs: [usize; 2],
    program_replays: u64,
    /// Under the program replay: the closure's length, and runs where deletion removed
    /// events beyond it.
    program_closure_len: Vec<usize>,
    program_deletion_shrank: usize,
    /// Truncations the binding refused, and truncations not a prefix of the journal.
    truncations: [usize; 2],
}

fn corpus_facts() -> &'static CorpusFacts {
    static CELL: OnceLock<CorpusFacts> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut f = CorpusFacts {
            runs: 0,
            by_target: BTreeMap::new(),
            trace_len: Vec::new(),
            closure_len: Vec::new(),
            core_len: [Vec::new(), Vec::new()],
            closure_not_preserving: 0,
            dropped_checked: 0,
            on_path: 0,
            atom_mates: 0,
            atoms_without_submit: 0,
            replays: 0,
            program_core_len: Vec::new(),
            program_mechanism: 0,
            program_licensed: 0,
            program_campaign: 0,
            substrate_not_runs: [0, 0],
            program_replays: 0,
            program_closure_len: Vec::new(),
            program_deletion_shrank: 0,
            truncations: [0, 0],
        };
        for c in corpus() {
            f.runs += 1;
            *f.by_target.entry(c.target).or_default() += 1;
            let order = order_of(c);
            let whole: Vec<usize> = (0..c.journal.len()).collect();
            // The anchors: the whole run's own replay's witnesses.
            let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
            let Replayed::Fails { witnesses } = oracle.replay(&whole) else {
                panic!("{}: the whole journal replays to its failure", c.label);
            };
            let closed = reduce::closure_pass(&order, &mut oracle, BUDGET);
            let Reduction::Reduced {
                core, transcript, ..
            } = &closed
            else {
                panic!("{}: closure finished: {closed:?}", c.label);
            };
            if matches!(transcript[0].end, PassEnd::ClosureNotReplayPreserving(_)) {
                eprintln!("{}: {:?}", c.label, transcript[0].end);
                f.closure_not_preserving += 1;
                continue;
            }
            // Every kept core still fails, on a fresh, independent replay.
            let mut fresh = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
            assert!(
                matches!(fresh.replay(&core.events), Replayed::Fails { .. }),
                "{}: the closure core fails",
                c.label
            );
            assert!(core.guarantees.contains(&Guarantee::CausallyClosed));
            // Against the independent reachability: no event on a happens-before path to
            // a witness is dropped, and the kept set is exactly that past closed under the
            // journal's atoms.
            let (hb, closed) = naive_past(c, &witnesses);
            let kept: BTreeSet<usize> = core.events.iter().copied().collect();
            assert!(
                hb.is_subset(&kept),
                "{}: closure dropped a happens-before predecessor",
                c.label
            );
            assert_eq!(kept, closed, "{}: closure is the atom-closed past", c.label);
            f.on_path += hb.len();
            f.atom_mates += kept.len() - hb.len();
            f.dropped_checked += c.journal.len() - kept.len();
            // Then each deletion pass: its core also fails on a fresh replay.
            f.trace_len.push(c.journal.len());
            f.closure_len.push(core.events.len());
            let mut substrate: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
            for (k, mode) in [Deletion::Configurations, Deletion::Atoms]
                .into_iter()
                .enumerate()
            {
                let (full, replays) = minimize(c, mode);
                f.replays += replays;
                let core2 = full.core().expect("reduced");
                assert!(
                    matches!(fresh.replay(&core2.events), Replayed::Fails { .. }),
                    "{}: the {mode:?} deletion core fails",
                    c.label
                );
                if mode == Deletion::Configurations {
                    assert!(core2.guarantees.contains(&Guarantee::CausallyMinimal));
                    assert!(order.is_down_closed(&core2.events));
                } else if !story(c, &core2.events)
                    .iter()
                    .any(|l| l.starts_with("Submit("))
                {
                    f.atoms_without_submit += 1;
                }
                f.core_len[k].push(core2.events.len());
                substrate[k] = core2.events.clone();
            }
            // The program-level replay: deletion over atoms, and the substrate cores
            // judged by it.
            let p = minimize_program(c, Deletion::Atoms);
            f.program_replays += p.replays;
            f.truncations[0] += p.ops.refused;
            f.truncations[1] += p.ops.diverged;
            let core3 = p.reduction.core().expect("reduced");
            assert!(
                core3.guarantees.contains(&Guarantee::ReplayPreserving),
                "{}",
                c.label
            );
            f.program_core_len.push(core3.events.len());
            let Reduction::Reduced { transcript, .. } = &p.reduction else {
                unreachable!("reduced");
            };
            f.program_closure_len.push(transcript[0].after);
            if transcript[1].after < transcript[1].before {
                f.program_deletion_shrank += 1;
            }
            if keeps_mechanism(&story(c, &core3.events), c.target) {
                f.program_mechanism += 1;
            }
            if core3.guarantees.contains(&Guarantee::AtomMinimal)
                && transcript_licenses(&p.order, core3, p.reduction.attempts().expect("attempts"))
                    .is_ok()
            {
                f.program_licensed += 1;
            }
            if campaign_finds_target(c, &p.ops, &core3.events) {
                f.program_campaign += 1;
            }
            let mut oracle = ProgramOracle {
                case: c,
                ops: &p.ops,
                replays: 0,
            };
            for (k, events) in substrate.iter().enumerate() {
                if matches!(
                    oracle.replay(events),
                    Replayed::NotReplayable(NotReplayable::Nonconforming(_))
                ) {
                    f.substrate_not_runs[k] += 1;
                }
            }
        }
        f
    })
}

fn corpus_section() -> String {
    let f = corpus_facts();
    let ratio = |v: &[usize]| {
        median(
            v.iter()
                .zip(&f.trace_len)
                .map(|(a, b)| *a as f64 / *b as f64)
                .collect(),
        )
    };
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-03-closure-preservation]");
    let _ = writeln!(
        s,
        "corpus: every failing run of M01's scenario plan ({} of {} logs), targets {:?}",
        f.runs,
        baseline::SCENARIO_LOGS,
        f.by_target
            .iter()
            .map(|(t, n)| format!("{}={n}", t.name()))
            .collect::<Vec<_>>()
    );
    let _ = writeln!(
        s,
        "closure: kept {} events on a happens-before path to a witness and {} more that share an operation atom with one; dropped {}, none on such a path; checked against pairwise footprint dependence walked apart from the predecessor lists and the reduction; not replay-preserving on {} runs",
        f.on_path, f.atom_mates, f.dropped_checked, f.closure_not_preserving
    );
    let _ = writeln!(
        s,
        "every closure core and every deletion core fails on a fresh replay; every closure core is causally closed; every core of deletion over configurations is causally closed and causally minimal"
    );
    let med = |v: &[usize]| median(v.iter().map(|x| *x as f64).collect());
    let _ = writeln!(
        s,
        "median length: trace {}, closure {} ({:.1}% of trace), core after deletion over configurations {} ({:.1}%), core after deletion over atoms {} ({:.1}%); {} replays in all",
        med(&f.trace_len),
        med(&f.closure_len),
        100.0 * ratio(&f.closure_len),
        med(&f.core_len[0]),
        100.0 * ratio(&f.core_len[0]),
        med(&f.core_len[1]),
        100.0 * ratio(&f.core_len[1]),
        f.replays
    );
    let _ = writeln!(
        s,
        "deletion over atoms: {} of {} cores keep no Submit, so their acknowledgement is not over any write: the substrate replay accepts a journal the program cannot produce, and the mechanism is lost; the program replay below keeps it",
        f.atoms_without_submit, f.runs
    );
    let _ = writeln!(
        s,
        "program replay (bn-2z08o): median closure under the program's order {} ({:.1}% of trace); deletion over atoms removed events beyond the closure on {} runs; median core {} ({:.1}% of trace); {} of {} cores keep the mechanism; {} are atom-minimal with the claim checked from their transcript alone; the campaign's checker on each core's re-execution reports the target on {}; {} replays; truncations refused by the binding {}, not a prefix of the journal {}",
        med(&f.program_closure_len),
        100.0 * ratio(&f.program_closure_len),
        f.program_deletion_shrank,
        med(&f.program_core_len),
        100.0 * ratio(&f.program_core_len),
        f.program_mechanism,
        f.runs,
        f.program_licensed,
        f.program_campaign,
        f.program_replays,
        f.truncations[0],
        f.truncations[1]
    );
    let _ = writeln!(
        s,
        "the substrate replay's cores under the program replay: {} of {} cores of deletion over configurations and {} of {} of deletion over atoms are not runs of the program",
        f.substrate_not_runs[0], f.runs, f.substrate_not_runs[1], f.runs
    );
    s
}

fn determinism_section() -> String {
    let c = agreement_witness();
    // A fresh run of the binding from the same plan and log.
    let (plan, logs) = m01_plan_and_logs(0, 0);
    let again = case(
        "again".to_owned(),
        &plan,
        &logs[146],
        Some(Target::Agreement),
    )
    .expect("fails");
    assert_eq!(
        again.journal.encode().expect("encodes"),
        c.journal.encode().expect("encodes")
    );
    let (a, ra) = minimize(c, Deletion::Configurations);
    let (b, rb) = minimize(&again, Deletion::Configurations);
    assert_eq!(a, b, "identical inputs give identical reductions");
    assert_eq!(ra, rb);
    let ca = core_journal(c, &a.core().expect("core").events)
        .encode()
        .expect("encodes");
    let cb = core_journal(&again, &b.core().expect("core").events)
        .encode()
        .expect("encodes");
    assert_eq!(ca, cb, "byte-identical cores");
    // The program-level replay, twice: identical reductions, transcripts included.
    let pa = minimize_program(c, Deletion::Atoms);
    let pb = minimize_program(&again, Deletion::Atoms);
    assert_eq!(
        pa.reduction, pb.reduction,
        "identical program-level reductions"
    );
    assert_eq!(pa.replays, pb.replays);
    let ta: Vec<String> = pa
        .reduction
        .attempts()
        .expect("attempts")
        .entries
        .iter()
        .map(attempt_line)
        .collect();
    let tb: Vec<String> = pb
        .reduction
        .attempts()
        .expect("attempts")
        .entries
        .iter()
        .map(attempt_line)
        .collect();
    assert_eq!(ta, tb, "identical transcripts");
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-04-determinism]");
    let _ = writeln!(
        s,
        "M01 scenario plan 0 log 146, run twice through the binding and reduced twice: identical reductions ({ra} replays each) and byte-identical core encodings ({} bytes)",
        ca.len()
    );
    let _ = writeln!(
        s,
        "the same, under the program replay with deletion over atoms: identical reductions and transcripts ({} replays and {} transcript entries each)",
        pa.replays,
        ta.len()
    );
    s
}

/// The value symmetry: swap v0 and v1 in every role. The journal carries no value, and
/// both targets are symmetric in values, so the core must not move.
fn swap_values(roles: &Roles) -> Roles {
    let swap = |v: u8| 1 - v;
    Roles {
        tasks: roles
            .tasks
            .iter()
            .map(|&(role, region)| {
                let role = match role {
                    Role::Writer { node, epoch, value } => Role::Writer {
                        node,
                        epoch,
                        value: swap(value),
                    },
                    Role::Coordinator { epoch, value } => Role::Coordinator {
                        epoch,
                        value: swap(value),
                    },
                    other => other,
                };
                (role, region)
            })
            .collect(),
        regions: roles.regions,
        mailboxes: roles.mailboxes.clone(),
    }
}

fn symmetry_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-05-symmetry-renaming]");
    for c in [agreement_witness(), acked_witness()] {
        let order = order_of(c);
        let swapped = swap_values(&c.built.roles);
        assert_ne!(swapped, c.built.roles);
        let mut o1 = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
        let mut o2 = RegisterOracle::new(&c.journal, &swapped, c.target);
        let a = reduce::minimize(&order, Deletion::Configurations, &mut o1, BUDGET);
        let b = reduce::minimize(&order, Deletion::Configurations, &mut o2, BUDGET);
        assert_eq!(a, b, "{}: the value swap moves nothing", c.label);
        let mut swapped_case = c.clone();
        swapped_case.built.roles = swapped;
        let pa = minimize_program(c, Deletion::Atoms);
        let pb = minimize_program(&swapped_case, Deletion::Atoms);
        assert_eq!(
            pa.reduction, pb.reduction,
            "{}: the value swap moves nothing under the program replay",
            c.label
        );
        let _ = writeln!(
            s,
            "M01 {}: roles with v0 and v1 swapped give the identical reduction, core {} events; under the program replay too, core {} events",
            c.label,
            a.core().expect("core").events.len(),
            pa.reduction.core().expect("core").events.len()
        );
    }
    s
}

fn evidence() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-3km4z): deletion and causal-closure reduction over the replicated register's failing journals.\n\
         # Regenerate: PR18_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl01_reduction\n\
         # Causality: continuum_asupersync::causal's declared footprint dependence. Replay: lift, A7 model, projection, target property.\n\
         # Program replay (bn-2z08o): the program truncated to the candidate's operations, re-executed through the binding, must emit the candidate; then the same checks. Per-removal transcripts: pr18_impl01_transcripts.txt.\n\n",
    );
    s.push_str(&core_section(
        "pr18-impl01-01-m01-agreement-core",
        agreement_witness(),
        agreement_program(),
    ));
    s.push('\n');
    s.push_str(&core_section(
        "pr18-impl01-02-m01-acked-not-durable-core",
        acked_witness(),
        acked_program(),
    ));
    s.push('\n');
    s.push_str(&corpus_section());
    s.push('\n');
    s.push_str(&determinism_section());
    s.push('\n');
    s.push_str(&symmetry_section());
    s
}

/// The retained per-removal transcripts (RFC 0028) of the program-level reductions of
/// M01's two witnesses: `tests/golden/pr18_impl01_transcripts.txt`.
fn transcripts() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-2z08o): RFC 0028 per-removal minimizer transcripts of M01's witnesses, deletion over atoms under the program replay.\n\
         # Regenerate: PR18_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl01_reduction\n\
         # Each line: #attempt pass trial v<version of the set tried against> from <its size> n=<granularity> removed <events removed from it> -> <candidate size> events: verdict [memo-of #attempt] [-- the oracle's reason]\n",
    );
    for (id, c, p) in [
        (
            "pr18-impl01-06-m01-agreement-transcript",
            agreement_witness(),
            agreement_program(),
        ),
        (
            "pr18-impl01-07-m01-acked-not-durable-transcript",
            acked_witness(),
            acked_program(),
        ),
    ] {
        let a = p.reduction.attempts().expect("attempts");
        let bound = reduce::TranscriptBound::DEFAULT;
        let _ = writeln!(s, "\n[{id}]");
        let _ = writeln!(s, "run: M01 {}, target {}", c.label, c.target.name());
        let _ = writeln!(
            s,
            "bound: {} entries, {} units; recorded {}, omitted {}; result version {}",
            bound.entries,
            bound.units,
            a.entries.len(),
            a.omitted,
            a.version
        );
        for e in &a.entries {
            let _ = writeln!(s, "{}", attempt_line(e));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// The witnesses are the ones `pr16_impl05_mutants.evidence.txt` records.
#[test]
fn the_witnesses_are_the_mutant_campaigns() {
    let a = agreement_witness();
    assert_eq!(a.journal.len(), 137);
    let acked = acked_witness();
    assert_eq!(acked.journal.len(), 97);
}

/// The events up to and including the last witness: the failing prefix a plain
/// truncation would keep.
fn failing_prefix(core: &reduce::Core) -> usize {
    core.witnesses.iter().max().map_or(0, |w| w + 1)
}

/// Exit: M01's ack-before-sync Agreement failure reduces to a causal core that keeps the
/// mechanism: `v0` acknowledged over volatile bytes, the crash loses them, `v1`
/// acknowledged. The core is smaller than the trace and than its failing prefix, is
/// replay-preserving, causally closed and causally minimal (1-minimal among configurations).
#[test]
fn m01_agreement_failure_reduces_to_a_causal_core_with_its_mechanism() {
    let c = agreement_witness();
    let (r, _) = minimize(c, Deletion::Configurations);
    let core = r.core().expect("reduced");
    assert!(core.events.len() < failing_prefix(core));
    assert!(core.events.len() < c.journal.len());
    assert_eq!(
        core.guarantees,
        vec![
            Guarantee::ReplayPreserving,
            Guarantee::CausallyClosed,
            Guarantee::CausallyMinimal
        ]
    );
    let st = story(c, &core.events);
    let at = |l: &str| st.iter().position(|s| s == l);
    let first = at("Ack(epoch=0,value=v0)").expect("v0 acked");
    let lost = at("Lose(n=a,epoch=0,value=v0)").expect("a's bytes lost");
    let second = at("Ack(epoch=0,value=v1)").expect("v1 acked");
    assert!(first < lost && lost < second, "{st:?}");
    // The v0 ack is over volatile bytes: no sync precedes it in the core.
    assert!(
        !st[..first].iter().any(|s| s.starts_with("Sync(")),
        "{st:?}"
    );
}

/// M01's RuntimeToAbstract failure reduces too: the core acknowledges `v0` over two
/// submits and no sync.
#[test]
fn m01_acked_not_durable_failure_reduces_to_a_causal_core() {
    let c = acked_witness();
    let (r, _) = minimize(c, Deletion::Configurations);
    let core = r.core().expect("reduced");
    assert!(core.events.len() < failing_prefix(core));
    assert!(core.guarantees.contains(&Guarantee::CausallyClosed));
    let st = story(c, &core.events);
    assert_eq!(
        st.iter().filter(|s| s.starts_with("Submit(")).count(),
        2,
        "{st:?}"
    );
    assert!(st.iter().any(|s| s.starts_with("Ack(")), "{st:?}");
    assert!(!st.iter().any(|s| s.starts_with("Sync(")), "{st:?}");
}

/// The negative result under the substrate replay. Deletion over arbitrary unions of
/// atoms (classic `ddmin`) goes far smaller, and every core it keeps replays to the
/// failure, but the replay here is the substrate's semantics (lift, A7 model,
/// projection), which knows nothing of the program's logic. So it keeps two
/// coordinators' acknowledgements and drops the confirmations, the submits and the loss
/// that make them M01's: it deletes the mechanism. The program-level replay (bn-2z08o)
/// turns this over: `deletion_over_atoms_keeps_the_mechanism_under_the_program_replay`,
/// and `the_substrate_cores_are_not_runs_of_the_program` refuses this core.
#[test]
fn deletion_over_atoms_loses_the_mechanism_under_a_substrate_replay() {
    let c = agreement_witness();
    let (r, _) = minimize(c, Deletion::Atoms);
    let core = r.core().expect("reduced");
    assert!(!core.guarantees.contains(&Guarantee::CausallyClosed));
    let st = story(c, &core.events);
    assert!(st.iter().all(|s| s.starts_with("Ack(")), "{st:?}");
    assert!(!st.iter().any(|s| s.starts_with("Lose(")), "{st:?}");
    // The mechanism predicate the program-replay tests use is not vacuous: it rejects
    // this core.
    assert!(!keeps_mechanism(&st, Target::Agreement));
}

/// Closure preservation over the corpus, and the differential: the reduction's closure
/// (`continuum_debugger::reduce`, over `causal::predecessors_with`'s lists) against
/// pairwise reachability computed apart from both, over every pair of
/// `continuum_asupersync::causal`'s footprints. The footprints themselves are held to
/// the lift by `independent_adjacent_swaps_of_every_corpus_journal_still_lift`. Every kept core still fails; every
/// dropped event is on no happens-before path to a witness; every kept one is.
#[test]
fn closure_keeps_exactly_the_happens_before_past_over_the_corpus() {
    let f = corpus_facts();
    assert!(f.runs > 0);
    assert_eq!(f.closure_not_preserving, 0);
    assert!(f.dropped_checked > 0 && f.on_path > 0);
}

/// The declared footprints over-approximate the lift's own order: in every fourth
/// journal of the corpus, every adjacent pair of events the footprints call independent
/// is swapped (`causal::permute`, ordinals renamed in the new order), and the swapped
/// journal still lifts as a conforming run. A missing dependence the lift enforces would
/// show here as a refused swap. Adjacent swaps only: a sampled check of the relation
/// against the semantics, not a proof.
#[test]
fn independent_adjacent_swaps_of_every_corpus_journal_still_lift() {
    let mut swaps = 0_usize;
    for c in corpus().iter().step_by(4) {
        let prints = causal::footprints(&c.journal, CAUSAL_WORK).expect("within the bound");
        let n = c.journal.len();
        for i in 0..n - 1 {
            if causal::dependent(&prints[i], &prints[i + 1]) {
                continue;
            }
            let mut seq: Vec<usize> = (0..n).collect();
            seq.swap(i, i + 1);
            let swapped = causal::permute(&c.journal, &seq).expect("permutes");
            let verdict = lift(&swapped.journal);
            assert!(
                matches!(verdict, LiftVerdict::Conforms(_)),
                "{}: swapping independent events {i} and {} gives {verdict:?}",
                c.label,
                i + 1
            );
            swaps += 1;
        }
    }
    assert!(swaps > 1_000, "{swaps} swaps");
}

/// The exit under a program-level replay (bn-2z08o). Deletion over atoms, classic
/// `ddmin` over any union of operation groups, where the program's own re-execution
/// judges each candidate, keeps M01's mechanism: `v0` is acknowledged over volatile
/// bytes that `a` and `b` submitted, nothing is synced before it, the crash loses `a`'s
/// bytes, and `v1` is acknowledged. The core is replay-preserving, causally closed under
/// the program's order and atom-minimal, and that minimality is checked again from the
/// transcript alone. The campaign's own checker, run on the core's re-execution, reports
/// the Agreement finding. This is the pin below turned over: the substrate replay lost
/// the mechanism, the program replay keeps it.
#[test]
fn deletion_over_atoms_keeps_the_mechanism_under_the_program_replay() {
    let c = agreement_witness();
    let p = agreement_program();
    let core = p.reduction.core().expect("reduced");
    assert_eq!(
        core.guarantees,
        vec![
            Guarantee::ReplayPreserving,
            Guarantee::CausallyClosed,
            Guarantee::AtomMinimal
        ]
    );
    assert!(core.events.len() < c.journal.len());
    let st = story(c, &core.events);
    let at = |l: &str| st.iter().position(|s| s == l);
    let first = at("Ack(epoch=0,value=v0)").expect("v0 acked");
    let submit = at("Submit(n=a,epoch=0,value=v0)").expect("a submits v0");
    let lost = at("Lose(n=a,epoch=0,value=v0)").expect("a's bytes lost");
    let second = at("Ack(epoch=0,value=v1)").expect("v1 acked");
    assert!(submit < first && first < lost && lost < second, "{st:?}");
    assert!(
        !st[..first].iter().any(|s| s.starts_with("Sync(")),
        "{st:?}"
    );
    assert!(keeps_mechanism(&st, Target::Agreement), "{st:?}");
    assert!(campaign_finds_target(c, &p.ops, &core.events));
    let attempts = p.reduction.attempts().expect("a transcript");
    assert_eq!(attempts.omitted, 0);
    assert!(transcript_licenses(&p.order, core, attempts).is_ok());
}

/// The same for M01's `RuntimeToAbstract` witness: `v0` is acknowledged over two submits
/// and no sync.
#[test]
fn m01_acked_not_durable_keeps_the_mechanism_under_the_program_replay() {
    let c = acked_witness();
    let p = acked_program();
    let core = p.reduction.core().expect("reduced");
    assert!(core.guarantees.contains(&Guarantee::AtomMinimal));
    let st = story(c, &core.events);
    assert_eq!(
        st.iter().filter(|s| s.starts_with("Submit(")).count(),
        2,
        "{st:?}"
    );
    assert!(st.iter().any(|s| s.starts_with("Ack(")), "{st:?}");
    assert!(!st.iter().any(|s| s.starts_with("Sync(")), "{st:?}");
    assert!(keeps_mechanism(&st, Target::AckedNotDurable));
    assert!(campaign_finds_target(c, &p.ops, &core.events));
    assert!(transcript_licenses(&p.order, core, p.reduction.attempts().expect("attempts")).is_ok());
}

/// The substrate replay's cores, of both deletion modes, are not runs of the program:
/// the program replay refuses them.
#[test]
fn the_substrate_cores_are_not_runs_of_the_program() {
    for (c, p) in [
        (agreement_witness(), agreement_program()),
        (acked_witness(), acked_program()),
    ] {
        let mut oracle = ProgramOracle {
            case: c,
            ops: &p.ops,
            replays: 0,
        };
        for mode in [Deletion::Configurations, Deletion::Atoms] {
            let (r, _) = minimize(c, mode);
            let events = &r.core().expect("reduced").events;
            let verdict = oracle.replay(events);
            assert!(
                matches!(
                    verdict,
                    Replayed::NotReplayable(NotReplayable::Nonconforming(_))
                ),
                "{} {mode:?}: {verdict:?}",
                c.label
            );
        }
    }
}

/// The program replay accepts the program's own run and judges what the program cannot
/// produce as no run: a group held in part, an actor's operation without the ones
/// before it, and coordinators that receive confirmations no replica sent (the binding's
/// own refusal). The operations that end before the run's first acknowledgement are a
/// run that holds.
#[test]
fn the_program_replay_refuses_what_the_program_cannot_produce() {
    let c = agreement_witness();
    let o = &agreement_program().ops;
    let mut oracle = ProgramOracle {
        case: c,
        ops: o,
        replays: 0,
    };
    let whole: Vec<usize> = (0..c.journal.len()).collect();
    assert!(matches!(oracle.replay(&whole), Replayed::Fails { .. }));
    let not_a_run = |r: Replayed, what: &str| match r {
        Replayed::NotReplayable(NotReplayable::Nonconforming(why)) => {
            assert!(why.contains(what), "{why}");
        }
        other => panic!("{what}: {other:?}"),
    };
    // A group held in part.
    let g = o
        .groups
        .iter()
        .find(|g| g.events.1 - g.events.0 >= 2)
        .expect("a group of several events");
    let part: Vec<usize> = whole.iter().copied().filter(|&e| e != g.events.0).collect();
    not_a_run(oracle.replay(&part), "held in part");
    // Replica `a`'s first operation dropped, its later ones kept.
    let first_of_a = o
        .groups
        .iter()
        .position(|g| o.ops[g.start..g.end].contains(&(1, 0)))
        .expect("a's first operation");
    let (x, y) = o.groups[first_of_a].events;
    let skip: Vec<usize> = whole.iter().copied().filter(|&e| e < x || e >= y).collect();
    not_a_run(oracle.replay(&skip), "without the ones before it");
    // Replicas that never confirm, coordinators that receive: each actor runs a prefix
    // of its program, and the binding refuses the command to a parked coordinator.
    let replicas = 1..=3;
    let coordinators = 4..c.built.programs.len() - 1;
    let kept: Vec<usize> = whole
        .iter()
        .copied()
        .filter(|&e| {
            let g = o.groups[o.group_of[e]];
            o.ops[g.start..g.end].iter().all(|&(actor, index)| {
                actor == 0
                    || (replicas.contains(&actor) && index < 2)
                    || coordinators.contains(&actor)
            })
        })
        .collect();
    not_a_run(oracle.replay(&kept), "binding:");
    // Every operation group that ends before the run's first acknowledgement: a run
    // that does not fail.
    let first_ack = register::observe(&c.built.roles, &c.journal)
        .expect("projects")
        .iter()
        .find(|s| matches!(&s.expect, Expect::Step(l) if l.starts_with("Ack(")))
        .map(|s| s.seq as usize)
        .expect("an acknowledgement");
    let before: Vec<usize> = whole
        .iter()
        .copied()
        .filter(|&e| o.groups[o.group_of[e]].events.1 <= first_ack)
        .collect();
    assert!(!before.is_empty());
    assert_eq!(oracle.replay(&before), Replayed::Holds);
}

/// The program replay over the corpus: every core of deletion over atoms keeps the
/// mechanism, is atom-minimal with the claim checked from its transcript alone, and is
/// confirmed by the campaign's own checker on its re-execution; every substrate-replay
/// core of deletion over atoms is not a run of the program.
#[test]
fn program_replay_cores_keep_the_mechanism_over_the_corpus() {
    let f = corpus_facts();
    assert_eq!(f.program_core_len.len(), f.runs);
    assert_eq!(f.program_mechanism, f.runs);
    assert_eq!(f.program_licensed, f.runs);
    assert_eq!(f.program_campaign, f.runs);
    assert_eq!(f.substrate_not_runs[1], f.runs);
}

/// The mechanism predicate is not vacuous: story mutants of the Agreement witness's
/// program-replay core that lack a part of the mechanism are rejected. No loss; both
/// submissions of v0 synced before its acknowledgement; one submitter of v0; the loss
/// after the other value's acknowledgement.
#[test]
fn the_mechanism_predicate_rejects_story_mutants() {
    let c = agreement_witness();
    let st = story(
        c,
        &agreement_program().reduction.core().expect("core").events,
    );
    assert!(keeps_mechanism(&st, Target::Agreement));
    let without =
        |label: &str| -> Vec<String> { st.iter().filter(|l| *l != label).cloned().collect() };
    let lose = "Lose(n=a,epoch=0,value=v0)";
    assert!(!keeps_mechanism(&without(lose), Target::Agreement));
    assert!(!keeps_mechanism(
        &without("Submit(n=b,epoch=0,value=v0)"),
        Target::Agreement
    ));
    let first = st
        .iter()
        .position(|l| l == "Ack(epoch=0,value=v0)")
        .expect("v0 acked");
    let mut synced = st.clone();
    synced.insert(first, "Sync(n=a,epoch=0)".to_owned());
    synced.insert(first, "Sync(n=b,epoch=0)".to_owned());
    assert!(!keeps_mechanism(&synced, Target::Agreement));
    let mut late = without(lose);
    late.push(lose.to_owned());
    assert!(!keeps_mechanism(&late, Target::Agreement));
}

/// An accepted candidate is a partial run of the whole program, not only a run of the
/// truncated one: the whole program, scheduled with the core's operations first in the
/// run's order and then the rest in the run's order, runs through the binding, and the
/// core's sub-journal is a prefix of its journal. Checked on both witnesses' cores.
#[test]
fn a_program_replay_core_is_a_prefix_of_a_whole_run() {
    for (c, p) in [
        (agreement_witness(), agreement_program()),
        (acked_witness(), acked_program()),
    ] {
        let core = &p.reduction.core().expect("core").events;
        let first = ops_of(&p.ops, core);
        let chosen: BTreeSet<(usize, usize)> = first.iter().copied().collect();
        let mut order = first.clone();
        order.extend(p.ops.ops.iter().copied().filter(|op| !chosen.contains(op)));
        let journal = rerun(&c.built.programs, &order)
            .expect("each actor's operations in order")
            .expect("the whole program runs");
        let sub = core_journal(c, core);
        assert!(
            journal.len() >= sub.len() && journal.events()[..sub.len()] == *sub.events(),
            "{}",
            c.label
        );
    }
}

/// INV-006: identical input traces give byte-identical cores.
#[test]
fn identical_inputs_give_byte_identical_cores() {
    let _ = determinism_section();
}

/// Metamorphic, symmetry renaming: swapping the values in the role table (an
/// event-irrelevant renaming: no journal event names a value) gives the identical core.
#[test]
fn symmetry_renaming_of_values_leaves_the_core_unchanged() {
    let _ = symmetry_section();
}

/// Metamorphic, symmetry renaming under the program replay: swapping the values in the
/// role table gives the identical program-level reduction and the identical per-removal
/// transcript, attempt for attempt, on both witnesses.
#[test]
fn symmetry_renaming_leaves_the_program_replay_and_its_transcript_unchanged() {
    for (c, p) in [
        (agreement_witness(), agreement_program()),
        (acked_witness(), acked_program()),
    ] {
        let mut swapped = c.clone();
        swapped.built.roles = swap_values(&c.built.roles);
        assert_ne!(swapped.built.roles, c.built.roles);
        let q = minimize_program(&swapped, Deletion::Atoms);
        assert_eq!(p.reduction, q.reduction, "{}", c.label);
        let lines = |r: &Reduction| -> Vec<String> {
            r.attempts()
                .expect("attempts")
                .entries
                .iter()
                .map(attempt_line)
                .collect()
        };
        assert_eq!(lines(&p.reduction), lines(&q.reduction), "{}", c.label);
    }
}

/// A candidate that cannot be replayed is not kept: dropping a spawn but keeping its
/// task's steps is refused by the lift, and the reduction never returns such a set.
#[test]
fn an_unreplayable_candidate_is_refused_not_kept() {
    let c = agreement_witness();
    let spawn = c
        .journal
        .events()
        .iter()
        .position(|e| {
            matches!(
                e.body(),
                EventBody::Lifecycle(LifecycleEvent::TaskSpawned { .. })
            )
        })
        .expect("a spawn");
    let kept: Vec<usize> = (0..c.journal.len()).filter(|&i| i != spawn).collect();
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    assert!(matches!(oracle.replay(&kept), Replayed::NotReplayable(_)));
    // As a starting set it is not even a configuration.
    let order = order_of(c);
    assert!(!order.is_down_closed(&kept));
    assert_eq!(
        reduce::deletion_pass(&order, kept, Deletion::Configurations, &mut oracle, BUDGET),
        Reduction::Refused(reduce::Refusal::NotAConfiguration)
    );
}

/// A correct run does not fail: the reduction refuses it rather than returning a core.
#[test]
fn a_run_that_does_not_fail_is_refused() {
    let base = baseline::baseline();
    let plan = &base.groups[0].plans[0];
    let built = register::build_with_shutdown(plan);
    let log = &baseline::logs_for(&built, 1, 0).1[0];
    let journal = run(&built.programs, log, &baseline::config()).expect("a journal");
    let observer = observer_footprints(&journal, &built.roles, Target::Agreement);
    let order = CausalOrder::with_atoms(
        causal::predecessors_with(&journal, CAUSAL_WORK, &observer).expect("within the bound"),
        causal::atoms(&journal),
    )
    .expect("a causal order");
    for target in [Target::Agreement, Target::AckedNotDurable] {
        let mut oracle = RegisterOracle::new(&journal, &built.roles, target);
        assert_eq!(
            reduce::minimize(&order, Deletion::Configurations, &mut oracle, BUDGET),
            Reduction::Refused(reduce::Refusal::InputHolds)
        );
    }
}

/// INV-008: a budget too small to finish is a typed inconclusive with the best
/// replay-validated set, never a success and never a minimality claim.
#[test]
fn an_exhausted_budget_is_inconclusive_with_the_best_core() {
    let c = agreement_witness();
    let order = order_of(c);
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(
        &order,
        Deletion::Configurations,
        &mut oracle,
        Budget::new(5, 1 << 26),
    );
    let Reduction::Inconclusive {
        reason,
        best,
        transcript,
        spent,
        ..
    } = r
    else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(
        reason,
        continuum_value::assurance::InconclusiveReason::ResourceExhausted
    );
    assert_eq!(spent.replays, 5);
    let best = best.expect("the closure core");
    assert!(!best.guarantees.contains(&Guarantee::CausallyMinimal));
    assert!(matches!(
        oracle.replay(&best.events),
        Replayed::Fails { .. }
    ));
    assert_eq!(
        transcript.last().map(|p| p.pass),
        Some(Pass::Deletion(Deletion::Configurations))
    );
    // No replay at all: nothing validated.
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(
        &order,
        Deletion::Configurations,
        &mut oracle,
        Budget::new(0, 1 << 26),
    );
    assert!(matches!(r, Reduction::Inconclusive { best: None, .. }));
    assert_eq!(oracle.replays, 0, "charged before the replay runs");
}

/// Compare `got` with the golden at `path` (rewrite it under `PR18_IMPL01_BLESS`), and
/// return its artifact IDs.
fn check_golden(path: &str, got: &str) -> Vec<String> {
    if std::env::var_os("PR18_IMPL01_BLESS").is_some() {
        std::fs::write(path, got).expect("writes the golden");
        panic!("the golden was rewritten: review the diff and rerun without PR18_IMPL01_BLESS");
    }
    let want = std::fs::read_to_string(path).expect("the golden exists");
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
        "{path} drifted at {first}; regenerate with PR18_IMPL01_BLESS=1 and review"
    );
    got.lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_evidence_matches_its_golden() {
    let ids = check_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr18_impl01_reduction.evidence.txt"
        ),
        &evidence(),
    );
    assert_eq!(
        ids,
        [
            "pr18-impl01-01-m01-agreement-core",
            "pr18-impl01-02-m01-acked-not-durable-core",
            "pr18-impl01-03-closure-preservation",
            "pr18-impl01-04-determinism",
            "pr18-impl01-05-symmetry-renaming",
        ]
    );
}

/// RFC 0028's per-removal transcripts are retained artifacts: every attempt of the
/// program-level reductions of M01's witnesses, in order, with its candidate, verdict and
/// reason.
#[test]
fn the_transcripts_match_their_golden() {
    let ids = check_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr18_impl01_transcripts.txt"
        ),
        &transcripts(),
    );
    assert_eq!(
        ids,
        [
            "pr18-impl01-06-m01-agreement-transcript",
            "pr18-impl01-07-m01-acked-not-durable-transcript",
        ]
    );
}
