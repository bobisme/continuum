//! PR-20 / IMPL-03 (bn-2pla): exact replay, gates 1 and 4 of a repair transaction, on
//! the replicated register's M01 failure (ack-before-sync) and its ack-after-sync repair.
//! START_HERE PR 20 ("exact replay"); RFC 0032 gates 1 and 4.
//!
//! # The instantiation
//!
//! `continuum_repair::replay` is generic over a [`Replayer`]. Here:
//!
//! - **The program** is the workspace the transaction names. Its replica,
//!   `src/replica.rs`, is the text of PR 20's register workspace
//!   (`crates/continuum-repair/tests/common`): a `write` whose statements are
//!   `storage.put(value);`, `reply.send(Ack);` and `storage.sync()?;` in some order.
//!   [`program_of`] reads that order and nothing else: the ack statements between the put
//!   and the sync confirm right after the submit, the ones after the sync confirm after
//!   the release, and the plan is the scenario's own (`cancel-after-submit-before-sync`,
//!   `register_baseline::scenario_plan`) with each replica's writes so ordered. The base,
//!   ack before sync, is exactly PR 16's M01 plan (`register_mutants::ack_before_sync`);
//!   the ack-after-sync candidate is exactly the correct plan. Any other statement is a
//!   program this replayer does not support.
//! - **The recorded run** is M01's witness on the scenario plan: the first of PR 16's
//!   400 campaign logs for that plan that fails `abstract_register::Agreement`. Its
//!   schedule is the order in which its operations ran, each named by a
//!   program-independent [`label`]: a replica's act by replica, incarnation, act and
//!   occurrence, and a coordinator's step by epoch, value, kind and occurrence. The
//!   setup's and the shutdown's operations are not choices (the setup runs first and the
//!   shutdown last in every admissible log) and are not recorded. Its journal is the
//!   binding's canonical journal bytes; its failure identity is the rendering of every
//!   finding PR 16's `run_one` reports, in check order.
//! - **The original choices on a program** ([`RegisterReplayer`]): the recorded order is
//!   a preference, realized as an admissible choice log of the program by
//!   `register::guided_run`. The correspondence is by label. A recorded step whose label
//!   the program lacks is *eliminated*: continued past, and its position in the recording
//!   reported (RFC 0032: "reports where replay diverges and continues"). A step run
//!   after a step recorded later is *reordered*, and counted: the recorded order is a
//!   preference, so the candidate may run common steps in another order. An operation of the program whose label the
//!   recording lacks is a step the recorded choices say nothing about, and a program that
//!   parks under the preference cannot follow it: both are a divergence. On the base
//!   the realization is the recorded log itself, choice for choice.
//!
//! This correspondence is by label, not the plan §16 correspondence over the CIR (PR 17),
//! and it holds only for programs [`program_of`] can read.
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr20_impl03_exact_replay.evidence.txt`, and the
//! three evaluated transaction versions into `tests/golden/pr20-impl03-*.json`, which
//! `notes/plan/tools/validate_dossier.py` checks against `repair-transaction.schema.json`.
//! Regenerate with `PR20_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test
//! pr20_impl03_exact_replay`, which rewrites and then fails, so the diff is reviewed.
//!
//! - `pr20-impl03-pos-01-m01-ack-after-sync`: M01's failure replays exactly on the base,
//!   gate 1 passes; the ack-after-sync candidate does not fail under the original
//!   choices, but it eliminates and reorders recorded steps, so under the exact policy
//!   gate 4 is inconclusive, never passed. A relaxed policy needs an authorization that
//!   nothing issues yet (bn-2vanm, bn-b6u4).
//! - `pr20-impl03-pos-02-every-m01-run`: the same over every one of the 400 failing
//!   runs, each its own crashpack, against the PR-16 oracle's own run of it: 400 of 400
//!   inconclusive `not-exact`, none passed and none failed.
//! - `pr20-impl03-neg-01-repair-that-does-not-fix`: a candidate that only comments the
//!   replica fails gate 4, the failure recurs; 400 of 400.
//! - `pr20-impl03-neg-02-tampered-crashpack`: altered bytes are refused, nothing runs.
//! - `pr20-impl03-neg-03-stale-failure-binding`: a crashpack whose recorded failure the
//!   base does not produce fails gate 1; gate 4 is not run and stays pending.
//! - `pr20-impl03-bnd-03-inconsistent-record`: a crashpack that records the base's
//!   failure with another run's journal: the base replay is inconsistent, an engine
//!   condition, inconclusive with a `defect_`.
//! - `pr20-impl03-bnd-01-divergence`: a candidate that acks both before and after the
//!   sync has steps the recording has no choice for: gate 4 is inconclusive, never
//!   passed; 400 of 400.
//! - `pr20-impl03-bnd-02-unsupported`: a replica statement the replayer cannot read is
//!   inconclusive `Unsupported`.

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

