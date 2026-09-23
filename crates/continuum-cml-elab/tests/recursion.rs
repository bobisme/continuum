//! Recursive `def`s: the termination check, the unfolding, and its correctness
//! (bn-36x3b, PORTING_WAVES Wave 0 "recursive definitions", docs/11 §8
//! "totality/termination for model functions").
//!
//! # What is pinned
//!
//! - **the termination rule** — each accepted shape, and one mutant of it per clause of
//!   the rule that makes the check fail, typed;
//! - **Transitive Closure, set level** — the `closure` def of the parser fixture,
//!   unfolded by the elaborator, is evaluated by an evaluator written here and compared
//!   with bounded path reachability computed independently, over a fixed pseudo-random
//!   sample of edge sets; a mutant of the def is told apart;
//! - **Transitive Closure, integer level** — a Floyd–Warshall recursive def lowers to the
//!   programmatic model, and the reference engine's verdicts agree with Warshall's
//!   algorithm over every loop-free relation on three nodes; flipped expectations are
//!   refuted;
//! - **identity** — a constant measure written two ways, and alpha-renaming inside the
//!   recursive body, give one identity; unfolding renames binders, so no capture.
//!
//! The resource bounds of the unfolding are in `tests/resource_bounds.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use continuum_cml_elab::norm::{BinOp, Binder, Expr, ExprKind, Quant};
use continuum_cml_elab::{
    ElabError, ElabErrorKind, NormModel, Unsupported, elaborate_source, lower,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::checking::{self, CheckOutcome, DeadlockPolicy, Obligations};

fn fails(src: &str) -> ElabError {
    match elaborate_source(src) {
        Ok(m) => panic!("expected an error, elaborated:\n{}", m.dump()),
        Err(e) => e,
    }
}

/// Wrap declarations into a model with one bounded variable `x`.
fn model(decls: &str) -> String {
    format!(
        "module T\nstate {{ x: Nat where x <= 3 }}\ninit {{ x == 0 }}\naction A {{ unchanged x }}\n{decls}\n"
    )
}

// ---------------------------------------------------------------------------
// the termination rule
// ---------------------------------------------------------------------------

/// Each accepted shape: a `Nat` or `Int` measure, guards written as `==`, `>`, `<`,
/// `!`, mirrored, and a decrement of more than one.
#[test]
fn accepted_termination_shapes_elaborate_and_unfold() {
    let cases = [
        // (def, call, expected unfolded dump fragment)
        (
            "def f(k: Nat): Int = if k == 0 then 0 else f(k - 1) + 1",
            "f(3) == 3",
            "(add (add (add 0 1) 1) 1)",
        ),
        (
            "def f(k: Nat): Int = if k > 0 then f(k - 1) + 2 else 0",
            "f(2) == 4",
            "(add (add 0 2) 2)",
        ),
        (
            "def f(k: Nat): Int = if !(k == 0) then f(k - 1) + 3 else 0",
            "f(1) == 3",
            "(add 0 3)",
        ),
        (
            "def f(k: Nat): Int = if 0 < k then f(k - 1) + 4 else 0",
            "f(1) == 4",
            "(add 0 4)",
        ),
        (
            "def f(k: Int): Int = if k <= 0 then 0 else f(k - 1) + 5",
            "f(1) == 5",
            "(add 0 5)",
        ),
        (
            "def f(k: Int): Int = if k <= 0 then 0 else f(k - 1) + 5",
            "f(-4) == 0",
            "(eq 0 0)",
        ),
        (
            "def f(k: Nat): Int = if k < 2 then k else f(k - 2) + 1",
            "f(5) == 3",
            "(add (add 1 1) 1)",
        ),
        (
            "def f(s: Int, k: Nat): Int = if k == 0 then s else f(s + k, k - 1)",
            "f(0, 2) == 3",
            "(add (add 0 2) 1)",
        ),
    ];
    for (def, inv, fragment) in cases {
        let src = model(&format!("{def}\ninvariant I {{ {inv} }}"));
        let m = elaborate_source(&src).unwrap_or_else(|e| panic!("{def}: {e}"));
        let dump = m.dump();
        assert!(dump.contains(fragment), "{def}: {dump}");
        assert!(!dump.contains("recur"), "{def}: every call is unfolded");
        // Every case is an integer invariant, so it lowers, and it holds.
        assert_eq!(
            engine(&lower(&m).expect("lowers")),
            vec![("I".to_owned(), true)]
        );
    }
}

/// One mutant per clause of the rule. Each one is the accepted
/// `if k == 0 then 0 else f(k - 1) + 1` with a single change, and each is refused.
#[test]
fn each_clause_of_the_rule_is_load_bearing() {
    let base = "def f(k: Nat): Int = if k == 0 then 0 else f(k - 1) + 1";
    elaborate_source(&model(base)).expect("the base case is accepted");
    let mutants = [
        // no guard: the call is reachable at k == 0
        "def f(k: Nat): Int = f(k - 1) + 1",
        // the guard on the wrong branch
        "def f(k: Nat): Int = if k == 0 then f(k - 1) else 0",
        // the argument does not decrease
        "def f(k: Nat): Int = if k == 0 then 0 else f(k) + 1",
        "def f(k: Nat): Int = if k == 0 then 0 else f(k + 1) + 1",
        "def f(k: Nat): Int = if k == 0 then 0 else f(k - 0) + 1",
        // the decrement is larger than the guard's lower bound
        "def f(k: Nat): Int = if k == 0 then 0 else f(k - 2) + 1",
        // `k != 0` bounds an `Int` below by nothing
        "def f(k: Int): Int = if k == 0 then 0 else f(k - 1) + 1",
        // the guard tests something other than the measure
        "def f(k: Nat): Int = if x == 0 then 0 else f(k - 1) + 1",
        // no integer parameter at all
        "def f(b: Bool): Bool = if b then true else f(!b)",
        // a recursive call inside another's arguments
        "def f(k: Nat, s: Int): Int = if k == 0 then s else f(k - 1, f(k - 1, s))",
    ];
    for def in mutants {
        let e = fails(&model(def));
        assert_eq!(
            e.kind,
            ElabErrorKind::Unsupported(Unsupported::NoDecreasingMeasure),
            "{def}: {e}"
        );
        assert!(e.is_unsupported());
    }
}

/// A def is checked where it is declared, whether or not it is called.
#[test]
fn an_uncalled_non_terminating_def_is_refused_at_its_recursive_call() {
    let e = fails(&model("def f(k: Nat): Nat = f(k)"));
    assert_eq!(
        e.kind,
        ElabErrorKind::Unsupported(Unsupported::NoDecreasingMeasure)
    );
    assert_eq!((e.span.line, e.span.col), (5, 22));
}

#[test]
fn mutual_recursion_is_refused_with_or_without_a_guard() {
    for defs in [
        "def f(k: Nat): Nat = g(k)\ndef g(k: Nat): Nat = f(k)",
        "def f(k: Nat): Nat = if k == 0 then 0 else g(k - 1)\ndef g(k: Nat): Nat = if k == 0 then 0 else f(k - 1)",
    ] {
        let e = fails(&model(defs));
        assert_eq!(
            e.kind,
            ElabErrorKind::Unsupported(Unsupported::MutualRecursion),
            "{defs}"
        );
    }
}

/// The unfolding needs the measure as a constant; a state variable, a parameter of
/// an enclosing def, or an action parameter is refused, typed.
#[test]
fn a_non_constant_measure_is_refused() {
    let f = "def f(k: Nat): Int = if k == 0 then 0 else f(k - 1) + 1";
    for use_site in [
        "invariant I { f(x) >= 0 }".to_owned(),
        "def g(n: Nat): Int = f(n)\ninvariant I { g(2) >= 0 }".to_owned(),
        "action B(n: Nat) { require f(n) >= 0\n unchanged x }".to_owned(),
    ] {
        let e = fails(&model(&format!("{f}\n{use_site}")));
        assert_eq!(
            e.kind,
            ElabErrorKind::Unsupported(Unsupported::RecursionBoundNotConstant),
            "{use_site}"
        );
    }
    // A constant expression is folded; a `let` of a constant is a constant.
    elaborate_source(&model(&format!(
        "{f}\ninvariant I {{\n  let n = 2 * 3 - min(1, 4)\n  f(n) == 5\n}}"
    )))
    .expect("folds to 5");
}

#[test]
fn a_negative_nat_measure_is_refused() {
    let e = fails(&model(
        "def f(k: Nat): Int = if k == 0 then 0 else f(k - 1)\ninvariant I { f(0 - 1) == 0 }",
    ));
    assert_eq!(
        e.kind,
        ElabErrorKind::MeasureOutOfDomain {
            function: "f".to_owned(),
            value: -1
        }
    );
    assert_eq!(e.code(), "cml.elab.measure_out_of_domain");
}

/// Factorial through the whole pipeline: the unfolding lowers (the measure tests are
/// decided, so no integer-valued `if` is left) and the reference engine checks it.
/// The wrong value is refuted, so the check is not vacuous.
#[test]
fn factorial_lowers_and_the_engine_agrees() {
    let fact = "def fact(k: Nat): Int = if k == 0 then 1 else k * fact(k - 1)";
    let src = model(&format!(
        "{fact}\ninvariant Right {{ fact(5) == 120 }}\ninvariant Wrong {{ fact(5) == 121 }}"
    ));
    let m = elaborate_source(&src).expect("elaborates");
    let verdicts = engine(&lower(&m).expect("lowers"));
    assert_eq!(
        verdicts,
        vec![("Right".to_owned(), true), ("Wrong".to_owned(), false)]
    );
}

// ---------------------------------------------------------------------------
// Transitive Closure, set level: the parser fixture's `closure`
// ---------------------------------------------------------------------------

fn fixture_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../continuum-cml-syntax/tests/fixtures/TransitiveClosure.ctm");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A value of the evaluator below.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum V {
    B(bool),
    I(i64),
    T(Vec<V>),
    S(BTreeSet<V>),
}

