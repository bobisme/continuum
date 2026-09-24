//! PR 15a exit: the replicated register's CML lowering and its programmatic
//! equivalent have one semantic model identity (bn-3jz, ADR-0013, RFC 0003).
//!
//! # What this pins
//!
//! [`ModelBuilder::build`] discards declaration order (`continuum-model-core/src/
//! identity.rs`), so a hand-built model and a lowered one are the same identity
//! exactly when they are the same variables, the same actions with the same guard and
//! outcome *expression trees*, the same initial states, the same predicates, and the
//! same fairness. [`programmatic_register`] below is that hand-built twin: it is
//! written from RFC 0003 ("Finite types and quantifiers", correction 4; "Fairness",
//! correction 5) and `notes/plan/examples/replicated_register.ctm`'s own text, not by
//! calling anything in `continuum_cml_elab::lower`. Its guard and update trees follow
//! that RFC's stated rules for what the flat layout and its expressions *mean*:
//!
//! - the flat layout ("Layout", RFC 0003) — 14 slots, `chosen[e]`, `stable[n]?` /
//!   `stable[n]![e]`, `alive{n}` — under the schema example configuration (`Nat` max
//!   1, three nodes, two values, the three majority quorums);
//! - a fully written composite update: every slot of a variable a `next` clause
//!   touches is re-assigned, including the slots it does not change (as a plain read
//!   of itself, or, for `alive`'s `union`/`\`, as `max`/`max(·-·,0)` over `0`), never
//!   omitted as `unchanged`.
//!
//! Where the RFC states a rule's *meaning* ("balanced", "the conjunction of its
//! items") but not its exact tree shape, and the shape is itself identity-bearing
//! (RFC 0003, "Layout": "changing it needs an epoch note"), [`balanced_and`] and
//! [`balanced_or`] reproduce the lowering's own documented canonical form instead of
//! guessing at one: `continuum_cml_elab::lower::balanced`'s doc comment states
//! "adjacent pairs left to right" (`crates/continuum-cml-elab/src/lower.rs`), and a
//! `#defined` predicate is one flat pool over every instance's items (`D(U)` omitted
//! entirely, not `G => true`, where an instance's update reads nothing composite),
//! confirmed against the real lowered model's `{:#?}` dump during development the same
//! way one checks a hand-written wire-compatible encoder against real traffic. No test
//! here builds [`programmatic_register`] by calling
//! `continuum_cml_elab::lower::lower_configured` or anything else in that module;
//! [`lower_register`] calls it once, to produce the *subject* the twin is compared
//! against, never to construct the twin itself.
//!
//! # What is asserted
//!
//! 1. the lowered register's identity equals the hand-built one (and, more strongly,
//!    the two `Model`s are equal outright — `assert_eq!` on the whole structure, not
//!    only its digest);
//! 2. both equal a committed golden hex file (regenerate with
//!    `CML_IDENTITY_BLESS=1 cargo test -p continuum-cml-elab --test
//!    pr15a_identity`, and review the diff);
//! 3. anti-vacuity: three mutants of the hand-built twin — one changed guard clause,
//!    one changed fairness scope, one widened slot bound — each give a different
//!    identity, so the golden is not trivially satisfied by any model of the right
//!    shape.

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::lower_configured;
use continuum_cml_elab::{Limits, elaborate_source};
use continuum_model_core::{ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder, Strength};

const NODES: [&str; 3] = ["a", "b", "c"];
const VALUES: [&str; 2] = ["v0", "v1"];
const EPOCHS: [i64; 2] = [0, 1];

/// The three majority quorums of `{a, b, c}`, in ascending characteristic-vector
/// order (RFC 0003 "Finite types"): `{b,c}` (3), `{a,c}` (5), `{a,b}` (6).
const QUORUMS: [[&str; 2]; 3] = [["b", "c"], ["a", "c"], ["a", "b"]];

// ---------------------------------------------------------------------------
// slot names (RFC 0003 "Layout")
// ---------------------------------------------------------------------------

fn alive(n: &str) -> String {
    format!("alive{{{n}}}")
}

fn chosen(e: i64) -> String {
    format!("chosen[{e}]")
}

fn stable_def(n: &str) -> String {
    format!("stable[{n}]?")
}

fn stable_val(n: &str, e: i64) -> String {
    format!("stable[{n}]![{e}]")
}

/// `Option[Value]`'s code: `None` is `0`, `Some(v)` is `1 + index(v)`.
fn value_code(v: &str) -> i64 {
    1 + i64::try_from(VALUES.iter().position(|x| *x == v).expect("declared value")).unwrap()
}