use baseline::Finding;
use continuum_asupersync::binding::{SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_repair::handle::{CrashpackId, SnapshotId};
use continuum_repair::hypothesis::{ChangeKind, Hypothesis, Proposal};
use continuum_repair::patch::{DeclaredChange, FileEdit, HashedIdentifier};
use continuum_repair::replay::{
    self, BaseReplay, Divergence, EvaluationPolicy, ExactRegression, ExactReplay,
    ExactReplayRefusal, IntentId, RecordedRun, ReplayOutcome, ReplayRun, Replayer, Snapshot,
    WorkspaceContent, WorkspacePath,
};
use continuum_repair::transaction::{
    FailureBinding, GateName, GateProfile, GateStatus, RepairTransaction, Resolution,
    TransactionStatus,
};
use continuum_value::identity::Blake3Hasher;
use register::{Act, Built, Plan, Raw, Role};

type Tx = RepairTransaction<Blake3Hasher>;

// ---------------------------------------------------------------------------
// the register workspace (PR 20's, `crates/continuum-repair/tests/common`)
// ---------------------------------------------------------------------------

const REPLICA: &str = "src/replica.rs";
const MODEL_PATH: &str = "model/register.cml";
const MANIFEST_PATH: &str = "Cargo.toml";

const REPLICA_BEFORE: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    reply.send(Ack);\n    storage.sync()?;\n    Ok(())\n}\n";
const REPLICA_AFTER: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    storage.sync()?;\n    reply.send(Ack);\n    Ok(())\n}\n";
const MODEL: &str = "model ReplicatedRegister {\n  var stored: Nat\n  var acked: Set[Nat]\n}\n";
const MANIFEST: &str = "[package]\nname = \"register\"\n";

/// A repair that only says what it should do: the ack still precedes the sync.
const REPLICA_COMMENTED: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    // the ack must follow the sync\n    reply.send(Ack);\n    storage.sync()?;\n    Ok(())\n}\n";
/// A repair that adds the late ack and keeps the early one.
const REPLICA_DOUBLE_ACK: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    reply.send(Ack);\n    storage.sync()?;\n    reply.send(Ack);\n    Ok(())\n}\n";
/// A repair with a statement this replayer cannot read.
const REPLICA_FLUSH: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    storage.flush();\n    storage.sync()?;\n    reply.send(Ack);\n    Ok(())\n}\n";

fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("a workspace path")
}

fn workspace(replica: &str) -> WorkspaceContent {
    let mut content = WorkspaceContent::new();
    for (name, text) in [
        (MANIFEST_PATH, MANIFEST),
        (MODEL_PATH, MODEL),
        (REPLICA, replica),
    ] {
        content
            .insert(path(name), text.as_bytes().to_vec())
            .expect("distinct paths");
    }
    content
}

fn tree(content: &WorkspaceContent) -> Snapshot {
    Snapshot::build(content, &HashedIdentifier::<Blake3Hasher>::new()).expect("a tree")
}

fn snapshot_id(content: &WorkspaceContent) -> SnapshotId {
    SnapshotId::new(&tree(content).identity().to_string()).expect("a ws_ handle")
}

/// The repair: replace the base replica with `after`.
fn change(after: &str) -> DeclaredChange {
    let base = workspace(REPLICA_BEFORE);
    let leaf = tree(&base)
        .node(&path(REPLICA))
        .and_then(|node| node.as_file())
        .map(|file| SnapshotId::new(&file.identity().to_string()).expect("a ws_ handle"))
        .expect("the replica is a file");
    DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path(REPLICA),
            FileEdit::Replace {
                before: leaf,
                content: after.as_bytes().to_vec(),
            },
        )],
    )
    .expect("a well-formed change")
}

// ---------------------------------------------------------------------------
// the program a workspace holds
// ---------------------------------------------------------------------------

/// The plan the workspace's replica runs, or `None` when the replica is not a `write`
/// this replayer can read: statements `storage.put(value);`, `storage.sync()?;` once
/// each, the put first, and any number of `reply.send(Ack);` after the put.
fn program_of(content: &WorkspaceContent) -> Option<Plan> {
    let text = std::str::from_utf8(content.get(&path(REPLICA))?).ok()?;
    let mut lines = text.lines().map(str::trim);
    if lines.next()?
        != "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {"
    {
        return None;
    }
    let (mut put, mut sync, mut early, mut late) = (false, false, 0_usize, 0_usize);
    let mut closed = false;
    for line in lines {
        match line {
            _ if closed => return None,
            "" => {}
            _ if line.starts_with("//") => {}
            "storage.put(value);" if !put => put = true,
            "storage.sync()?;" if put && !sync => sync = true,
            "reply.send(Ack);" if put && !sync => early += 1,
            "reply.send(Ack);" if sync => late += 1,
            "Ok(())" if put && sync => {}
            "}" => closed = true,
            _ => return None,
        }
    }
    if !(put && sync && closed) {
        return None;
    }
    let correct = baseline::scenario_plan();
    let mut plan = correct.clone();
    for (to, from) in plan.replicas.iter_mut().zip(&correct.replicas) {
        let mut out = Vec::new();
        for act in from {
            match *act {
                Act::Submit(e) => {
                    out.push(Act::Submit(e));
                    out.extend(std::iter::repeat_n(Act::Confirm(e), early));
                }
                Act::Confirm(e) => out.extend(std::iter::repeat_n(Act::Confirm(e), late)),
                other => out.push(other),
            }
        }
        *to = out;
    }
    Some(plan)
}

// ---------------------------------------------------------------------------
// labels, schedules and the replayer
// ---------------------------------------------------------------------------

