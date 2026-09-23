//! The replicated register, lowered under the flat layout, against an independent
//! simulation (bn-23hzh, RFC 0003 correction 4, "Example").
//!
//! # The pair
//!
//! - **subject**: `notes/plan/examples/replicated_register.ctm` (without its fairness
//!   line, which bn-1ln12 owns), elaborated and lowered under the schema example
//!   configuration (`Nat` bounded to `0..=1`, nodes `a b c`, values `v0 v1`, and the
//!   three majority quorums), explored by the reference engine;
//! - **oracle**: [`Register`], a hand-written Rust simulation of the register over
//!   `BTreeMap` and `BTreeSet` values. It shares no code with the lowering or the
//!   elaborator: it is written from the model text, with strict CML evaluation (every
//!   operand of a clause is evaluated, so a read of a missing key fails the whole
//!   clause, even where another operand decides it).
//!
//! # What is pinned
//!
//! 1. the layout the RFC states: 14 slots with their names and domains, and 50 actions
//!    (12 `Stabilize`, 32 `Choose`, 3 `Crash`, 3 `Recover`); the init domain of
//!    419,904 flat states, and `Nat` max 2 refused as `cml.lower.init_domain_too_large`;
//! 2. the reachable states, each decoded from its slots, equal the simulation's, and so
//!    do the labelled transitions from every one of them;
//! 3. the invariant verdicts: `Agreement` and `StableWitness` hold in both, at every
//!    reachable state, and the reference engine's check establishes them;
//! 4. definedness: under a configuration whose `Nodes` omits `c`, `stable[c]` is
//!    missing, and the states where a clause of `Stabilize`, `Choose`, or
//!    `StableWitness` reads it are exactly those where its `#defined` predicate is
//!    false — the typed outcome "undefined read", never a default value;
//! 5. anti-vacuity: dropping `Choose`'s stability requirement makes `StableWitness`
//!    fail in both, at the same states.

use std::collections::{BTreeMap, BTreeSet};

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{definedness_subject, lower_configured};
use continuum_cml_elab::{Limits, LowerErrorKind, Unlowerable, elaborate_source};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::checking::{self, CheckOutcome, DeadlockPolicy, Obligations};
use continuum_model_core::{Model, State};

fn dossier(rel: &str) -> String {
    let path = format!("{}/../../notes/plan/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn source() -> String {
    dossier("examples/replicated_register.ctm").replace("fairness weak Recover", "")
}

fn schema_config() -> String {
    dossier("schemas/examples/replicated-register.run-config.json")
}

fn lowered(src: &str, config: &str) -> Result<Model, LowerErrorKind> {
    let model = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}"));
    let config = RunConfig::parse(config.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"));
    lower_configured(&model, &config, Limits::default())
        .0
        .map(|c| c.model().clone())
        .map_err(|e| e.kind)
}

// ---------------------------------------------------------------------------
// the oracle: an independent simulation
// ---------------------------------------------------------------------------

const NODES: [&str; 3] = ["a", "b", "c"];
const VALUES: [&str; 2] = ["v0", "v1"];
const EPOCHS: [u8; 2] = [0, 1];

/// A read of a key a map lacks: CML's undefined read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Undefined;

/// One state of the register: nodes, epochs, and values by their positions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Reg {
    chosen: BTreeMap<u8, u8>,
    stable: BTreeMap<u8, BTreeMap<u8, u8>>,
    alive: BTreeSet<u8>,
}

/// The register's configuration: the node set and the quorums.
struct Register {
    nodes: BTreeSet<u8>,
    quorums: Vec<BTreeSet<u8>>,
    /// Whether `Choose` requires a stable quorum (the anti-vacuity mutant drops it).
    choose_needs_quorum_stable: bool,
}

/// `stable[n]`, strictly: missing is undefined.
fn at(s: &Reg, n: u8) -> Result<&BTreeMap<u8, u8>, Undefined> {
    s.stable.get(&n).ok_or(Undefined)
}

