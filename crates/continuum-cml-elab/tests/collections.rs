//! Collection values under the flat layout: exactness against a brute-force oracle,
//! definedness, static keys, and typed refusals (bn-23hzh, RFC 0003 correction 4,
//! "Layout", "Static keys", "Expressions", "Definedness").
//!
//! # The oracle
//!
//! [`eval`] evaluates the normalized AST over real values — `BTreeSet`, `BTreeMap`,
//! options, tuples — written here, independent of the lowering: every operand is
//! evaluated (strict, as CML is), a read `m[k]` of a missing key fails with
//! [`Fail::Undefined`], and checked arithmetic fails with [`Fail::Overflow`]. A
//! quantifier evaluates its body at the members of its domain and nowhere else.
//!
//! # What is pinned
//!
//! - **invariants**: for every canonical state of the frame's slot domain (30,720 of
//!   49,152 flat states), and every hand-written invariant: where the oracle has a value
//!   the lowered invariant has the same one and its `#defined` predicate (if any) holds;
//!   where the oracle reads a missing key, `#defined` is false; where the oracle fails,
//!   the lowered model fails or `#defined` is false — the lowering neither adds nor
//!   drops an error;
//! - **actions**: at every canonical state, the labelled successors of the lowered
//!   model are the oracle's, and `A#defined` is false exactly where some instance of
//!   `A` fails in the oracle (a guard read, or an update read where the guard holds);
//! - **refusals**: dynamic keys, static values outside the universe, unsafe computed
//!   payloads, duplicate map keys, an undefined read in init, a composite relational
//!   variable, and a guarded body that may overflow — each its typed code.

use std::collections::{BTreeMap, BTreeSet};

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::lower_configured;
use continuum_cml_elab::norm::{BinOp, Builtin, Expr, ExprKind, Next, Quant};
use continuum_cml_elab::{Limits, LowerErrorKind, NormModel, Type, Unlowerable, elaborate_source};
use continuum_model_core::{Model, State};

const HEAD: &str = r#""schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1"#;

/// `Nat` bounded to `0..=2`, `Int` to `-3..=3`.
fn config(model: &str) -> RunConfig {
    let text = format!(
        r#"{{{HEAD},"model":"{model}","bounds":{{"Nat":{{"max":2}},"Int":{{"min":-3,"max":3}}}},"sorts":{{}},"constants":{{}}}}"#
    );
    RunConfig::parse(text.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"))
}

fn lowered(src: &str) -> Result<(NormModel, Model), Unlowerable> {
    let model = elaborate_source(src).unwrap_or_else(|e| panic!("elaborates: {e}\n{src}"));
    let low = lower_configured(&model, &config(&model.name), Limits::default())
        .0
        .map(|c| c.model().clone())
        .map_err(|e| match e.kind {
            LowerErrorKind::Unlowerable(u) => u,
            other => panic!("not a typed refusal: {other:?}"),
        })?;
    Ok((model, low))
}

// ---------------------------------------------------------------------------
// the oracle
// ---------------------------------------------------------------------------

/// A CML value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum V {
    I(i64),
    B(bool),
    Set(BTreeSet<V>),
    Map(BTreeMap<V, V>),
    Opt(Option<Box<V>>),
    Tup(Vec<V>),
}

/// Why the oracle has no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fail {
    Undefined,
    Overflow,
}

const NAT_MAX: i64 = 2;
const INT: (i64, i64) = (-3, 3);

struct Ev<'a> {
    state: &'a BTreeMap<String, V>,
    params: BTreeMap<String, V>,
    binders: BTreeMap<u32, V>,
}

fn int(v: &V) -> i64 {
    match v {
        V::I(n) => *n,
        V::B(b) => i64::from(*b),
        other => panic!("not an integer: {other:?}"),
    }
}

fn boolean(v: &V) -> bool {
    match v {
        V::B(b) => *b,
        other => panic!("not a Boolean: {other:?}"),
    }
}