/// Each operation's program-independent label, by actor and operation index; `None` for
/// the setup's and the shutdown's, which are not choices.
fn label(plan: &Plan, built: &Built) -> Vec<Vec<Option<String>>> {
    let actors = built.programs.len();
    let mut out: Vec<Vec<Option<String>>> = built
        .programs
        .iter()
        .map(|program| vec![None; program.len()])
        .collect();
    for (n, script) in plan.replicas.iter().enumerate() {
        let (mut inc, mut seen) = (0, BTreeMap::<String, usize>::new());
        assert_eq!(
            script.len(),
            built.programs[n + 1].len(),
            "one operation per act"
        );
        for (i, act) in script.iter().enumerate() {
            let key = format!("{act:?}");
            let k = seen.entry(key.clone()).or_default();
            out[n + 1][i] = Some(format!("r{n}/i{inc}/{key}#{k}"));
            *k += 1;
            if matches!(act, Act::Crash | Act::CrashRepropose(..)) {
                inc += 1;
                seen.clear();
            }
        }
    }
    let coordinators: Vec<(u8, u8)> = built
        .roles
        .tasks
        .iter()
        .filter_map(|(role, _)| match role {
            Role::Coordinator { epoch, value } => Some((*epoch, *value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        4 + coordinators.len() + 1,
        actors,
        "setup, replicas, coordinators, shutdown"
    );
    for (k, (e, v)) in coordinators.iter().enumerate() {
        let mut recv = 0;
        for (i, op) in built.programs[4 + k].iter().enumerate() {
            out[4 + k][i] = Some(match op {
                SubstrateOp::Recv { .. } => {
                    recv += 1;
                    format!("c{e}v{v}/recv#{}", recv - 1)
                }
                SubstrateOp::Reserve { .. } => format!("c{e}v{v}/ack-reserve"),
                SubstrateOp::Commit { .. } => format!("c{e}v{v}/ack-commit"),
                other => panic!("a coordinator operation: {other:?}"),
            });
        }
    }
    out
}

/// The labels of the operations `log` runs, in order.
fn schedule_of(plan: &Plan, built: &Built, log: &ChoiceLog) -> Vec<String> {
    let labels = label(plan, built);
    let mut cursors = vec![0_usize; built.programs.len()];
    let mut out = Vec::new();
    for choice in log.choices() {
        let enabled: Vec<usize> = (0..built.programs.len())
            .filter(|a| cursors[*a] < built.programs[*a].len())
            .collect();
        let actor = enabled[usize::try_from(choice.0).expect("small")];
        if let Some(l) = &labels[actor][cursors[actor]] {
            out.push(l.clone());
        }
        cursors[actor] += 1;
    }
    out
}

fn encode_schedule(schedule: &[String]) -> Vec<u8> {
    schedule
        .iter()
        .flat_map(|l| format!("{l}\n").into_bytes())
        .collect()
}

fn failure_of(findings: &[Finding]) -> Option<Vec<u8>> {
    (!findings.is_empty()).then(|| {
        findings
            .iter()
            .map(|f| format!("{f:?}"))
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    })
}

/// The replicated register's replayer: see the module docs.
struct RegisterReplayer {
    reach: BTreeSet<Raw>,
}

impl RegisterReplayer {
    fn new() -> Self {
        Self {
            reach: register::reachable(1),
        }
    }
}

impl Replayer for RegisterReplayer {
    fn identity(&self) -> &str {
        "continuum-asupersync/replicated-register/label-correspondence/1"
    }

    fn replay(&self, program: &WorkspaceContent, schedule: &[u8]) -> ReplayOutcome {
        let Some(plan) = program_of(program) else {
            return ReplayOutcome::Unsupported;
        };
        let Ok(text) = std::str::from_utf8(schedule) else {
            return ReplayOutcome::Diverged(Divergence { at: 0 });
        };
        let recorded: Vec<&str> = text.lines().collect();
        let rank: BTreeMap<&str, usize> =
            recorded.iter().enumerate().map(|(i, l)| (*l, i)).collect();
        if rank.len() != recorded.len() {
            return ReplayOutcome::Diverged(Divergence { at: 0 });
        }
        let built = register::build_with_shutdown(&plan);
        let labels = label(&plan, &built);
        let present: BTreeSet<&str> = labels
            .iter()
            .flatten()
            .flatten()
            .map(String::as_str)
            .collect();
        let eliminated: Vec<u64> = (0..recorded.len())
            .filter(|i| !present.contains(recorded[*i]))
            .map(count)
            .collect();
        let ranked = |actor: usize, index: usize| {
            labels[actor][index]
                .as_deref()
                .map(|l| rank.get(l).copied().unwrap_or(usize::MAX - 1))
        };
        let setup = built.programs[0].len();
        let (log, taken) = match register::guided_run(&built, ranked) {
            Ok(realized) => realized,
            Err(prefix) => {
                let at = prefix.len().saturating_sub(setup);
                return ReplayOutcome::Diverged(Divergence { at: count(at) });
            }
        };
        // A step the recording has no choice for: the recorded choices do not place it.
        // A step taken after a later-recorded one ran out of the recorded order: counted.
        let (mut followed, mut reordered, mut latest) = (0, 0_u64, None::<usize>);
        for (actor, index) in &taken {
            if let Some(l) = labels[*actor][*index].as_deref() {
                let Some(&r) = rank.get(l) else {
                    return ReplayOutcome::Diverged(Divergence {
                        at: count(followed),
                    });
                };
                followed += 1;
                if latest.is_some_and(|m| r < m) {
                    reordered += 1;
                }
                latest = latest.max(Some(r));
            }
        }
        let journal = match run(&built.programs, &log, &baseline::config()) {
            Ok(journal) => journal,
            Err(_) => {
                return ReplayOutcome::Diverged(Divergence {
                    at: count(followed),
                });
            }
        };
        let report = baseline::run_one(&built, plan.epochs, &log, &self.reach);
        ReplayOutcome::Ran(ReplayRun::new(
            journal.encode().expect("a journal encodes"),
            failure_of(&report.findings),
            eliminated,
            reordered,
        ))
    }
}

fn count(n: usize) -> u64 {
    u64::try_from(n).expect("small")
}

// ---------------------------------------------------------------------------
// the recorded failure and the transaction
// ---------------------------------------------------------------------------

/// One M01 run recorded as a crashpack.
struct Recorded {
    log: ChoiceLog,
    run: RecordedRun,
    /// The PR-16 oracle's digest of the original run.
    oracle_digest: String,
}

/// Every M01 run of the scenario plan in PR 16's campaign that fails
/// `abstract_register::Agreement`, recorded, in campaign order.
fn m01_runs() -> &'static [Recorded] {
    static CELL: OnceLock<Vec<Recorded>> = OnceLock::new();
    CELL.get_or_init(|| {
        let base = workspace(REPLICA_BEFORE);
        let plan = program_of(&base).expect("the base is readable");
        let built = register::build_with_shutdown(&plan);
        let campaign = baseline::baseline();
        let (_, logs) =
            baseline::logs_for(&built, baseline::SCENARIO_LOGS, campaign.plan_seed(0, 0));
        let reach = register::reachable(1);
        logs.into_iter()
            .filter_map(|log| {
                let report = baseline::run_one(&built, plan.epochs, &log, &reach);
                if !report
                    .findings
                    .iter()
                    .any(|f| matches!(f, Finding::Agreement(_)))
                {
                    return None;
                }
                let journal = run(&built.programs, &log, &baseline::config()).expect("runs");
                let recorded = RecordedRun::new(
                    snapshot_id(&base),
                    encode_schedule(&schedule_of(&plan, &built, &log)),
                    journal.encode().expect("encodes"),
                    failure_of(&report.findings).expect("fails"),
                )
                .expect("a failure");
                Some(Recorded {
                    log,
                    run: recorded,
                    oracle_digest: report.digest,
                })
            })
            .collect()
    })
}

/// The witness: the first failing run.
fn witness() -> &'static Recorded {
    &m01_runs()[0]
}

struct Binding(CrashpackId);

impl FailureBinding for Binding {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        if *failure == self.0 {
            Resolution::Found(snapshot_id(&workspace(REPLICA_BEFORE)))
        } else {
            Resolution::Unknown
        }
    }

    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if *snapshot == snapshot_id(&workspace(REPLICA_BEFORE)) {
            // The intent is not read by gates 1 and 4; this handle stands for the
            // register's Intent Contract.
            Resolution::Found(IntentId::new("in_replicated_register").expect("an in_ handle"))
        } else {
            Resolution::Unknown
        }
    }
}