/// Every subset of the nodes, as `Choose`'s `q` ranges over `Set[Node]`.
fn subsets() -> Vec<BTreeSet<u8>> {
    (0..8_u8)
        .map(|bits| (0..3_u8).filter(|n| bits & (4 >> n) != 0).collect())
        .collect()
}

fn set_text(q: &BTreeSet<u8>) -> String {
    let names: Vec<&str> = q.iter().map(|n| NODES[usize::from(*n)]).collect();
    format!("{{{}}}", names.join(","))
}

impl Register {
    fn init(&self) -> Reg {
        Reg {
            chosen: BTreeMap::new(),
            stable: self.nodes.iter().map(|n| (*n, BTreeMap::new())).collect(),
            alive: self.nodes.clone(),
        }
    }

    /// `Stabilize(n, epoch, value)`: both requires evaluated, then the update.
    fn stabilize(&self, s: &Reg, n: u8, e: u8, v: u8) -> Result<Option<Reg>, Undefined> {
        let alive = s.alive.contains(&n);
        let got = at(s, n)?.get(&e).copied();
        let open = got.is_none() || got == Some(v);
        if !(alive && open) {
            return Ok(None);
        }
        let mut t = s.clone();
        let mut inner = at(s, n)?.clone();
        inner.insert(e, v);
        t.stable.insert(n, inner);
        Ok(Some(t))
    }

    /// `Choose(epoch, value, q)`: every require evaluated (the `forall` at every member
    /// of `q`), then the update.
    fn choose(&self, s: &Reg, e: u8, v: u8, q: &BTreeSet<u8>) -> Result<Option<Reg>, Undefined> {
        let quorum = self.quorums.contains(q);
        let mut stable = true;
        if self.choose_needs_quorum_stable {
            for n in q {
                let got = at(s, *n)?.get(&e).copied();
                stable &= got == Some(v);
            }
        }
        let got = s.chosen.get(&e).copied();
        let open = got.is_none() || got == Some(v);
        if !(quorum && stable && open) {
            return Ok(None);
        }
        let mut t = s.clone();
        t.chosen.insert(e, v);
        Ok(Some(t))
    }

    fn crash(s: &Reg, n: u8) -> Option<Reg> {
        s.alive.contains(&n).then(|| {
            let mut t = s.clone();
            t.alive.remove(&n);
            t
        })
    }

    fn recover(s: &Reg, n: u8) -> Option<Reg> {
        (!s.alive.contains(&n)).then(|| {
            let mut t = s.clone();
            t.alive.insert(n);
            t
        })
    }

    /// Every labelled successor, and the actions with an undefined read here.
    fn step(&self, s: &Reg) -> (BTreeSet<(String, Reg)>, BTreeSet<&'static str>) {
        let mut next = BTreeSet::new();
        let mut undefined = BTreeSet::new();
        for e in EPOCHS {
            for (vi, vn) in VALUES.iter().enumerate() {
                let v = vi as u8;
                for q in subsets() {
                    let label = format!("Choose(epoch={e},value={vn},q={})", set_text(&q));
                    match self.choose(s, e, v, &q) {
                        Ok(Some(t)) => {
                            next.insert((label, t));
                        }
                        Ok(None) => {}
                        Err(Undefined) => {
                            undefined.insert("Choose");
                        }
                    }
                }
                for (ni, nn) in NODES.iter().enumerate() {
                    let label = format!("Stabilize(n={nn},epoch={e},value={vn})");
                    match self.stabilize(s, ni as u8, e, v) {
                        Ok(Some(t)) => {
                            next.insert((label, t));
                        }
                        Ok(None) => {}
                        Err(Undefined) => {
                            undefined.insert("Stabilize");
                        }
                    }
                }
            }
        }
        for (ni, nn) in NODES.iter().enumerate() {
            if let Some(t) = Self::crash(s, ni as u8) {
                next.insert((format!("Crash(n={nn})"), t));
            }
            if let Some(t) = Self::recover(s, ni as u8) {
                next.insert((format!("Recover(n={nn})"), t));
            }
        }
        (next, undefined)
    }

