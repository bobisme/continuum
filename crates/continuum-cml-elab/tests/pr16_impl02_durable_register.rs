//! PR-16/IMPL-02: the operational durable register, checked by the reference engine and
//! related step by step to the abstract atomic register (bn-2rsp, START_HERE PR 16
//! second bullet).
//!
//! # The subject
//!
//! `notes/plan/examples/durable_register.ctm`: three replicas, each with one log slot
//! per epoch. A write takes a permit (`Reserve`), puts its bytes in the volatile log
//! (`Submit`), and makes them durable (`Sync`). A cancelled writer releases its permit
//! (`Abort`). A crash loses in-flight writes (`Lose`) and never a durable record. `Ack`
//! publishes an acknowledgement once a quorum holds the value durably
//! (`notes/plan/examples/replicated_register.md`, "Service contract" and "Operational
//! protocol"). It abstracts that protocol to a storage-level voting register: there is
//! no coordinator, no `Prepare`/`Prepared(prior)`/`Commit` message, no `current_epoch`,
//! and no process epoch, and `Ack` reads replica durability directly. `Agreement` rests
//! on two things only: a replica slot takes one value (`Reserve` and `Submit` need an
//! empty slot) and every two configured quorums intersect (`bnd-04`). The engine checks
//! three invariants: `Agreement` (the scenario's
//! `abstract_register::Agreement`, read through the refinement map), `AckedIsDurable`
//! (every acknowledgement is durable on a quorum), and `OneValuePerSlot`.
//!
//! # Acceptance claim A
//!
//! `replicated_register.md` claim A is exact finite exploration over Nodes = 3,
//! Values = 2, Epochs = 2, crashes ≤ 2. The committed run configuration is that scope,
//! and the engine establishes all three invariants over its complete reachable set,
//! 61,504 states. Two facts about the scope, stated so no one takes more from it:
//!
//! - Crash is not counted. `Lose(n, e, v)` loses one in-flight write, so the model has
//!   every behaviour of atomic crash-and-restart with any number of crashes, which
//!   contains crashes ≤ 2. A counted crash is a typed refusal at this scope: the counter
//!   triples the init domain past `MAX_INIT_ENUMERATION` (`bnd-02`). The literal
//!   claim-A set — atomic crashes of a whole replica, at most two, built from the
//!   engine's own steps — is computed here and is the established set
//!   ([`atomic_crash_reachable`]). At one epoch, where `Lose` is the atomic crash, the
//!   counted model lowers and the engine establishes it directly (`pos-03`).
//! - The init domain is exactly `2^22 = MAX_INIT_ENUMERATION` flat states, and its
//!   enumeration costs more work than `Limits::default()`. The claim-A lowering runs
//!   under the explicit budget [`CLAIM_A_LIMITS`]; the default budget is a typed
//!   refusal before any state is enumerated (`bnd-01`).
//!
//! Crash adds no reachable state to the correct register: every state is reachable
//! with no crash, because a slot keeps no history and an acknowledgement depends only
//! on present durability. So "any number of crashes" is true of the invariants but not
//! where crash is exercised. Crash adds transitions, which the step checks cover, and
//! it is what the M01 and `neg-02` refutations need. Deadlock freedom holds at this
//! scope because two values over three nodes always leave a durable majority once
//! every slot is synced, and a repeated `Ack` is a successor; three values would break
//! it.
//!
//! Because each `Lose` touches one slot, the two epochs are independent components:
//! the reachable set is the product of two one-epoch registers (248² = 61,504). The
//! epochs couple only through an atomic crash, which the literal set above covers.
//!
//! # Step correspondence, not invariants on projections
//!
//! The refinement map sends `acks` to the abstract `chosen` and sets `history` to its
//! graph. On such a projected state the abstract invariants are tautologies (bn-3e3v),
//! so [`correspondence`] checks steps: every engine step at every reachable state is,
//! projected, a stutter, or — for `Ack(e, v)` only — a step of the lowered abstract
//! register labelled `Choose(e, v)`. That is acceptance claim C over this model. It is
//! a test-level check, not the PR-17 refinement checker. `neg-05` is a mutant that
//! every invariant accepts and only this check refutes.
//!
//! # The evidence, by stable artifact ID
//!
//! Every scenario renders into `tests/golden/pr16_impl02_durable_register.evidence.txt`,
//! compared byte for byte; the run identity of the committed configuration is
//! `tests/golden/durable_register.run-identity.hex`. Regenerate both with
//! `CML_BLESS=1 cargo test -p continuum-cml-elab --test pr16_impl02_durable_register`,
//! and review the diff.
//!
//! - **positive** `pr16-impl02-pos-01-claim-a` (the committed configuration),
//!   `pos-02-one-epoch`, `pos-03-one-epoch-counted-crashes` (crashes ≤ 2, counted);
//! - **negative** seeded bugs, each a one-place edit of the model text:
//!   `neg-01-ack-before-sync` (M01; its witnesses also replay at the claim-A scope),
//!   `neg-02-lose-durable`, `neg-03-double-vote`, and `neg-04-one-replica-quorum` (a
//!   stronger bug than M05) are refuted by invariants with shortest witnesses that
//!   replay; `neg-05-retract` and `neg-06-release-without-sync` (the cancellation-cleanup
//!   class of M02) pass every invariant and are refuted only by the step checks, at the
//!   failing steps, with no witness path;
//! - **boundary** `bnd-01`…`05`: the default work budget and a counted crash are
//!   typed refusals at the claim-A scope; without crashes, ack-before-sync keeps
//!   `Agreement`; disjoint quorums break it; a depth bound is inconclusive for a hold.
//!
//! Not modelled here, so they stay with IMPL-05 or later: lost-abort as M02 states it
//! (a cancelled sender that loses a reserved message) needs writer tasks; M03 and M08
//! need process epochs and timers; M04 needs `Commit` messages; M05 as stated (one
//! replica counted twice across a restart) needs restarts with identity; M06 needs
//! task regions; M07 needs checksums and torn tails; M09 and M10 are about views and
//! independence rules, which PR 17 and the DPOR engine own.
//!
//! # Independence
//!
//! [`Spec`] is an independent re-implementation of the slot protocol over `BTreeMap`
//! slot states, with no code shared with the elaborator or the lowering. The
//! differential shows the lowered model's reachable states and labelled transitions
//! are the slot protocol's. Its design is not independent of the model: it follows the
//! same four slot states, and it copies one encoding detail, that `Lose` of a reserved
//! slot is enabled under every value label.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock};

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{
    Configured, MAX_INIT_ENUMERATION, identity_encodings_on_this_thread, lower_configured,
};
use continuum_cml_elab::{Limits, LowerErrorKind, Unlowerable, elaborate_source};
use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::checking::{
    self, CheckOutcome, CheckReport, DeadlockOutcome, DeadlockPolicy, Evidence, Obligations,
    Unresolved,
};
use continuum_model_core::{Model, State, Step};

// ---------------------------------------------------------------------------
// the subject
// ---------------------------------------------------------------------------

/// The explicit work budget of the claim-A lowering: `2^34` units. The enumeration of
/// its `2^22` init candidates needs about `1.26 * 2^33`, five times the default
/// `2^31`.
const CLAIM_A_LIMITS: Limits = Limits {
    nodes: continuum_cml_elab::budget::MAX_NODES,
    work: 1 << 34,
};

/// The committed configuration's scope, in states.
const CLAIM_A_STATES: usize = 61_504;

const NODES: [&str; 3] = ["a", "b", "c"];
const MAJORITY: &[&[&str]] = &[&["a", "b"], &["a", "c"], &["b", "c"]];

