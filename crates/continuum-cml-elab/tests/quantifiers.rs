//! Quantifier expansion and action schemas (bn-10j7z, RFC 0003 correction 4, "Action
//! schemas" and "Quantifier expansion").
//!
//! # What is pinned
//!
//! - **an expansion differential** — every invariant of a generated corpus (fixed seed)
//!   and of a hand-written one, lowered under a run configuration, against a brute-force
//!   evaluator of the normalized AST written here, independent of the lowering: on every
//!   state of the domain, the same verdict, and an evaluation error exactly where the
//!   brute force has one. The corpus covers every domain form — the whole type, static
//!   and state-dependent ranges, static and state-dependent set literals, and a
//!   configuration constant set — nested and with several binders;
//! - **action schemas** — a parameterized model lowered against the same model built by
//!   hand through `ModelBuilder`: one `Model`, one identity, one reference-engine
//!   exploration; anti-vacuity: a different configuration is told apart;
//! - **typed refusals** — unbounded types (`cml.lower.unbounded_type`, and the legacy
//!   codes without a configuration), non-scalar binders and parameters, a Boolean binder
//!   with a domain, and a guarded body that may overflow (`cml.lower.guarded_overflow`).
//!
//! Resource cases are in `tests/resource_bounds.rs`.

use std::collections::BTreeMap;

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::lower_configured;
use continuum_cml_elab::norm::{BinOp, Builtin, Expr, ExprKind, Quant};
use continuum_cml_elab::{
    Limits, LowerErrorKind, NormModel, Type, Unlowerable, elaborate_source, lower,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_model_core::{ActionDecl, BoolExpr, CmpOp, IntExpr, Model, ModelBuilder};

const HEAD: &str = r#""schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1"#;

fn config(model: &str, bounds: &str, sorts: &str, constants: &str) -> RunConfig {
    let text = format!(
        r#"{{{HEAD},"model":"{model}","bounds":{bounds},"sorts":{sorts},"constants":{constants}}}"#
    );
    RunConfig::parse(text.as_bytes()).unwrap_or_else(|e| panic!("reads: {e}"))
}

fn lowered(model: &NormModel, config: &RunConfig) -> Result<Model, LowerErrorKind> {
    lower_configured(model, config, Limits::default())
        .0
        .map(|c| c.model().clone())
        .map_err(|e| e.kind)
}

// ---------------------------------------------------------------------------
// the brute-force oracle
// ---------------------------------------------------------------------------

/// What the oracle reads: the state, the bound variables in scope, and the
/// configuration's constants and bounds.
struct Ev {
    state: BTreeMap<String, i64>,
    binders: BTreeMap<u32, i64>,
    sets: BTreeMap<String, Vec<i64>>,
    nat_max: i64,
    int_range: (i64, i64),
}

/// An evaluation error (an overflow), as the model core reports one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failed;

/// The meaning of a normalized expression, by direct recursion: Booleans are `0`/`1`,
/// every operand is evaluated (strict, as the model core is), a quantifier evaluates its
/// body at every value of its domain and nowhere else.
fn eval(e: &Expr, ev: &mut Ev) -> Result<i64, Failed> {
    let b = |v: bool| i64::from(v);
    Ok(match &e.kind {
        ExprKind::Bool(v) => b(*v),
        ExprKind::Int(n) => *n,
        ExprKind::State(v) => *ev.state.get(v).expect("a state variable"),
        ExprKind::Bound { binder, .. } => *ev.binders.get(binder).expect("a bound variable"),
        ExprKind::Neg(a) => 0_i64.checked_sub(eval(a, ev)?).ok_or(Failed)?,
        ExprKind::Not(a) => b(eval(a, ev)? == 0),
        ExprKind::If(c, x, y) => {
            let (c, x, y) = (eval(c, ev)?, eval(x, ev)?, eval(y, ev)?);
            if c != 0 { x } else { y }
        }
        ExprKind::Builtin(f @ (Builtin::Min | Builtin::Max), args) => {
            let x = eval(&args[0], ev)?;
            let y = eval(&args[1], ev)?;
            if *f == Builtin::Min {
                x.min(y)
            } else {
                x.max(y)
            }
        }
        ExprKind::Binary(BinOp::In | BinOp::NotIn, x, set) => {
            let v = eval(x, ev)?;
            let members = members(set, ev)?;
            let inside = members.contains(&v);
            b(if matches!(e.kind, ExprKind::Binary(BinOp::In, ..)) {
                inside
            } else {
                !inside
            })
        }
        ExprKind::Binary(op, x, y) => {
            let (x, y) = (eval(x, ev)?, eval(y, ev)?);
            match op {
                BinOp::Add => x.checked_add(y).ok_or(Failed)?,
                BinOp::Sub => x.checked_sub(y).ok_or(Failed)?,
                BinOp::Mul => x.checked_mul(y).ok_or(Failed)?,
                BinOp::And => b(x != 0 && y != 0),
                BinOp::Or => b(x != 0 || y != 0),
                BinOp::Implies => b(x == 0 || y != 0),
                BinOp::Iff => b((x != 0) == (y != 0)),
                BinOp::Eq => b(x == y),
                BinOp::Ne => b(x != y),
                BinOp::Lt => b(x < y),
                BinOp::Le => b(x <= y),
                BinOp::Gt => b(x > y),
                BinOp::Ge => b(x >= y),
                other => panic!("the oracle does not evaluate {other:?}"),
            }
        }
        ExprKind::Quant(q, binders, body) => b(quant(*q, binders, body, ev)?),
        other => panic!("the oracle does not evaluate {other:?}"),
    })
}

/// The members of a set expression: a range, a set literal, or a constant set.
fn members(set: &Expr, ev: &mut Ev) -> Result<Vec<i64>, Failed> {
    Ok(match &set.kind {
        ExprKind::Binary(BinOp::Range, a, z) => {
            let (a, z) = (eval(a, ev)?, eval(z, ev)?);
            (a..=z).collect()
        }
        ExprKind::SetLit(es) => {
            let mut out = Vec::new();
            for m in es {
                out.push(eval(m, ev)?);
            }
            out.sort_unstable();
            out.dedup();
            out
        }
        ExprKind::Const(c) => ev.sets.get(c).cloned().expect("a constant set"),
        other => panic!("the oracle has no set {other:?}"),
    })
}

fn quant(
    q: Quant,
    binders: &[(u32, continuum_cml_elab::norm::Binder)],
    body: &Expr,
    ev: &mut Ev,
) -> Result<bool, Failed> {
    let Some(((id, binder), rest)) = binders.split_first() else {
        return Ok(eval(body, ev)? != 0);
    };
    let values: Vec<i64> = match &binder.domain {
        None => match &binder.ty {
            Type::Bool => vec![0, 1],
            Type::Nat => (0..=ev.nat_max).collect(),
            Type::Int => (ev.int_range.0..=ev.int_range.1).collect(),
            other => panic!("the oracle has no universe for {other:?}"),
        },
        Some(d) => members(d, ev)?,
    };
    let mut verdicts = Vec::with_capacity(values.len());
    for v in values {
        // Lexical scope: an inner binder with the same number shadows, then restores.
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

// ---------------------------------------------------------------------------
// the expansion differential
// ---------------------------------------------------------------------------

/// The frame every corpus invariant is checked in: `x` in `0..=3`, `y` in `-2..=2`,
/// `Nat` bounded to `0..=3`, `Int` to `-2..=2`, and `S = {-1, 1, 2}`.
fn frame(invariants: &[String]) -> String {
    let mut src = String::from(
        "module Q\nconst S: Set[Int]\nconst E: Set[Int]\nconst B: Set[Bool]\nstate {\n  x: Nat where x <= 3\n  y: Int where y in -2..2\n}\ninit { x == 0 && y == 0 }\naction A { unchanged x, y }\n",
    );
    for (i, inv) in invariants.iter().enumerate() {
        src.push_str(&format!("invariant I{i:03} {{ {inv} }}\n"));
    }
    src
}

fn frame_config() -> RunConfig {
    config(
        "Q",
        r#"{"Nat":{"max":3},"Int":{"min":-2,"max":2}}"#,
        "{}",
        r#"{"S":{"set":[{"int":-1},{"int":1},{"int":2}]},"E":{"set":[]},"B":{"set":[{"bool":true}]}}"#,
    )
}

/// Checks every invariant of `model` on every state against the oracle; returns how
/// many (state, invariant) pairs were compared, and how many failed in both.
fn differential(model: &NormModel, lowered: &Model) -> (usize, usize) {
    let mut compared = 0;
    let mut failed = 0;
    for x in 0..=3_i64 {
        for y in -2..=2_i64 {
            let state = lowered.state(&[x, y]).expect("in the domain");
            for inv in &model.invariants {
                let mut ev = Ev {
                    state: BTreeMap::from([("x".to_owned(), x), ("y".to_owned(), y)]),
                    binders: BTreeMap::new(),
                    sets: BTreeMap::from([
                        ("S".to_owned(), vec![-1, 1, 2]),
                        ("E".to_owned(), Vec::new()),
                        ("B".to_owned(), vec![1]),
                    ]),
                    nat_max: 3,
                    int_range: (-2, 2),
                };
                // Every clause is evaluated (strict), then conjoined.
                let mut want: Result<bool, Failed> = Ok(true);
                for c in &inv.clauses {
                    let v = eval(c, &mut ev);
                    want = match (want, v) {
                        (Err(f), _) | (_, Err(f)) => Err(f),
                        (Ok(w), Ok(v)) => Ok(w && v != 0),
                    };
                }
                let index = lowered.predicate_index(&inv.name).expect("lowered");
                let got = lowered.evaluate_predicate(index, &state);
                match (want, got) {
                    (Ok(w), Ok(g)) => assert_eq!(w, g, "{} at x={x}, y={y}", inv.name),
                    (Err(_), Err(_)) => failed += 1,
                    (w, g) => panic!("{} at x={x}, y={y}: oracle {w:?}, lowered {g:?}", inv.name),
                }
                compared += 1;
            }
        }
    }
    (compared, failed)
}

/// Hand-written invariants: one per domain form, nesting, several binders, a later
/// domain that reads an earlier binder, empty domains, and an overflow the oracle and
/// the lowering must both report.
const HAND: &[&str] = &[
    "forall i: i <= x || i > x",
    "exists i: i == x",
    "forall i in 0..2: i + x >= 0",
    "exists i in 0..x: i == 2",
    "forall i in x..3: i >= x",
    "forall i in y..x: i + y <= 5",
    "exists i in {1, x}: i == y",
    "forall i in {0, 2, 3}: i != y",
    "forall i in S: i != x",
    "exists i in S: i == y",
    "forall i in 3..1: false",
    "exists i in 3..1: true",
    "forall i in 0..2: exists j in 0..2: i + j == x",
    "forall a, b in 0..2: a * b <= 4",
    "forall a in 0..2, b in a..2: b >= a",
    "exists a in 0..x, b in {a, y}: a + b == 3",
    "forall i: exists j: i + j == x || i + j != x",
    "x in {1, y}",
    "y notin S",
    "x in S || y in {0}",
    "(x == 0) in {true}",
    "(x > 1) notin {true, false}",
    "(y < 0) in B",
    "(x == y) notin {}",
    "forall b: b in B || b notin B",
    "(x < 2) in {x == 1, y == 0}",
    "forall i in 0..1: i * 9223372036854775807 + i >= 0",
];

#[test]
fn hand_written_quantifiers_agree_with_the_brute_force_oracle() {
    let invariants: Vec<String> = HAND.iter().map(|s| (*s).to_owned()).collect();
    let model = elaborate_source(&frame(&invariants)).unwrap_or_else(|e| panic!("{e}"));
    let low = lowered(&model, &frame_config()).expect("lowers");
    let (compared, failed) = differential(&model, &low);
    assert_eq!(compared, 20 * HAND.len());
    // The last invariant overflows at `i = 1` in every state: both report it.
    assert_eq!(failed, 20, "the overflow is reported, not hidden");
}

/// A small deterministic generator (a linear congruential sequence), so the corpus is
/// the same on every run (INV-005): no ambient randomness.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

fn term(r: &mut Lcg, scope: &[String]) -> String {
    let leaf = |r: &mut Lcg| -> String {
        match r.next(4) {
            0 => "x".to_owned(),
            1 => "y".to_owned(),
            2 if !scope.is_empty() => scope[r.next(scope.len() as u64) as usize].clone(),
            _ => r.next(4).to_string(),
        }
    };
    match r.next(6) {
        0 => format!("{} + {}", leaf(r), leaf(r)),
        1 => format!("{} * {}", leaf(r), leaf(r)),
        2 => format!("{} * {}", leaf(r), BIG[r.next(3) as usize]),
        _ => leaf(r),
    }
}

/// Multipliers: small, and large enough to overflow `i64` at small operands.
const BIG: [&str; 3] = ["2", "4611686018427387904", "9223372036854775807"];

/// A domain bound or member: a variable, an enclosing binder, or a constant, possibly
/// under arithmetic, `min`, or `max` (cr-3aqchd: domains with arithmetic, so a nested
/// domain can overflow at an enclosing candidate the source excludes).
fn bound(r: &mut Lcg, scope: &[String]) -> String {
    let atom = |r: &mut Lcg| -> String {
        match r.next(3) {
            0 => "x".to_owned(),
            1 if !scope.is_empty() => scope[r.next(scope.len() as u64) as usize].clone(),
            _ => r.next(4).to_string(),
        }
    };
    match r.next(7) {
        0 => format!("min({}, {})", atom(r), atom(r)),
        1 => format!("max({}, {})", atom(r), atom(r)),
        2 => format!("{} + {}", atom(r), atom(r)),
        3 => format!("{} * {}", atom(r), BIG[r.next(3) as usize]),
        4 => format!(
            "min({} * {}, {})",
            atom(r),
            BIG[r.next(3) as usize],
            atom(r)
        ),
        _ => atom(r),
    }
}

fn domain(r: &mut Lcg, scope: &[String]) -> String {
    let t = |r: &mut Lcg| bound(r, scope);
    match r.next(6) {
        0 => String::new(),
        1 => format!(" in {}..{}", r.next(3), r.next(4)),
        2 => format!(" in {}..{}", t(r), t(r)),
        3 => format!(" in {{{}, {}}}", t(r), t(r)),
        4 => format!(" in {{{}, {}, {}}}", r.next(4), r.next(4), r.next(4)),
        _ => " in S".to_owned(),
    }
}

fn formula(r: &mut Lcg, depth: u32, scope: &mut Vec<String>) -> String {
    let pick = if depth == 0 { r.next(2) } else { r.next(10) };
    match pick {
        0..=1 if r.next(4) == 0 => {
            // Membership, empty sets included (cr-3aqchd): the operand is evaluated
            // even when no member can match.
            let set = match r.next(5) {
                0 => "{}".to_owned(),
                1 => "E".to_owned(),
                2 => "S".to_owned(),
                3 => format!("{{{}, {}}}", term(r, scope), r.next(4)),
                _ => format!("{{{}}}", r.next(4)),
            };
            let op = if r.next(2) == 0 { "in" } else { "notin" };
            format!("{} {op} {set}", term(r, scope))
        }
        0..=1 => {
            let op = ["==", "!=", "<", "<=", ">", ">="][r.next(6) as usize];
            format!("{} {op} {}", term(r, scope), term(r, scope))
        }
        5 => {
            // A constant side: the other side is still evaluated.
            let f = formula(r, depth - 1, scope);
            match r.next(4) {
                0 => format!("(true && {f})"),
                1 => format!("({f} || false)"),
                2 => format!("(false => {f})"),
                _ => format!("(if true then {f} else {})", formula(r, depth - 1, scope)),
            }
        }
        2 => format!(
            "({} && {})",
            formula(r, depth - 1, scope),
            formula(r, depth - 1, scope)
        ),
        3 => format!(
            "({} || {})",
            formula(r, depth - 1, scope),
            formula(r, depth - 1, scope)
        ),
        4 => format!("!({})", formula(r, depth - 1, scope)),
        _ => {
            let q = if r.next(2) == 0 { "forall" } else { "exists" };
            let name = format!("v{}", scope.len());
            let dom = domain(r, scope);
            scope.push(name.clone());
            // The binder is always used, so its type is always inferred.
            let body = format!("({name} >= {name} && {})", formula(r, depth - 1, scope));
            scope.pop();
            format!("({q} {name}{dom}: {body})")
        }
    }
}

/// Four hundred generated invariants, each checked on every state. Anti-vacuity: most
/// of them elaborate and lower, and the corpus reaches every domain form.
#[test]
fn generated_quantifiers_agree_with_the_brute_force_oracle() {
    let mut r = Lcg(0x15_2a_10_7a);
    let mut accepted = Vec::new();
    let mut refused = BTreeMap::<&'static str, usize>::new();
    let mut errors = 0;
    for _ in 0..600 {
        let f = formula(&mut r, 4, &mut Vec::new());
        let src = frame(std::slice::from_ref(&f));
        let Ok(model) = elaborate_source(&src) else {
            *refused.entry("elaboration").or_default() += 1;
            continue;
        };
        match lowered(&model, &frame_config()) {
            Ok(low) => {
                errors += differential(&model, &low).1;
                accepted.push(f);
            }
            Err(LowerErrorKind::Unlowerable(u)) => *refused.entry(u.code()).or_default() += 1,
            Err(other) => panic!("{f}: {other:?}"),
        }
    }
    let quantified = accepted
        .iter()
        .filter(|f| f.contains("forall") || f.contains("exists"))
        .count();
    assert!(
        accepted.len() >= 300 && quantified >= 150,
        "{} accepted, {quantified} quantified; refused {refused:?}",
        accepted.len()
    );
    // Anti-vacuity: evaluation errors occur, and both sides report them in the same
    // states; guarded bodies and nested domains that may overflow are refused.
    assert!(errors > 0, "no evaluation error was exercised");
    assert!(
        refused
            .get("cml.lower.guarded_overflow")
            .copied()
            .unwrap_or(0)
            >= 5,
        "{refused:?}"
    );
    for form in [
        " in S",
        ": (",
        "..x",
        "{x,",
        "in min(",
        "..max(",
        " * 9223372036854775807",
        "notin {}",
        " in {}",
        " in E",
        "notin E",
        "(true && ",
        " || false)",
        "(false => ",
        "(if true then ",
    ] {
        assert!(
            accepted.iter().any(|f| f.contains(form)),
            "no accepted formula has {form:?}"
        );
    }
    // Only typed refusals, of the kinds this corpus can produce: a guarded body or
    // nested domain that may overflow, or a hull too wide for the model's depth or the
    // output or work budget (a bound multiplied by a large constant).
    for code in refused.keys() {
        assert!(
            [
                "elaboration",
                "cml.lower.guarded_overflow",
                "cml.lower.expression_too_deep",
                "cml.lower.output_too_large",
                "cml.lower.work_limit_exceeded",
            ]
            .contains(code),
            "unexpected refusal {code}: {refused:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// action schemas against the programmatic model
// ---------------------------------------------------------------------------

const SCHEMA: &str = "module Lock
type Node
enum Phase { Idle, Busy }
const Start: Node
state {
  owner: Node
  phase: Phase
  count: Nat where count <= 2
}
init { owner == Start && phase == Idle && count == 0 }
action Take(n: Node, fresh: Bool) {
  require phase == Idle && (fresh || n != owner)
  next owner = n
  next phase = Busy
  unchanged count
}
action Release {
  require phase == Busy
  next phase = Idle
  next count = min(count + 1, 2)
  unchanged owner
}
invariant Bounded { forall p in {Idle, Busy}: p == phase || count <= 2 }
";

fn schema_config(start: &str) -> RunConfig {
    config(
        "Lock",
        r#"{"Nat":{"max":2}}"#,
        r#"{"Node":{"elements":["a","b"]}}"#,
        &format!(r#"{{"Start":{{"elem":{{"sort":"Node","name":"{start}"}}}}}}"#),
    )
}

/// `Lock` built by hand: `Node = {a, b}` (0, 1), `Phase = {Idle, Busy}` (0, 1),
/// `Start = a`; one `Take` per `(n, fresh)` in declaration order, the last fastest.
fn programmatic_lock() -> Model {
    let var = IntExpr::var;
    let c = IntExpr::constant;
    let mut b = ModelBuilder::new()
        .variable("count", 0, 2)
        .variable("owner", 0, 1)
        .variable("phase", 0, 1)
        .initial_state(&[("count", 0), ("owner", 0), ("phase", 0)])
        .action(ActionDecl::deterministic(
            "Release",
            BoolExpr::compare(CmpOp::Eq, var("phase"), c(1)),
            vec![
                (
                    "count",
                    IntExpr::min(IntExpr::plus(var("count"), c(1)), c(2)),
                ),
                ("phase", c(0)),
            ],
        ))
        .predicate(
            "Bounded",
            BoolExpr::and(
                BoolExpr::or(
                    BoolExpr::compare(CmpOp::Eq, c(0), var("phase")),
                    BoolExpr::compare(CmpOp::Le, var("count"), c(2)),
                ),
                BoolExpr::or(
                    BoolExpr::compare(CmpOp::Eq, c(1), var("phase")),
                    BoolExpr::compare(CmpOp::Le, var("count"), c(2)),
                ),
            ),
        );
    for (n, node) in ["a", "b"].iter().enumerate() {
        for fresh in [false, true] {
            b = b.action(ActionDecl::deterministic(
                &format!("Take(n={node},fresh={fresh})"),
                BoolExpr::and(
                    BoolExpr::compare(CmpOp::Eq, var("phase"), c(0)),
                    BoolExpr::or(
                        BoolExpr::constant(fresh),
                        BoolExpr::compare(CmpOp::Ne, c(n as i64), var("owner")),
                    ),
                ),
                vec![("owner", c(n as i64)), ("phase", c(1))],
            ));
        }
    }
    b.build().expect("a valid programmatic model")
}

#[test]
fn an_action_schema_is_the_programmatic_model() {
    let model = elaborate_source(SCHEMA).unwrap_or_else(|e| panic!("{e}"));
    let low = lowered(&model, &schema_config("a")).expect("lowers");
    let oracle = programmatic_lock();
    assert_eq!(low, oracle);
    assert_eq!(low.identity(), oracle.identity());
    let explore = |m: &Model| bfs::explore(m, Bounds::CERTIFIABLE).expect("explores");
    assert_eq!(explore(&low), explore(&oracle));
    let names: Vec<&str> = low.actions().iter().map(|a| a.name().as_str()).collect();
    assert_eq!(
        names,
        [
            "Release",
            "Take(n=a,fresh=false)",
            "Take(n=a,fresh=true)",
            "Take(n=b,fresh=false)",
            "Take(n=b,fresh=true)",
        ]
    );
}

/// Anti-vacuity: another start element is another model, which the differential
/// tells apart.
#[test]
fn a_different_configuration_is_told_apart() {
    let model = elaborate_source(SCHEMA).unwrap_or_else(|e| panic!("{e}"));
    let low = lowered(&model, &schema_config("b")).expect("lowers");
    assert_ne!(low.identity(), programmatic_lock().identity());
}

// ---------------------------------------------------------------------------
// typed refusals
// ---------------------------------------------------------------------------

fn unlowerable(src: &str, config: Option<&RunConfig>) -> Unlowerable {
    let model = elaborate_source(src).unwrap_or_else(|e| panic!("{e}"));
    let kind = match config {
        Some(c) => lowered(&model, c).expect_err("refused"),
        None => lower(&model).expect_err("refused").kind,
    };
    match kind {
        LowerErrorKind::Unlowerable(u) => u,
        other => panic!("not an Unlowerable: {other:?}"),
    }
}

fn plain(decls: &str) -> String {
    format!(
        "module P\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\n{decls}\n"
    )
}

fn no_bounds(model: &str) -> RunConfig {
    let text = format!(r#"{{{HEAD},"model":"{model}","sorts":{{}},"constants":{{}}}}"#);
    RunConfig::parse(text.as_bytes()).expect("reads")
}

#[test]
fn unbounded_types_are_refused_typed_and_never_bounded_silently() {
    let whole = plain("invariant I { forall i: i + x >= 0 }");
    assert_eq!(unlowerable(&whole, None), Unlowerable::Quantifier);
    assert_eq!(
        unlowerable(&whole, Some(&no_bounds("P"))),
        Unlowerable::UnboundedType
    );
    assert_eq!(
        Unlowerable::UnboundedType.code(),
        "cml.lower.unbounded_type"
    );
    // A parameter of an unbounded type.
    let param = "module P\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A(n: Nat) { require n <= 3\n next x = n }\n";
    assert_eq!(unlowerable(param, None), Unlowerable::ParameterizedAction);
    assert_eq!(
        unlowerable(param, Some(&no_bounds("P"))),
        Unlowerable::UnboundedType
    );
    // With the bounds, both lower.
    let bounded = config(
        "P",
        r#"{"Nat":{"max":3},"Int":{"min":-3,"max":3}}"#,
        "{}",
        "{}",
    );
    lowered(&elaborate_source(&whole).expect("elaborates"), &bounded).expect("lowers");
    lowered(&elaborate_source(param).expect("elaborates"), &bounded).expect("lowers");
}

/// Collection-typed parameters and binders expand over their universes (bn-23hzh): a
/// `Set[Nat]` parameter is one action per subset, a binder over a constant set of sets
/// takes each member, and a static comprehension lowers. A state-reading member tested
/// against a set that is neither a literal nor a constant is a dynamic key; a Boolean
/// binder with a domain keeps its refusal.
#[test]
fn collection_binders_and_parameters_expand() {
    let set_param = "module P\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A(s: Set[Nat]) { require 1 in s\n unchanged x }\n";
    let bounded = config("P", r#"{"Nat":{"max":3}}"#, "{}", "{}");
    let model = lowered(&elaborate_source(set_param).expect("elaborates"), &bounded)
        .expect("a set parameter expands");
    assert_eq!(model.actions().len(), 16, "one action per subset of 0..=3");
    let enabled = |name: &str| {
        let i = model.action_index(name).unwrap_or_else(|| panic!("{name}"));
        model
            .is_enabled(i, &model.initial_states()[0])
            .expect("evaluates")
    };
    assert!(enabled("A(s={1,3})"));
    assert!(!enabled("A(s={0,2})"));
    assert!(!enabled("A(s={})"));

    let set_binder = "module P\nconst Q: Set[Set[Nat]]\nstate { x: Nat where x <= 3 }\ninit { x == 0 }\naction A { unchanged x }\ninvariant I { exists q in Q: 1 in q }\ninvariant J { exists q in Q: x in q }\n";
    let with_q = config(
        "P",
        r#"{"Nat":{"max":3}}"#,
        "{}",
        r#"{"Q":{"set":[{"set":[{"int":1}]}]}}"#,
    );
    assert_eq!(
        unlowerable(set_binder, Some(&with_q)),
        Unlowerable::DynamicKey,
        "`x in q` tests a state-reading member against a bound set"
    );
    let only_i = set_binder.replace("invariant J { exists q in Q: x in q }\n", "");
    let model = lowered(&elaborate_source(&only_i).expect("elaborates"), &with_q)
        .expect("a binder over a constant set of sets expands");
    let i = model.predicate_index("I").expect("declared");
    assert_eq!(
        model.evaluate_predicate(i, &model.initial_states()[0]),
        Ok(true)
    );

    let bool_domain = plain("invariant I { forall b in {true}: b || x >= 0 }");
    assert_eq!(
        unlowerable(&bool_domain, Some(&bounded)),
        Unlowerable::NonIntegerValue
    );
    // A static comprehension is a set value (bn-23hzh).
    let comprehension = plain("invariant I { {i | i in 0..2} == {0, 1, 2} }");
    assert_eq!(
        unlowerable(&comprehension, Some(&bounded)),
        Unlowerable::UnboundedType,
        "a `Set[Int]` needs an `Int` bound"
    );
    let with_int = config(
        "P",
        r#"{"Nat":{"max":3},"Int":{"min":0,"max":3}}"#,
        "{}",
        "{}",
    );
    let model = lowered(
        &elaborate_source(&comprehension).expect("elaborates"),
        &with_int,
    )
    .expect("a static comprehension lowers");
    let i = model.predicate_index("I").expect("declared");
    assert_eq!(
        model.evaluate_predicate(i, &model.initial_states()[0]),
        Ok(true)
    );
    // Without a configuration nothing is finite but the scalars it had.
    assert_eq!(
        unlowerable(set_param, None),
        Unlowerable::ParameterizedAction
    );
}

/// A guarded body that may overflow outside the domain is refused, since the model core
/// would evaluate it there; the same body over a static domain lowers.
#[test]
fn a_guarded_body_that_may_overflow_is_refused() {
    let bounded = config("P", r#"{"Nat":{"max":3}}"#, "{}", "{}");
    let guarded = plain("invariant I { forall i in 0..x: i * 4611686018427387904 >= 0 }");
    assert_eq!(
        unlowerable(&guarded, Some(&bounded)),
        Unlowerable::GuardedOverflow
    );
    assert_eq!(
        Unlowerable::GuardedOverflow.code(),
        "cml.lower.guarded_overflow"
    );
    let small = plain("invariant I { forall i in 0..x: i * 2 >= 0 }");
    lowered(&elaborate_source(&small).expect("elaborates"), &bounded).expect("no overflow");
    let fixed = plain("invariant I { forall i in 0..1: i * 4611686018427387904 >= 0 }");
    lowered(&elaborate_source(&fixed).expect("elaborates"), &bounded).expect("static");
}

// ---------------------------------------------------------------------------
// metamorphic
// ---------------------------------------------------------------------------

/// Metamorphic relation: alpha-renaming. Renaming every bound variable of the
/// hand-written corpus lowers to the same `Model`, with the same identity; renaming the
/// state variable `x` does not (anti-vacuity: the relation is not trivially true).
#[test]
fn alpha_renaming_bound_variables_keeps_the_lowered_model() {
    // Whole identifiers only: `i`, `j`, `a`, `b` become `k`, `m`, `c`, `d`.
    let rename = |s: &str| {
        rename_words(s, |w| match w {
            "i" => "k",
            "j" => "m",
            "a" => "c",
            "b" => "d",
            w => w,
        })
    };
    let original: Vec<String> = HAND.iter().map(|s| (*s).to_owned()).collect();
    let renamed: Vec<String> = HAND.iter().map(|s| rename(s)).collect();
    assert_ne!(original, renamed, "the renaming renames");
    let lower_all = |invs: &[String]| {
        let model = elaborate_source(&frame(invs)).unwrap_or_else(|e| panic!("{e}"));
        lowered(&model, &frame_config()).expect("lowers")
    };
    let (a, b) = (lower_all(&original), lower_all(&renamed));
    assert_eq!(a, b);
    assert_eq!(a.identity(), b.identity());
    let moved: Vec<String> = original
        .iter()
        .map(|s| rename_words(s, |w| if w == "x" { "z" } else { w }))
        .collect();
    let src = frame(&moved).replace("x: Nat where x <= 3", "z: Nat where z <= 3");
    let src = src
        .replace("x == 0 && y == 0", "z == 0 && y == 0")
        .replace("unchanged x, y", "unchanged z, y");
    let model = elaborate_source(&src).unwrap_or_else(|e| panic!("{e}"));
    let c = lowered(&model, &frame_config()).expect("lowers");
    assert_ne!(a.identity(), c.identity());
}

/// `s` with each whole identifier `w` replaced by `f(w)`.
fn rename_words(s: &str, f: impl Fn(&str) -> &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            out.push_str(f(&word));
            word.clear();
            out.push(ch);
        }
    }
    out.push_str(f(&word));
    out
}

// ---------------------------------------------------------------------------
// cr-3aqchd: everything a guarded instance evaluates is proved not to overflow
// ---------------------------------------------------------------------------

/// The finding's reproducer, and its neighbours. Inside `forall i in 0..x` the
/// instances at `i = 1, 2` are evaluated also where `x` excludes them (the model core's
/// `=>` evaluates both sides), so a nested domain, a nested membership guard, or a body
/// that may overflow there would add an error the CML model does not have: each is
/// refused. The same arithmetic where CML itself evaluates it — a top-level domain, or an
/// empty range whose bound may overflow — lowers, and fails in exactly the states the
/// brute-force oracle fails in.
#[test]
fn cr_3aqchd_nested_domains_under_a_guard_are_proved_not_to_overflow() {
    let bounded = frame_config();
    let refused = |inv: &str| -> Unlowerable {
        let model = elaborate_source(&frame(&[inv.to_owned()])).unwrap_or_else(|e| panic!("{e}"));
        match lowered(&model, &bounded).expect_err(inv) {
            LowerErrorKind::Unlowerable(u) => u,
            other => panic!("{inv}: {other:?}"),
        }
    };
    for inv in [
        "forall i in 0..x: forall j in 0..min(i * 9223372036854775807, 0): true",
        "forall i in 0..x: exists j in {i * 9223372036854775807, 0}: true",
        "exists i in {x, 1}: forall j in max(0, i * 4611686018427387904)..3: j >= 0",
        "forall i in 0..x: i * 4611686018427387904 >= 0",
    ] {
        assert_eq!(refused(inv), Unlowerable::GuardedOverflow, "{inv}");
    }
    let agree = |invs: &[&str]| -> (usize, usize) {
        let invs: Vec<String> = invs.iter().map(|s| (*s).to_owned()).collect();
        let model = elaborate_source(&frame(&invs)).unwrap_or_else(|e| panic!("{e}"));
        let low = lowered(&model, &bounded).expect("lowers");
        differential(&model, &low)
    };
    // CML evaluates `x * MAX` once per state: an error at x = 2, 3 (ten states).
    assert_eq!(
        agree(&["forall j in 0..min(x * 9223372036854775807, 0): true"]),
        (20, 10)
    );
    // An empty range whose lower bound always overflows: an error in every state.
    assert_eq!(
        agree(&["forall v in min(3 * 4611686018427387904, 2)..0: true"]),
        (20, 20)
    );
    // The same shapes without overflow lower and agree everywhere.
    assert_eq!(
        agree(&[
            "forall i in 0..x: forall j in 0..min(i, 1): j <= i",
            "forall i in 0..x: exists j in {i * 2, 0}: j >= 0",
        ]),
        (40, 0)
    );
}

/// The same proof covers guarded quantifiers wherever a clause is lowered: in an
/// action schema's guard and in a relational postcondition.
#[test]
fn cr_3aqchd_guarded_domains_in_actions_are_proved_too() {
    let bounded = config(
        "P",
        r#"{"Nat":{"max":3},"Int":{"min":-3,"max":3}}"#,
        "{}",
        "{}",
    );
    let bad = "forall i in 0..x: forall j in 0..min(i * 9223372036854775807, 0): true";
    let schema = format!(
        "module P\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\naction A(b: Bool) {{ require b || ({bad})\n unchanged x }}\n"
    );
    assert_eq!(
        unlowerable(&schema, Some(&bounded)),
        Unlowerable::GuardedOverflow
    );
    let relational = format!(
        "module P\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\naction A {{ x' != x && ({bad}) }}\n"
    );
    assert_eq!(
        unlowerable(&relational, Some(&bounded)),
        Unlowerable::GuardedOverflow
    );
    let ok = bad.replace(" * 9223372036854775807", "");
    let schema = schema.replace(bad, &ok);
    let relational = relational.replace(bad, &ok);
    lowered(&elaborate_source(&schema).expect("elaborates"), &bounded).expect("lowers");
    lowered(
        &elaborate_source(&relational).expect("elaborates"),
        &bounded,
    )
    .expect("lowers");
}

/// cr-3aqchd round 3: membership in an empty set — a literal `{}` or a configured empty
/// set `E` — is false (`notin`: true) but still evaluates its operand, which may fail.
/// Oracle and lowering agree at safe and overflowing states in an invariant, an action
/// guard, and an init; under a guarded quantifier an operand that may overflow is
/// refused, and a safe one lowers and agrees.
#[test]
fn cr_3aqchd_empty_membership_keeps_the_evaluation_of_its_operand() {
    const BIG: &str = "4611686018427387904";
    let config = frame_config();
    let agree = |invs: &[String]| -> (usize, usize) {
        let model = elaborate_source(&frame(invs)).unwrap_or_else(|e| panic!("{e}"));
        let low = lowered(&model, &config).expect("lowers");
        differential(&model, &low)
    };
    // Invariants: `x * 2^62` overflows at x = 2, 3 (ten of twenty states), for each of
    // the four forms; the safe forms never fail.
    let failing: Vec<String> = ["notin {}", "in {}", "in E", "notin E"]
        .iter()
        .map(|m| format!("x * {BIG} {m}"))
        .collect();
    assert_eq!(agree(&failing), (80, 40));
    let safe: Vec<String> = ["x notin {}", "x in {}", "y in E", "y notin E"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(agree(&safe), (80, 0));

    // Under a guarded quantifier: refused when the operand may overflow on the hull.
    for inv in [
        format!("forall i in 0..x: i * {BIG} notin {{}}"),
        format!("exists i in 0..x: i * {BIG} in E"),
    ] {
        let model = elaborate_source(&frame(std::slice::from_ref(&inv))).expect("elaborates");
        assert_eq!(
            lowered(&model, &config).expect_err(&inv),
            LowerErrorKind::Unlowerable(Unlowerable::GuardedOverflow),
            "{inv}"
        );
    }
    assert_eq!(
        agree(&[
            "forall i in 0..x: i notin {}".to_owned(),
            "exists i in 0..x: i in E".to_owned()
        ]),
        (40, 0)
    );

    // An action guard: enabled where the operand evaluates, an evaluation error where it
    // overflows — as the oracle evaluates the guard.
    for set in ["{}", "E"] {
        let src = frame(&[]).replace(
            "action A { unchanged x, y }",
            &format!("action A {{ require x * {BIG} notin {set}\n unchanged x, y }}"),
        );
        let model = elaborate_source(&src).unwrap_or_else(|e| panic!("{e}"));
        let low = lowered(&model, &config).expect("lowers");
        let guard = &model.actions[0].guard[0];
        for x in 0..=3_i64 {
            let state = low.state(&[x, 0]).expect("in the domain");
            let mut ev = Ev {
                state: BTreeMap::from([("x".to_owned(), x), ("y".to_owned(), 0)]),
                binders: BTreeMap::new(),
                sets: BTreeMap::from([("E".to_owned(), Vec::new())]),
                nat_max: 3,
                int_range: (-2, 2),
            };
            let want = eval(guard, &mut ev).map(|v| v != 0);
            let got = low.is_enabled(0, &state);
            assert_eq!(want.is_ok(), got.is_ok(), "{set} at x={x}");
            if let (Ok(w), Ok(g)) = (want, got) {
                assert_eq!(w, g, "{set} at x={x}");
            }
            assert_eq!(want.is_err(), x >= 2, "the oracle fails exactly at x >= 2");
        }
    }

    // An init: the lowering evaluates init at every candidate state, so an operand that
    // overflows at some candidate is the evaluation error the CML init has there.
    for (set, overflow) in [("{}", true), ("E", true), ("{}", false)] {
        let operand = if overflow {
            format!("x * {BIG}")
        } else {
            "x".to_owned()
        };
        let src = frame(&[]).replace(
            "init { x == 0 && y == 0 }",
            &format!("init {{ x <= 3 && y == 0 && {operand} notin {set} }}"),
        );
        let model = elaborate_source(&src).unwrap_or_else(|e| panic!("{e}"));
        match lowered(&model, &config) {
            Err(LowerErrorKind::Evaluation(_)) => assert!(overflow, "{src}"),
            Ok(low) => {
                assert!(!overflow, "an overflowing init lowered");
                assert_eq!(low.initial_states().len(), 4);
            }
            Err(other) => panic!("{other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// cr-3aqchd round 4
// ---------------------------------------------------------------------------

/// Instance labels are injective: sort elements may contain the label's delimiters,
/// which are escaped. Without escaping, `(p = "x,q=y", q = "z")` and
/// `(p = "x", q = "y,q=z")` would both be `A(p=x,q=y,q=z)`.
#[test]
fn cr_3aqchd_instance_labels_are_injective() {
    let model = elaborate_source(
        "module L\ntype Node\nstate { c: Nat where c <= 1 }\ninit { c == 0 }\naction A(p: Node, q: Node) { require p != q\n next c = 1 - c }\n",
    )
    .expect("elaborates");
    let config = config(
        "L",
        r#"{"Nat":{"max":1}}"#,
        r#"{"Node":{"elements":["x,q=y","z","x","y,q=z","a\\b","(c)"]}}"#,
        "{}",
    );
    let low = lowered(&model, &config).expect("thirty-six distinct actions");
    let names: Vec<&str> = low.actions().iter().map(|a| a.name().as_str()).collect();
    assert_eq!(names.len(), 36);
    let distinct: std::collections::BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(distinct.len(), 36);
    assert!(names.contains(&r"A(p=x\,q\=y,q=z)"), "{names:?}");
    assert!(names.contains(&r"A(p=x,q=y\,q\=z)"), "{names:?}");
    assert!(names.contains(&r"A(p=a\\b,q=\(c\))"), "{names:?}");
}

/// A hand-built model (the public `lower` API takes any `NormModel`) whose binder
/// domain reads a post-state outside a postcondition, or of a variable that is not
/// relational, is refused — also when the variable's domain is a single value, where an
/// interval fold would otherwise have dropped the read.
#[test]
fn cr_3aqchd_a_primed_domain_is_never_folded_away() {
    fn prime(e: &mut Expr, var: &str) {
        match &mut e.kind {
            ExprKind::State(v) if v == var => e.kind = ExprKind::Primed(var.to_owned()),
            ExprKind::Quant(_, bs, body) => {
                for (_, b) in bs.iter_mut() {
                    if let Some(d) = &mut b.domain {
                        prime(d, var);
                    }
                }
                prime(body, var);
            }
            ExprKind::Binary(_, a, b) => {
                prime(a, var);
                prime(b, var);
            }
            ExprKind::Neg(a) | ExprKind::Not(a) => prime(a, var),
            _ => {}
        }
    }
    let head = "module P\nstate {\n  x: Nat where x <= 0\n  y: Nat where y <= 1\n}\ninit { x == 0 && y == 0 }\n";
    let refused = |model: &NormModel| match lower(model).expect_err("refused").kind {
        LowerErrorKind::Unlowerable(u) => u,
        other => panic!("{other:?}"),
    };
    // An invariant.
    let mut m = elaborate_source(&format!(
        "{head}action A {{ unchanged x, y }}\ninvariant I {{ forall i in x..x: i >= 0 }}\n"
    ))
    .expect("elaborates");
    prime(&mut m.invariants[0].clauses[0], "x");
    assert_eq!(refused(&m), Unlowerable::PrimedOutsidePostcondition);
    // An action guard.
    let mut m = elaborate_source(&format!(
        "{head}action A {{ require forall i in x..x: i >= 0\n unchanged x, y }}\n"
    ))
    .expect("elaborates");
    prime(&mut m.actions[0].guard[0], "x");
    assert_eq!(refused(&m), Unlowerable::PrimedOutsidePostcondition);
    // A postcondition, but `x` is kept, not relational.
    let mut m = elaborate_source(&format!(
        "{head}action A {{ y' != y && (forall i in x..x: i + y' >= 0)\n unchanged x }}\n"
    ))
    .expect("elaborates");
    let post = m.actions[0]
        .post
        .iter()
        .position(|c| matches!(c.kind, ExprKind::Quant(..)))
        .expect("the quantified postcondition");
    prime(&mut m.actions[0].post[post], "x");
    assert_eq!(refused(&m), Unlowerable::PrimedOutsidePostcondition);
}

/// Boolean set membership, literal and configured, agrees with the oracle (it is part of
/// the hand-written corpus); a configured `Set[Bool]` lowers as its codes.
#[test]
fn cr_3aqchd_boolean_membership_lowers() {
    let model = elaborate_source(&frame(&["(x == 0) in B".to_owned()])).expect("elaborates");
    let low = lowered(&model, &frame_config()).expect("lowers");
    assert_eq!(differential(&model, &low), (20, 0));
}

/// Adversarial pre-review findings (cr-3aqchd round 4).
///
/// 1. A range empty at build keeps one candidate at its lower bound, which may depend on
///    an enclosing binder or parameter: the plan checks the body at every value that
///    candidate can take, so an overflow there is refused, and a safe body agrees with
///    the oracle.
/// 2. A domain that is fixed once parameters and binders are fixed is planned
///    unguarded, as the RFC states, so it is not refused as a guarded overflow.
/// 3. A def inlined inside its own argument nests the same binder number: the inner
///    scope shadows and restores the outer one, and the result agrees with the oracle.
#[test]
fn adversarial_review_regressions() {
    let head =
        "module G\nstate { y: Nat where y <= 1 }\ninit { y == 0 }\naction A { unchanged y }\n";
    let check = |src: &str| -> Result<(), Unlowerable> {
        let model = elaborate_source(src).unwrap_or_else(|e| panic!("{e}"));
        let low = match lower(&model) {
            Ok(low) => low,
            Err(e) => match e.kind {
                LowerErrorKind::Unlowerable(u) => return Err(u),
                other => panic!("{other:?}"),
            },
        };
        for y in 0..=1_i64 {
            let state = low.state(&[y]).expect("in the domain");
            for inv in &model.invariants {
                let mut ev = Ev {
                    state: BTreeMap::from([("y".to_owned(), y)]),
                    binders: BTreeMap::new(),
                    sets: BTreeMap::new(),
                    nat_max: 1,
                    int_range: (0, 0),
                };
                let mut want: Result<bool, Failed> = Ok(true);
                for c in &inv.clauses {
                    let v = eval(c, &mut ev);
                    want = match (want, v) {
                        (Err(f), _) | (_, Err(f)) => Err(f),
                        (Ok(w), Ok(v)) => Ok(w && v != 0),
                    };
                }
                let index = low.predicate_index(&inv.name).expect("lowered");
                let got = low.evaluate_predicate(index, &state);
                assert_eq!(want.is_ok(), got.is_ok(), "{} at y={y}", inv.name);
                if let (Ok(w), Ok(g)) = (want, got) {
                    assert_eq!(w, g, "{} at y={y}", inv.name);
                }
            }
        }
        Ok(())
    };
    // 1. The finding's reproducers are refused; the same shapes with a safe body agree.
    for body in [
        "forall p in {0, 3}: forall i in (p + 10)..(0 - y * 9223372036854775807 - 9223372036854775807): i * 800000000000000000 >= 0",
        "forall p in {0, 10}: forall i in p..((y * 9223372036854775807 + 9223372036854775807) - 9223372036854775807 - 9223372036854775807 + 5): i * 1000000000000000000 >= 0",
    ] {
        assert_eq!(
            check(&format!("{head}invariant I {{ {body} }}\n")),
            Err(Unlowerable::GuardedOverflow),
            "{body}"
        );
        let safe = body
            .replace("800000000000000000", "8")
            .replace("1000000000000000000", "10");
        check(&format!("{head}invariant I {{ {safe} }}\n")).expect("agrees");
    }
    // 2. Fixed once parameters and binders are fixed: unguarded, lowers and agrees.
    check(&format!(
        "{head}invariant I {{ forall j in {{0, 1}}: forall i in 0..j: i <= j }}\n"
    ))
    .expect("agrees");
    check(&format!(
        "{head}invariant I {{ forall j in {{0, 1}}: forall i in 0..j: i * 4611686018427387904 + y >= 0 }}\n"
    ))
    .expect("static domains are evaluated only at their members");
    let schema = "module G\nstate { x: Nat where x <= 1 }\ninit { x == 0 }\naction A(n: Nat) { require forall i in 0..n: x + 9223372036854775806 >= i\n unchanged x }\n";
    lowered(
        &elaborate_source(schema).expect("elaborates"),
        &config("G", r#"{"Nat":{"max":2}}"#, "{}", "{}"),
    )
    .expect("a parameter-fixed domain is not a guarded one");
    // 3. A def inlined inside its own argument.
    check(&format!(
        "{head}def q(b: Bool): Bool = forall i in {{0, 1}}: b && i >= 0\ninvariant I {{ q(q(y == 0)) }}\n"
    ))
    .expect("the repeated binder shadows and restores");
}