/// The 8 subsets of `NODES`, ascending by characteristic-vector index (`a` heaviest),
/// each as its member names ascending and its own index.
fn subsets() -> Vec<(i64, Vec<&'static str>)> {
    (0_i64..8)
        .map(|i| {
            let members: Vec<&'static str> = (0_i64..3)
                .filter(|j| (i >> (2 - j)) & 1 == 1)
                .map(|j| NODES[usize::try_from(j).unwrap()])
                .collect();
            (i, members)
        })
        .collect()
}

fn set_text(members: &[&str]) -> String {
    format!("{{{}}}", members.join(","))
}

/// A `Set[Node]` value's index in `U(Set[Node])`: the characteristic vector read as a
/// binary number, `a`'s bit heaviest.
fn set_index(members: &[&str]) -> i64 {
    members
        .iter()
        .map(|m| {
            let pos =
                i64::try_from(NODES.iter().position(|n| n == m).expect("declared node")).unwrap();
            1_i64 << (2 - pos)
        })
        .sum()
}

// ---------------------------------------------------------------------------
// the balanced fold (RFC 0003 "Quantifier expansion")
// ---------------------------------------------------------------------------

/// Pair adjacent items front to back; an odd one out carries to the next round
/// unchanged. Repeat until one item remains. This is the shape every guard, its
/// definedness items, and every quantifier expansion in the register share.
fn balanced(items: Vec<BoolExpr>, combine: fn(BoolExpr, BoolExpr) -> BoolExpr) -> BoolExpr {
    let mut level = items;
    assert!(!level.is_empty(), "a balanced fold needs at least one item");
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut it = level.into_iter();
        while let Some(first) = it.next() {
            match it.next() {
                Some(second) => next.push(combine(first, second)),
                None => next.push(first),
            }
        }
        level = next;
    }
    level.pop().expect("checked non-empty above")
}

fn balanced_and(items: Vec<BoolExpr>) -> BoolExpr {
    balanced(items, BoolExpr::and)
}

fn balanced_or(items: Vec<BoolExpr>) -> BoolExpr {
    balanced(items, BoolExpr::or)
}

fn eq_var(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, IntExpr::var(name), IntExpr::constant(value))
}

// ---------------------------------------------------------------------------
// whole-layout writes (RFC 0003: "every lowered write writes a whole canonical
// layout, slot by slot")
// ---------------------------------------------------------------------------

/// `alive = alive \ {target}`: `max(alive{n} - (1 if n == target else 0), 0)` for
/// every node, the untouched ones included.
fn whole_alive_crash(target: &str) -> Vec<(String, IntExpr)> {
    NODES
        .iter()
        .map(|&n| {
            let delta = i64::from(n == target);
            let expr = IntExpr::max(
                IntExpr::minus(IntExpr::var(&alive(n)), IntExpr::constant(delta)),
                IntExpr::constant(0),
            );
            (alive(n), expr)
        })
        .collect()
}

/// `alive = alive union {target}`: `max(alive{n}, 1 if n == target else 0)`.
fn whole_alive_recover(target: &str) -> Vec<(String, IntExpr)> {
    NODES
        .iter()
        .map(|&n| {
            let add = i64::from(n == target);
            let expr = IntExpr::max(IntExpr::var(&alive(n)), IntExpr::constant(add));
            (alive(n), expr)
        })
        .collect()
}

/// `chosen = chosen.put(target_e, new_code)`: the target key gets the new code, every
/// other key is a plain read of itself.
fn whole_chosen_write(target_e: i64, new_code: i64) -> Vec<(String, IntExpr)> {
    EPOCHS
        .iter()
        .map(|&e| {
            let expr = if e == target_e {
                IntExpr::constant(new_code)
            } else {
                IntExpr::var(&chosen(e))
            };
            (chosen(e), expr)
        })
        .collect()
}

/// `stable = stable[target_n := stable[target_n].put(target_e, new_code)]`: the
/// target node's presence bit is reasserted and its target epoch gets the new code;
/// every other slot of every node, target node included, is a plain read of itself.
fn whole_stable_write(target_n: &str, target_e: i64, new_code: i64) -> Vec<(String, IntExpr)> {
    let mut out = Vec::new();
    for &n in &NODES {
        if n == target_n {
            out.push((stable_def(n), IntExpr::constant(1)));
            for &e in &EPOCHS {
                let expr = if e == target_e {
                    IntExpr::constant(new_code)
                } else {
                    IntExpr::var(&stable_val(n, e))
                };
                out.push((stable_val(n, e), expr));
            }
        } else {
            out.push((stable_def(n), IntExpr::var(&stable_def(n))));
            for &e in &EPOCHS {
                out.push((stable_val(n, e), IntExpr::var(&stable_val(n, e))));
            }
        }
    }
    out
}