/// The transaction on `recorded`, applied with the replica replaced by `after`.
fn applied(recorded: &RecordedRun, after: &str) -> Tx {
    let failure = recorded.identity::<Blake3Hasher>();
    let draft = Tx::begin(failure.clone(), GateProfile::PhaseB, &Binding(failure)).expect("begins");
    let proposal = Proposal::new(
        Hypothesis::new(
            "move the ack after the storage sync: the replica publishes its reply only once \
             the write is durable, so an acknowledged write survives a crash",
        ),
        vec![change(after)],
    );
    draft
        .apply(&proposal, &workspace(REPLICA_BEFORE))
        .expect("applies")
        .into_parts()
        .0
}

/// With no policy authorization: under the fail-closed [`EvaluationPolicy::Exact`],
/// the only policy a caller outside `continuum-repair` can reach (bn-2vanm, bn-b6u4).
fn evaluate(
    recorded: &RecordedRun,
    after: &str,
) -> Result<ExactReplay<Blake3Hasher>, ExactReplayRefusal> {
    replay::evaluate(
        &applied(recorded, after),
        &recorded.canonical_bytes(),
        &workspace(REPLICA_BEFORE),
        &workspace(after),
        &RegisterReplayer::new(),
        None,
    )
}

fn gate_line(result: &ExactReplay<Blake3Hasher>, name: GateName) -> String {
    let gate = result
        .transaction()
        .gates()
        .iter()
        .find(|gate| gate.name() == name)
        .expect("listed");
    let evidence: Vec<&str> = gate.evidence().iter().map(|e| e.as_str()).collect();
    format!(
        "{} {} [{}]",
        name.token(),
        gate.status().token(),
        evidence.join(", ")
    )
}

// ---------------------------------------------------------------------------
// the evidence
// ---------------------------------------------------------------------------

const GOLDEN: &str = "tests/golden/pr20_impl03_exact_replay.evidence.txt";

struct Tally {
    runs: usize,
    outcomes: BTreeMap<String, usize>,
    eliminated: BTreeMap<(u64, u64), usize>,
}