/// An evaluator for the part of the normalized AST the closure unfolding uses. It is
/// written here, independently of the elaborator, and it resolves a bound variable to
/// its innermost binder by number, so a captured variable would evaluate wrongly.
struct Eval<'c> {
    consts: &'c BTreeMap<String, V>,
    env: Vec<(u32, V)>,
}

impl Eval<'_> {
    fn eval(&mut self, e: &Expr) -> V {
        match &e.kind {
            ExprKind::Bool(b) => V::B(*b),
            ExprKind::Int(n) => V::I(*n),
            ExprKind::Const(c) => self.consts[c].clone(),
            ExprKind::Bound { binder, .. } => self
                .env
                .iter()
                .rev()
                .find(|(b, _)| b == binder)
                .map(|(_, v)| v.clone())
                .expect("bound"),
            ExprKind::Tuple(items) => V::T(items.iter().map(|x| self.eval(x)).collect()),
            ExprKind::SetLit(items) => V::S(items.iter().map(|x| self.eval(x)).collect()),
            ExprKind::Not(a) => V::B(!self.bool(a)),
            ExprKind::If(c, a, b) => {
                if self.bool(c) {
                    self.eval(a)
                } else {
                    self.eval(b)
                }
            }
            ExprKind::Binary(op, a, b) => self.binary(*op, a, b),
            ExprKind::Quant(q, bs, body) => {
                let mut any = false;
                let mut all = true;
                self.each(bs, &mut |me| {
                    let v = me.bool(body);
                    any |= v;
                    all &= v;
                });
                V::B(if *q == Quant::Exists { any } else { all })
            }
            ExprKind::SetComp(x, bs, filter) => {
                let mut out = BTreeSet::new();
                self.each(bs, &mut |me| {
                    if filter.as_ref().is_none_or(|f| me.bool(f)) {
                        out.insert(me.eval(x));
                    }
                });
                V::S(out)
            }
            other => panic!("the evaluator does not cover {other:?}"),
        }
    }

    fn bool(&mut self, e: &Expr) -> bool {
        match self.eval(e) {
            V::B(b) => b,
            other => panic!("not a Bool: {other:?}"),
        }
    }

    fn set(&mut self, e: &Expr) -> BTreeSet<V> {
        match self.eval(e) {
            V::S(s) => s,
            other => panic!("not a set: {other:?}"),
        }
    }

    fn int(&mut self, e: &Expr) -> i64 {
        match self.eval(e) {
            V::I(n) => n,
            other => panic!("not an integer: {other:?}"),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> V {
        match op {
            BinOp::And => V::B(self.bool(a) && self.bool(b)),
            BinOp::Or => V::B(self.bool(a) || self.bool(b)),
            BinOp::Implies => V::B(!self.bool(a) || self.bool(b)),
            BinOp::Eq => V::B(self.eval(a) == self.eval(b)),
            BinOp::Ne => V::B(self.eval(a) != self.eval(b)),
            BinOp::In => {
                let x = self.eval(a);
                V::B(self.set(b).contains(&x))
            }
            BinOp::SubsetEq => {
                let x = self.set(a);
                V::B(x.is_subset(&self.set(b)))
            }
            BinOp::Union => {
                let mut x = self.set(a);
                x.extend(self.set(b));
                V::S(x)
            }
            BinOp::Add => V::I(self.int(a) + self.int(b)),
            BinOp::Sub => V::I(self.int(a) - self.int(b)),
            other => panic!("the evaluator does not cover {other:?}"),
        }
    }

    /// Run `f` once per assignment of the binders, each ranging over its domain in
    /// order; each domain is evaluated with the binders before it in scope.
    fn each(&mut self, bs: &[(u32, Binder)], f: &mut dyn FnMut(&mut Self)) {
        let Some(((id, b), rest)) = bs.split_first() else {
            f(self);
            return;
        };
        let domain = self.set(b.domain.as_ref().expect("a domain"));
        for v in domain {
            self.env.push((*id, v));
            self.each(rest, f);
            self.env.pop();
        }
    }
}

const NODES: i64 = 4;

/// A fixed pseudo-random sample of edge relations over `NODES` nodes (a linear
/// congruential generator with a fixed seed: no ambient randomness).
fn sample_relations(count: usize) -> Vec<BTreeSet<(i64, i64)>> {
    let mut state: u64 = 0x5eed_363b;
    let mut out = Vec::new();
    for _ in 0..count {
        let mut r = BTreeSet::new();
        for a in 0..NODES {
            for b in 0..NODES {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                if (state >> 33) % 3 == 0 {
                    r.insert((a, b));
                }
            }
        }
        out.push(r);
    }
    out
}

/// The independent computation: pairs joined by a path of 1 to `max_len` edges.
fn paths_up_to(edges: &BTreeSet<(i64, i64)>, max_len: usize) -> BTreeSet<(i64, i64)> {
    let mut reach = edges.clone();
    let mut frontier = edges.clone();
    for _ in 1..max_len {
        let mut next = BTreeSet::new();
        for &(a, b) in &frontier {
            for &(c, d) in edges {
                if b == c {
                    next.insert((a, d));
                }
            }
        }
        reach.extend(next.iter().copied());
        frontier = next;
    }
    reach
}

/// Warshall's algorithm: the transitive closure.
fn warshall(edges: &BTreeSet<(i64, i64)>, n: i64) -> BTreeSet<(i64, i64)> {
    let mut m = vec![vec![false; n as usize]; n as usize];
    for &(a, b) in edges {
        m[a as usize][b as usize] = true;
    }
    for k in 0..n as usize {
        for i in 0..n as usize {
            for j in 0..n as usize {
                m[i][j] = m[i][j] || (m[i][k] && m[k][j]);
            }
        }
    }
    let mut out = BTreeSet::new();
    for i in 0..n {
        for j in 0..n {
            if m[i as usize][j as usize] {
                out.insert((i, j));
            }
        }
    }
    out
}

fn as_value(r: &BTreeSet<(i64, i64)>) -> V {
    V::S(
        r.iter()
            .map(|&(a, b)| V::T(vec![V::I(a), V::I(b)]))
            .collect(),
    )
}

/// The unfolded `closure(Edges, k)` of `src`, read back from a probe invariant.
fn unfolded_closure(src: &str, k: u32) -> Expr {
    let probe = format!("{src}\ninvariant Probe {{ closure(Edges, {k}) subseteq reach }}\n");
    let m = elaborate_source(&probe).unwrap_or_else(|e| panic!("elaborates: {e}"));
    let inv = m
        .invariants
        .iter()
        .find(|i| i.name == "Probe")
        .expect("the probe");
    let [clause] = inv.clauses.as_slice() else {
        panic!("one clause");
    };
    let ExprKind::Binary(BinOp::SubsetEq, lhs, _) = &clause.kind else {
        panic!("the probe is a subseteq");
    };
    (**lhs).clone()
}

/// Whether the unfolded `closure` of `src` agrees with bounded path reachability on
/// every sampled relation, for `k` in `0..=2` (`closure(r, k)` holds the paths of at
/// most `2^k` edges, and `2^2` edges reach everything on four nodes).
fn closure_agrees(src: &str) -> bool {
    let nodes = V::S((0..NODES).map(V::I).collect());
    for k in 0..=2_u32 {
        let expr = unfolded_closure(src, k);
        for edges in sample_relations(24) {
            let consts = BTreeMap::from([
                ("Nodes".to_owned(), nodes.clone()),
                ("Edges".to_owned(), as_value(&edges)),
            ]);
            let mut ev = Eval {
                consts: &consts,
                env: Vec::new(),
            };
            let got = ev.eval(&expr);
            let want = as_value(&paths_up_to(&edges, 1 << k));
            if got != want {
                return false;
            }
            if k == 2 {
                assert_eq!(want, as_value(&warshall(&edges, NODES)));
            }
        }
    }
    true
}

#[test]
fn the_fixture_closure_unfolds_to_bounded_reachability() {
    assert!(closure_agrees(&fixture_source()));
}

/// The differential is not vacuous: dropping `r union` from the recursive step (so
/// `closure` keeps only the longest paths) is told apart.
#[test]
fn a_mutated_closure_is_told_apart() {
    let src = fixture_source();
    let mutated = src.replace(
        "closure(r union compose(r, r), k - 1)",
        "closure(compose(r, r), k - 1)",
    );
    assert_ne!(mutated, src, "the mutation applies");
    assert!(!closure_agrees(&mutated));
}

// ---------------------------------------------------------------------------
// Transitive Closure, integer level: lowered and checked by the reference engine
// ---------------------------------------------------------------------------

/// The reference engine's verdict for each invariant, by name: whether it holds.
fn engine(model: &continuum_model_core::Model) -> Vec<(String, bool)> {
    let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("explores");
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    let report = checking::check(model, &exploration, &obligations).expect("checks");
    model
        .predicates()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let outcome = report.invariant(i).expect("checked").outcome().clone();
            let holds = match outcome {
                CheckOutcome::Holds { .. } => true,
                CheckOutcome::Violated { .. } => false,
                other => panic!("{other:?}"),
            };
            (p.name().to_string(), holds)
        })
        .collect()
}