fn dossier(rel: &str) -> String {
    let path = format!("{}/../../notes/plan/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The model as committed.
fn source() -> String {
    dossier("examples/durable_register.ctm")
}

/// The committed run configuration: claim A's Nodes = 3, Values = 2, Epochs = 2.
fn committed_config() -> String {
    dossier("examples/durable_register.run-config.json")
}

fn elem(sort: &str, name: &str) -> String {
    format!(r#"{{"elem":{{"sort":"{sort}","name":"{name}"}}}}"#)
}

/// A run configuration: nodes `a b c`, `Nat` bounded to `0..=nat_max` (the epochs),
/// the given values and quorums.
fn config(nat_max: u32, values: &[&str], quorums: &[&[&str]]) -> String {
    let quote = |v: &&str| format!("\"{v}\"");
    let nodes: Vec<String> = NODES.iter().map(quote).collect();
    let values: Vec<String> = values.iter().map(quote).collect();
    let quorums: Vec<String> = quorums
        .iter()
        .map(|q| {
            let members: Vec<String> = q.iter().map(|n| elem("Node", n)).collect();
            format!(r#"{{"set":[{}]}}"#, members.join(","))
        })
        .collect();
    format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"DurableRegister","bounds":{{"Nat":{{"max":{nat_max}}}}},"sorts":{{"Node":{{"elements":[{}]}},"Value":{{"elements":[{}]}}}},"constants":{{"Quorum":{{"set":[{}]}}}}}}"#,
        nodes.join(","),
        values.join(","),
        quorums.join(",")
    )
}

/// One epoch, two values, majority quorums: one independent component of claim A.
fn one_epoch() -> String {
    config(0, &["v0", "v1"], MAJORITY)
}

/// The abstract register's run configuration at the same epochs and values.
fn abstract_config(nat_max: u32, values: &[&str]) -> String {
    let elements: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
    format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"AbstractRegister","bounds":{{"Nat":{{"max":{nat_max}}}}},"sorts":{{"Value":{{"elements":[{}]}}}},"constants":{{}}}}"#,
        elements.join(",")
    )
}

fn lower_with(src: &str, config: &str, limits: Limits) -> Result<Configured, LowerErrorKind> {
    let model = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    let config = RunConfig::parse(config.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"));
    lower_configured(&model, &config, limits)
        .0
        .map_err(|e| e.kind)
}

type Lowered = Arc<Result<Model, LowerErrorKind>>;

/// Lower once per source, configuration, and budget: the claim-A scope enumerates
/// `2^22` init candidates, and several tests read the same models.
fn lowered_cached(src: &str, config: &str, limits: Limits) -> Lowered {
    type Key = (String, String, usize, u64);
    static CACHE: OnceLock<Mutex<BTreeMap<Key, Arc<OnceLock<Lowered>>>>> = OnceLock::new();
    let key = (src.to_owned(), config.to_owned(), limits.nodes, limits.work);
    let cell = Arc::clone(
        CACHE
            .get_or_init(Mutex::default)
            .lock()
            .expect("no test panicked holding the cache")
            .entry(key)
            .or_default(),
    );
    Arc::clone(
        cell.get_or_init(|| Arc::new(lower_with(src, config, limits).map(|c| c.model().clone()))),
    )
}

fn lowered(src: &str, config: &str) -> Model {
    lowered_cached(src, config, CLAIM_A_LIMITS)
        .as_ref()
        .clone()
        .unwrap_or_else(|e| panic!("lowers: {e:?}"))
}

fn complete(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates")
}

/// The committed model at the claim-A scope and its complete exploration, built once:
/// its lowering enumerates `2^22` init candidates.
fn claim_a() -> &'static (Model, Exploration) {
    static CELL: OnceLock<(Model, Exploration)> = OnceLock::new();
    CELL.get_or_init(|| {
        let model = lowered(&source(), &committed_config());
        let exploration = complete(&model);
        assert_eq!(exploration.reachable().len(), CLAIM_A_STATES);
        (model, exploration)
    })
}

/// Replace exactly one occurrence of `from` in the model text, or fail: a seeded bug
/// that silently did not apply would make its refutation test vacuous.
fn mutate_text(src: &str, from: &str, to: &str) -> String {
    assert_eq!(
        src.matches(from).count(),
        1,
        "the mutation site {from:?} occurs exactly once"
    );
    let out = src.replacen(from, to, 1);
    assert_ne!(out, src, "the mutation changed the model text");
    out
}

fn mutate(from: &str, to: &str) -> String {
    mutate_text(&source(), from, to)
}

const ACK_GUARD: &str =
    "  require exists q in Quorum: forall n in q: durable(n, epoch, value)\n  next acks";
const LOSE_GUARD: &str =
    "  require (n, epoch) in pending\n  require forall v: (n, epoch, v) in log => v == value\n";
const SUBMIT_GUARD: &str =
    "  require forall v: (n, epoch, v) notin log\n  next log = log union {(n, epoch, value)}\n";
const ACK_END: &str = "  next acks = acks union {(epoch, value)}\n  unchanged log, pending\n}\n";

/// `neg-01`, M01: the coordinator acknowledges once a quorum has the bytes, synced or
/// not.
fn ack_before_sync() -> String {
    mutate(
        ACK_GUARD,
        "  require exists q in Quorum: forall n in q: (n, epoch, value) in log\n  next acks",
    )
}

/// `neg-02`: a crash may lose a synced record — `sync` lies.
fn lose_durable() -> String {
    mutate(
        LOSE_GUARD,
        "  require forall v: (n, epoch, v) in log => v == value\n",
    )
}

/// `neg-03`: a replica takes a second value into a slot that already holds bytes.
fn double_vote() -> String {
    mutate(SUBMIT_GUARD, "  next log = log union {(n, epoch, value)}\n")
}

/// `neg-04`, the shadow of M05: the coordinator counts one replica as a quorum.
fn one_replica_quorum() -> String {
    mutate(
        ACK_GUARD,
        "  require exists n: durable(n, epoch, value)\n  next acks",
    )
}

const ABORT_GUARD: &str =
    "  require forall v: (n, epoch, v) notin log\n  next pending = pending \\ {(n, epoch)}\n";

/// `neg-06`, cancellation cleanup: a writer cancelled after `Submit` releases its
/// permit, and its unsynced bytes stay as if synced.
fn release_without_sync() -> String {
    mutate(ABORT_GUARD, "  next pending = pending \\ {(n, epoch)}\n")
}

/// `neg-05`: an action withdraws a published acknowledgement.
fn retract() -> String {
    mutate(
        ACK_END,
        "  next acks = acks union {(epoch, value)}\n  unchanged log, pending\n}\n\naction Retract(epoch: Nat, value: Value) {\n  next acks = acks \\ {(epoch, value)}\n  unchanged log, pending\n}\n",
    )
}

/// The `Lose` action and its comment, whole.
fn lose_action(src: &str) -> String {
    let start = src
        .find("// A crash of replica `n` loses its in-flight write")
        .expect("Lose comment");
    let end = src
        .find("// The coordinator acknowledges")
        .expect("Ack comment");
    src[start..end].to_owned()
}

/// `bnd-03`: a model text without crashes.
fn without_crash(src: &str) -> String {
    let lose = lose_action(src);
    mutate_text(src, &lose, "")
}