/// Evaluate `after` against every recorded M01 run.
fn campaign(after: &str) -> Tally {
    let mut tally = Tally {
        runs: 0,
        outcomes: BTreeMap::new(),
        eliminated: BTreeMap::new(),
    };
    let replayer = RegisterReplayer::new();
    for recorded in m01_runs() {
        let result = replay::evaluate(
            &applied(&recorded.run, after),
            &recorded.run.canonical_bytes(),
            &workspace(REPLICA_BEFORE),
            &workspace(after),
            &replayer,
            None,
        )
        .expect("evaluates");
        tally.runs += 1;
        let base = match result.base_replay() {
            BaseReplay::Reproduced { .. } => "reproduced",
            BaseReplay::NotReproduced { .. } => "not-reproduced",
            BaseReplay::Inconsistent { .. } => "inconsistent",
            BaseReplay::Diverged { .. } => "diverged",
            BaseReplay::Unsupported { .. } => "unsupported",
        };
        let candidate = match result.exact_regression() {
            ExactRegression::NoLongerFails {
                eliminated,
                reordered,
                ..
            } => {
                *tally
                    .eliminated
                    .entry((*eliminated, *reordered))
                    .or_default() += 1;
                "no-longer-fails"
            }
            ExactRegression::Recurs { .. } => "recurs",
            ExactRegression::FailsDifferently { .. } => "fails-differently",
            ExactRegression::Diverged { .. } => "diverged",
            ExactRegression::NotExact { .. } => "not-exact",
            ExactRegression::Unsupported { .. } => "unsupported",
            ExactRegression::NotRun => "not-run",
        };
        *tally
            .outcomes
            .entry(format!(
                "base_replay {} ({base}) / exact_regression {} ({candidate})",
                result.base_replay().status().token(),
                result.exact_regression().status().token()
            ))
            .or_default() += 1;
    }
    tally
}

fn render_tally(out: &mut String, tally: &Tally) {
    let _ = writeln!(out, "  runs: {}", tally.runs);
    for (outcome, n) in &tally.outcomes {
        let _ = writeln!(out, "  {n}: {outcome}");
    }
    if !tally.eliminated.is_empty() {
        let spread: Vec<String> = tally
            .eliminated
            .iter()
            .map(|((e, r), n)| format!("{e} eliminated and {r} reordered x{n}"))
            .collect();
        let _ = writeln!(out, "  correspondence: {}", spread.join(", "));
    }
}

fn render_single(out: &mut String, id: &str, what: &str, result: &ExactReplay<Blake3Hasher>) {
    let _ = writeln!(out, "{id}: {what}");
    let tx = result.transaction();
    let _ = writeln!(
        out,
        "  transaction: {} v{} supersedes {} status {}",
        tx.repair_id(),
        tx.version(),
        tx.supersedes().map_or("-", |id| id.as_str()),
        tx.status().token()
    );
    let _ = writeln!(out, "  failure: {}", tx.failure());
    let _ = writeln!(out, "  {}", gate_line(result, GateName::BaseReplay));
    let _ = writeln!(out, "  {}", gate_line(result, GateName::ExactRegression));
    let _ = writeln!(out, "  base: {:?}", result.base_replay());
    let _ = writeln!(out, "  candidate: {:?}", result.exact_regression());
}

/// The rendered evidence, and each fixture's path and bytes.
type Evidence = (String, Vec<(&'static str, Vec<u8>)>);

fn evidence() -> &'static Evidence {
    static CELL: OnceLock<Evidence> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut out = String::new();
        let mut fixtures = Vec::new();
        let _ = writeln!(
            out,
            "# PR-20 / IMPL-03 exact replay on M01 (bn-2pla). Regenerate: PR20_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test pr20_impl03_exact_replay"
        );
        let w = witness();
        let _ = writeln!(
            out,
            "witness: first of {} M01 runs failing Agreement on plan {}; log {}; {} recorded steps; crashpack {}",
            m01_runs().len(),
            baseline::SCENARIO_NAME,
            w.log,
            std::str::from_utf8(w.run.schedule()).expect("utf-8").lines().count(),
            w.run.identity::<Blake3Hasher>()
        );

        let fix = evaluate(&w.run, REPLICA_AFTER).expect("evaluates");
        render_single(&mut out, "pr20-impl03-pos-01-m01-ack-after-sync", "M01 replays exactly on the base; the ack-after-sync candidate does not fail under the original choices, but it eliminates and reorders recorded steps, so under the exact policy (no authority for a relaxed one: bn-2vanm, bn-b6u4) gate 4 is inconclusive, never passed", &fix);
        fixtures.push(("tests/golden/pr20-impl03-m01-ack-after-sync-evaluated.json", fix.transaction().to_artifact_bytes()));

        let _ = writeln!(out, "pr20-impl03-pos-02-every-m01-run: every failing M01 run, each its own crashpack, against the ack-after-sync candidate");
        render_tally(&mut out, &campaign(REPLICA_AFTER));
        let _ = writeln!(out, "  relaxed policy: {:?}", replay::request_continue_and_disclose(fix.transaction()));

        let nofix = evaluate(&w.run, REPLICA_COMMENTED).expect("evaluates");
        render_single(&mut out, "pr20-impl03-neg-01-repair-that-does-not-fix", "a candidate that only comments the replica: the failure recurs", &nofix);
        render_tally(&mut out, &campaign(REPLICA_COMMENTED));
        fixtures.push(("tests/golden/pr20-impl03-m01-no-fix-evaluated.json", nofix.transaction().to_artifact_bytes()));

        let _ = writeln!(out, "pr20-impl03-neg-02-tampered-crashpack: every single-byte change of the witness crashpack is refused CrashpackTampered; {} bytes", w.run.canonical_bytes().len());

        let stale = stale_binding();
        render_single(&mut out, "pr20-impl03-neg-03-stale-failure-binding", "a crashpack recording a failure the base does not produce under its schedule", &stale);
        let inconsistent = inconsistent_record();
        render_single(&mut out, "pr20-impl03-bnd-03-inconsistent-record", "a crashpack recording the base's failure with another run's journal: an engine condition", &inconsistent);

        let diverged = evaluate(&w.run, REPLICA_DOUBLE_ACK).expect("evaluates");
        render_single(&mut out, "pr20-impl03-bnd-01-divergence", "a candidate that acks before and after the sync: steps the recording has no choice for", &diverged);
        render_tally(&mut out, &campaign(REPLICA_DOUBLE_ACK));
        fixtures.push(("tests/golden/pr20-impl03-m01-double-ack-evaluated.json", diverged.transaction().to_artifact_bytes()));

        let unsupported = evaluate(&w.run, REPLICA_FLUSH).expect("evaluates");
        render_single(&mut out, "pr20-impl03-bnd-02-unsupported", "a replica statement the replayer cannot read", &unsupported);
        (out, fixtures)
    })
}

