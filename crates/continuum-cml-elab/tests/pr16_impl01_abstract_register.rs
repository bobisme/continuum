//! PR-16/IMPL-01: the abstract atomic register, checked by the reference engine
//! (bn-3e3v, START_HERE PR 16 first bullet).
//!
//! # The subject
//!
//! `notes/plan/examples/abstract_register.ctm`, the specification-level model of the
//! replicated register's service contract (`notes/plan/examples/replicated_register.md`,
//! "Service contract"): abstract state `chosen: Map[Epoch, Value]`, one atomic action
//! `Choose(e, v)` guarded by `e notin chosen || chosen[e] == v`, and the invariant
//! `forall e, v1, v2: Chosen(e, v1) && Chosen(e, v2) => v1 == v2` — the property the
//! register scenario names `abstract_register::Agreement`
//! (`notes/plan/examples/replicated_register.scenario.toml`).
//!
//! Over a map, "two values at one epoch at the same time" cannot be written down, so
//! `Agreement` read as a state predicate of `chosen` alone is structurally true
//! (`notes/plan/examples/pseudo_api.rs` says so of its own sketch). The contract's
//! claim is about *time*: the register never acknowledges two values for one epoch,
//! so a chosen value never changes and never disappears (docs/02 §9, "Transition
//! safety"). The model therefore carries a history variable, `history`, the set of
//! every `(epoch, value)` ever chosen, and states the contract as two state
//! invariants the explicit-state engine can decide:
//!
//! - `Agreement` — no epoch has ever been given two values;
//! - `Stability` — `history` and `chosen` agree: `(e, v)` was ever chosen exactly when
//!   `chosen[e] == Some(v)` now.
//!
//! # The evidence, by stable artifact ID
//!
//! Every scenario below renders into one committed golden,
//! `tests/golden/pr16_impl01_abstract_register.evidence.txt`, compared byte for byte;
//! the run identity of the committed configuration is committed beside it as
//! `tests/golden/abstract_register.run-identity.hex`. Regenerate both with
//! `CML_BLESS=1 cargo test -p continuum-cml-elab --test pr16_impl01_abstract_register`,
//! and review the diff.
//!
//! - **positive** `pr16-impl01-pos-01-values2-epochs2` (the committed run configuration:
//!   `Nat` max 1, two values — the Values = 2, Epochs = 2 domain of
//!   `replicated_register.md` acceptance claim A; claim A itself, over three nodes and
//!   up to two crashes, is about the operational model and stays with IMPL-02) and `pr16-impl01-pos-02-wide` (three epochs, three values): both
//!   invariants established over a complete exploration, no deadlock;
//! - **negative** seeded bugs, each a one-place edit of the model text:
//!   `pr16-impl01-neg-01-overwrite` (the guard dropped: a chosen epoch is rewritten),
//!   `pr16-impl01-neg-02-amnesia` (an action forgets every chosen value, the abstract
//!   shadow of ack-before-sync), `pr16-impl01-neg-03-unrecorded` (`Choose` stops
//!   recording `history`). Each is refuted with a shortest counterexample, and the
//!   third shows why `Stability` is needed: with the history unrecorded, `Agreement`
//!   alone still holds. `pr16-impl01-neg-04-lockstep-forget` pins the limit of the
//!   invariants: an action that clears `history` together with `chosen` breaks the
//!   side-condition that `history` only grows, both invariants still hold, and only
//!   the step check [`write_once_violations`] refutes it;
//! - **boundary** `pr16-impl01-bnd-01`…`06`: one value hides the overwrite bug, one
//!   epoch with two values is the smallest scope that exposes it, a depth bound turns
//!   "holds" into a typed inconclusive result but not a found violation into a pass,
//!   and a scope past the lowering's init-enumeration limit is a typed refusal.
//!
//! # Independence
//!
//! [`Spec`] is written from the prose contract in `replicated_register.md`, over a
//! `BTreeMap`, with no history variable and no code shared with the elaborator or the
//! lowering. The differential shows the lowered model's reachable `chosen` maps and
//! labelled transitions are the prose contract's, and that `history` is a function of
//! `chosen` at every reachable state: the history variable adds no behaviour. A
//! refinement mapping onto this model (PR 17) therefore produces `chosen` and sets
//! `history` to its graph — and on such projected states both invariants are
//! tautologies, so the refinement carries the contract only through its step
//! correspondence (every concrete step maps to a stutter or an enabled `Choose`), never
//! through an invariant check of projected states (`neg-04` shows the gap).
//!
//! Two further readings, stated so no one takes them for evidence: `Stability`
//! implies `Agreement` because `chosen` is a map, so every refutation of `Agreement`
//! is also one of `Stability`; and deadlock freedom holds by the model's shape, since
//! re-choosing an epoch's own value is always enabled.
//! [`write_once_violations`] checks the transition property directly on the engine's
//! steps, reading only the `chosen` slots, so the verdict does not rest on the
//! history variable being wired correctly.