/// Every value of a finite type the frame uses.
fn universe(ty: &Type) -> Vec<V> {
    match ty {
        Type::Bool => vec![V::B(false), V::B(true)],
        Type::Nat => (0..=NAT_MAX).map(V::I).collect(),
        Type::Int => (INT.0..=INT.1).map(V::I).collect(),
        Type::Option(t) => std::iter::once(V::Opt(None))
            .chain(universe(t).into_iter().map(|v| V::Opt(Some(Box::new(v)))))
            .collect(),
        Type::Set(t) => {
            let items = universe(t);
            (0..1_u32 << items.len())
                .map(|bits| {
                    V::Set(
                        items
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| bits & (1 << i) != 0)
                            .map(|(_, v)| v.clone())
                            .collect(),
                    )
                })
                .collect()
        }
        other => panic!("the oracle has no universe for {other:?}"),
    }
}

fn members(v: V) -> BTreeSet<V> {
    match v {
        V::Set(s) => s,
        other => panic!("not a set: {other:?}"),
    }
}

fn eval(e: &Expr, ev: &mut Ev<'_>) -> Result<V, Fail> {
    let arith = |x: i64, y: i64, op: BinOp| -> Result<V, Fail> {
        match op {
            BinOp::Add => x.checked_add(y),
            BinOp::Sub => x.checked_sub(y),
            _ => x.checked_mul(y),
        }
        .map(V::I)
        .ok_or(Fail::Overflow)
    };
    Ok(match &e.kind {
        ExprKind::Bool(b) => V::B(*b),
        ExprKind::Int(n) => V::I(*n),
        ExprKind::State(v) => ev.state.get(v).cloned().expect("a state variable"),
        ExprKind::Param(p) => ev.params.get(p).cloned().expect("a parameter"),
        ExprKind::Bound { binder, .. } => ev.binders.get(binder).cloned().expect("a binder"),
        ExprKind::OptionNone => V::Opt(None),
        ExprKind::OptionSome(x) => V::Opt(Some(Box::new(eval(x, ev)?))),
        ExprKind::Tuple(xs) => {
            let mut out = Vec::new();
            for x in xs {
                out.push(eval(x, ev)?);
            }
            V::Tup(out)
        }
        ExprKind::SetLit(xs) => {
            let mut out = BTreeSet::new();
            for x in xs {
                out.insert(eval(x, ev)?);
            }
            V::Set(out)
        }
        ExprKind::MapLit(pairs) => {
            let mut out = BTreeMap::new();
            for (k, v) in pairs {
                let (k, v) = (eval(k, ev)?, eval(v, ev)?);
                out.insert(k, v);
            }
            V::Map(out)
        }
        ExprKind::Not(a) => V::B(!boolean(&eval(a, ev)?)),
        ExprKind::Neg(a) => arith(0, int(&eval(a, ev)?), BinOp::Sub)?,
        ExprKind::If(c, a, b) => {
            let (c, a, b) = (eval(c, ev)?, eval(a, ev)?, eval(b, ev)?);
            if boolean(&c) { a } else { b }
        }
        ExprKind::Index(m, k) => {
            let (m, k) = (eval(m, ev)?, eval(k, ev)?);
            match m {
                V::Map(m) => m.get(&k).cloned().ok_or(Fail::Undefined)?,
                other => panic!("index of {other:?}"),
            }
        }
        ExprKind::Update(m, k, v) => {
            let (m, k, v) = (eval(m, ev)?, eval(k, ev)?, eval(v, ev)?);
            match m {
                V::Map(mut m) => {
                    m.insert(k, v);
                    V::Map(m)
                }
                other => panic!("update of {other:?}"),
            }
        }
        ExprKind::Builtin(f, args) => {
            let mut vs = Vec::new();
            for a in args {
                vs.push(eval(a, ev)?);
            }
            match (f, vs.as_slice()) {
                (Builtin::Min, [a, b]) => V::I(int(a).min(int(b))),
                (Builtin::Max, [a, b]) => V::I(int(a).max(int(b))),
                (Builtin::MapGet, [V::Map(m), k]) => V::Opt(m.get(k).cloned().map(Box::new)),
                (Builtin::MapPut, [V::Map(m), k, v]) => {
                    let mut m = m.clone();
                    m.insert(k.clone(), v.clone());
                    V::Map(m)
                }
                other => panic!("the oracle has no {other:?}"),
            }
        }
        ExprKind::Quant(q, binders, body) => V::B(quant(*q, binders, body, ev)?),
        ExprKind::SetComp(x, binders, filter) => {
            let mut out = BTreeSet::new();
            comp(binders, ev, &mut |ev| {
                let keep = match filter {
                    Some(f) => boolean(&eval(f, ev)?),
                    None => true,
                };
                let x = eval(x, ev)?;
                if keep {
                    out.insert(x);
                }
                Ok(())
            })?;
            V::Set(out)
        }
        ExprKind::MapComp(k, v, binders, filter) => {
            let mut out = BTreeMap::new();
            comp(binders, ev, &mut |ev| {
                let keep = match filter {
                    Some(f) => boolean(&eval(f, ev)?),
                    None => true,
                };
                let (k, v) = (eval(k, ev)?, eval(v, ev)?);
                if keep {
                    assert!(out.insert(k, v).is_none(), "the frame has no repeated key");
                }
                Ok(())
            })?;
            V::Map(out)
        }
        ExprKind::Binary(op, a, b) => {
            let (a, b) = (eval(a, ev)?, eval(b, ev)?);
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul => arith(int(&a), int(&b), *op)?,
                BinOp::And => V::B(boolean(&a) && boolean(&b)),
                BinOp::Or => V::B(boolean(&a) || boolean(&b)),
                BinOp::Implies => V::B(!boolean(&a) || boolean(&b)),
                BinOp::Iff => V::B(boolean(&a) == boolean(&b)),
                BinOp::Eq => V::B(a == b),
                BinOp::Ne => V::B(a != b),
                BinOp::Lt => V::B(int(&a) < int(&b)),
                BinOp::Le => V::B(int(&a) <= int(&b)),
                BinOp::Gt => V::B(int(&a) > int(&b)),
                BinOp::Ge => V::B(int(&a) >= int(&b)),
                BinOp::In => V::B(members(b).contains(&a)),
                BinOp::NotIn => V::B(!members(b).contains(&a)),
                BinOp::SubsetEq => V::B(members(a).is_subset(&members(b))),
                BinOp::Union => V::Set(members(a).union(&members(b)).cloned().collect()),
                BinOp::Intersect => V::Set(members(a).intersection(&members(b)).cloned().collect()),
                BinOp::Diff => V::Set(members(a).difference(&members(b)).cloned().collect()),
                BinOp::Range => panic!("a range outside a domain"),
                other => panic!("the oracle has no {other:?}"),
            }
        }
        other => panic!("the oracle has no {other:?}"),
    })
}