/// A genuine crashpack for the witness's schedule and journal that records a failure
/// identity the base does not produce: the failure binding is stale.
fn stale_binding() -> ExactReplay<Blake3Hasher> {
    let w = witness();
    let stale = RecordedRun::new(
        w.run.base_snapshot().clone(),
        w.run.schedule().to_vec(),
        w.run.journal().to_vec(),
        b"Agreement(0)".to_vec(),
    )
    .expect("a failure");
    assert_ne!(stale.failure(), w.run.failure());
    evaluate(&stale, REPLICA_AFTER).expect("evaluates")
}

/// A genuine crashpack for the witness's schedule and failure whose journal is another
/// run's: the failure reproduces and the run does not, which on a verified base only an
/// inconsistent recorder or replayer can explain.
fn inconsistent_record() -> ExactReplay<Blake3Hasher> {
    let (w, other) = (witness(), &m01_runs()[1]);
    assert_ne!(w.run.journal(), other.run.journal());
    let record = RecordedRun::new(
        w.run.base_snapshot().clone(),
        w.run.schedule().to_vec(),
        other.run.journal().to_vec(),
        w.run.failure().to_vec(),
    )
    .expect("a failure");
    evaluate(&record, REPLICA_AFTER).expect("evaluates")
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn pr20_impl03_the_evidence_matches_its_golden() {
    let (text, fixtures) = evidence();
    if std::env::var_os("PR20_IMPL03_BLESS").is_some() {
        std::fs::write(GOLDEN, text).expect("writes the golden");
        for (path, bytes) in fixtures {
            let mut bytes = bytes.clone();
            bytes.push(b'\n');
            std::fs::write(path, bytes).expect("writes a fixture");
        }
        panic!(
            "blessed {GOLDEN} and {} fixtures: review the diff and rerun",
            fixtures.len()
        );
    }
    let golden = std::fs::read_to_string(GOLDEN).expect("the golden exists");
    for (i, (want, got)) in golden.lines().zip(text.lines()).enumerate() {
        assert_eq!(
            want,
            got,
            "line {} drifted; regenerate with PR20_IMPL03_BLESS=1 and review",
            i + 1
        );
    }
    assert_eq!(
        golden, *text,
        "the evidence drifted; regenerate with PR20_IMPL03_BLESS=1"
    );
    for (path, bytes) in fixtures {
        let mut want = bytes.clone();
        want.push(b'\n');
        assert_eq!(
            std::fs::read(path).expect("the fixture exists"),
            want,
            "{path}"
        );
    }
}

/// The replica text is the program: the base is PR 16's M01 plan and the ack-after-sync
/// candidate is the correct plan, act for act.
#[test]
fn pr20_impl03_the_workspace_programs_are_pr16s_plans() {
    let render = |p: &Plan| baseline::render_plan(p);
    let correct = baseline::scenario_plan();
    assert_eq!(
        render(&program_of(&workspace(REPLICA_BEFORE)).unwrap()),
        render(&mutants::ack_before_sync(&correct))
    );
    assert_eq!(
        render(&program_of(&workspace(REPLICA_AFTER)).unwrap()),
        render(&correct)
    );
    assert_eq!(
        render(&program_of(&workspace(REPLICA_COMMENTED)).unwrap()),
        render(&mutants::ack_before_sync(&correct)),
        "a comment is not a program change"
    );
    assert!(program_of(&workspace(REPLICA_FLUSH)).is_none());
}

/// Positive: M01's failure reproduces exactly on the base (gate 1 passes), and the
/// ack-after-sync candidate does not fail under the original choices. The candidate
/// eliminates the early confirmation and reorders recorded steps, so under the exact
/// policy, the only one reachable without an authority, gate 4 is `NotExact`:
/// inconclusive, never passed. `NotExact` arises only from a run with no failure.
#[test]
fn pr20_impl03_m01_reproduces_on_the_base_and_not_on_the_ack_after_sync_candidate() {
    let w = witness();
    let result = evaluate(&w.run, REPLICA_AFTER).unwrap();
    assert_eq!(
        result.transaction().evaluation_policy(),
        Some(EvaluationPolicy::Exact.token())
    );
    assert!(
        matches!(result.base_replay(), BaseReplay::Reproduced { .. }),
        "{:?}",
        result.base_replay()
    );
    let ExactRegression::NotExact { eliminated, .. } = result.exact_regression() else {
        panic!("{:?}", result.exact_regression());
    };
    assert!(
        *eliminated > 0,
        "the repair eliminates the early confirmation, and says so"
    );
    for (name, status) in [
        (GateName::BaseReplay, GateStatus::Passed),
        (GateName::ExactRegression, GateStatus::Inconclusive),
    ] {
        let gate = result
            .transaction()
            .gates()
            .iter()
            .find(|g| g.name() == name)
            .unwrap();
        assert_eq!(gate.status(), status);
        assert_eq!(gate.evidence().len(), 2);
        assert_eq!(
            gate.evidence()[0].as_str(),
            w.run.identity::<Blake3Hasher>().as_str()
        );
        assert!(gate.evidence()[1].as_str().starts_with("ev_"));
    }
    assert_eq!(result.transaction().status(), TransactionStatus::Evaluating);
    // The candidate's realized log, run through PR 16's `run_one` directly, has no
    // finding. This re-runs the replayer's own realization, so it checks the gate's
    // reading of the run, not the correspondence.
    let text = std::str::from_utf8(w.run.schedule()).unwrap();
    let plan = program_of(&workspace(REPLICA_AFTER)).unwrap();
    let built = register::build_with_shutdown(&plan);
    let labels = label(&plan, &built);
    let rank: BTreeMap<&str, usize> = text.lines().enumerate().map(|(i, l)| (l, i)).collect();
    let (log, _) = register::guided_run(&built, |a, i| {
        labels[a][i]
            .as_deref()
            .map(|l| rank.get(l).copied().unwrap_or(usize::MAX - 1))
    })
    .unwrap();
    let report = baseline::run_one(&built, 1, &log, &register::reachable(1));
    assert!(report.findings.is_empty(), "{:?}", report.findings);
}

/// Differential, oracle `continuum_asupersync` (PR 16's `run_one` over the binding),
/// subject `continuum_repair::replay`: over every failing M01 run, the gate's base
/// replay reproduces exactly the run the PR-16 oracle recorded (the realized log is the
/// original log, and the journal digests agree), and the candidate passes.
#[test]
fn pr20_impl03_every_m01_run_agrees_with_the_pr16_oracle() {
    let runs = m01_runs();
    assert!(
        runs.len() >= 100,
        "the campaign yields the failing runs: {}",
        runs.len()
    );
    let base = workspace(REPLICA_BEFORE);
    let plan = program_of(&base).unwrap();
    let built = register::build_with_shutdown(&plan);
    let labels = label(&plan, &built);
    for recorded in runs {
        let text = std::str::from_utf8(recorded.run.schedule()).unwrap();
        let rank: BTreeMap<&str, usize> = text.lines().enumerate().map(|(i, l)| (l, i)).collect();
        let (log, _) =
            register::guided_run(&built, |a, i| labels[a][i].as_deref().map(|l| rank[l])).unwrap();
        assert_eq!(
            log, recorded.log,
            "the base realizes the recorded log, choice for choice"
        );
        let journal =
            continuum_asupersync::journal::Journal::decode(recorded.run.journal()).unwrap();
        assert_eq!(
            journal.digest().unwrap().to_string(),
            recorded.oracle_digest
        );
    }
    let tally = campaign(REPLICA_AFTER);
    assert_eq!(tally.runs, runs.len());
    assert_eq!(
        tally.outcomes.keys().collect::<Vec<_>>(),
        ["base_replay passed (reproduced) / exact_regression inconclusive (not-exact)"]
    );
}

/// Negative: under the fail-closed exact policy the ack-after-sync candidate, which
/// eliminates and reorders recorded steps, is not passed on any failing run: its gate 4
/// is inconclusive `AbstractionAmbiguity`.
#[test]
fn pr20_impl03_the_exact_policy_refuses_an_eliminated_or_reordered_run() {
    let result = evaluate(&witness().run, REPLICA_AFTER).unwrap();
    let ExactRegression::NotExact {
        eliminated,
        reordered,
        ..
    } = result.exact_regression()
    else {
        panic!("{:?}", result.exact_regression());
    };
    assert!(*eliminated > 0 && *reordered > 0);
    assert_eq!(result.exact_regression().status(), GateStatus::Inconclusive);
    assert_eq!(
        result.transaction().evaluation_policy(),
        Some(EvaluationPolicy::Exact.token())
    );
    let tally = campaign(REPLICA_AFTER);
    assert_eq!(
        tally.outcomes.keys().collect::<Vec<_>>(),
        ["base_replay passed (reproduced) / exact_regression inconclusive (not-exact)"]
    );
    // No caller can relax the policy: the seam that would issue an authorization has no
    // authority.
    assert_eq!(
        replay::request_continue_and_disclose(result.transaction()),
        Err(replay::AuthorizationAbsence::NoAuthority)
    );
}

/// Negative: a repair that does not fix M01 fails gate 4, on every failing run.
#[test]
fn pr20_impl03_a_repair_that_does_not_fix_fails_gate_4() {
    let result = evaluate(&witness().run, REPLICA_COMMENTED).unwrap();
    assert_eq!(result.base_replay().status(), GateStatus::Passed);
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::Recurs { .. }
    ));
    assert_eq!(result.exact_regression().status(), GateStatus::Failed);
    let tally = campaign(REPLICA_COMMENTED);
    assert_eq!(
        tally.outcomes.keys().collect::<Vec<_>>(),
        ["base_replay passed (reproduced) / exact_regression failed (recurs)"]
    );
}