use std::collections::{BTreeMap, BTreeSet};

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{Configured, identity_encodings_on_this_thread, lower_configured};
use continuum_cml_elab::{Limits, LowerErrorKind, Unlowerable, elaborate_source};
use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::checking::{
    self, CheckOutcome, CheckReport, DeadlockOutcome, DeadlockPolicy, Evidence, Obligations,
    Unresolved,
};
use continuum_model_core::{Model, State};

// ---------------------------------------------------------------------------
// the subject
// ---------------------------------------------------------------------------

fn dossier(rel: &str) -> String {
    let path = format!("{}/../../notes/plan/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The model as committed.
fn source() -> String {
    dossier("examples/abstract_register.ctm")
}

/// The committed run configuration: two epochs, two values.
fn committed_config() -> String {
    dossier("examples/abstract_register.run-config.json")
}

/// A run configuration with `Nat` bounded to `0..=nat_max` and the given values.
fn config(nat_max: u32, values: &[&str]) -> String {
    let elements: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
    format!(
        r#"{{"schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1,"model":"AbstractRegister","bounds":{{"Nat":{{"max":{nat_max}}}}},"sorts":{{"Value":{{"elements":[{}]}}}},"constants":{{}}}}"#,
        elements.join(",")
    )
}

fn lower(src: &str, config: &str) -> Result<Configured, LowerErrorKind> {
    let model = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    let config = RunConfig::parse(config.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"));
    lower_configured(&model, &config, Limits::default())
        .0
        .map_err(|e| e.kind)
}

fn lowered(src: &str, config: &str) -> Model {
    lower(src, config)
        .unwrap_or_else(|e| panic!("lowers: {e:?}"))
        .model()
        .clone()
}

/// Replace exactly one occurrence of `from` in the model text, or fail: a seeded bug
/// that silently did not apply would make its refutation test vacuous.
fn mutate(from: &str, to: &str) -> String {
    let src = source();
    assert_eq!(
        src.matches(from).count(),
        1,
        "the mutation site {from:?} occurs exactly once"
    );
    let out = src.replacen(from, to, 1);
    assert_ne!(out, src, "the mutation changed the model text");
    out
}

const GUARD: &str = "  require chosen.get(epoch) in {None, Some(value)}\n";
const RECORD: &str = "  next history = history union {(epoch, value)}\n";
const LAST_ACTION_END: &str = "  next history = history union {(epoch, value)}\n}\n";

/// `neg-01`: `Choose` without its guard, so a chosen epoch can be rewritten.
fn overwrite() -> String {
    mutate(GUARD, "")
}

/// `neg-02`: an action that forgets every chosen value while the history remembers
/// them — the abstract shadow of an acknowledged write lost before it was durable.
fn amnesia() -> String {
    mutate(
        LAST_ACTION_END,
        "  next history = history union {(epoch, value)}\n}\n\naction Forget {\n  next chosen = {}\n  unchanged history\n}\n",
    )
}

/// `neg-04`: an action that resets both `chosen` and `history`, which breaks the
/// side-condition that `history` only grows.
fn lockstep_forget() -> String {
    mutate(
        LAST_ACTION_END,
        "  next history = history union {(epoch, value)}\n}\n\naction Forget {\n  next chosen = {}\n  next history = {}\n}\n",
    )
}

/// `neg-03`: `Choose` stops recording its choice in `history`.
fn unrecorded() -> String {
    mutate(RECORD, "  unchanged history\n")
}

// ---------------------------------------------------------------------------
// the oracle: the prose contract, independently
// ---------------------------------------------------------------------------

/// `replicated_register.md`, "Service contract": `chosen: Map[Epoch, Value]` and
/// `Choose(e, v): require e notin chosen || chosen[e] == v; chosen' = chosen[e := v]`.
/// Epochs and values by position.
struct Spec {
    epochs: u8,
    values: Vec<&'static str>,
}

type Chosen = BTreeMap<u8, u8>;

impl Spec {
    fn step(&self, s: &Chosen) -> BTreeSet<(String, Chosen)> {
        let mut out = BTreeSet::new();
        for e in 0..self.epochs {
            for (vi, name) in self.values.iter().enumerate() {
                let v = u8::try_from(vi).expect("few values");
                if s.get(&e).is_none_or(|w| *w == v) {
                    let mut t = s.clone();
                    t.insert(e, v);
                    out.insert((format!("Choose(epoch={e},value={name})"), t));
                }
            }
        }
        out
    }

    fn reachable(&self) -> BTreeSet<Chosen> {
        let mut seen = BTreeSet::from([Chosen::new()]);
        let mut work = vec![Chosen::new()];
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
// decoding the flat layout (RFC 0003 "Layout")
// ---------------------------------------------------------------------------

fn slot(model: &Model, state: &State, name: &str) -> i64 {
    model
        .binding(state, name)
        .unwrap_or_else(|| panic!("slot {name} is declared"))
}

/// `chosen` from its `Option[Value]` slots `chosen[e]`: `0` is `None`, `i + 1` is the
/// `i`-th value.
fn chosen(model: &Model, state: &State, epochs: u8) -> Chosen {
    (0..epochs)
        .filter_map(|e| {
            let code = slot(model, state, &format!("chosen[{e}]"));
            (code != 0).then(|| (e, u8::try_from(code - 1).expect("small code")))
        })
        .collect()
}

/// `history` from its `0..=1` membership slots `history{(e,v)}`.
fn history(model: &Model, state: &State, spec: &Spec) -> BTreeSet<(u8, u8)> {
    let mut out = BTreeSet::new();
    for e in 0..spec.epochs {
        for (vi, name) in spec.values.iter().enumerate() {
            if slot(model, state, &format!("history{{({e},{name})}}")) == 1 {
                out.insert((e, u8::try_from(vi).expect("few values")));
            }
        }
    }
    out
}

/// The declared name of the action a model step fired.
fn label(model: &Model, step: &continuum_model_core::Step) -> String {
    model.actions()[step.action()].name().to_string()
}

fn complete(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates")
}

/// Every engine step, at every reachable state, that rewrites or removes a chosen
/// value — the contract's transition property read off the `chosen` slots alone.
fn write_once_violations(model: &Model, epochs: u8) -> Vec<(String, Chosen, Chosen)> {
    let exploration = complete(model);
    let mut bad = Vec::new();
    for s in exploration.reachable().states() {
        let pre = chosen(model, s, epochs);
        for step in model.successors(s).expect("evaluates") {
            let post = chosen(model, step.target(), epochs);
            if pre.iter().any(|(e, v)| post.get(e) != Some(v)) {
                bad.push((label(model, &step), pre.clone(), post));
            }
        }
    }
    bad
}

fn report(model: &Model, exploration: &Exploration) -> CheckReport {
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    checking::check(model, exploration, &obligations).expect("every predicate is declared")
}

fn outcome<'r>(model: &Model, report: &'r CheckReport, name: &str) -> &'r CheckOutcome {
    let i = model.predicate_index(name).expect("declared");
    report.invariant(i).expect("checked").outcome()
}

/// The action labels of a refutation's shortest counterexample.
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

// ---------------------------------------------------------------------------
// the evidence artifacts
// ---------------------------------------------------------------------------

/// One scenario: a stable artifact ID, a model text, a configuration, and bounds.
struct Scenario {
    id: &'static str,
    what: &'static str,
    src: String,
    config: String,
    bounds: Bounds,
}

fn scenarios() -> Vec<Scenario> {
    let two = ["v0", "v1"];
    let s = |id, what, src, config| Scenario {
        id,
        what,
        src,
        config,
        bounds: Bounds::CERTIFIABLE,
    };
    vec![
        s(
            "pr16-impl01-pos-01-values2-epochs2",
            "committed configuration: epochs {0,1}, values {v0,v1}",
            source(),
            committed_config(),
        ),
        s(
            "pr16-impl01-pos-02-wide",
            "epochs {0,1,2}, values {v0,v1,v2}",
            source(),
            config(2, &["v0", "v1", "v2"]),
        ),
        s(
            "pr16-impl01-neg-01-overwrite",
            "seeded bug: Choose without its guard rewrites a chosen epoch",
            overwrite(),
            committed_config(),
        ),
        s(
            "pr16-impl01-neg-02-amnesia",
            "seeded bug: Forget drops every chosen value, history keeps them",
            amnesia(),
            committed_config(),
        ),
        s(
            "pr16-impl01-neg-03-unrecorded",
            "seeded bug: Choose does not record history",
            unrecorded(),
            committed_config(),
        ),
        s(
            "pr16-impl01-neg-04-lockstep-forget",
            "seeded bug outside the invariants: Forget clears chosen and history together",
            lockstep_forget(),
            committed_config(),
        ),
        s(
            "pr16-impl01-bnd-01-one-value",
            "epochs {0,1}, values {v0}",
            source(),
            config(1, &["v0"]),
        ),
        s(
            "pr16-impl01-bnd-02-one-value-overwrite",
            "the overwrite bug under one value: unobservable",
            overwrite(),
            config(1, &["v0"]),
        ),
        s(
            "pr16-impl01-bnd-03-one-epoch-overwrite",
            "the overwrite bug under one epoch, two values: the smallest exposing scope",
            overwrite(),
            config(0, &two),
        ),
        Scenario {
            id: "pr16-impl01-bnd-04-depth-bound",
            what: "the committed configuration under a depth bound of 1",
            src: source(),
            config: committed_config(),
            bounds: Bounds::CERTIFIABLE.with_depth(1),
        },
        Scenario {
            id: "pr16-impl01-bnd-05-depth-bound-overwrite",
            what: "the overwrite bug under a depth bound of 2",
            src: overwrite(),
            config: committed_config(),
            bounds: Bounds::CERTIFIABLE.with_depth(2),
        },
        s(
            "pr16-impl01-bnd-06-refused",
            "epochs {0..3}, values {v0..v3}: past MAX_INIT_ENUMERATION, a limit of the history encoding, 2^16 membership slots, not of the 625 chosen maps",
            source(),
            config(3, &["v0", "v1", "v2", "v3"]),
        ),
    ]
}

fn render(sc: &Scenario) -> String {
    let mut out = format!("[{}]\n{}\n", sc.id, sc.what);
    let model = match lower(&sc.src, &sc.config) {
        Ok(c) => c.model().clone(),
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
    let exploration = bfs::explore(&model, sc.bounds).expect("evaluates");
    let rep = report(&model, &exploration);
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
    if matches!(rep.scope(), checking::Scope::Complete { .. }) {
        let epochs = model
            .variables()
            .iter()
            .filter(|v| v.name().to_string().starts_with("chosen["))
            .count();
        let bad = write_once_violations(&model, u8::try_from(epochs).expect("few epochs"));
        out.push_str(&format!("write-once step violations: {}\n", bad.len()));
    }
    out
}

fn evidence() -> String {
    let mut out = String::from(
        "# PR-16/IMPL-01 abstract atomic register (bn-3e3v): notes/plan/examples/abstract_register.ctm\n\
         # checked by continuum-engine-reference. Regenerate: CML_BLESS=1 cargo test -p\n\
         # continuum-cml-elab --test pr16_impl01_abstract_register\n",
    );
    for sc in scenarios() {
        out.push('\n');
        out.push_str(&render(&sc));
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
fn the_model_lowers_to_six_slots_and_four_choose_instances() {
    let model = lowered(&source(), &committed_config());
    let slots: Vec<String> = model
        .variables()
        .iter()
        .map(|v| v.name().to_string())
        .collect();
    assert_eq!(
        slots,
        [
            "chosen[0]",
            "chosen[1]",
            "history{(0,v0)}",
            "history{(0,v1)}",
            "history{(1,v0)}",
            "history{(1,v1)}",
        ]
    );
    let actions: Vec<String> = model
        .actions()
        .iter()
        .map(|a| a.name().to_string())
        .collect();
    assert_eq!(
        actions,
        [
            "Choose(epoch=0,value=v0)",
            "Choose(epoch=0,value=v1)",
            "Choose(epoch=1,value=v0)",
            "Choose(epoch=1,value=v1)",
        ]
    );
    let predicates: Vec<String> = model
        .predicates()
        .iter()
        .map(|p| p.name().to_string())
        .collect();
    assert_eq!(
        predicates,
        ["Agreement", "Stability"],
        "no #defined item: every read is total"
    );
    assert_eq!(
        model.initial_states().len(),
        1,
        "one initial state: nothing chosen"
    );
}

#[test]
fn the_committed_configuration_is_two_epochs_two_values_and_its_run_identity_is_pinned() {
    let committed = lower(&source(), &committed_config()).expect("lowers");
    let built = lower(&source(), &config(1, &["v0", "v1"])).expect("lowers");
    assert_eq!(
        committed.run_identity(),
        built.run_identity(),
        "the committed file is Nat max 1 over {{v0, v1}}"
    );
    golden(
        "abstract_register.run-identity.hex",
        &hex(&committed.run_identity().encode()),
    );
}

// ---------------------------------------------------------------------------
// positive
// ---------------------------------------------------------------------------

#[test]
fn values2_epochs2_the_engine_establishes_agreement_and_stability() {
    let model = lowered(&source(), &committed_config());
    let exploration = complete(&model);
    let rep = report(&model, &exploration);
    assert_eq!(rep.scope(), checking::Scope::Complete { states: 9 });
    for name in ["Agreement", "Stability"] {
        assert_eq!(
            outcome(&model, &rep, name),
            &CheckOutcome::Holds { states: 9 },
            "{name}"
        );
    }
    assert_eq!(rep.deadlock(), &DeadlockOutcome::Free { states: 9 });
    assert_eq!(rep.verdict(), checking::Verdict::Established);
}

#[test]
fn the_wider_scope_establishes_both_invariants_over_all_sixty_four_maps() {
    let model = lowered(&source(), &config(2, &["v0", "v1", "v2"]));
    let rep = report(&model, &complete(&model));
    for name in ["Agreement", "Stability"] {
        assert_eq!(
            outcome(&model, &rep, name),
            &CheckOutcome::Holds { states: 64 },
            "{name}: (1 + 3)^3 maps"
        );
    }
}

/// The model is the prose contract: its reachable `chosen` maps and labelled
/// transitions are [`Spec`]'s, `history` is the graph of `chosen` at every reachable
/// state, and every one of the `(1 + |Value|)^|Epoch|` maps is reached — the
/// invariants are established over the whole contract, not a corner of it.
#[test]
fn the_model_agrees_with_the_prose_contract_and_the_history_adds_no_behaviour() {
    for (epochs, values, nat_max) in [
        (2_u8, vec!["v0", "v1"], 1_u32),
        (3, vec!["v0", "v1", "v2"], 2),
        (1, vec!["v0", "v1"], 0),
        (2, vec!["v0"], 1),
    ] {
        let spec = Spec {
            epochs,
            values: values.clone(),
        };
        let model = lowered(&source(), &config(nat_max, &values));
        let exploration = complete(&model);
        let states = exploration.reachable().states();

        let maps: BTreeSet<Chosen> = states.iter().map(|s| chosen(&model, s, epochs)).collect();
        assert_eq!(
            maps,
            spec.reachable(),
            "reachable maps, {epochs}x{values:?}"
        );
        let every = (values.len() + 1).pow(u32::from(epochs));
        assert_eq!(maps.len(), every, "every map is reachable");
        assert_eq!(states.len(), every, "chosen determines the whole state");

        let mut fired = BTreeSet::new();
        for s in states {
            let pre = chosen(&model, s, epochs);
            let graph: BTreeSet<(u8, u8)> = pre.iter().map(|(e, v)| (*e, *v)).collect();
            assert_eq!(
                history(&model, s, &spec),
                graph,
                "history is the graph of chosen"
            );
            let got: BTreeSet<(String, Chosen)> = model
                .successors(s)
                .expect("evaluates")
                .into_iter()
                .map(|t| (label(&model, &t), chosen(&model, t.target(), epochs)))
                .collect();
            assert_eq!(got, spec.step(&pre), "transitions at {pre:?}");
            for (label, post) in &got {
                if *post != pre {
                    fired.insert(label.clone());
                }
            }
        }
        assert_eq!(
            fired.len(),
            model.actions().len(),
            "every Choose instance makes a real move somewhere"
        );
    }
}

#[test]
fn no_engine_step_rewrites_or_removes_a_chosen_value() {
    let model = lowered(&source(), &committed_config());
    assert_eq!(write_once_violations(&model, 2), []);
    let wide = lowered(&source(), &config(2, &["v0", "v1", "v2"]));
    assert_eq!(write_once_violations(&wide, 3), []);
}

// ---------------------------------------------------------------------------
// negative: seeded bugs
// ---------------------------------------------------------------------------

#[test]
fn overwrite_the_unguarded_choose_violates_both_invariants_with_a_two_step_witness() {
    let model = lowered(&overwrite(), &committed_config());
    let rep = report(&model, &complete(&model));
    for name in ["Agreement", "Stability"] {
        let o = outcome(&model, &rep, name);
        let CheckOutcome::Violated { depth, .. } = o else {
            panic!("{name}: {o}")
        };
        assert_eq!(*depth, 2, "{name}");
        let labels = witness_labels(o);
        assert_eq!(labels.len(), 2);
        let (first, second) = (&labels[0], &labels[1]);
        assert_ne!(first, second, "two different choices");
        assert_eq!(
            first.split(",value=").next(),
            second.split(",value=").next(),
            "at one epoch"
        );
        replays(&model, o);
    }
    assert!(
        !write_once_violations(&model, 2).is_empty(),
        "the transition check sees the rewrite from the chosen slots alone"
    );
}

#[test]
fn amnesia_losing_chosen_values_violates_stability_then_agreement() {
    let model = lowered(&amnesia(), &committed_config());
    let rep = report(&model, &complete(&model));
    let stability = outcome(&model, &rep, "Stability");
    assert!(
        matches!(stability, CheckOutcome::Violated { depth: 2, .. }),
        "{stability}"
    );
    assert_eq!(witness_labels(stability)[1], "Forget");
    replays(&model, stability);

    let agreement = outcome(&model, &rep, "Agreement");
    assert!(
        matches!(agreement, CheckOutcome::Violated { depth: 3, .. }),
        "{agreement}"
    );
    let labels = witness_labels(agreement);
    assert_eq!(labels[1], "Forget", "forget, then choose the other value");
    assert_ne!(labels[0], labels[2]);
    replays(&model, agreement);

    assert!(!write_once_violations(&model, 2).is_empty());
}

/// Why `Stability` exists: with the history unrecorded, `Agreement` alone still holds
/// — it is only as strong as the history it reads. `Stability` ties the history to
/// `chosen` and refutes the mutant at the first choice.
#[test]
fn unrecorded_history_is_caught_by_stability_where_agreement_alone_is_blind() {
    let model = lowered(&unrecorded(), &committed_config());
    let rep = report(&model, &complete(&model));
    assert_eq!(
        outcome(&model, &rep, "Agreement"),
        &CheckOutcome::Holds { states: 9 }
    );
    let stability = outcome(&model, &rep, "Stability");
    assert!(
        matches!(stability, CheckOutcome::Violated { depth: 1, .. }),
        "{stability}"
    );
    replays(&model, stability);
    assert_eq!(
        write_once_violations(&model, 2),
        [],
        "the register itself still behaves: only the history is wrong"
    );
}

// ---------------------------------------------------------------------------
// boundary
// ---------------------------------------------------------------------------

#[test]
fn one_value_hides_the_overwrite_bug_and_one_epoch_with_two_values_exposes_it() {
    let one_value = config(1, &["v0"]);
    for src in [source(), overwrite()] {
        let model = lowered(&src, &one_value);
        let rep = report(&model, &complete(&model));
        for name in ["Agreement", "Stability"] {
            assert_eq!(
                outcome(&model, &rep, name),
                &CheckOutcome::Holds { states: 4 }
            );
        }
    }
    let model = lowered(&overwrite(), &config(0, &["v0", "v1"]));
    let rep = report(&model, &complete(&model));
    let agreement = outcome(&model, &rep, "Agreement");
    // The engine's endpoint is the canonically least refuting state, and its path the
    // one breadth-first search recorded: v1 first, then v0 over it.
    assert_eq!(
        witness_labels(agreement),
        ["Choose(epoch=0,value=v1)", "Choose(epoch=0,value=v0)"]
    );
    replays(&model, agreement);
}

/// Choosing the value an epoch already holds is enabled and changes nothing (a
/// stutter); choosing another value there is disabled.
#[test]
fn rechoosing_the_held_value_stutters_and_a_different_value_is_disabled() {
    let model = lowered(&source(), &committed_config());
    let exploration = complete(&model);
    let held = exploration
        .reachable()
        .states()
        .iter()
        .find(|s| chosen(&model, s, 2) == Chosen::from([(0, 0)]))
        .expect("chosen[0] = v0 is reachable")
        .clone();
    let same = model
        .action_index("Choose(epoch=0,value=v0)")
        .expect("declared");
    let other = model
        .action_index("Choose(epoch=0,value=v1)")
        .expect("declared");
    assert!(model.is_enabled(same, &held).expect("evaluates"));
    assert!(!model.is_enabled(other, &held).expect("evaluates"));
    let steps = model.successors(&held).expect("evaluates");
    let rechoose: Vec<_> = steps
        .iter()
        .filter(|s| label(&model, s) == "Choose(epoch=0,value=v0)")
        .collect();
    assert_eq!(rechoose.len(), 1);
    assert_eq!(rechoose[0].target(), &held, "a stutter");
}

/// INV-008: a bound that stops the exploration makes "holds" inconclusive, never a
/// pass; a violation found inside the bound is still a refutation.
#[test]
fn a_depth_bound_is_inconclusive_for_a_hold_but_not_for_a_found_violation() {
    let model = lowered(&source(), &committed_config());
    let bounded = bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(1)).expect("evaluates");
    let rep = report(&model, &bounded);
    for name in ["Agreement", "Stability"] {
        assert!(
            matches!(
                outcome(&model, &rep, name),
                CheckOutcome::Inconclusive(Unresolved::ResourceExhausted {
                    tripped: Bound::Depth,
                    ..
                })
            ),
            "{name}: {}",
            outcome(&model, &rep, name)
        );
    }
    assert_ne!(rep.verdict(), checking::Verdict::Established);

    let mutant = lowered(&overwrite(), &committed_config());
    let bounded = bfs::explore(&mutant, Bounds::CERTIFIABLE.with_depth(2)).expect("evaluates");
    let rep = report(&mutant, &bounded);
    assert!(matches!(
        outcome(&mutant, &rep, "Agreement"),
        CheckOutcome::Violated { depth: 2, .. }
    ));
    assert_eq!(rep.verdict(), checking::Verdict::Refuted);
}

#[test]
fn a_scope_past_the_init_enumeration_limit_is_a_typed_refusal_before_any_identity() {
    let before = identity_encodings_on_this_thread();
    assert_eq!(
        lower(&source(), &config(3, &["v0", "v1", "v2", "v3"])).expect_err("too wide"),
        LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge),
        "5^4 * 2^16 flat states"
    );
    assert_eq!(identity_encodings_on_this_thread(), before);
}

// ---------------------------------------------------------------------------
// the committed evidence
// ---------------------------------------------------------------------------

#[test]
fn the_evidence_artifacts_match_their_committed_golden() {
    let got = evidence();
    assert_eq!(got, evidence(), "rendering is deterministic");
    golden("pr16_impl01_abstract_register.evidence.txt", &got);
}

/// The golden cannot be satisfied by a rendering that ignores its input: the result
/// bodies (every line after the ID and the description) differ between every two
/// scenarios except the one pair the boundary evidence says must agree — the correct
/// register and the overwrite bug under one value, where the bug is unobservable.
#[test]
fn every_artifact_renders_distinctly_except_the_unobservable_pair() {
    let all = scenarios();
    let bodies: Vec<String> = all
        .iter()
        .map(|sc| {
            render(sc)
                .splitn(3, '\n')
                .nth(2)
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    let mut equal = Vec::new();
    for (i, a) in bodies.iter().enumerate() {
        for (j, b) in bodies.iter().enumerate().skip(i + 1) {
            if a == b {
                equal.push((all[i].id, all[j].id));
            }
        }
    }
    assert_eq!(
        equal,
        [(
            "pr16-impl01-bnd-01-one-value",
            "pr16-impl01-bnd-02-one-value-overwrite"
        )]
    );
    let ids: BTreeSet<&str> = all.iter().map(|sc| sc.id).collect();
    assert_eq!(ids.len(), all.len(), "artifact IDs are unique");
}

/// The invariants' limit: `Forget` clears `chosen` and `history` together, so every
/// reachable state is one the correct register also reaches and both invariants hold,
/// yet a chosen value disappears. Only the step check refutes it.
#[test]
fn lockstep_forget_escapes_both_invariants_and_only_the_step_check_refutes_it() {
    let model = lowered(&lockstep_forget(), &committed_config());
    let rep = report(&model, &complete(&model));
    for name in ["Agreement", "Stability"] {
        assert_eq!(
            outcome(&model, &rep, name),
            &CheckOutcome::Holds { states: 9 },
            "{name}"
        );
    }
    let bad = write_once_violations(&model, 2);
    assert!(!bad.is_empty());
    assert!(
        bad.iter()
            .all(|(label, _, post)| label == "Forget" && post.is_empty())
    );
}

/// `Stability` implies `Agreement`: over every negative scenario, no invariant result
/// refutes `Agreement` while `Stability` holds.
#[test]
fn no_scenario_refutes_agreement_while_stability_holds() {
    for sc in scenarios() {
        let Ok(configured) = lower(&sc.src, &sc.config) else {
            continue;
        };
        let model = configured.model();
        let exploration = bfs::explore(model, sc.bounds).expect("evaluates");
        let rep = report(model, &exploration);
        let agreement = outcome(model, &rep, "Agreement").verdict();
        let stability = outcome(model, &rep, "Stability").verdict();
        if agreement == checking::Verdict::Refuted {
            assert_eq!(stability, checking::Verdict::Refuted, "{}", sc.id);
        }
    }
}