/// `bnd-02` and `pos-03`: crash counted, at most two, with `Lose` the counted fault.
/// The counter is a `Nat` up to 2, so the epochs become a sort of their own and the
/// configuration ([`counted_config`]) bounds `Nat` to `0..=2`.
fn counted_crashes() -> String {
    let src = source().replace("epoch: Nat", "epoch: Epoch");
    let src = mutate_text(&src, "type Value\n", "type Value\ntype Epoch\n");
    let src = src
        .replace("Set[(Node, Nat, Value)]", "Set[(Node, Epoch, Value)]")
        .replace("Set[(Node, Nat)]", "Set[(Node, Epoch)]")
        .replace("Set[(Nat, Value)]", "Set[(Epoch, Value)]");
    assert!(!src.contains("Nat"), "every epoch is an Epoch");
    let src = mutate_text(
        &src,
        "  acks: Set[(Epoch, Value)]\n}",
        "  acks: Set[(Epoch, Value)]\n  crashes: Nat where crashes <= 2\n}",
    );
    let src = mutate_text(&src, "  acks = {}\n}", "  acks = {}\n  crashes = 0\n}");
    let src = mutate_text(
        &src,
        "  next log = log \\ {(n, epoch, value)}\n  unchanged acks\n",
        "  require crashes < 2\n  next log = log \\ {(n, epoch, value)}\n  next crashes = crashes + 1\n  unchanged acks\n",
    );
    let before = src.matches("unchanged ").count();
    let out = src
        .replace(
            "  unchanged log, acks\n",
            "  unchanged crashes, log, acks\n",
        )
        .replace(
            "  unchanged pending, acks\n",
            "  unchanged crashes, pending, acks\n",
        )
        .replace(
            "  unchanged log, pending\n",
            "  unchanged crashes, log, pending\n",
        );
    assert_eq!(
        out.matches("unchanged crashes, ").count(),
        before - 1,
        "every action but Lose"
    );
    out
}

/// The counted model's configuration: `Nat` is the counter's `0..=2`, and the epochs
/// are the named elements of the sort `Epoch`.
fn counted_config(epochs: &[&str]) -> String {
    let elements: Vec<String> = epochs.iter().map(|e| format!("\"{e}\"")).collect();
    let out = config(0, &["v0", "v1"], MAJORITY)
        .replace(
            r#""bounds":{"Nat":{"max":0}}"#,
            r#""bounds":{"Nat":{"max":2}}"#,
        )
        .replace(
            r#""sorts":{"#,
            &format!(
                r#""sorts":{{"Epoch":{{"elements":[{}]}},"#,
                elements.join(",")
            ),
        );
    assert!(out.contains("Epoch") && out.contains(r#""max":2"#));
    out
}

// ---------------------------------------------------------------------------
// decoding the flat layout
// ---------------------------------------------------------------------------

/// A concrete state as sets, by position: node, epoch, value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Raw {
    log: BTreeSet<(u8, u8, u8)>,
    pending: BTreeSet<(u8, u8)>,
    acks: BTreeSet<(u8, u8)>,
}

impl Raw {
    fn durable(&self) -> BTreeSet<(u8, u8, u8)> {
        self.log
            .iter()
            .filter(|(n, e, _)| !self.pending.contains(&(*n, *e)))
            .copied()
            .collect()
    }
}

/// The epochs and values of a lowered model, read from its `acks` slots.
fn shape(model: &Model) -> (u8, Vec<String>) {
    let mut epochs = BTreeSet::new();
    let mut values = Vec::new();
    for v in model.variables() {
        let name = v.name().to_string();
        if let Some(inner) = name
            .strip_prefix("acks{(")
            .and_then(|s| s.strip_suffix(")}"))
        {
            let (e, value) = inner.split_once(',').expect("(epoch,value)");
            epochs.insert(e.to_owned());
            if !values.contains(&value.to_owned()) {
                values.push(value.to_owned());
            }
        }
    }
    (u8::try_from(epochs.len()).expect("few epochs"), values)
}

fn decode(model: &Model, state: &State) -> Raw {
    let (epochs, values) = shape(model);
    let get = |name: String| {
        model
            .binding(state, &name)
            .unwrap_or_else(|| panic!("slot {name} is declared"))
            == 1
    };
    let mut raw = Raw {
        log: BTreeSet::new(),
        pending: BTreeSet::new(),
        acks: BTreeSet::new(),
    };
    for e in 0..epochs {
        for (vi, v) in values.iter().enumerate() {
            let vi = u8::try_from(vi).expect("few values");
            if get(format!("acks{{({e},{v})}}")) {
                raw.acks.insert((e, vi));
            }
            for (ni, n) in NODES.iter().enumerate() {
                let ni = u8::try_from(ni).expect("three nodes");
                if get(format!("log{{({n},{e},{v})}}")) {
                    raw.log.insert((ni, e, vi));
                }
            }
        }
        for (ni, n) in NODES.iter().enumerate() {
            if get(format!("pending{{({n},{e})}}")) {
                raw.pending
                    .insert((u8::try_from(ni).expect("three nodes"), e));
            }
        }
    }
    raw
}

fn label(model: &Model, step: &Step) -> String {
    model.actions()[step.action()].name().to_string()
}

// ---------------------------------------------------------------------------
// the oracle: the prose protocol, independently
// ---------------------------------------------------------------------------

/// One replica's log slot for one epoch, as `replicated_register.md` and the storage
/// strata of RFC 0007 describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Free,
    Reserved,
    Volatile(u8),
    Durable(u8),
}

/// The protocol over three replicas `a b c`, majority quorums, `epochs` epochs, and
/// `values` values, by position.
struct Spec {
    epochs: u8,
    values: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Protocol {
    slots: BTreeMap<(u8, u8), Slot>,
    acks: BTreeSet<(u8, u8)>,
}

impl Protocol {
    fn raw(&self) -> Raw {
        let mut raw = Raw {
            log: BTreeSet::new(),
            pending: BTreeSet::new(),
            acks: self.acks.clone(),
        };
        for (&(n, e), slot) in &self.slots {
            match *slot {
                Slot::Free => {}
                Slot::Reserved => {
                    raw.pending.insert((n, e));
                }
                Slot::Volatile(v) => {
                    raw.pending.insert((n, e));
                    raw.log.insert((n, e, v));
                }
                Slot::Durable(v) => {
                    raw.log.insert((n, e, v));
                }
            }
        }
        raw
    }
}

impl Spec {
    fn init(&self) -> Protocol {
        let mut slots = BTreeMap::new();
        for n in 0..3 {
            for e in 0..self.epochs {
                slots.insert((n, e), Slot::Free);
            }
        }
        Protocol {
            slots,
            acks: BTreeSet::new(),
        }
    }

    fn step(&self, s: &Protocol) -> BTreeSet<(String, Protocol)> {
        let mut out = BTreeSet::new();
        let with = |n: u8, e: u8, slot: Slot| {
            let mut t = s.clone();
            t.slots.insert((n, e), slot);
            t
        };
        for (&(n, e), &slot) in &s.slots {
            let at = format!("n={},epoch={e}", NODES[usize::from(n)]);
            match slot {
                Slot::Free => {
                    out.insert((format!("Reserve({at})"), with(n, e, Slot::Reserved)));
                }
                Slot::Reserved => {
                    out.insert((format!("Abort({at})"), with(n, e, Slot::Free)));
                    for (v, name) in self.values.iter().enumerate() {
                        let v = u8::try_from(v).expect("few values");
                        out.insert((
                            format!("Submit({at},value={name})"),
                            with(n, e, Slot::Volatile(v)),
                        ));
                        // A crash takes the permit: every value names the empty bytes.
                        out.insert((format!("Lose({at},value={name})"), with(n, e, Slot::Free)));
                    }
                }
                Slot::Volatile(v) => {
                    out.insert((format!("Sync({at})"), with(n, e, Slot::Durable(v))));
                    let name = self.values[usize::from(v)];
                    out.insert((format!("Lose({at},value={name})"), with(n, e, Slot::Free)));
                }
                Slot::Durable(_) => {}
            }
        }
        for e in 0..self.epochs {
            for (v, name) in self.values.iter().enumerate() {
                let v = u8::try_from(v).expect("few values");
                let durable = |n: &str| {
                    let i = u8::try_from(NODES.iter().position(|x| *x == n).expect("node"))
                        .expect("three nodes");
                    s.slots[&(i, e)] == Slot::Durable(v)
                };
                if MAJORITY.iter().any(|q| q.iter().all(|n| durable(n))) {
                    let mut t = s.clone();
                    t.acks.insert((e, v));
                    out.insert((format!("Ack(epoch={e},value={name})"), t));
                }
            }
        }
        out
    }