fn deterministic(name: String, guard: BoolExpr, updates: Vec<(String, IntExpr)>) -> ActionDecl {
    let updates: Vec<(&str, IntExpr)> = updates
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();
    ActionDecl::deterministic(&name, guard, updates)
}

// ---------------------------------------------------------------------------
// one deliberate change, to prove the golden is not vacuous
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutation {
    /// The real register.
    None,
    /// `Stabilize(n=a,epoch=0,value=v0)` drops its `require n in alive` clause: one
    /// changed guard.
    DroppedAliveGuard,
    /// `fairness weak Recover` becomes a weak assumption over `Crash`'s three
    /// instances instead: one changed fairness scope.
    FairnessOverCrash,
    /// `chosen[0]`'s domain widens from `0..=2` to `0..=3`: one changed slot bound.
    WidenedChosenBound,
}

// ---------------------------------------------------------------------------
// Stabilize, Choose, Crash, Recover (RFC 0003 "Definedness", "Action schemas")
// ---------------------------------------------------------------------------

/// `Stabilize(n, epoch, value)`'s explicit guard (`require n in alive`, `require
/// stable[n].get(epoch) in {None, Some(value)}`) and the one definedness item its
/// reads of `stable[n]` share (the same read occurs once in the guard and once in the
/// update, so the item appears twice, undeduplicated — `Model::identity`'s doc
/// module and `tests/register.rs`'s `Stabilize#defined` agree).
fn stabilize_clauses(n: &str, e: i64, v: &str, mutation: Mutation) -> (Vec<BoolExpr>, BoolExpr) {
    let mut explicit = Vec::new();
    if !(mutation == Mutation::DroppedAliveGuard && n == "a" && e == 0 && v == "v0") {
        explicit.push(eq_var(&alive(n), 1));
    }
    explicit.push(BoolExpr::or(
        eq_var(&stable_val(n, e), 0),
        eq_var(&stable_val(n, e), value_code(v)),
    ));
    let defined = eq_var(&stable_def(n), 1);
    (explicit, defined)
}

fn add_stabilize(mut b: ModelBuilder, mutation: Mutation) -> ModelBuilder {
    for &n in &NODES {
        for &e in &EPOCHS {
            for &v in &VALUES {
                let (explicit, defined) = stabilize_clauses(n, e, v, mutation);
                let mut items = explicit;
                items.push(defined.clone());
                items.push(defined);
                let guard = balanced_and(items);
                let updates = whole_stable_write(n, e, value_code(v));
                b = b.action(deterministic(
                    format!("Stabilize(n={n},epoch={e},value={v})"),
                    guard,
                    updates,
                ));
            }
        }
    }
    b
}

/// `Stabilize#defined`: one flat pool of every instance's `[D(G), G => D(U)]`, in
/// instance order (`n` outer, `epoch` middle, `value` fastest), one balanced
/// conjunction over the whole 24-item pool.
fn stabilize_defined_items() -> Vec<BoolExpr> {
    let mut items = Vec::new();
    for &n in &NODES {
        for &e in &EPOCHS {
            for &v in &VALUES {
                let (explicit, defined) = stabilize_clauses(n, e, v, Mutation::None);
                let guard_explicit = balanced_and(explicit);
                items.push(defined.clone());
                items.push(BoolExpr::implies(guard_explicit, defined));
            }
        }
    }
    items
}

/// `Choose(epoch, value, q)`'s explicit guard (`require q in Quorum`, `require forall
/// n in q: ...`, `require chosen.get(epoch) in {...}`) and the definedness items its
/// `forall n in q` reads of `stable[n]` give, one per node of `q`, ascending.
fn choose_clauses(e: i64, v: &str, members: &[&str]) -> (Vec<BoolExpr>, Vec<BoolExpr>) {
    let quorum_disjunction = balanced_or(
        QUORUMS
            .iter()
            .map(|q| {
                BoolExpr::compare(
                    CmpOp::Eq,
                    IntExpr::constant(set_index(members)),
                    IntExpr::constant(set_index(q)),
                )
            })
            .collect(),
    );
    let forall_stable = if members.is_empty() {
        BoolExpr::constant(true)
    } else {
        balanced_and(
            members
                .iter()
                .map(|&n| eq_var(&stable_val(n, e), value_code(v)))
                .collect(),
        )
    };
    let chosen_in_set = balanced_or(vec![
        eq_var(&chosen(e), 0),
        eq_var(&chosen(e), value_code(v)),
    ]);
    let defined: Vec<BoolExpr> = members.iter().map(|&n| eq_var(&stable_def(n), 1)).collect();
    (
        vec![quorum_disjunction, forall_stable, chosen_in_set],
        defined,
    )
}