const INT_NODES: i64 = 3;

/// Floyd–Warshall as a recursive def: `reach(i, j, k)` holds when a path from `i` to
/// `j` has every intermediate node below `k`. One invariant per pair states the
/// expected answer, `expect(i, j)`.
fn floyd_warshall_source(
    edges: &BTreeSet<(i64, i64)>,
    expect: impl Fn(i64, i64) -> bool,
) -> String {
    let mut edge: Vec<String> = edges
        .iter()
        .map(|(a, b)| format!("(i == {a} && j == {b})"))
        .collect();
    edge.push("false".to_owned());
    let mut src = format!(
        "module FloydWarshall\n\
         state {{ x: Nat where x <= 0 }}\n\
         init {{ x == 0 }}\n\
         action Idle {{ unchanged x }}\n\
         def edge(i: Int, j: Int): Bool = {}\n\
         def reach(i: Int, j: Int, k: Nat): Bool =\n  \
           if k == 0 then edge(i, j)\n  \
           else reach(i, j, k - 1) || (reach(i, k - 1, k - 1) && reach(k - 1, j, k - 1))\n",
        edge.join(" || ")
    );
    for i in 0..INT_NODES {
        for j in 0..INT_NODES {
            let not = if expect(i, j) { "" } else { "!" };
            src.push_str(&format!(
                "invariant R{i}{j} {{ {not}reach({i}, {j}, {INT_NODES}) }}\n"
            ));
        }
    }
    src
}