/// Negative: a tampered crashpack is refused and nothing is replayed.
#[test]
fn pr20_impl03_a_tampered_crashpack_is_refused() {
    struct Never;
    impl Replayer for Never {
        fn identity(&self) -> &str {
            "never/1"
        }

        fn replay(&self, _: &WorkspaceContent, _: &[u8]) -> ReplayOutcome {
            panic!("a tampered crashpack is never replayed")
        }
    }
    let w = witness();
    let transaction = applied(&w.run, REPLICA_AFTER);
    let genuine = w.run.canonical_bytes();
    for index in 0..genuine.len() {
        let mut bytes = genuine.clone();
        bytes[index] = bytes[index].wrapping_add(1);
        assert_eq!(
            replay::evaluate(
                &transaction,
                &bytes,
                &workspace(REPLICA_BEFORE),
                &workspace(REPLICA_AFTER),
                &Never,
                None,
            )
            .map(|_| ()),
            Err(ExactReplayRefusal::CrashpackTampered {
                expected: transaction.failure().clone()
            }),
            "byte {index}"
        );
    }
}

/// Negative: a crashpack whose recorded failure the base does not produce fails gate 1
/// (the failure binding is stale), and gate 4 is not run and stays pending.
#[test]
fn pr20_impl03_a_stale_failure_binding_fails_gate_1() {
    let result = stale_binding();
    let BaseReplay::NotReproduced {
        journal_differs,
        failure,
        ..
    } = result.base_replay()
    else {
        panic!("{:?}", result.base_replay());
    };
    assert!(!*journal_differs);
    assert_eq!(*failure, replay::FailureMatch::Different);
    assert_eq!(result.base_replay().status(), GateStatus::Failed);
    assert_eq!(*result.exact_regression(), ExactRegression::NotRun);
    let gate4 = result
        .transaction()
        .gates()
        .iter()
        .find(|g| g.name() == GateName::ExactRegression)
        .unwrap();
    assert_eq!(gate4.status(), GateStatus::Pending);
}