    fn reachable(&self) -> BTreeSet<Protocol> {
        let mut seen = BTreeSet::from([self.init()]);
        let mut work = vec![self.init()];
        while let Some(s) = work.pop() {
            for (_, t) in self.step(&s) {
                if seen.insert(t.clone()) {
                    work.push(t);
                }
            }
        }
        seen
    }
}

// ---------------------------------------------------------------------------
// step correspondence onto the abstract register
// ---------------------------------------------------------------------------

/// Why one concrete step does not correspond to the abstract register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mismatch {
    /// A step from a state where `acks` gives some epoch two values, so the refinement
    /// map is not defined there: a consequence of an earlier failing step, which is
    /// classified by one of the other two.
    Unprojectable,
    /// A step other than `Ack` changes the projected state.
    NotAStutter,
    /// An `Ack(e, v)` step whose projection is no `Choose(e, v)` step of the abstract
    /// register.
    NotAnEnabledChoose,
}

#[derive(Debug, Default)]
struct Correspondence {
    steps: usize,
    stutters: usize,
    chooses: usize,
    /// The distinct `Choose` labels some concrete step realizes as a real move.
    realized: BTreeSet<String>,
    /// The distinct abstract states some reachable concrete state projects to.
    covered: BTreeSet<State>,
    failures: Vec<(Mismatch, String, Raw, Raw)>,
}

/// The refinement map: `chosen` is `acks` read as a map, `history` its graph.
fn project(abs: &Model, raw: &Raw, values: &[String]) -> Option<State> {
    let mut chosen: BTreeMap<u8, u8> = BTreeMap::new();
    for &(e, v) in &raw.acks {
        if chosen.insert(e, v).is_some() {
            return None;
        }
    }
    let vector: Vec<i64> = abs
        .variables()
        .iter()
        .map(|var| {
            let name = var.name().to_string();
            if let Some(e) = name
                .strip_prefix("chosen[")
                .and_then(|s| s.strip_suffix(']'))
            {
                let e: u8 = e.parse().expect("epoch");
                chosen.get(&e).map_or(0, |v| i64::from(*v) + 1)
            } else {
                let inner = name
                    .strip_prefix("history{(")
                    .and_then(|s| s.strip_suffix(")}"))
                    .expect("history slot");
                let (e, v) = inner.split_once(',').expect("(epoch,value)");
                let e: u8 = e.parse().expect("epoch");
                let v = u8::try_from(values.iter().position(|x| x == v).expect("value"))
                    .expect("few values");
                i64::from(chosen.get(&e) == Some(&v))
            }
        })
        .collect();
    Some(abs.state(&vector).expect("an abstract state"))
}

/// Every engine step at every reachable state of `model`, projected onto the lowered
/// abstract register at the same epochs and values.
fn correspondence(model: &Model, exploration: &Exploration) -> Correspondence {
    let (epochs, values) = shape(model);
    let names: Vec<&str> = values.iter().map(String::as_str).collect();
    let abs = lower_with(
        &dossier("examples/abstract_register.ctm"),
        &abstract_config(u32::from(epochs) - 1, &names),
        Limits::default(),
    )
    .expect("the abstract register lowers")
    .model()
    .clone();
    let mut out = Correspondence::default();
    let mut abstract_steps: BTreeMap<State, Vec<(String, State)>> = BTreeMap::new();
    for init in model.initial_states() {
        assert_eq!(abs.initial_states().len(), 1, "one abstract initial state");
        let p = project(&abs, &decode(model, init), &values);
        assert_eq!(
            p.as_ref(),
            abs.initial_states().first(),
            "init projects to init"
        );
    }
    for s in exploration.reachable().states() {
        let pre = decode(model, s);
        let from = project(&abs, &pre, &values);
        if let Some(a) = &from {
            out.covered.insert(a.clone());
        }
        for step in model.successors(s).expect("evaluates") {
            out.steps += 1;
            let name = label(model, &step);
            let post = decode(model, step.target());
            let Some(a) = &from else {
                out.failures
                    .push((Mismatch::Unprojectable, name, pre.clone(), post));
                continue;
            };
            // A post-state the map does not define is no abstract state: the step is
            // neither a stutter nor a `Choose`.
            let to = project(&abs, &post, &values);
            if let Some(args) = name.strip_prefix("Ack(") {
                let want = format!("Choose({args}");
                let matched = abstract_steps
                    .entry(a.clone())
                    .or_insert_with(|| {
                        abs.successors(a)
                            .expect("evaluates")
                            .iter()
                            .map(|t| (label(&abs, t), t.target().clone()))
                            .collect()
                    })
                    .iter()
                    .any(|(l, t)| *l == want && Some(t) == to.as_ref());
                if !matched {
                    out.failures
                        .push((Mismatch::NotAnEnabledChoose, name, pre.clone(), post));
                } else if to.as_ref() == Some(a) {
                    out.stutters += 1;
                } else {
                    out.chooses += 1;
                    out.realized.insert(want);
                }
            } else if to.as_ref() == Some(a) {
                out.stutters += 1;
            } else {
                out.failures
                    .push((Mismatch::NotAStutter, name, pre.clone(), post));
            }
        }
    }
    out
}

/// A labelled step, decoded: its action, its source, and its target.
type Labelled = (String, Raw, Raw);

/// [`correspondence`] and [`durability_violations`] over the claim-A exploration,
/// computed once for the tests and the evidence.
fn claim_a_steps() -> &'static (Correspondence, Vec<Labelled>) {
    static CELL: OnceLock<(Correspondence, Vec<Labelled>)> = OnceLock::new();
    CELL.get_or_init(|| {
        let (model, exploration) = claim_a();
        (
            correspondence(model, exploration),
            durability_violations(model, exploration),
        )
    })
}

/// Every engine step that loses or changes a durable record, withdraws an
/// acknowledgement, or ends a write in flight while its bytes stay without being a
/// `Sync` — the durability claim as a transition property.
///
/// The last clause is needed because the model encodes "durable" as "bytes, not
/// pending": any step that clears `pending` over bytes promotes them, so a release
/// without sync looks like a `Sync` to every state predicate (`neg-06`).
fn durability_violations(model: &Model, exploration: &Exploration) -> Vec<Labelled> {
    let mut bad = Vec::new();
    for s in exploration.reachable().states() {
        let pre = decode(model, s);
        let durable = pre.durable();
        for step in model.successors(s).expect("evaluates") {
            let post = decode(model, step.target());
            let name = label(model, &step);
            let promoted = pre.pending.iter().any(|&(n, e)| {
                !post.pending.contains(&(n, e))
                    && post.log.iter().any(|&(m, f, _)| (m, f) == (n, e))
            });
            if !durable.is_subset(&post.durable())
                || !pre.acks.is_subset(&post.acks)
                || (promoted && !name.starts_with("Sync("))
            {
                bad.push((name, pre.clone(), post));
            }
        }
    }
    bad
}