/// Every loop-free relation on three nodes (64 of them): the lowered recursive def
/// agrees with Warshall's algorithm on every pair.
#[test]
fn floyd_warshall_lowers_and_agrees_with_warshall_everywhere() {
    let pairs: Vec<(i64, i64)> = (0..INT_NODES)
        .flat_map(|a| (0..INT_NODES).map(move |b| (a, b)))
        .filter(|(a, b)| a != b)
        .collect();
    for mask in 0..(1_u32 << pairs.len()) {
        let edges: BTreeSet<(i64, i64)> = pairs
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, p)| *p)
            .collect();
        let closure = warshall(&edges, INT_NODES);
        let src = floyd_warshall_source(&edges, |i, j| closure.contains(&(i, j)));
        let m = elaborate_source(&src).unwrap_or_else(|e| panic!("{mask}: {e}"));
        let lowered = lower(&m).unwrap_or_else(|e| panic!("{mask}: {e}"));
        for (name, holds) in engine(&lowered) {
            assert!(holds, "relation {mask:06b}: {name} is refuted");
        }
    }
}

/// Anti-vacuity: flip the expectation of one pair and the engine refutes exactly that
/// invariant.
#[test]
fn a_flipped_floyd_warshall_expectation_is_refuted() {
    let edges = BTreeSet::from([(0, 1), (1, 2)]);
    let closure = warshall(&edges, INT_NODES);
    let src = floyd_warshall_source(&edges, |i, j| {
        closure.contains(&(i, j)) != (i == 0 && j == 2)
    });
    let m = elaborate_source(&src).expect("elaborates");
    let refuted: Vec<String> = engine(&lower(&m).expect("lowers"))
        .into_iter()
        .filter(|(_, holds)| !holds)
        .map(|(name, _)| name)
        .collect();
    assert_eq!(refuted, vec!["R02".to_owned()]);
}