    /// `Agreement`: total, it reads only `chosen`.
    fn agreement(s: &Reg) -> bool {
        let mut ok = true;
        for e in EPOCHS {
            for v1 in 0..2_u8 {
                for v2 in 0..2_u8 {
                    let got = s.chosen.get(&e).copied();
                    ok &= !(got == Some(v1) && got == Some(v2)) || v1 == v2;
                }
            }
        }
        ok
    }

    /// `StableWitness`: strict, so the `exists` is evaluated at every `(epoch, value)`
    /// and the `forall` at every member of every quorum.
    fn stable_witness(&self, s: &Reg) -> Result<bool, Undefined> {
        let mut ok = true;
        for e in EPOCHS {
            for v in 0..2_u8 {
                let chosen = s.chosen.get(&e).copied() == Some(v);
                let mut witness = false;
                for q in &self.quorums {
                    let mut all = true;
                    for n in q {
                        all &= at(s, *n)?.get(&e).copied() == Some(v);
                    }
                    witness |= all;
                }
                ok &= !chosen || witness;
            }
        }
        Ok(ok)
    }

    /// Breadth-first closure from the initial state.
    fn reachable(&self) -> BTreeSet<Reg> {
        let mut seen = BTreeSet::from([self.init()]);
        let mut frontier = vec![self.init()];
        while let Some(s) = frontier.pop() {
            for (_, t) in self.step(&s).0 {
                if seen.insert(t.clone()) {
                    frontier.push(t);
                }
            }
        }
        seen
    }
}

fn majority() -> Vec<BTreeSet<u8>> {
    vec![
        BTreeSet::from([0, 1]),
        BTreeSet::from([0, 2]),
        BTreeSet::from([1, 2]),
    ]
}

// ---------------------------------------------------------------------------
// reading a lowered state
// ---------------------------------------------------------------------------

/// The value of the slot named `name` in `state`.
fn slot(model: &Model, state: &State, name: &str) -> i64 {
    model
        .binding(state, name)
        .unwrap_or_else(|| panic!("no slot {name}"))
}

/// A lowered state, read slot by slot by the names the RFC's layout gives them.
fn decode(model: &Model, state: &State) -> Reg {
    let mut r = Reg {
        chosen: BTreeMap::new(),
        stable: BTreeMap::new(),
        alive: BTreeSet::new(),
    };
    for e in EPOCHS {
        let c = slot(model, state, &format!("chosen[{e}]"));
        if c > 0 {
            r.chosen.insert(e, (c - 1) as u8);
        }
    }
    for (ni, nn) in NODES.iter().enumerate() {
        if slot(model, state, &format!("alive{{{nn}}}")) == 1 {
            r.alive.insert(ni as u8);
        }
        let present = slot(model, state, &format!("stable[{nn}]?"));
        let mut inner = BTreeMap::new();
        for e in EPOCHS {
            let c = slot(model, state, &format!("stable[{nn}]![{e}]"));
            if present == 1 && c > 0 {
                inner.insert(e, (c - 1) as u8);
            } else if present == 0 {
                assert_eq!(c, 0, "an absent entry's slots are at their minimum");
            }
        }
        if present == 1 {
            r.stable.insert(ni as u8, inner);
        }
    }
    r
}

fn predicate(model: &Model, state: &State, name: &str) -> bool {
    let i = model
        .predicate_index(name)
        .unwrap_or_else(|| panic!("no predicate {name}"));
    model.evaluate_predicate(i, state).expect("total")
}

/// A definedness predicate, which exists only where the lowering found a map read.
fn defined(model: &Model, state: &State, name: &str) -> bool {
    model
        .predicate_index(name)
        .is_none_or(|i| model.evaluate_predicate(i, state).expect("total"))
}