/// The members of a binder's domain: a range, or any set-valued expression.
fn domain(binder: &continuum_cml_elab::norm::Binder, ev: &mut Ev<'_>) -> Result<Vec<V>, Fail> {
    Ok(match &binder.domain {
        None => universe(&binder.ty),
        Some(d) => match &d.kind {
            ExprKind::Binary(BinOp::Range, a, b) => {
                let (a, b) = (int(&eval(a, ev)?), int(&eval(b, ev)?));
                (a..=b).map(V::I).collect()
            }
            _ => members(eval(d, ev)?).into_iter().collect(),
        },
    })
}

fn quant(
    q: Quant,
    binders: &[(u32, continuum_cml_elab::norm::Binder)],
    body: &Expr,
    ev: &mut Ev<'_>,
) -> Result<bool, Fail> {
    let Some(((id, binder), rest)) = binders.split_first() else {
        return Ok(boolean(&eval(body, ev)?));
    };
    let mut verdicts = Vec::new();
    for v in domain(binder, ev)? {
        let shadowed = ev.binders.insert(*id, v);
        let r = quant(q, rest, body, ev);
        match shadowed {
            Some(old) => ev.binders.insert(*id, old),
            None => ev.binders.remove(id),
        };
        verdicts.push(r?);
    }
    Ok(match q {
        Quant::Forall => verdicts.iter().all(|v| *v),
        Quant::Exists => verdicts.iter().any(|v| *v),
    })
}