fn add_choose(mut b: ModelBuilder) -> ModelBuilder {
    for &e in &EPOCHS {
        for &v in &VALUES {
            for (_, members) in subsets() {
                let (explicit, defined) = choose_clauses(e, v, &members);
                let mut items = explicit;
                items.extend(defined);
                let guard = balanced_and(items);
                let updates = whole_chosen_write(e, value_code(v));
                b = b.action(deterministic(
                    format!("Choose(epoch={e},value={v},q={})", set_text(&members)),
                    guard,
                    updates,
                ));
            }
        }
    }
    b
}

/// `Choose#defined`: `D(U)` is empty (its update reads no `stable`), so each
/// instance's contribution is its raw `D(G)` items, undeduplicated and ungrouped —
/// one flat pool of 48 items over all 32 instances (`epoch`, `value`, `q` ascending),
/// one balanced conjunction over the whole pool.
fn choose_defined_items() -> Vec<BoolExpr> {
    let mut items = Vec::new();
    for &e in &EPOCHS {
        for &v in &VALUES {
            for (_, members) in subsets() {
                let (_, defined) = choose_clauses(e, v, &members);
                items.extend(defined);
            }
        }
    }
    items
}

fn add_crash_recover(mut b: ModelBuilder) -> ModelBuilder {
    for &n in &NODES {
        let guard = eq_var(&alive(n), 1);
        b = b.action(deterministic(
            format!("Crash(n={n})"),
            guard,
            whole_alive_crash(n),
        ));
    }
    for &n in &NODES {
        let guard = BoolExpr::negate(eq_var(&alive(n), 1));
        b = b.action(deterministic(
            format!("Recover(n={n})"),
            guard,
            whole_alive_recover(n),
        ));
    }
    b
}

// ---------------------------------------------------------------------------
// Agreement, StableWitness (RFC 0003 "Quantifier expansion")
// ---------------------------------------------------------------------------

fn agreement_body() -> BoolExpr {
    let mut items = Vec::new();
    for &e in &EPOCHS {
        for &v1 in &VALUES {
            for &v2 in &VALUES {
                let antecedent = BoolExpr::and(
                    eq_var(&chosen(e), value_code(v1)),
                    eq_var(&chosen(e), value_code(v2)),
                );
                let i1 = i64::try_from(VALUES.iter().position(|x| *x == v1).unwrap()).unwrap();
                let i2 = i64::try_from(VALUES.iter().position(|x| *x == v2).unwrap()).unwrap();
                let consequent =
                    BoolExpr::compare(CmpOp::Eq, IntExpr::constant(i1), IntExpr::constant(i2));
                items.push(BoolExpr::implies(antecedent, consequent));
            }
        }
    }
    balanced_and(items)
}

fn stable_witness_body() -> BoolExpr {
    let mut items = Vec::new();
    for &e in &EPOCHS {
        for &v in &VALUES {
            let antecedent = eq_var(&chosen(e), value_code(v));
            let quorum_items: Vec<BoolExpr> = QUORUMS
                .iter()
                .map(|members| {
                    balanced_and(
                        members
                            .iter()
                            .map(|&n| eq_var(&stable_val(n, e), value_code(v)))
                            .collect(),
                    )
                })
                .collect();
            items.push(BoolExpr::implies(antecedent, balanced_or(quorum_items)));
        }
    }
    balanced_and(items)
}

/// `StableWitness#defined`: `exists q in Quorum` is an unguarded exact enumeration
/// (a configuration constant set of exactly `q`'s type), so its items are recorded
/// directly, with no `guard =>` wrapper; likewise the outer `forall epoch, value`
/// (the whole finite universe). One flat pool of 24 items, one balanced conjunction.
fn stable_witness_defined_items() -> Vec<BoolExpr> {
    let mut items = Vec::new();
    for _e in EPOCHS {
        for _v in VALUES {
            for members in &QUORUMS {
                for &n in members {
                    items.push(eq_var(&stable_def(n), 1));
                }
            }
        }
    }
    items
}

// ---------------------------------------------------------------------------
// the whole model
// ---------------------------------------------------------------------------