/// Boundary: the failure reproduces but the recorded journal does not: an engine
/// condition, inconclusive with a `defect_`, never failed and never passed.
#[test]
fn pr20_impl03_an_inconsistent_record_is_an_engine_condition() {
    let result = inconsistent_record();
    let BaseReplay::Inconsistent {
        defect,
        journal_differs,
        ..
    } = result.base_replay()
    else {
        panic!("{:?}", result.base_replay());
    };
    assert!(*journal_differs);
    assert!(defect.as_str().starts_with("defect_"));
    assert_eq!(result.base_replay().status(), GateStatus::Inconclusive);
    assert_eq!(*result.exact_regression(), ExactRegression::NotRun);
}

/// Boundary: a candidate the recorded choices do not determine is inconclusive, never
/// passed, on every failing run.
#[test]
fn pr20_impl03_a_divergence_is_inconclusive() {
    let result = evaluate(&witness().run, REPLICA_DOUBLE_ACK).unwrap();
    assert!(matches!(
        result.exact_regression(),
        ExactRegression::Diverged { .. }
    ));
    assert_eq!(result.exact_regression().status(), GateStatus::Inconclusive);
    let tally = campaign(REPLICA_DOUBLE_ACK);
    assert_eq!(
        tally.outcomes.keys().collect::<Vec<_>>(),
        ["base_replay passed (reproduced) / exact_regression inconclusive (diverged)"]
    );
    let result = evaluate(&witness().run, REPLICA_FLUSH).unwrap();
    let ExactRegression::Unsupported { record } = result.exact_regression() else {
        panic!("{:?}", result.exact_regression());
    };
    let bytes = result
        .records()
        .iter()
        .find(|r| r.handle() == record)
        .expect("the cited record")
        .bytes();
    for part in [
        RegisterReplayer::new().identity(),
        "Unsupported",
        EvaluationPolicy::Exact.token(),
    ] {
        assert!(
            bytes.windows(part.len()).any(|w| w == part.as_bytes()),
            "the record binds {part}"
        );
    }
}

/// Boundary: the evaluation is deterministic: two evaluations give the same bytes.
#[test]
fn pr20_impl03_the_evaluation_is_deterministic() {
    let w = witness();
    let a = evaluate(&w.run, REPLICA_AFTER).unwrap();
    let b = evaluate(&w.run, REPLICA_AFTER).unwrap();
    assert_eq!(
        a.transaction().to_artifact_bytes(),
        b.transaction().to_artifact_bytes()
    );
    assert_eq!(a.records(), b.records());
}