fn comp(
    binders: &[(u32, continuum_cml_elab::norm::Binder)],
    ev: &mut Ev<'_>,
    leaf: &mut dyn FnMut(&mut Ev<'_>) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let Some(((id, binder), rest)) = binders.split_first() else {
        return leaf(ev);
    };
    for v in domain(binder, ev)? {
        let shadowed = ev.binders.insert(*id, v);
        let r = comp(rest, ev, leaf);
        match shadowed {
            Some(old) => ev.binders.insert(*id, old),
            None => ev.binders.remove(id),
        };
        r?;
    }
    Ok(())
}

/// Every clause, strictly, then conjoined.
fn clauses(cs: &[Expr], ev: &mut Ev<'_>) -> Result<bool, Fail> {
    let mut out: Result<bool, Fail> = Ok(true);
    for c in cs {
        let v = eval(c, ev);
        out = match (out, v) {
            (Err(f), _) | (_, Err(f)) => Err(f),
            (Ok(w), Ok(v)) => Ok(w && boolean(&v)),
        };
    }
    out
}

// ---------------------------------------------------------------------------
// the frame
// ---------------------------------------------------------------------------

/// `m: Map[Nat, Nat]` (3 slots, `0..=3`), `s: Set[Nat]` (3 slots), `o: Option[Nat]`
/// (1 slot, `0..=3`), `p: Option[Set[Bool]]` (`p?`, `p!{false}`, `p!{true}`: canonical
/// only when `p?` is `1` or both others are `0`), and `x: Nat where x <= 2`.
const FRAME: &str = "module C
state {
  m: Map[Nat, Nat]
  s: Set[Nat]
  o: Option[Nat]
  p: Option[Set[Bool]]
  x: Nat where x <= 2
}
init { m == {} && s == {} && o == None && p == None && x == 0 }
";

const ACTIONS: &str = "
action Put(k: Nat, v: Nat) {
  require k notin s
  next m = m.put(k, v)
  next s = s union {k}
  unchanged o, p, x
}
action Read {
  require o != None
  next x = m[1]
  unchanged m, s, o, p
}
action Drop(k: Nat) {
  require m[k] > 0
  next m = m[k := 0]
  unchanged s, o, p, x
}
action Mark(v: Nat) {
  next o = Some(v)
  next p = Some({v == 1})
  unchanged m, s, x
}
";

/// A lowered state as values, or `None` when it is not canonical.
fn decode(model: &Model, state: &State) -> Option<BTreeMap<String, V>> {
    let at = |name: &str| model.binding(state, name).expect(name);
    let mut m = BTreeMap::new();
    let mut s = BTreeSet::new();
    for k in 0..=NAT_MAX {
        let c = at(&format!("m[{k}]"));
        if c > 0 {
            m.insert(V::I(k), V::I(c - 1));
        }
        if at(&format!("s{{{k}}}")) == 1 {
            s.insert(V::I(k));
        }
    }
    let o = at("o");
    let o = V::Opt((o > 0).then(|| Box::new(V::I(o - 1))));
    let (f, t) = (at("p!{false}"), at("p!{true}"));
    let p = match at("p?") {
        0 if f == 0 && t == 0 => V::Opt(None),
        0 => return None,
        _ => {
            let mut inner = BTreeSet::new();
            if f == 1 {
                inner.insert(V::B(false));
            }
            if t == 1 {
                inner.insert(V::B(true));
            }
            V::Opt(Some(Box::new(V::Set(inner))))
        }
    };
    Some(BTreeMap::from([
        ("m".to_owned(), V::Map(m)),
        ("s".to_owned(), V::Set(s)),
        ("o".to_owned(), o),
        ("p".to_owned(), p),
        ("x".to_owned(), V::I(at("x"))),
    ]))
}