fn build_register(mutation: Mutation) -> Model {
    let mut b = ModelBuilder::new();
    for &n in &NODES {
        b = b.variable(&alive(n), 0, 1);
    }
    for &e in &EPOCHS {
        let hi = if mutation == Mutation::WidenedChosenBound && e == 0 {
            3
        } else {
            2
        };
        b = b.variable(&chosen(e), 0, hi);
    }
    for &n in &NODES {
        b = b.variable(&stable_def(n), 0, 1);
        for &e in &EPOCHS {
            b = b.variable(&stable_val(n, e), 0, 2);
        }
    }

    let mut init: Vec<(String, i64)> = Vec::new();
    for &n in &NODES {
        init.push((alive(n), 1));
    }
    for &e in &EPOCHS {
        init.push((chosen(e), 0));
    }
    for &n in &NODES {
        init.push((stable_def(n), 1));
        for &e in &EPOCHS {
            init.push((stable_val(n, e), 0));
        }
    }
    let init_ref: Vec<(&str, i64)> = init.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    b = b.initial_state(&init_ref);

    b = add_stabilize(b, mutation);
    b = add_choose(b);
    b = add_crash_recover(b);

    b = b.predicate("Agreement", agreement_body());
    b = b.predicate("Choose#defined", balanced_and(choose_defined_items()));
    b = b.predicate("Stabilize#defined", balanced_and(stabilize_defined_items()));
    b = b.predicate("StableWitness", stable_witness_body());
    b = b.predicate(
        "StableWitness#defined",
        balanced_and(stable_witness_defined_items()),
    );

    let fairness_scope: Vec<String> = if mutation == Mutation::FairnessOverCrash {
        NODES.iter().map(|n| format!("Crash(n={n})")).collect()
    } else {
        NODES.iter().map(|n| format!("Recover(n={n})")).collect()
    };
    b = b.fairness(Strength::Weak, fairness_scope);

    b.build().expect("a valid programmatic register")
}

/// The hand-built twin: RFC 0003 and the register's own text, never
/// `continuum_cml_elab::lower`.
fn programmatic_register() -> Model {
    build_register(Mutation::None)
}

// ---------------------------------------------------------------------------
// the subject: the real CML lowering
// ---------------------------------------------------------------------------

fn dossier(rel: &str) -> String {
    let path = format!("{}/../../notes/plan/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn lower_register() -> Model {
    let src = dossier("examples/replicated_register.ctm");
    let config_text = dossier("schemas/examples/replicated-register.run-config.json");
    let model = elaborate_source(&src).expect("the register elaborates");
    let config = RunConfig::parse(config_text.as_bytes()).expect("the schema example reads");
    lower_configured(&model, &config, Limits::default())
        .0
        .expect("the whole register lowers (bn-1ln12)")
        .model()
        .clone()
}

// ---------------------------------------------------------------------------
// the golden
// ---------------------------------------------------------------------------

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

fn golden_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/replicated_register.identity.hex")
}

#[test]
fn the_lowered_register_and_the_programmatic_register_have_one_identity() {
    let lowered = lower_register();
    let hand = programmatic_register();

    // The stronger check first: not only one identity, one whole model. A mismatch
    // here names the differing field; a bare identity mismatch would only differ in
    // bytes.
    assert_eq!(lowered, hand, "the lowered and hand-built registers differ");
    assert_eq!(lowered.identity(), hand.identity());

    let got = hex(hand.identity().as_bytes());
    let path = golden_path();
    if std::env::var_os("CML_IDENTITY_BLESS").is_some() {
        std::fs::write(&path, &got).expect("write golden");
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (regenerate with CML_IDENTITY_BLESS=1)",
            path.display()
        )
    });
    assert_eq!(got, want, "the replicated register's identity changed");
    assert_eq!(hex(lowered.identity().as_bytes()), want);
}

#[test]
fn a_dropped_guard_clause_changes_the_identity() {
    let base = programmatic_register();
    let mutant = build_register(Mutation::DroppedAliveGuard);
    assert_ne!(base, mutant);
    assert_ne!(base.identity(), mutant.identity());
}

#[test]
fn a_different_fairness_scope_changes_the_identity() {
    let base = programmatic_register();
    let mutant = build_register(Mutation::FairnessOverCrash);
    assert_ne!(base, mutant);
    assert_ne!(base.identity(), mutant.identity());
}

#[test]
fn a_widened_slot_bound_changes_the_identity() {
    let base = programmatic_register();
    let mutant = build_register(Mutation::WidenedChosenBound);
    assert_ne!(base, mutant);
    assert_ne!(base.identity(), mutant.identity());
}