/// Explore the lowered model and compare it with the simulation, state by state.
/// Returns the number of reachable states and the actions and invariant found
/// undefined somewhere.
fn differential(model: &Model, oracle: &Register) -> (usize, BTreeSet<String>) {
    let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates");
    let reached = exploration.closed().expect("the exploration closes");
    let mut decoded = BTreeSet::new();
    let mut undefined_anywhere = BTreeSet::new();
    for state in reached.states() {
        let r = decode(model, state);
        // The labelled transitions.
        let lowered: BTreeSet<(String, Reg)> = model
            .successors(state)
            .expect("no evaluation error")
            .into_iter()
            .map(|step| {
                (
                    model.actions()[step.action()].name().as_str().to_owned(),
                    decode(model, step.target()),
                )
            })
            .collect();
        let (expected, undefined) = oracle.step(&r);
        assert_eq!(lowered, expected, "transitions from {r:?}");
        // Definedness of each action.
        for action in ["Choose", "Stabilize"] {
            let defined = defined(model, state, &format!("{action}#defined"));
            assert_eq!(!defined, undefined.contains(action), "{action} at {r:?}");
            if !defined {
                undefined_anywhere.insert(action.to_owned());
            }
        }
        // The invariants.
        assert_eq!(
            predicate(model, state, "Agreement"),
            Register::agreement(&r)
        );
        let defined = predicate(model, state, "StableWitness#defined");
        match oracle.stable_witness(&r) {
            Ok(v) => {
                assert!(defined, "StableWitness is defined at {r:?}");
                assert_eq!(predicate(model, state, "StableWitness"), v, "at {r:?}");
            }
            Err(Undefined) => {
                assert!(!defined, "StableWitness is undefined at {r:?}");
                undefined_anywhere.insert("StableWitness".to_owned());
            }
        }
        decoded.insert(r);
    }
    assert_eq!(decoded, oracle.reachable(), "reachable states");
    (decoded.len(), undefined_anywhere)
}

// ---------------------------------------------------------------------------
// the tests
// ---------------------------------------------------------------------------

#[test]
fn the_register_lowers_to_the_layout_the_rfc_states() {
    let model = lowered(&source(), &schema_config()).expect("lowers");
    let slots: Vec<(String, i64, i64)> = model
        .variables()
        .iter()
        .map(|v| {
            (
                v.name().as_str().to_owned(),
                v.domain().lo(),
                v.domain().hi(),
            )
        })
        .collect();
    let mut expected = vec![
        ("chosen[0]".to_owned(), 0, 2),
        ("chosen[1]".to_owned(), 0, 2),
    ];
    for n in NODES {
        expected.push((format!("stable[{n}]?"), 0, 1));
        expected.push((format!("stable[{n}]![0]"), 0, 2));
        expected.push((format!("stable[{n}]![1]"), 0, 2));
        expected.push((format!("alive{{{n}}}"), 0, 1));
    }
    expected.sort();
    assert_eq!(slots, expected, "14 slots");
    assert_eq!(slots.len(), 14);

    let count = |prefix: &str| {
        model
            .actions()
            .iter()
            .filter(|a| a.name().as_str().starts_with(prefix))
            .count()
    };
    assert_eq!(model.actions().len(), 50);
    assert_eq!(
        (
            count("Stabilize("),
            count("Choose("),
            count("Crash("),
            count("Recover(")
        ),
        (12, 32, 3, 3)
    );
    assert!(
        model
            .action_index("Choose(epoch=1,value=v0,q={a,c})")
            .is_some()
    );
    // The flat init domain is the product of the slot domains.
    assert_eq!(model.domain_cardinality(), 419_904);
    assert_eq!(
        model.initial_states().len(),
        1,
        "one canonical initial state"
    );
    // The map reads of `stable[n]` give definedness predicates; `chosen.get` is total.
    let predicates: Vec<&str> = model
        .predicates()
        .iter()
        .map(|p| p.name().as_str())
        .collect();
    assert_eq!(
        predicates,
        [
            "Agreement",
            "Choose#defined",
            "Stabilize#defined",
            "StableWitness",
            "StableWitness#defined"
        ]
    );
    assert_eq!(definedness_subject("Choose#defined"), Some("Choose"));
    assert_eq!(definedness_subject("Agreement"), None);
}