/// Every flat state of the model's slot domain, with its decoded value when canonical.
fn states(model: &Model) -> Vec<(State, Option<BTreeMap<String, V>>)> {
    let vars = model.variables();
    let mut values: Vec<i64> = vars.iter().map(|v| v.domain().lo()).collect();
    let mut out = Vec::new();
    loop {
        let state = model.state(&values).expect("in the domain");
        let decoded = decode(model, &state);
        out.push((state, decoded));
        let mut i = vars.len();
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if values[i] < vars[i].domain().hi() {
                values[i] += 1;
                break;
            }
            values[i] = vars[i].domain().lo();
        }
    }
}

fn frame(invariants: &[&str]) -> String {
    let mut src = String::from(FRAME);
    src.push_str("action Stay { unchanged m, s, o, p, x }\n");
    for (i, inv) in invariants.iter().enumerate() {
        src.push_str(&format!("invariant I{i:02} {{ {inv} }}\n"));
    }
    src
}

/// Hand-written invariants: every expression form of the RFC's list, map reads that
/// are undefined in some states (also under a guarded quantifier, where only the
/// members of the domain count), overflow, and constants of every collection type.
const INVARIANTS: &[&str] = &[
    "m.get(0) == None || m.get(0) == Some(x)",
    "m[0] >= 0",
    "0 in s => m[0] == 1",
    "forall k in s: m[k] >= 1",
    "exists k in s: m.get(k) == Some(2)",
    "s == {} || s == {0, 1} || 2 in s",
    "s subseteq {0, 1}",
    "m == {0 -> 1}",
    "m.put(1, 2)[1] == 2",
    "m.put(1, 2)[0] == m[0]",
    "m[1 := x] == m.put(1, x)",
    "o == None || o == Some(x)",
    "o in {None, Some(1)}",
    "((s \\ {0}) intersect {1, 2}) == (s intersect {1, 2})",
    "p == None || p == Some({true}) || p == Some({}) || p == Some({false, true})",
    "forall b: forall c: {b, c} subseteq {false, true}",
    "forall k in 0..2: k in s => m[k] <= 2",
    "exists q: q == s",
    "(x, 1) in {(0, 1), (2, 1)}",
    "Some(x) == o",
    "m[0] * 4611686018427387904 >= 0",
    "forall k in s: m.get(k) != None && m[k] != 5",
    "{k | k in 0..2 where k > 0} == {1, 2}",
    "{k -> k | k in 0..1} == {0 -> 0, 1 -> 1}",
    "m != {} => m.put(0, 1) != {}",
    "1 notin s || s != {}",
    // Literals are typed `Int`, state here `Nat`: compared at the state's type.
    "o == Some(0)",
    "Some(1) == m.get(0)",
    "{0, 1} == s",
    "{2 -> 1} != m",
    "(s union {2}) == {0, 2}",
];

#[test]
fn collection_invariants_agree_with_the_oracle_on_every_canonical_state() {
    let invs: Vec<&str> = INVARIANTS.to_vec();
    let (norm, model) = lowered(&frame(&invs)).unwrap_or_else(|u| panic!("lowers: {u:?}"));
    let mut compared = 0;
    let (mut undefined, mut overflow, mut canonical) = (0, 0, 0);
    for (state, decoded) in states(&model) {
        let Some(values) = decoded else { continue };
        canonical += 1;
        for inv in &norm.invariants {
            let mut ev = Ev {
                state: &values,
                params: BTreeMap::new(),
                binders: BTreeMap::new(),
            };
            let want = clauses(&inv.clauses, &mut ev);
            let i = model.predicate_index(&inv.name).expect("lowered");
            let got = model.evaluate_predicate(i, &state);
            let defined = model
                .predicate_index(&format!("{}#defined", inv.name))
                .is_none_or(|d| model.evaluate_predicate(d, &state).expect("total"));
            match want {
                Ok(w) => {
                    assert!(defined, "{} defined at {values:?}", inv.name);
                    assert_eq!(got.expect("no error"), w, "{} at {values:?}", inv.name);
                }
                Err(Fail::Undefined) => {
                    assert!(!defined, "{} undefined at {values:?}", inv.name);
                    undefined += 1;
                }
                Err(Fail::Overflow) => {
                    assert!(got.is_err() || !defined, "{} fails at {values:?}", inv.name);
                    overflow += 1;
                }
            }
            compared += 1;
        }
    }
    assert_eq!(canonical, 30_720, "canonical states");
    assert_eq!(compared, canonical * invs.len());
    assert!(
        undefined > 0 && overflow > 0,
        "{undefined} undefined, {overflow} overflow"
    );
    // No definedness predicate for an invariant without a map read.
    assert!(model.predicate_index("I00#defined").is_none());
    assert!(model.predicate_index("I01#defined").is_some());
}