// ---------------------------------------------------------------------------
// identity and capture
// ---------------------------------------------------------------------------

fn identity_of(src: &str) -> continuum_cml_elab::NormIdentity {
    let m: NormModel = elaborate_source(src).unwrap_or_else(|e| panic!("{e}"));
    let again = elaborate_source(src).expect("again");
    assert_eq!(m, again, "elaboration is deterministic");
    m.identity()
}

fn with_probe(src: &str, k: &str) -> String {
    format!("{src}\ninvariant P {{ closure(Edges, {k}) subseteq reach }}\n")
}

/// Metamorphic relation "alpha-renaming": renaming the binders of the recursive def's
/// body (through the `compose` it inlines) and its parameters leaves the identity of
/// the unfolded model unchanged. A different measure value changes it.
#[test]
fn alpha_renaming_a_recursive_body_preserves_identity() {
    let src = fixture_source();
    let base = identity_of(&with_probe(&src, "2"));
    let renamed = src
        .replace(
            "{(a, c) | a in Nodes, c in Nodes where exists b in Nodes: (a, b) in r && (b, c) in s}",
            "{(p, q) | p in Nodes, q in Nodes where exists m in Nodes: (p, m) in r && (m, q) in s}",
        )
        .replace(
            "def closure(r: Set[(Node, Node)], k: Nat): Set[(Node, Node)] =\n  if k == 0 then r else closure(r union compose(r, r), k - 1)",
            "def closure(rel: Set[(Node, Node)], n: Nat): Set[(Node, Node)] =\n  if n == 0 then rel else closure(rel union compose(rel, rel), n - 1)",
        );
    assert!(
        renamed.contains("(p, m) in r") && renamed.contains("n == 0 then rel"),
        "both renamings apply"
    );
    assert_eq!(base, identity_of(&with_probe(&renamed, "2")));
    assert_ne!(base, identity_of(&with_probe(&src, "1")));
}