#[test]
fn a_wider_bound_is_refused_before_enumeration() {
    let wider = schema_config().replace(r#""max": 1"#, r#""max": 2"#);
    assert_ne!(wider, schema_config());
    assert_eq!(
        lowered(&source(), &wider).expect_err("34,012,224 flat states"),
        LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge)
    );
}

#[test]
fn the_register_agrees_with_the_simulation_and_the_engine_establishes_its_invariants() {
    let model = lowered(&source(), &schema_config()).expect("lowers");
    let oracle = Register {
        nodes: BTreeSet::from([0, 1, 2]),
        quorums: majority(),
        choose_needs_quorum_stable: true,
    };
    let (states, undefined) = differential(&model, &oracle);
    assert!(states > 1, "the register moves");
    assert!(
        undefined.is_empty(),
        "every node has a stable map: {undefined:?}"
    );

    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("evaluates");
    let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
    let report = checking::check(&model, &exploration, &obligations).expect("checks");
    for name in [
        "Agreement",
        "StableWitness",
        "StableWitness#defined",
        "Choose#defined",
        "Stabilize#defined",
    ] {
        let i = model.predicate_index(name).expect("declared");
        assert_eq!(
            report.invariant(i).expect("checked").outcome(),
            &CheckOutcome::Holds { states },
            "{name}"
        );
    }
}

/// `Nodes` without `c`: `stable[c]` is missing, so every clause that reads it is
/// undefined — `Stabilize(n=c, …)`'s guard (strict: even where `c` is not alive),
/// `Choose` for every `q` that holds `c`, and `StableWitness` (strict: at every state,
/// since two quorums hold `c`). The `#defined` predicates are false at exactly the
/// simulation's undefined states, and the engine reports them as violated.
#[test]
fn a_missing_key_is_a_typed_undefined_read_at_exactly_the_simulations_states() {
    let short = schema_config().replacen(
        r#",
        {
          "elem": {
            "sort": "Node",
            "name": "c"
          }
        }
      ]
    },
    "Quorum""#,
        r#"
      ]
    },
    "Quorum""#,
        1,
    );
    assert_ne!(
        short,
        schema_config(),
        "the configuration drops c from Nodes"
    );
    let model = lowered(&source(), &short).expect("lowers");
    let oracle = Register {
        nodes: BTreeSet::from([0, 1]),
        quorums: majority(),
        choose_needs_quorum_stable: true,
    };
    let (_, undefined) = differential(&model, &oracle);
    assert_eq!(
        undefined,
        BTreeSet::from([
            "Choose".to_owned(),
            "Stabilize".to_owned(),
            "StableWitness".to_owned()
        ])
    );
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("evaluates");
    let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
    let report = checking::check(&model, &exploration, &obligations).expect("checks");
    for name in [
        "Choose#defined",
        "Stabilize#defined",
        "StableWitness#defined",
    ] {
        let i = model.predicate_index(name).expect("declared");
        let outcome = report.invariant(i).expect("checked").outcome();
        assert!(
            matches!(outcome, CheckOutcome::Violated { depth: 0, .. }),
            "{name}: {outcome}"
        );
        assert!(definedness_subject(name).is_some());
    }
}

/// Anti-vacuity: without `Choose`'s stability requirement a value can be chosen with no
/// stable quorum, and `StableWitness` fails — in the simulation and in the lowered
/// model, at the same states.
#[test]
fn dropping_the_stability_requirement_breaks_stable_witness_in_both() {
    let mutant = source().replace(
        "  require forall n in q: stable[n].get(epoch) == Some(value)\n",
        "",
    );
    assert_ne!(mutant, source());
    let model = lowered(&mutant, &schema_config()).expect("lowers");
    let oracle = Register {
        nodes: BTreeSet::from([0, 1, 2]),
        quorums: majority(),
        choose_needs_quorum_stable: false,
    };
    differential(&model, &oracle);
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("evaluates");
    let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
    let report = checking::check(&model, &exploration, &obligations).expect("checks");
    let i = model.predicate_index("StableWitness").expect("declared");
    assert!(matches!(
        report.invariant(i).expect("checked").outcome(),
        CheckOutcome::Violated { depth: 1, .. }
    ));
}