/// A labelled successor: an action instance's name and the state it reaches.
type Labelled = (String, BTreeMap<String, V>);

/// The oracle's successors of `values` under every instance of every action, and the
/// actions with an instance that fails.
fn oracle_step(
    norm: &NormModel,
    values: &BTreeMap<String, V>,
) -> (BTreeSet<Labelled>, BTreeSet<String>) {
    let mut next = BTreeSet::new();
    let mut failed = BTreeSet::new();
    for a in &norm.actions {
        let tuples: Vec<Vec<V>> = a.params.iter().fold(vec![Vec::new()], |acc, p| {
            acc.into_iter()
                .flat_map(|t| {
                    universe(&p.ty).into_iter().map(move |v| {
                        let mut t = t.clone();
                        t.push(v);
                        t
                    })
                })
                .collect()
        });
        for t in tuples {
            let mut ev = Ev {
                state: values,
                params: a
                    .params
                    .iter()
                    .zip(&t)
                    .map(|(p, v)| (p.name.clone(), v.clone()))
                    .collect(),
                binders: BTreeMap::new(),
            };
            let label = if t.is_empty() {
                a.name.clone()
            } else {
                let parts: Vec<String> = a
                    .params
                    .iter()
                    .zip(&t)
                    .map(|(p, v)| format!("{}={}", p.name, int(v)))
                    .collect();
                format!("{}({})", a.name, parts.join(","))
            };
            let result = clauses(&a.guard, &mut ev).and_then(|enabled| {
                if !enabled {
                    return Ok(None);
                }
                let mut after = values.clone();
                for (v, n) in &a.next {
                    if let Next::Set(e) = n {
                        after.insert(v.clone(), eval(e, &mut ev)?);
                    }
                }
                Ok(Some(after))
            });
            match result {
                Ok(Some(after)) => {
                    next.insert((label, after));
                }
                Ok(None) => {}
                Err(_) => {
                    failed.insert(a.name.clone());
                }
            }
        }
    }
    (next, failed)
}

#[test]
fn collection_actions_agree_with_the_oracle_on_every_canonical_state() {
    let src = format!("{FRAME}{ACTIONS}");
    let (norm, model) = lowered(&src).unwrap_or_else(|u| panic!("lowers: {u:?}"));
    let predicates: Vec<&str> = model
        .predicates()
        .iter()
        .map(|p| p.name().as_str())
        .collect();
    assert_eq!(predicates, ["Drop#defined", "Read#defined"]);
    let (mut failed_somewhere, mut moves) = (BTreeSet::new(), 0);
    for (state, decoded) in states(&model) {
        let Some(values) = decoded else { continue };
        let (want, failed) = oracle_step(&norm, &values);
        let got: BTreeSet<Labelled> = model
            .successors(&state)
            .expect("no evaluation error")
            .into_iter()
            .map(|step| {
                (
                    model.actions()[step.action()].name().as_str().to_owned(),
                    decode(&model, step.target()).expect("a successor is canonical"),
                )
            })
            .collect();
        assert_eq!(got, want, "successors of {values:?}");
        moves += got.len();
        for a in ["Drop", "Read"] {
            let i = model
                .predicate_index(&format!("{a}#defined"))
                .expect("declared");
            let defined = model.evaluate_predicate(i, &state).expect("total");
            assert_eq!(!defined, failed.contains(a), "{a} at {values:?}");
        }
        assert!(!failed.contains("Put") && !failed.contains("Mark"));
        failed_somewhere.extend(failed);
    }
    assert!(moves > 0);
    assert_eq!(
        failed_somewhere,
        BTreeSet::from(["Drop".to_owned(), "Read".to_owned()])
    );
}