/// Metamorphic relation "equivalent guard normalization": the measure test written as
/// `k == 0`, `k <= 0`, `!(k > 0)` with its branches swapped, or `0 < k` swapped, and the
/// measure written as `2` or `1 + 1`, all unfold to one model with one identity.
#[test]
fn equivalent_guard_normalization_of_a_recursive_def_preserves_identity() {
    let src = fixture_source();
    let original = "if k == 0 then r else closure(r union compose(r, r), k - 1)";
    assert!(src.contains(original));
    let base = identity_of(&with_probe(&src, "2"));
    assert_eq!(base, identity_of(&with_probe(&src, "1 + 1")));
    for guard in [
        "if k <= 0 then r else closure(r union compose(r, r), k - 1)",
        "if !(k > 0) then r else closure(r union compose(r, r), k - 1)",
        "if 0 < k then closure(r union compose(r, r), k - 1) else r",
    ] {
        let variant = src.replace(original, guard);
        assert_eq!(base, identity_of(&with_probe(&variant, "2")), "{guard}");
    }
}

/// The arguments of a recursive call may mention the body's own binders. Each
/// unfolding renumbers them, so the next unfolding's binder does not capture the
/// argument. With capture, `g(0, 2)` would evaluate to `false`.
#[test]
fn unfolding_does_not_capture_bound_variables() {
    let src = model(
        "def g(y: Int, k: Nat): Bool =\n  if k == 0 then false\n  else exists m in {k}: (y == m + 1 || g(m, k - 1))\ninvariant I { g(0, 2) }",
    );
    let m = elaborate_source(&src).expect("elaborates");
    let inv = m.invariants.first().expect("I");
    let consts = BTreeMap::new();
    let mut ev = Eval {
        consts: &consts,
        env: Vec::new(),
    };
    // g(0, 2) = exists m1 in {2}: 0 == m1 + 1 || (exists m2 in {1}: m1 == m2 + 1 || false)
    assert_eq!(ev.eval(inv.clauses.first().expect("a clause")), V::B(true));
}