/// Claim A literally: the states reachable when a crash is atomic — replica `n` loses
/// every in-flight write at once — and at most two crashes happen. Built from the
/// engine's own steps: an atomic crash of `n` is `Lose(n=…)` steps until none is
/// enabled at `n`; every other step is taken as it is. Returns, per state, the fewest
/// crashes that reach it.
fn atomic_crash_reachable(model: &Model) -> BTreeMap<State, u8> {
    let mut successors: BTreeMap<State, Vec<(String, State)>> = BTreeMap::new();
    let mut next = |s: &State| -> Vec<(String, State)> {
        successors
            .entry(s.clone())
            .or_insert_with(|| {
                model
                    .successors(s)
                    .expect("evaluates")
                    .iter()
                    .map(|t| (label(model, t), t.target().clone()))
                    .collect()
            })
            .clone()
    };
    let mut best: BTreeMap<State, u8> = BTreeMap::new();
    let mut seen: BTreeSet<(u8, State)> = BTreeSet::new();
    let mut work: Vec<(u8, State)> = model
        .initial_states()
        .iter()
        .map(|s| (0, s.clone()))
        .collect();
    seen.extend(work.iter().cloned());
    while let Some((crashes, s)) = work.pop() {
        let entry = best.entry(s.clone()).or_insert(crashes);
        *entry = (*entry).min(crashes);
        let mut out: Vec<(u8, State)> = next(&s)
            .into_iter()
            .filter(|(l, _)| !l.starts_with("Lose("))
            .map(|(_, t)| (crashes, t))
            .collect();
        if crashes < 2 {
            for n in NODES {
                let prefix = format!("Lose(n={n},");
                let mut at = s.clone();
                while let Some((_, t)) = next(&at).into_iter().find(|(l, _)| l.starts_with(&prefix))
                {
                    at = t;
                }
                out.push((crashes + 1, at));
            }
        }
        for t in out {
            if seen.insert(t.clone()) {
                work.push(t);
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// checking and witnesses
// ---------------------------------------------------------------------------

fn report(model: &Model, exploration: &Exploration) -> CheckReport {
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    checking::check(model, exploration, &obligations).expect("every predicate is declared")
}

fn outcome<'r>(model: &Model, report: &'r CheckReport, name: &str) -> &'r CheckOutcome {
    let i = model.predicate_index(name).expect("declared");
    report.invariant(i).expect("checked").outcome()
}

fn witness_labels(outcome: &CheckOutcome) -> Vec<String> {
    let CheckOutcome::Violated {
        evidence: Evidence::Shortest(w),
        ..
    } = outcome
    else {
        panic!("a witnessed refutation: {outcome}")
    };
    w.steps().iter().map(|s| s.name().to_string()).collect()
}

/// Replay a counterexample against the model: every step is a genuine successor of
/// the one before, from an initial state, and it ends at the refuting state.
fn replays(model: &Model, outcome: &CheckOutcome) {
    let CheckOutcome::Violated {
        state,
        evidence: Evidence::Shortest(w),
        ..
    } = outcome
    else {
        panic!("a witnessed refutation: {outcome}")
    };
    assert!(model.initial_states().contains(w.start()));
    let mut at = w.start().clone();
    for step in w.steps() {
        let found = model
            .successors(&at)
            .expect("evaluates")
            .into_iter()
            .any(|s| label(model, &s) == step.name().to_string() && s.target() == step.target());
        assert!(found, "{} is a successor of {at}", step.name());
        at = step.target().clone();
    }
    assert_eq!(&at, state, "the witness ends at the refuting state");
}

/// Replay a witness by its labels on another lowering of the same model text: each
/// label names one action instance, whose step must be enabled where the path is.
fn replay_labels(model: &Model, labels: &[String]) -> State {
    let mut at = model.initial_states()[0].clone();
    for name in labels {
        let steps = model.successors(&at).expect("evaluates");
        let step = steps
            .iter()
            .find(|s| label(model, s) == *name)
            .unwrap_or_else(|| panic!("{name} is enabled at {at}"));
        at = step.target().clone();
    }
    at
}

/// The claim-A lowering of M01, where its one-epoch witnesses are replayed. Exploring it
/// is out of reach: its 864² states pass the certifiable transition bound.
fn ack_before_sync_claim_a() -> Model {
    lowered(&ack_before_sync(), &committed_config())
}

fn violated_at(model: &Model, rep: &CheckReport, name: &str) -> usize {
    match outcome(model, rep, name) {
        CheckOutcome::Violated { depth, .. } => *depth,
        o => panic!("{name}: {o}"),
    }
}

// ---------------------------------------------------------------------------
// the evidence artifacts
// ---------------------------------------------------------------------------

struct Scenario {
    id: &'static str,
    what: &'static str,
    src: String,
    config: String,
    limits: Limits,
    bounds: Bounds,
}

fn scenarios() -> Vec<Scenario> {
    let s = |id, what, src, config| Scenario {
        id,
        what,
        src,
        config,
        limits: CLAIM_A_LIMITS,
        bounds: Bounds::CERTIFIABLE,
    };
    vec![
        s(
            "pr16-impl02-pos-01-claim-a",
            "committed configuration: nodes {a,b,c}, majority quorums, epochs {0,1}, values {v0,v1}, crashes unbounded",
            source(),
            committed_config(),
        ),
        s(
            "pr16-impl02-pos-02-one-epoch",
            "one epoch: one independent component of claim A",
            source(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-pos-03-one-epoch-counted-crashes",
            "one epoch, crashes counted and at most two: Lose is the atomic crash here",
            counted_crashes(),
            counted_config(&["0"]),
        ),
        s(
            "pr16-impl02-neg-01-ack-before-sync",
            "seeded bug M01: Ack counts volatile bytes; its witnesses replay at the claim-A scope",
            ack_before_sync(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-neg-02-lose-durable",
            "seeded bug: a crash may lose a synced record",
            lose_durable(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-neg-03-double-vote",
            "seeded bug: Submit into a slot that holds bytes",
            double_vote(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-neg-04-one-replica-quorum",
            "seeded bug, shadow of M05: one durable replica counts as a quorum",
            one_replica_quorum(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-neg-05-retract",
            "seeded bug outside the invariants: Retract withdraws an acknowledgement",
            retract(),
            one_epoch(),
        ),
        s(
            "pr16-impl02-neg-06-release-without-sync",
            "seeded bug outside the invariants: Abort after Submit leaves unsynced bytes as if synced",
            release_without_sync(),
            one_epoch(),
        ),
        Scenario {
            id: "pr16-impl02-bnd-01-default-limits",
            what: "the committed configuration under Limits::default()",
            src: source(),
            config: committed_config(),
            limits: Limits::default(),
            bounds: Bounds::CERTIFIABLE,
        },
        s(
            "pr16-impl02-bnd-02-counted-crashes-claim-a",
            "crashes counted at the claim-A scope: the counter triples the init domain",
            counted_crashes(),
            counted_config(&["0", "1"]),
        ),
        s(
            "pr16-impl02-bnd-03-ack-before-sync-no-crash",
            "M01 without Lose: no crash, so the early acknowledgement is never contradicted",
            without_crash(&ack_before_sync()),
            one_epoch(),
        ),
        s(
            "pr16-impl02-bnd-04-disjoint-quorums",
            "the correct register under quorums {a} and {b,c}, which do not intersect",
            source(),
            config(0, &["v0", "v1"], &[&["a"], &["b", "c"]]),
        ),
        Scenario {
            id: "pr16-impl02-bnd-05-depth-bound",
            what: "M01 at one epoch under a depth bound of 6",
            src: ack_before_sync(),
            config: one_epoch(),
            limits: CLAIM_A_LIMITS,
            bounds: Bounds::CERTIFIABLE.with_depth(6),
        },
    ]
}

fn render_raw(raw: &Raw) -> String {
    let values = ["v0", "v1"];
    let log: Vec<String> = raw
        .log
        .iter()
        .map(|(n, e, v)| {
            let tag = if raw.pending.contains(&(*n, *e)) {
                "~"
            } else {
                ""
            };
            format!(
                "{}{e}{tag}={}",
                NODES[usize::from(*n)],
                values[usize::from(*v)]
            )
        })
        .collect();
    let reserved: Vec<String> = raw
        .pending
        .iter()
        .filter(|(n, e)| !raw.log.iter().any(|(m, f, _)| m == n && f == e))
        .map(|(n, e)| format!("{}{e}", NODES[usize::from(*n)]))
        .collect();
    let acks: Vec<String> = raw
        .acks
        .iter()
        .map(|(e, v)| format!("{e}={}", values[usize::from(*v)]))
        .collect();
    format!(
        "log[{}] reserved[{}] acks[{}]",
        log.join(" "),
        reserved.join(" "),
        acks.join(" ")
    )
}

fn render(sc: &Scenario) -> String {
    let mut out = format!("[{}]\n{}\n", sc.id, sc.what);
    let model = match lowered_cached(&sc.src, &sc.config, sc.limits).as_ref() {
        Ok(m) => m.clone(),
        Err(e) => {
            out.push_str(&format!("lowering: refused {e:?}\n"));
            return out;
        }
    };
    out.push_str(&format!(
        "model: slots={} actions={} predicates={}\n",
        model.variables().len(),
        model.actions().len(),
        model
            .predicates()
            .iter()
            .map(|p| p.name().to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
    let owned;
    let exploration = if sc.id == "pr16-impl02-pos-01-claim-a" {
        assert_eq!(&model, &claim_a().0);
        &claim_a().1
    } else {
        owned = bfs::explore(&model, sc.bounds).expect("evaluates");
        &owned
    };
    let rep = report(&model, exploration);
    out.push_str(&format!("scope: {:?}\n", rep.scope()));
    for result in rep.invariants() {
        out.push_str(&format!("{}: {}\n", result.name(), result.outcome()));
        if let CheckOutcome::Violated { evidence, .. } = result.outcome() {
            match evidence {
                Evidence::Shortest(w) => {
                    let labels: Vec<String> =
                        w.steps().iter().map(|s| s.name().to_string()).collect();
                    out.push_str(&format!("  witness: {}\n", labels.join(" ; ")));
                }
                Evidence::Unwitnessed(why) => {
                    out.push_str(&format!("  witness: none ({why})\n"));
                }
            }
        }
    }
    out.push_str(&format!("{}\n", rep.deadlock()));
    if sc.id == "pr16-impl02-neg-01-ack-before-sync" {
        let wide = ack_before_sync_claim_a();
        for name in ["AckedIsDurable", "Agreement"] {
            let labels = witness_labels(outcome(&model, &rep, name));
            let end = replay_labels(&wide, &labels);
            let i = wide.predicate_index(name).expect("declared");
            let holds = wide.evaluate_predicate(i, &end).expect("evaluates");
            out.push_str(&format!(
                "claim-A replay: {name} witness of {} steps ends where {name} is {holds}\n",
                labels.len()
            ));
        }
    }
    if matches!(rep.scope(), checking::Scope::Complete { .. }) {
        let claim = sc.id == "pr16-impl02-pos-01-claim-a";
        let owned_steps;
        let (c, bad) = if claim {
            let steps = claim_a_steps();
            (&steps.0, &steps.1)
        } else {
            owned_steps = (
                correspondence(&model, exploration),
                durability_violations(&model, exploration),
            );
            (&owned_steps.0, &owned_steps.1)
        };
        out.push_str(&format!(
            "correspondence: steps={} stutters={} chooses={} realized={} covered={} failures={}\n",
            c.steps,
            c.stutters,
            c.chooses,
            c.realized.len(),
            c.covered.len(),
            c.failures.len()
        ));
        let root: Vec<_> = c
            .failures
            .iter()
            .filter(|f| f.0 != Mismatch::Unprojectable)
            .collect();
        out.push_str(&format!(
            "  failing steps from projectable states: {}\n",
            root.len()
        ));
        if let Some((why, name, pre, post)) = root.into_iter().min_by_key(|f| {
            (
                f.2.log.len() + f.2.pending.len(),
                f.0,
                f.1.clone(),
                f.2.clone(),
                f.3.clone(),
            )
        }) {
            out.push_str(&format!(
                "  first: {why:?} {name} from {} to {}\n",
                render_raw(pre),
                render_raw(post)
            ));
        }
        out.push_str(&format!("durability step violations: {}\n", bad.len()));
    }
    out
}

fn evidence(rendered: &[String]) -> String {
    let mut out = String::from(
        "# PR-16/IMPL-02 operational durable register (bn-2rsp): notes/plan/examples/durable_register.ctm\n\
         # checked by continuum-engine-reference, steps related to abstract_register.ctm.\n\
         # Regenerate: CML_BLESS=1 cargo test -p continuum-cml-elab --test pr16_impl02_durable_register\n",
    );
    for r in rendered {
        out.push('\n');
        out.push_str(r);
    }
    out
}

fn golden(name: &str, got: &str) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("CML_BLESS").is_some() {
        std::fs::write(&path, got).expect("write golden");
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (regenerate with CML_BLESS=1)", path.display()));
    assert_eq!(got, want, "{name} changed");
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        out.push_str(&format!("{b:02x}"));
        if i % 32 == 31 {
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// the model
// ---------------------------------------------------------------------------

#[test]
fn the_model_lowers_to_twenty_two_slots_and_forty_six_actions() {
    let model = &claim_a().0;
    let slots: Vec<String> = model
        .variables()
        .iter()
        .map(|v| v.name().to_string())
        .collect();
    let mut want = Vec::new();
    for e in 0..2 {
        for v in ["v0", "v1"] {
            want.push(format!("acks{{({e},{v})}}"));
        }
    }
    for n in NODES {
        for e in 0..2 {
            for v in ["v0", "v1"] {
                want.push(format!("log{{({n},{e},{v})}}"));
            }
        }
    }
    for n in NODES {
        for e in 0..2 {
            want.push(format!("pending{{({n},{e})}}"));
        }
    }
    assert_eq!(slots, want);
    assert!(
        model
            .variables()
            .iter()
            .all(|v| v.domain().cardinality() == 2)
    );
    assert_eq!(
        1_u128 << model.variables().len(),
        MAX_INIT_ENUMERATION,
        "the init domain is exactly the enumeration limit"
    );
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for a in model.actions() {
        let name = a.name().to_string();
        *counts
            .entry(name.split('(').next().expect("name").to_owned())
            .or_default() += 1;
    }
    assert_eq!(
        counts,
        BTreeMap::from(
            [
                ("Abort", 6),
                ("Ack", 4),
                ("Lose", 12),
                ("Reserve", 6),
                ("Submit", 12),
                ("Sync", 6)
            ]
            .map(|(k, v)| (k.to_owned(), v))
        )
    );
    let predicates: Vec<String> = model
        .predicates()
        .iter()
        .map(|p| p.name().to_string())
        .collect();
    assert_eq!(
        predicates,
        ["AckedIsDurable", "Agreement", "OneValuePerSlot"],
        "no #defined item: every read is total"
    );
    assert_eq!(
        model.initial_states().len(),
        1,
        "nothing written, nothing acknowledged"
    );
}

#[test]
fn the_committed_configuration_is_claim_a_and_its_run_identity_is_pinned() {
    let committed = lower_with(&source(), &committed_config(), CLAIM_A_LIMITS).expect("lowers");
    let built = lower_with(
        &source(),
        &config(1, &["v0", "v1"], MAJORITY),
        CLAIM_A_LIMITS,
    )
    .expect("lowers");
    assert_eq!(
        committed.run_identity(),
        built.run_identity(),
        "the committed file is nodes a b c, Nat max 1, {{v0, v1}}, majority quorums"
    );
    golden(
        "durable_register.run-identity.hex",
        &hex(&committed.run_identity().encode()),
    );
}

// ---------------------------------------------------------------------------
// positive
// ---------------------------------------------------------------------------

#[test]
fn claim_a_the_engine_establishes_agreement_durability_and_one_value_per_slot() {
    let (model, exploration) = claim_a();
    let rep = report(model, exploration);
    assert_eq!(
        rep.scope(),
        checking::Scope::Complete {
            states: CLAIM_A_STATES
        }
    );
    for name in ["Agreement", "AckedIsDurable", "OneValuePerSlot"] {
        assert_eq!(
            outcome(model, &rep, name),
            &CheckOutcome::Holds {
                states: CLAIM_A_STATES
            },
            "{name}"
        );
    }
    assert_eq!(
        rep.deadlock(),
        &DeadlockOutcome::Free {
            states: CLAIM_A_STATES
        }
    );
    assert_eq!(rep.verdict(), checking::Verdict::Established);
}

/// The model is the prose protocol: its reachable states and labelled transitions are
/// [`Spec`]'s, at one epoch and at the claim-A scope.
#[test]
fn the_model_agrees_with_the_prose_protocol() {
    let one = lowered(&source(), &one_epoch());
    let one_exploration = complete(&one);
    for (epochs, model, exploration) in [
        (1_u8, &one, &one_exploration),
        (2, &claim_a().0, &claim_a().1),
    ] {
        let spec = Spec {
            epochs,
            values: vec!["v0", "v1"],
        };
        let states = exploration.reachable().states();
        let got: BTreeSet<Raw> = states.iter().map(|s| decode(model, s)).collect();
        let want: BTreeSet<Raw> = spec.reachable().iter().map(Protocol::raw).collect();
        assert_eq!(got.len(), states.len(), "decoding is injective");
        assert_eq!(got, want, "reachable states, {epochs} epochs");
        let by_raw: BTreeMap<Raw, Protocol> =
            spec.reachable().into_iter().map(|p| (p.raw(), p)).collect();
        let mut fired = BTreeSet::new();
        for s in states {
            let pre = decode(model, s);
            let got: BTreeSet<(String, Raw)> = model
                .successors(s)
                .expect("evaluates")
                .iter()
                .map(|t| (label(model, t), decode(model, t.target())))
                .collect();
            let want: BTreeSet<(String, Raw)> = spec
                .step(&by_raw[&pre])
                .into_iter()
                .map(|(l, t)| (l, t.raw()))
                .collect();
            assert_eq!(got, want, "transitions at {}", render_raw(&pre));
            for (l, post) in &got {
                if *post != pre {
                    fired.insert(l.clone());
                }
            }
        }
        assert_eq!(
            fired.len(),
            model.actions().len(),
            "every action instance makes a real move somewhere"
        );
    }
    assert_eq!(one_exploration.reachable().len(), 248);
    assert_eq!(
        248 * 248,
        claim_a().1.reachable().len(),
        "two independent epochs"
    );
}

/// Acceptance claim C over this model: every step is a stutter or an enabled `Choose`,
/// every `Choose` instance is realized, and every abstract map is reached.
#[test]
fn every_step_is_a_stutter_or_an_enabled_choose_of_the_abstract_register() {
    let c = &claim_a_steps().0;
    assert_eq!(c.failures, []);
    assert_eq!(c.steps, c.stutters + c.chooses);
    assert!(c.chooses > 0 && c.stutters > 0);
    let want: BTreeSet<String> = (0..2)
        .flat_map(|e| ["v0", "v1"].map(|v| format!("Choose(epoch={e},value={v})")))
        .collect();
    assert_eq!(c.realized, want, "every Choose instance is realized");
    assert_eq!(
        c.covered.len(),
        9,
        "every map of two epochs and two values is reached"
    );
}

#[test]
fn no_engine_step_loses_a_durable_record_or_withdraws_an_acknowledgement() {
    assert_eq!(claim_a_steps().1, []);
}

/// Claim A literally: atomic crashes, at most two. The set is the established one,
/// and every state in it is reachable with no crash at all: a slot keeps no history,
/// every slot state is reachable from `Free` without `Lose`, and an acknowledgement
/// depends only on present durability. So crash adds no reachable state to the
/// correct register; it adds transitions, which the step checks cover, and it is what
/// the M01 and `neg-02` refutations need.
#[test]
fn claim_a_literal_atomic_crashes_at_most_two_reach_exactly_the_established_states() {
    let (model, exploration) = claim_a();
    let best = atomic_crash_reachable(model);
    let states: BTreeSet<&State> = best.keys().collect();
    let all: BTreeSet<&State> = exploration.reachable().states().iter().collect();
    assert_eq!(states, all);
    assert!(best.values().all(|c| *c == 0));
}

/// At one epoch `Lose` is an atomic crash, so the counted model is claim A's crash
/// bound taken literally, and the engine establishes it.
#[test]
fn one_epoch_with_at_most_two_counted_crashes_is_established() {
    let model = lowered(&counted_crashes(), &counted_config(&["0"]));
    let rep = report(&model, &complete(&model));
    for name in ["Agreement", "AckedIsDurable", "OneValuePerSlot"] {
        assert!(
            matches!(outcome(&model, &rep, name), CheckOutcome::Holds { .. }),
            "{name}: {}",
            outcome(&model, &rep, name)
        );
    }
    assert_eq!(rep.verdict(), checking::Verdict::Established);
    let crashes = model
        .variables()
        .iter()
        .find(|v| v.name().to_string() == "crashes")
        .expect("counted");
    assert_eq!(crashes.domain().cardinality(), 3, "0..=2");
}

// ---------------------------------------------------------------------------
// negative: seeded bugs
// ---------------------------------------------------------------------------

/// M01. The early acknowledgement breaks durability at once; one crash and a second
/// writer then break `Agreement`, and the correspondence fails at the second `Ack`.
/// Refuted at one epoch, a component of claim A, with shortest witnesses, and both
/// witnesses replay at the claim-A scope to a violating state.
#[test]
fn ack_before_sync_is_refuted_and_its_witnesses_replay_at_the_claim_a_scope() {
    let model = lowered(&ack_before_sync(), &one_epoch());
    let exploration = complete(&model);
    let rep = report(&model, &exploration);

    let durable = outcome(&model, &rep, "AckedIsDurable");
    assert_eq!(violated_at(&model, &rep, "AckedIsDurable"), 5);
    let durable_labels = witness_labels(durable);
    assert!(durable_labels[4].starts_with("Ack("), "{durable_labels:?}");
    assert!(
        !durable_labels.iter().any(|l| l.starts_with("Sync(")),
        "nothing synced: {durable_labels:?}"
    );
    replays(&model, durable);

    let agreement = outcome(&model, &rep, "Agreement");
    assert_eq!(violated_at(&model, &rep, "Agreement"), 11);
    let agreement_labels = witness_labels(agreement);
    assert_eq!(
        agreement_labels
            .iter()
            .filter(|l| l.starts_with("Ack("))
            .count(),
        2
    );
    assert_eq!(
        agreement_labels
            .iter()
            .filter(|l| l.starts_with("Lose("))
            .count(),
        1,
        "one crash"
    );
    replays(&model, agreement);

    let c = correspondence(&model, &exploration);
    let root: Vec<_> = c
        .failures
        .iter()
        .filter(|f| f.0 != Mismatch::Unprojectable)
        .collect();
    assert!(!root.is_empty());
    assert!(
        root.iter()
            .all(|f| f.0 == Mismatch::NotAnEnabledChoose && f.1.starts_with("Ack(")),
        "only an acknowledgement fails, and only as a Choose that is not enabled"
    );
    assert!(
        root.iter()
            .all(|f| f.3.acks.len() == 2 && f.2.acks.len() == 1)
    );

    let wide = ack_before_sync_claim_a();
    for (name, labels) in [
        ("AckedIsDurable", &durable_labels),
        ("Agreement", &agreement_labels),
    ] {
        let end = replay_labels(&wide, labels);
        let i = wide.predicate_index(name).expect("declared");
        assert!(
            !wide.evaluate_predicate(i, &end).expect("evaluates"),
            "{name}"
        );
    }
}

#[test]
fn a_crash_that_loses_a_synced_record_is_refuted() {
    let model = lowered(&lose_durable(), &one_epoch());
    let exploration = complete(&model);
    let rep = report(&model, &exploration);
    let durable = outcome(&model, &rep, "AckedIsDurable");
    assert_eq!(violated_at(&model, &rep, "AckedIsDurable"), 8);
    assert!(witness_labels(durable)[7].starts_with("Lose("));
    replays(&model, durable);
    let agreement = outcome(&model, &rep, "Agreement");
    replays(&model, agreement);
    assert!(!durability_violations(&model, &exploration).is_empty());
}

#[test]
fn a_replica_that_votes_twice_is_refuted() {
    let model = lowered(&double_vote(), &one_epoch());
    let rep = report(&model, &complete(&model));
    let slot = outcome(&model, &rep, "OneValuePerSlot");
    assert_eq!(violated_at(&model, &rep, "OneValuePerSlot"), 3);
    replays(&model, slot);
    let agreement = outcome(&model, &rep, "Agreement");
    replays(&model, agreement);
    assert_eq!(
        outcome(&model, &rep, "AckedIsDurable").verdict(),
        checking::Verdict::Established,
        "every acknowledgement is still durable: the defect is the double vote"
    );
}

#[test]
fn counting_one_replica_as_a_quorum_is_refuted() {
    let model = lowered(&one_replica_quorum(), &one_epoch());
    let rep = report(&model, &complete(&model));
    assert_eq!(violated_at(&model, &rep, "AckedIsDurable"), 4);
    assert_eq!(violated_at(&model, &rep, "Agreement"), 8);
    for name in ["AckedIsDurable", "Agreement"] {
        replays(&model, outcome(&model, &rep, name));
    }
}

/// The invariants' limit: withdrawing an acknowledgement leaves every reachable state
/// one the invariants accept. Only the step checks refute it.
#[test]
fn retract_escapes_every_invariant_and_only_the_step_checks_refute_it() {
    let model = lowered(&retract(), &one_epoch());
    let exploration = complete(&model);
    let rep = report(&model, &exploration);
    assert_eq!(rep.verdict(), checking::Verdict::Established);
    let c = correspondence(&model, &exploration);
    assert!(!c.failures.is_empty());
    assert!(
        c.failures
            .iter()
            .all(|f| f.0 == Mismatch::NotAStutter && f.1.starts_with("Retract("))
    );
    let bad = durability_violations(&model, &exploration);
    assert!(!bad.is_empty() && bad.iter().all(|b| b.0.starts_with("Retract(")));
}

/// Cancellation cleanup (the M02 class): an `Abort` after `Submit` ends the write in
/// flight and its unsynced bytes count as durable. The reachable states are the
/// correct register's, so every invariant and the correspondence accept it; only the
/// durability step check refutes it, at the `Abort` steps over bytes.
#[test]
fn release_without_sync_escapes_every_state_check_and_only_the_durability_step_check_refutes_it() {
    let model = lowered(&release_without_sync(), &one_epoch());
    let exploration = complete(&model);
    let rep = report(&model, &exploration);
    assert_eq!(rep.verdict(), checking::Verdict::Established);
    assert_eq!(
        exploration.reachable().len(),
        248,
        "the correct register's states"
    );
    assert_eq!(correspondence(&model, &exploration).failures, []);
    let bad = durability_violations(&model, &exploration);
    assert!(!bad.is_empty());
    assert!(
        bad.iter()
            .all(|(name, pre, post)| name.starts_with("Abort(")
                && pre.durable().len() < post.durable().len())
    );
}

// ---------------------------------------------------------------------------
// boundary
// ---------------------------------------------------------------------------

/// The claim-A lowering needs more work than the default budget: a typed refusal,
/// charged before any candidate is enumerated. The explicit budget covers it.
#[test]
fn the_default_work_budget_is_a_typed_refusal_before_enumeration() {
    let before = identity_encodings_on_this_thread();
    assert_eq!(
        lower_with(&source(), &committed_config(), Limits::default()).expect_err("too much work"),
        LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded)
    );
    assert_eq!(identity_encodings_on_this_thread(), before);
    let model = elaborate_source(&source()).expect("elaborates");
    let config = RunConfig::parse(committed_config().as_bytes()).expect("reads");
    let (result, usage) = lower_configured(&model, &config, Limits::default());
    assert!(result.is_err());
    assert!(
        usage.work < 1 << 20,
        "refused before enumerating: {usage:?}"
    );
    let (result, usage) = lower_configured(&model, &config, CLAIM_A_LIMITS);
    assert!(result.is_ok());
    assert!(usage.work > Limits::default().work && usage.work <= CLAIM_A_LIMITS.work);
}

/// A crash counter, even `0..=2`, triples the init domain past the enumeration limit.
/// This is the limit that keeps crash uncounted at the claim-A scope.
#[test]
fn a_counted_crash_at_the_claim_a_scope_is_a_typed_refusal() {
    assert_eq!(
        lower_with(
            &counted_crashes(),
            &counted_config(&["0", "1"]),
            CLAIM_A_LIMITS
        )
        .expect_err("too wide"),
        LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge),
        "3 * 2^22 flat states"
    );
}

/// Why M01 needs a crash: with no `Lose`, every early acknowledgement is eventually
/// true, so `Agreement` holds. `AckedIsDurable` still refutes it.
#[test]
fn without_crash_ack_before_sync_keeps_agreement_but_not_durability() {
    let model = lowered(&without_crash(&ack_before_sync()), &one_epoch());
    let rep = report(&model, &complete(&model));
    assert!(matches!(
        outcome(&model, &rep, "Agreement"),
        CheckOutcome::Holds { .. }
    ));
    assert_eq!(violated_at(&model, &rep, "AckedIsDurable"), 5);
    let correct = lowered(&without_crash(&source()), &one_epoch());
    assert_eq!(
        report(&correct, &complete(&correct)).verdict(),
        checking::Verdict::Established
    );
}

/// `Agreement` rests on quorum intersection, a configuration assumption: disjoint
/// quorums break the correct register.
#[test]
fn disjoint_quorums_break_agreement_in_the_correct_register() {
    let model = lowered(&source(), &config(0, &["v0", "v1"], &[&["a"], &["b", "c"]]));
    let exploration = complete(&model);
    let rep = report(&model, &exploration);
    let agreement = outcome(&model, &rep, "Agreement");
    replays(&model, agreement);
    assert_eq!(
        outcome(&model, &rep, "AckedIsDurable"),
        &CheckOutcome::Holds {
            states: exploration.reachable().len()
        }
    );
}

/// INV-008: a depth bound makes "holds" inconclusive, never a pass; a violation found
/// inside the bound is still a refutation.
#[test]
fn a_depth_bound_is_inconclusive_for_a_hold_but_not_for_a_found_violation() {
    let model = lowered(&ack_before_sync(), &one_epoch());
    let bounded = bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(6)).expect("evaluates");
    let rep = report(&model, &bounded);
    assert!(matches!(
        outcome(&model, &rep, "AckedIsDurable"),
        CheckOutcome::Violated { depth: 5, .. }
    ));
    assert!(matches!(
        outcome(&model, &rep, "Agreement"),
        CheckOutcome::Inconclusive(Unresolved::ResourceExhausted {
            tripped: Bound::Depth,
            ..
        })
    ));
    assert_eq!(rep.verdict(), checking::Verdict::Refuted);

    let correct = lowered(&source(), &one_epoch());
    let bounded = bfs::explore(&correct, Bounds::CERTIFIABLE.with_depth(6)).expect("evaluates");
    let rep = report(&correct, &bounded);
    assert_ne!(rep.verdict(), checking::Verdict::Established);
}

// ---------------------------------------------------------------------------
// the committed evidence
// ---------------------------------------------------------------------------

/// The committed golden, and why it cannot be satisfied by a rendering that ignores
/// its input: the result bodies differ between every two scenarios, and the artifact
/// IDs are unique. Rendered once: two scenarios lower at the claim-A scope.
#[test]
fn the_evidence_artifacts_match_their_golden_and_render_distinctly() {
    let all = scenarios();
    let rendered: Vec<String> = all.iter().map(render).collect();
    golden(
        "pr16_impl02_durable_register.evidence.txt",
        &evidence(&rendered),
    );
    let bodies: Vec<String> = rendered
        .iter()
        .map(|r| r.splitn(3, '\n').nth(2).unwrap_or_default().to_owned())
        .collect();
    for (i, a) in bodies.iter().enumerate() {
        for (j, b) in bodies.iter().enumerate().skip(i + 1) {
            assert_ne!(a, b, "{} and {}", all[i].id, all[j].id);
        }
    }
    let ids: BTreeSet<&str> = all.iter().map(|sc| sc.id).collect();
    assert_eq!(ids.len(), all.len(), "artifact IDs are unique");
}