// ---------------------------------------------------------------------------
// refusals
// ---------------------------------------------------------------------------

fn refusal(invariant: &str) -> Unlowerable {
    match lowered(&frame(&[invariant])) {
        Ok(_) => panic!("{invariant} lowers"),
        Err(u) => u,
    }
}

#[test]
fn keys_that_read_state_are_dynamic_keys() {
    for inv in [
        "m.get(x) == None",
        "m[x] >= 0",
        "x in s",
        "(s union {x}) == s",
        "{k | k in 0..2 where k in s} == s",
        "m.put(x, 1) != {}",
    ] {
        assert_eq!(refusal(inv), Unlowerable::DynamicKey, "{inv}");
    }
    assert_eq!(Unlowerable::DynamicKey.code(), "cml.lower.dynamic_key");
}

#[test]
fn static_values_outside_the_universe_are_refused() {
    for inv in [
        "m.get(3) == None",
        "m.get(0) != Some(3)",
        "forall k in 0..2: m.get(k) != Some(k + 5)",
        "s != {7}",
        "3 in s",
        "m != {0 -> 9}",
    ] {
        assert_eq!(refusal(inv), Unlowerable::ValueOutsideBound, "{inv}");
    }
    assert_eq!(
        Unlowerable::ValueOutsideBound.code(),
        "cml.lower.value_outside_bound"
    );
}

#[test]
fn unsafe_computed_payloads_are_refused() {
    // `x - 1` may be `-1`, whose code would be `0`, the code of `None`.
    assert_eq!(refusal("Some(x - 1) != o"), Unlowerable::DynamicValue);
    // A computed Boolean has no integer code in the model.
    assert_eq!(
        refusal("p != Some({x == 1})"),
        Unlowerable::DynamicKey,
        "a set member reads state"
    );
    // A payload that may overflow.
    assert_eq!(
        refusal("o != Some(x * 9223372036854775807)"),
        Unlowerable::DynamicValue
    );
    assert_eq!(Unlowerable::DynamicValue.code(), "cml.lower.dynamic_value");
}

#[test]
fn a_map_literal_with_a_repeated_key_is_refused() {
    assert_eq!(refusal("m != {0 -> 1, 0 -> 2}"), Unlowerable::DuplicateKey);
    assert_eq!(
        refusal("m != {0 -> 1, (1 - 1) -> 1}"),
        Unlowerable::DuplicateKey
    );
    assert_eq!(Unlowerable::DuplicateKey.code(), "cml.lower.duplicate_key");
}

#[test]
fn an_undefined_read_in_init_is_refused() {
    let src = FRAME.replace("init { m == {} &&", "init { m[0] == 0 && m == {} &&")
        + "action Stay { unchanged m, s, o, p, x }\n";
    assert_eq!(lowered(&src).err(), Some(Unlowerable::UndefinedRead));
    assert_eq!(
        Unlowerable::UndefinedRead.code(),
        "cml.lower.undefined_read"
    );
    // Defined at every candidate: lowers.
    let fine = FRAME.replace("init { m == {} &&", "init { m.get(0) == None && m == {} &&")
        + "action Stay { unchanged m, s, o, p, x }\n";
    let (_, model) = lowered(&fine).expect("lowers");
    assert_eq!(model.initial_states().len(), 1);
}

#[test]
fn a_composite_relational_variable_is_refused() {
    let src = format!("{FRAME}action R {{ 0 in s' \n unchanged m, o, p, x }}\n");
    assert_eq!(lowered(&src).err(), Some(Unlowerable::NonIntegerState));
}

#[test]
fn a_guarded_body_over_a_state_set_that_may_overflow_is_refused() {
    assert_eq!(
        refusal("forall k in s: m.get(k) == None || m[k] * 4611686018427387904 >= 0"),
        Unlowerable::GuardedOverflow
    );
}

// ---------------------------------------------------------------------------
// metamorphic relations
// ---------------------------------------------------------------------------

/// Spellings that mean one thing lower to one model: `m.put(k, v)` and `m[k := v]`,
/// and alpha-renaming (a collection binder and the binders under it renamed).
#[test]
fn equal_meanings_lower_to_one_model() {
    let put = format!("{FRAME}{ACTIONS}");
    let update = put.replace("next m = m.put(k, v)", "next m = m[k := v]");
    assert_ne!(put, update);
    let (_, a) = lowered(&put).expect("lowers");
    let (_, b) = lowered(&update).expect("lowers");
    assert_eq!(a, b);
    assert_eq!(a.identity(), b.identity());

    let named = frame(&["forall k in s: exists q: k in q && q subseteq s"]);
    let renamed = frame(&["forall j in s: exists r: j in r && r subseteq s"]);
    let (_, a) = lowered(&named).expect("lowers");
    let (_, b) = lowered(&renamed).expect("lowers");
    assert_eq!(a.identity(), b.identity());
    // Anti-vacuity: a different meaning is told apart.
    let other = frame(&["forall k in s: exists q: k in q && s subseteq q"]);
    let (_, c) = lowered(&other).expect("lowers");
    assert_ne!(a.identity(), c.identity());
}

// ---------------------------------------------------------------------------
// pre-review adversarial findings (bn-23hzh)
// ---------------------------------------------------------------------------

/// A map read in a relational postcondition whose map reads the post-state would give
/// `A#defined` a definedness that depends on the candidate: refused typed, never an
/// unresolved placeholder in a predicate.
#[test]
fn a_post_state_map_read_in_a_relational_action_is_refused() {
    let src = "module R\nstate {\n m: Map[Nat, Nat]\n x: Nat where x <= 2\n}\ninit { m == {} && x == 0 }\naction A { m.put(0, x')[0] == x'\n unchanged m }\n";
    assert_eq!(lowered(src).err(), Some(Unlowerable::DynamicValue));
    // A read of the pre-state map in a postcondition is candidate-independent: lowers.
    let pre = "module R\nstate {\n m: Map[Nat, Nat]\n x: Nat where x <= 2\n}\ninit { m == {} && x == 0 }\naction A { m[0] == x'\n unchanged m }\n";
    let (_, model) = lowered(pre).expect("lowers");
    assert!(model.predicate_index("A#defined").is_some());
}

/// A composite member of no set: its lowered layout is dropped, and the plan counts
/// that output, so the site's output and work agree with its plan (debug-asserted).
#[test]
fn composite_membership_in_an_empty_set_is_planned() {
    for inv in ["o in {}", "!(o in {})", "p in {}", "(x, 1) in {}"] {
        let (norm, model) = lowered(&frame(&[inv])).unwrap_or_else(|u| panic!("{inv}: {u:?}"));
        let i = model
            .predicate_index(&norm.invariants[0].name)
            .expect("lowered");
        let want = inv.starts_with('!');
        assert_eq!(
            model.evaluate_predicate(i, &model.initial_states()[0]),
            Ok(want),
            "{inv}"
        );
    }
}

/// A composite set-literal domain's sort is predicted by the plan (debug-asserted).
#[test]
fn a_composite_literal_domain_is_planned() {
    for inv in [
        "forall q in {(0, 0, 0)}: q == q",
        "forall q in {(0, 1), (1, 0)}: q != (x, 1)",
        "forall q in {{0}, {1, 2}}: q subseteq s || true",
    ] {
        lowered(&frame(&[inv])).unwrap_or_else(|u| panic!("{inv}: {u:?}"));
    }
}

/// Variant names of a hand-built model are escaped in slot names like sort elements,
/// so two tuples never share a slot.
#[test]
fn hand_built_variant_names_are_escaped() {
    let src = "module E\nenum En { a, b }\nstate { s: Set[(En, En)] }\ninit { s == {} }\naction A { unchanged s }\n";
    let mut model = elaborate_source(src).expect("elaborates");
    model.enums[0].variants = vec!["a".to_owned(), "a,a".to_owned()];
    let low = lower_configured(&model, &config("E"), Limits::default())
        .0
        .expect("lowers");
    let slots: BTreeSet<&str> = low
        .model()
        .variables()
        .iter()
        .map(|v| v.name().as_str())
        .collect();
    assert_eq!(slots.len(), 4, "{slots:?}");
}
