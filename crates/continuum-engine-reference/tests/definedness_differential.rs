//! Definedness differential (bn-24a5c): a generated corpus of models with a partial
//! map, where reads are undefined at some reachable states, checked by the reference
//! engine and by an independent evaluator written here.
//!
//! # The two sides
//!
//! Each case is a small "CML-like" specification: a scalar `x` in `0..=2`, a partial
//! map `m` from keys `{0, 1}` to `0..=2`, actions with guard clauses and updates, and
//! invariants. The expressions read `m[k]`, which is defined only when `k` is a key of
//! `m` (RFC 0003, "Definedness"). Evaluation is strict: an undefined read anywhere in a
//! clause makes the clause undefined.
//!
//! - **The oracle** in this file interprets the specification directly, with
//!   `Option` values: it explores the state space by its own breadth-first search, it
//!   marks every state where an action's guard or (enabled) updates read an absent key,
//!   and it derives the expected outcome of every obligation by RFC 0003's reading and
//!   the precedence of `continuum_engine_reference::definedness`: undefined action read,
//!   then undefined read in the invariant, then violation, then holds. It shares no
//!   evaluation code with the engine: it never calls `Model::evaluate_predicate`,
//!   `Model::successors`, `bfs::explore`, or `checking::check`.
//! - **The subject** is `continuum-engine-reference` over the model this file lowers
//!   the specification to, the way `continuum-cml-elab` lowers a partial map (RFC 0003,
//!   "Definedness"): a presence slot `m?k` and a value slot `m!k` per key, the
//!   predicate `I#defined` for an invariant with reads, `A#defined = D(G) && (G =>
//!   D(U))` for an action with reads, and each action guard conjoined with `D(G)` and
//!   `D(U)`.
//!
//! Every checking path is compared: the reachable set, `checking::check` for every
//! predicate (the definedness predicates too) and the deadlock outcome, the shortest
//! witnesses, the finite-closure and invariant-closure emitters (with the kernel's
//! verdict on what is emitted), liveness, and a bounded exploration.
//!
//! # Metamorphic relations
//!
//! "stable reordering of declarations" and "alpha-renaming" (docs/19 §3): reversing
//! the declaration order, and renaming every action and invariant together with its
//! `#defined` predicate, preserve every outcome. The non-preservation control renames
//! only the `#defined` predicate, which detaches it from its subject.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{
    self, ClaimEnvelope, ClosedSet, EmissionError, PRODUCER,
};
use continuum_engine_reference::checking::{
    self, CheckOutcome, DeadlockOutcome, DeadlockPolicy, Obligations,
};
use continuum_engine_reference::liveness::{Goal, LivenessOutcome, Stuttering, check_liveness};
use continuum_engine_reference::witness::{self, NoWitness, Target};
use continuum_engine_reference::{
    ActionDecl, BoolExpr, CmpOp, Guarded, IntExpr, Model, ModelBuilder, State, Undefined,
};
use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::Verdict as KernelVerdict;

// ---------------------------------------------------------------------------
// the specification language, and its seeded generator
// ---------------------------------------------------------------------------

const KEYS: usize = 2;
const TOP: i64 = 2;

#[derive(Debug, Clone)]
enum IE {
    C(i64),
    X,
    Get(usize),
    Min(Box<IE>, Box<IE>),
    Max(Box<IE>, Box<IE>),
}

#[derive(Debug, Clone)]
enum BE {
    Cmp(CmpOp, IE, IE),
    Has(usize),
    Not(Box<BE>),
    And(Box<BE>, Box<BE>),
    Or(Box<BE>, Box<BE>),
    Imp(Box<BE>, Box<BE>),
}

#[derive(Debug, Clone)]
enum Upd {
    X(IE),
    Put(usize, IE),
    Remove(usize),
}

#[derive(Debug, Clone)]
struct Act {
    name: String,
    guard: Vec<BE>,
    updates: Vec<Upd>,
}

#[derive(Debug, Clone)]
struct Spec {
    acts: Vec<Act>,
    invs: Vec<(String, BE)>,
    init: Vec<OState>,
    /// Nested and programmatic definedness guards (cr-pt5h3a): `base#defined` repeated
    /// `depth` times, with a total body. Where the body is false, `base` has an
    /// undefined read. A base that names no declaration (`Ghost`) is an orphan, read as
    /// an action's.
    metas: Vec<Meta>,
    /// Extra, never-enabled actions whose names collide with a name of an invariant's
    /// chain (cr-pt5h3a round 2): `(base, depth, schema)` names the action
    /// `base` + `#defined` × `depth`, as an instance `…(k=0)` when `schema`. A collision
    /// at any position makes the chain an action's (fail closed).
    collisions: Vec<(String, usize, bool)>,
}

/// The name of a collision action.
fn collision_name(base: &str, depth: usize, schema: bool) -> String {
    let name = format!("{base}{}", "#defined".repeat(depth));
    if schema { format!("{name}(k=0)") } else { name }
}

#[derive(Debug, Clone)]
struct Meta {
    base: String,
    depth: usize,
    body: BE,
}

/// A body with no map read, so it is total (a guard's own reads are its deeper guards).
fn gen_total_be(rng: &mut Rng, depth: u32) -> BE {
    let ops = [
        CmpOp::Eq,
        CmpOp::Ne,
        CmpOp::Lt,
        CmpOp::Le,
        CmpOp::Gt,
        CmpOp::Ge,
    ];
    let leaf = |rng: &mut Rng| {
        if rng.chance(50) {
            IE::X
        } else {
            IE::C(rng.below(3) as i64)
        }
    };
    match (depth, rng.below(if depth == 0 { 2 } else { 4 })) {
        (_, 0) => BE::Cmp(ops[rng.below(6) as usize], leaf(rng), leaf(rng)),
        (_, 1) => BE::Has(rng.below(KEYS as u64) as usize),
        (_, 2) => BE::Not(Box::new(gen_total_be(rng, depth - 1))),
        _ => BE::And(
            Box::new(gen_total_be(rng, depth - 1)),
            Box::new(gen_total_be(rng, depth - 1)),
        ),
    }
}

/// An oracle state: `x` and the partial map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OState {
    x: i64,
    m: [Option<i64>; KEYS],
}

/// splitmix64: a seeded, deterministic source (INV-005).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn gen_ie(rng: &mut Rng, depth: u32) -> IE {
    match (depth, rng.below(if depth == 0 { 3 } else { 5 })) {
        (_, 0) => IE::C(rng.below(3) as i64),
        (_, 1) => IE::X,
        (_, 2) => IE::Get(rng.below(KEYS as u64) as usize),
        (_, 3) => IE::Min(
            Box::new(gen_ie(rng, depth - 1)),
            Box::new(gen_ie(rng, depth - 1)),
        ),
        _ => IE::Max(
            Box::new(gen_ie(rng, depth - 1)),
            Box::new(gen_ie(rng, depth - 1)),
        ),
    }
}

fn gen_be(rng: &mut Rng, depth: u32) -> BE {
    let ops = [
        CmpOp::Eq,
        CmpOp::Ne,
        CmpOp::Lt,
        CmpOp::Le,
        CmpOp::Gt,
        CmpOp::Ge,
    ];
    match (depth, rng.below(if depth == 0 { 2 } else { 6 })) {
        (_, 0) => BE::Cmp(ops[rng.below(6) as usize], gen_ie(rng, 1), gen_ie(rng, 1)),
        (_, 1) => BE::Has(rng.below(KEYS as u64) as usize),
        (_, 2) => BE::Not(Box::new(gen_be(rng, depth - 1))),
        (_, 3) => BE::And(
            Box::new(gen_be(rng, depth - 1)),
            Box::new(gen_be(rng, depth - 1)),
        ),
        (_, 4) => BE::Or(
            Box::new(gen_be(rng, depth - 1)),
            Box::new(gen_be(rng, depth - 1)),
        ),
        _ => BE::Imp(
            Box::new(gen_be(rng, depth - 1)),
            Box::new(gen_be(rng, depth - 1)),
        ),
    }
}

fn gen_state(rng: &mut Rng) -> OState {
    let mut m = [None; KEYS];
    for slot in &mut m {
        if rng.chance(50) {
            *slot = Some(rng.below(3) as i64);
        }
    }
    OState {
        x: rng.below(3) as i64,
        m,
    }
}

fn gen_spec(seed: u64) -> Spec {
    let mut rng = Rng(seed);
    let mut acts = Vec::new();
    for a in 0..(2 + rng.below(2)) {
        let guard = (0..rng.below(3)).map(|_| gen_be(&mut rng, 2)).collect();
        let mut updates = Vec::new();
        if rng.chance(60) {
            updates.push(Upd::X(gen_ie(&mut rng, 1)));
        }
        for k in 0..KEYS {
            match rng.below(3) {
                0 => updates.push(Upd::Put(k, gen_ie(&mut rng, 1))),
                1 if rng.chance(50) => updates.push(Upd::Remove(k)),
                _ => {}
            }
        }
        acts.push(Act {
            name: format!("A{a}"),
            guard,
            updates,
        });
    }
    let invs: Vec<(String, BE)> = (0..(1 + rng.below(2)))
        .map(|i| (format!("I{i}"), gen_be(&mut rng, 2)))
        .collect();
    let mut init: Vec<OState> = (0..(1 + rng.below(2)))
        .map(|_| gen_state(&mut rng))
        .collect();
    init.sort();
    init.dedup();
    // Nested guards on some declarations, and now and then an orphan chain. The depth
    // is 2 or 3 for a declared base, so a chain may skip `X#defined` (when `X` reads
    // nothing) or stack on it; an orphan may also be depth 1.
    let mut metas: Vec<Meta> = Vec::new();
    let mut bases: Vec<String> = acts.iter().map(|a: &Act| a.name.clone()).collect();
    bases.extend(invs.iter().map(|(n, _): &(String, BE)| n.clone()));
    for base in &bases {
        if rng.chance(30) {
            let depth = 2 + rng.below(2) as usize;
            metas.push(Meta {
                base: base.clone(),
                depth,
                body: gen_total_be(&mut rng, 1),
            });
            if depth == 3 && rng.chance(50) {
                metas.push(Meta {
                    base: base.clone(),
                    depth: 2,
                    body: gen_total_be(&mut rng, 1),
                });
            }
        }
    }
    if rng.chance(10) {
        metas.push(Meta {
            base: "Ghost".to_owned(),
            depth: 1 + rng.below(2) as usize,
            body: gen_total_be(&mut rng, 1),
        });
    }
    // Collisions at every chain position: the base, each member's full name (depth 1,
    // 2 or 3), as an exact action name or as a schema of instances.
    let mut collisions: Vec<(String, usize, bool)> = Vec::new();
    if rng.chance(30) {
        let (name, _) = &invs[rng.below(invs.len() as u64) as usize];
        collisions.push((name.clone(), rng.below(4) as usize, rng.chance(50)));
    }
    Spec {
        acts,
        invs,
        init,
        metas,
        collisions,
    }
}

/// Whether some nested guard of `base` at depth `min_depth` or deeper is false at `s`.
fn meta_false(spec: &Spec, base: &str, min_depth: usize, s: &OState) -> bool {
    spec.metas
        .iter()
        .any(|m| m.base == base && m.depth >= min_depth && ev_b(&m.body, s) == Some(false))
}

/// The action-level bases, in name order: every action, and every orphan.
fn action_bases(spec: &Spec) -> Vec<String> {
    let mut out: BTreeSet<String> = spec.acts.iter().map(|a| a.name.clone()).collect();
    for m in &spec.metas {
        if !spec.invs.iter().any(|(n, _)| *n == m.base) {
            out.insert(m.base.clone());
        }
    }
    for (i, (name, _)) in spec.invs.iter().enumerate() {
        if !inv_chain_well_formed(spec, i) {
            out.insert(name.clone());
        }
    }
    // The engine orders action chains by their shallowest member's name, not by the
    // base: they differ when one base is a prefix of another followed by a byte below
    // `#`. So sort by the shallowest member's name, computed here independently.
    let mut out: Vec<String> = out.into_iter().collect();
    out.sort_by_key(|base| format!("{base}{}", "#defined".repeat(shallowest(spec, base))));
    out
}

/// The shallowest chain depth of `base` (1 when it has no nested guard: an action's or
/// invariant's `#defined` for its own reads, or no chain at all).
fn shallowest(spec: &Spec, base: &str) -> usize {
    let own_reads = spec.acts.iter().any(|a| {
        a.name == base && {
            let mut reads = BTreeSet::new();
            for g in &a.guard {
                reads_b(g, &mut reads);
            }
            for u in &a.updates {
                match u {
                    Upd::X(e) | Upd::Put(_, e) => reads_i(e, &mut reads),
                    Upd::Remove(_) => {}
                }
            }
            !reads.is_empty()
        }
    }) || spec.invs.iter().any(|(n, b)| {
        n == base && {
            let mut reads = BTreeSet::new();
            reads_b(b, &mut reads);
            !reads.is_empty()
        }
    });
    if own_reads {
        return 1;
    }
    spec.metas
        .iter()
        .filter(|m| m.base == base)
        .map(|m| m.depth)
        .min()
        .unwrap_or(1)
}

/// Whether invariant `i`'s chain has no gap: its depths are exactly `1..=max` (depth
/// 1 is `I#defined`, present when `I` reads the map). A chain with a gap is not the
/// invariant's; it is read as an action's (fail closed).
fn inv_chain_well_formed(spec: &Spec, i: usize) -> bool {
    let (name, body) = &spec.invs[i];
    // A collision at the base, or at the full name of a member that exists (an
    // intermediate is a member), makes the chain an action's.
    let present = |depth: usize| {
        depth == 0
            || spec
                .metas
                .iter()
                .any(|m| m.base == *name && m.depth == depth)
            || (depth == 1 && {
                let mut reads = BTreeSet::new();
                reads_b(body, &mut reads);
                !reads.is_empty()
            })
    };
    let has_chain = (1..=3).any(present);
    if has_chain
        && spec
            .collisions
            .iter()
            .any(|(base, depth, _)| base == name && present(*depth))
    {
        return false;
    }
    let mut depths: BTreeSet<usize> = spec
        .metas
        .iter()
        .filter(|m| m.base == *name)
        .map(|m| m.depth)
        .collect();
    let mut reads = BTreeSet::new();
    reads_b(body, &mut reads);
    if !reads.is_empty() {
        depths.insert(1);
    }
    depths.iter().copied().eq(1..=depths.len())
}

/// Whether the action-level base has an undefined read at `s`: the action's CML
/// meaning is undefined there, or one of its nested guards is false; for an invariant
/// whose chain has a gap, a read of the invariant is undefined, or a guard is false.
fn action_undefined(spec: &Spec, base: &str, s: &OState) -> bool {
    let inv = spec.invs.iter().position(|(n, _)| n == base);
    if let Some(i) = inv {
        if inv_chain_well_formed(spec, i) {
            return false;
        }
        return ev_b(&spec.invs[i].1, s).is_none() || meta_false(spec, base, 1, s);
    }
    spec.acts
        .iter()
        .find(|a| a.name == base)
        .is_some_and(|a| matches!(fire(a, s), Fire::Undefined))
        || meta_false(spec, base, 1, s)
}

// ---------------------------------------------------------------------------
// the independent oracle: direct, strict, Option-valued interpretation
// ---------------------------------------------------------------------------

fn ev_i(e: &IE, s: &OState) -> Option<i64> {
    match e {
        IE::C(c) => Some(*c),
        IE::X => Some(s.x),
        IE::Get(k) => s.m[*k],
        IE::Min(a, b) => {
            let (a, b) = (ev_i(a, s), ev_i(b, s));
            Some(a?.min(b?))
        }
        IE::Max(a, b) => {
            let (a, b) = (ev_i(a, s), ev_i(b, s));
            Some(a?.max(b?))
        }
    }
}

fn ev_b(e: &BE, s: &OState) -> Option<bool> {
    match e {
        BE::Cmp(op, a, b) => {
            let (a, b) = (ev_i(a, s), ev_i(b, s));
            let (a, b) = (a?, b?);
            Some(match op {
                CmpOp::Eq => a == b,
                CmpOp::Ne => a != b,
                CmpOp::Lt => a < b,
                CmpOp::Le => a <= b,
                CmpOp::Gt => a > b,
                CmpOp::Ge => a >= b,
            })
        }
        BE::Has(k) => Some(s.m[*k].is_some()),
        BE::Not(a) => Some(!ev_b(a, s)?),
        // Strict: both operands are evaluated, and an undefined one is undefined
        // whatever the other is. Both are unwrapped before the connective, because
        // Rust's `&&` and `||` would short-circuit past an undefined right operand.
        BE::And(a, b) => {
            let (a, b) = (ev_b(a, s), ev_b(b, s));
            let (a, b) = (a?, b?);
            Some(a && b)
        }
        BE::Or(a, b) => {
            let (a, b) = (ev_b(a, s), ev_b(b, s));
            let (a, b) = (a?, b?);
            Some(a || b)
        }
        BE::Imp(a, b) => {
            let (a, b) = (ev_b(a, s), ev_b(b, s));
            let (a, b) = (a?, b?);
            Some(!a || b)
        }
    }
}

/// One action at one state, in CML's meaning.
enum Fire {
    Undefined,
    Disabled,
    To(OState),
}

fn fire(act: &Act, s: &OState) -> Fire {
    // `D(G)`: every guard clause is evaluated, strictly.
    let clauses: Vec<Option<bool>> = act.guard.iter().map(|g| ev_b(g, s)).collect();
    if clauses.iter().any(Option::is_none) {
        return Fire::Undefined;
    }
    if !clauses.iter().all(|c| *c == Some(true)) {
        return Fire::Disabled;
    }
    // `G => D(U)`: the updates are evaluated where the guard holds.
    let mut next = *s;
    for update in &act.updates {
        match update {
            Upd::X(e) => match ev_i(e, s) {
                Some(v) => next.x = v,
                None => return Fire::Undefined,
            },
            Upd::Put(k, e) => match ev_i(e, s) {
                Some(v) => next.m[*k] = Some(v),
                None => return Fire::Undefined,
            },
            Upd::Remove(k) => next.m[*k] = None,
        }
    }
    Fire::To(next)
}

/// The oracle's own breadth-first search. Each state carries its depth and whether
/// any action fires there.
struct OSpace {
    depth: BTreeMap<OState, usize>,
    fires: BTreeMap<OState, bool>,
}

fn oracle_explore(spec: &Spec) -> OSpace {
    let mut depth = BTreeMap::new();
    let mut fires = BTreeMap::new();
    let mut queue = VecDeque::new();
    for s in &spec.init {
        depth.insert(*s, 0);
        queue.push_back(*s);
    }
    while let Some(s) = queue.pop_front() {
        let d = depth[&s];
        let mut any = false;
        for act in &spec.acts {
            match fire(act, &s) {
                Fire::Undefined | Fire::Disabled => {}
                Fire::To(t) => {
                    any = true;
                    if let std::collections::btree_map::Entry::Vacant(slot) = depth.entry(t) {
                        slot.insert(d + 1);
                        queue.push_back(t);
                    }
                }
            }
        }
        fires.insert(s, any);
    }
    OSpace { depth, fires }
}

// ---------------------------------------------------------------------------
// the lowering, written from RFC 0003 "Definedness"
// ---------------------------------------------------------------------------

fn has(k: usize) -> String {
    format!("m?{k}")
}
fn val(k: usize) -> String {
    format!("m!{k}")
}

fn low_i(e: &IE) -> IntExpr {
    match e {
        IE::C(c) => IntExpr::constant(*c),
        IE::X => IntExpr::var("x"),
        IE::Get(k) => IntExpr::var(&val(*k)),
        IE::Min(a, b) => IntExpr::min(low_i(a), low_i(b)),
        IE::Max(a, b) => IntExpr::max(low_i(a), low_i(b)),
    }
}

fn low_b(e: &BE) -> BoolExpr {
    match e {
        BE::Cmp(op, a, b) => BoolExpr::compare(*op, low_i(a), low_i(b)),
        BE::Has(k) => present(*k),
        BE::Not(a) => BoolExpr::negate(low_b(a)),
        BE::And(a, b) => BoolExpr::and(low_b(a), low_b(b)),
        BE::Or(a, b) => BoolExpr::or(low_b(a), low_b(b)),
        BE::Imp(a, b) => BoolExpr::implies(low_b(a), low_b(b)),
    }
}

fn present(k: usize) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, IntExpr::var(&has(k)), IntExpr::constant(1))
}

fn reads_i(e: &IE, out: &mut BTreeSet<usize>) {
    match e {
        IE::C(_) | IE::X => {}
        IE::Get(k) => {
            out.insert(*k);
        }
        IE::Min(a, b) | IE::Max(a, b) => {
            reads_i(a, out);
            reads_i(b, out);
        }
    }
}

fn reads_b(e: &BE, out: &mut BTreeSet<usize>) {
    match e {
        BE::Cmp(_, a, b) => {
            reads_i(a, out);
            reads_i(b, out);
        }
        BE::Has(_) => {}
        BE::Not(a) => reads_b(a, out),
        BE::And(a, b) | BE::Or(a, b) | BE::Imp(a, b) => {
            reads_b(a, out);
            reads_b(b, out);
        }
    }
}

/// `D(reads)`: every read key is present; `None` when nothing is read.
fn defined(reads: &BTreeSet<usize>) -> Option<BoolExpr> {
    reads.iter().map(|k| present(*k)).reduce(BoolExpr::and)
}

fn conj(parts: Vec<BoolExpr>) -> BoolExpr {
    parts
        .into_iter()
        .reduce(BoolExpr::and)
        .unwrap_or(BoolExpr::constant(true))
}

/// Names for the declarations: the identity, or a renaming (alpha-renaming), and
/// whether the definedness predicates follow their subjects' names.
struct Naming {
    rename: bool,
    detach_guards: bool,
    reverse: bool,
}

const PLAIN: Naming = Naming {
    rename: false,
    detach_guards: false,
    reverse: false,
};

/// The renaming reverses the declarations' name order (`A0` sorts last, `I1` first),
/// so a classifier or a tie rule that depended on it would show.
const RENAMES: [(&str, &str); 5] = [
    ("A0", "zq"),
    ("A1", "zm"),
    ("A2", "zb"),
    ("I0", "za"),
    ("I1", "Z"),
];

fn name_of(naming: &Naming, name: &str) -> String {
    if naming.rename {
        RENAMES
            .iter()
            .find(|(from, _)| *from == name)
            .map(|(_, to)| (*to).to_owned())
            .unwrap()
    } else {
        name.to_owned()
    }
}

fn unname(name: &str) -> String {
    RENAMES
        .iter()
        .find(|(_, to)| *to == name)
        .map_or(name, |(from, _)| from)
        .to_owned()
}

fn guard_name(naming: &Naming, subject: &str) -> String {
    if naming.detach_guards {
        format!("{}_defined", name_of(naming, subject))
    } else {
        format!("{}#defined", name_of(naming, subject))
    }
}

fn lower(spec: &Spec, naming: &Naming) -> Model {
    let mut b = ModelBuilder::new().variable("x", 0, TOP);
    for k in 0..KEYS {
        b = b.variable(&has(k), 0, 1).variable(&val(k), 0, TOP);
    }
    let mut acts: Vec<&Act> = spec.acts.iter().collect();
    let mut invs: Vec<&(String, BE)> = spec.invs.iter().collect();
    if naming.reverse {
        acts.reverse();
        invs.reverse();
    }
    for act in acts {
        let mut dg = BTreeSet::new();
        for g in &act.guard {
            reads_b(g, &mut dg);
        }
        let mut du = BTreeSet::new();
        let mut updates: Vec<(String, IntExpr)> = Vec::new();
        for update in &act.updates {
            match update {
                Upd::X(e) => {
                    reads_i(e, &mut du);
                    updates.push(("x".to_owned(), low_i(e)));
                }
                Upd::Put(k, e) => {
                    reads_i(e, &mut du);
                    updates.push((has(*k), IntExpr::constant(1)));
                    updates.push((val(*k), low_i(e)));
                }
                Upd::Remove(k) => {
                    updates.push((has(*k), IntExpr::constant(0)));
                    updates.push((val(*k), IntExpr::constant(0)));
                }
            }
        }
        let g = conj(act.guard.iter().map(low_b).collect());
        let mut guard_parts = vec![g.clone()];
        guard_parts.extend(defined(&dg));
        guard_parts.extend(defined(&du));
        let guard = conj(guard_parts);
        let name = name_of(naming, &act.name);
        let refs: Vec<(&str, IntExpr)> = updates
            .iter()
            .map(|(n, e)| (n.as_str(), e.clone()))
            .collect();
        b = b.action(ActionDecl::deterministic(&name, guard, refs));
        if !dg.is_empty() || !du.is_empty() {
            let dg = defined(&dg).unwrap_or(BoolExpr::constant(true));
            let du = defined(&du).unwrap_or(BoolExpr::constant(true));
            b = b.predicate(
                &guard_name(naming, &act.name),
                BoolExpr::and(dg, BoolExpr::implies(g, du)),
            );
        }
    }
    for (name, body) in invs {
        b = b.predicate(&name_of(naming, name), low_b(body));
        let mut reads = BTreeSet::new();
        reads_b(body, &mut reads);
        if let Some(d) = defined(&reads) {
            b = b.predicate(&guard_name(naming, name), d);
        }
    }
    for (base, depth, schema) in &spec.collisions {
        let name = collision_name(&name_of(naming, base), *depth, *schema);
        b = b.action(ActionDecl::deterministic(
            &name,
            BoolExpr::constant(false),
            vec![],
        ));
    }
    for m in &spec.metas {
        let suffix = if naming.detach_guards {
            "_defined"
        } else {
            "#defined"
        };
        let base = if spec.acts.iter().any(|a| a.name == m.base)
            || spec.invs.iter().any(|(n, _)| *n == m.base)
        {
            name_of(naming, &m.base)
        } else {
            m.base.clone()
        };
        b = b.predicate(&format!("{base}{}", suffix.repeat(m.depth)), low_b(&m.body));
    }
    for s in &spec.init {
        let mut bindings: Vec<(String, i64)> = vec![("x".to_owned(), s.x)];
        for k in 0..KEYS {
            bindings.push((has(k), i64::from(s.m[k].is_some())));
            bindings.push((val(k), s.m[k].unwrap_or(0)));
        }
        let refs: Vec<(&str, i64)> = bindings.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        b = b.initial_state(&refs);
    }
    b.build().expect("the lowered model is well formed")
}

/// The oracle state an engine state encodes.
fn decode(model: &Model, state: &State) -> OState {
    let get = |name: &str| model.binding(state, name).expect("declared");
    let mut m = [None; KEYS];
    for (k, slot) in m.iter_mut().enumerate() {
        if get(&has(k)) == 1 {
            *slot = Some(get(&val(k)));
        }
    }
    OState { x: get("x"), m }
}

/// The engine's canonical order on oracle states: the lowered vector's order.
fn key(model: &Model, s: &OState) -> Vec<i64> {
    model
        .variables()
        .iter()
        .map(|v| {
            let name = v.name().as_str();
            if name == "x" {
                return s.x;
            }
            let k: usize = name[2..].parse().unwrap();
            if name.starts_with("m?") {
                i64::from(s.m[k].is_some())
            } else {
                s.m[k].unwrap_or(0)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// the oracle's expectations
// ---------------------------------------------------------------------------

/// The expected outcome of one obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expect {
    /// `subject` is the action or predicate name; `read_is_action` its kind.
    Undefined {
        subject: String,
        action: bool,
        state: OState,
        depth: usize,
        count: usize,
    },
    Violated {
        state: OState,
        depth: usize,
        count: usize,
    },
    Holds,
}

/// Least depth, then least canonical key.
fn first_of(model: &Model, marked: &[(OState, usize)]) -> Option<(OState, usize)> {
    marked
        .iter()
        .min_by_key(|(s, d)| (*d, key(model, s)))
        .copied()
}

/// What an obligation over `space` should be. `subject` is a spec invariant name, a
/// spec action name (its `A#defined`), or an invariant's `I#defined`; `None` is the
/// actions alone (the deadlock and closure question).
fn expect(
    spec: &Spec,
    model: &Model,
    states: &[(OState, usize)],
    subject: Option<&Subject>,
) -> Expect {
    // 1. an undefined action read anywhere explored
    let bases = action_bases(spec);
    let bad: Vec<(OState, usize)> = states
        .iter()
        .filter(|(s, _)| bases.iter().any(|b| action_undefined(spec, b, s)))
        .copied()
        .collect();
    if let Some((state, depth)) = first_of(model, &bad) {
        let first = bases
            .iter()
            .find(|b| action_undefined(spec, b, &state))
            .unwrap();
        return Expect::Undefined {
            subject: first.clone(),
            action: true,
            state,
            depth,
            count: bad.len(),
        };
    }
    let Some(subject) = subject else {
        return Expect::Holds;
    };
    let (name, body, level) = match subject {
        // An action's chain is decided by step 1.
        Subject::ActionGuard => return Expect::Holds,
        Subject::Invariant(i) => (&spec.invs[*i].0, &spec.invs[*i].1, 1),
        Subject::InvariantGuard(i, level) => (&spec.invs[*i].0, &spec.invs[*i].1, *level),
    };
    // 2. an undefined read in the invariant: its reads (the chain member at depth 1),
    // or a nested guard, at the obligation's own depth or deeper
    let undefined: Vec<(OState, usize)> = states
        .iter()
        .filter(|(s, _)| {
            (level <= 1 && ev_b(body, s).is_none()) || meta_false(spec, name, level.max(2), s)
        })
        .copied()
        .collect();
    if let Some((state, depth)) = first_of(model, &undefined) {
        return Expect::Undefined {
            subject: name.clone(),
            action: false,
            state,
            depth,
            count: undefined.len(),
        };
    }
    if matches!(subject, Subject::InvariantGuard(..)) {
        return Expect::Holds;
    }
    // 3. a violation
    let violated: Vec<(OState, usize)> = states
        .iter()
        .filter(|(s, _)| ev_b(body, s) == Some(false))
        .copied()
        .collect();
    match first_of(model, &violated) {
        Some((state, depth)) => Expect::Violated {
            state,
            depth,
            count: violated.len(),
        },
        None => Expect::Holds,
    }
}

#[derive(Debug)]
enum Subject {
    Invariant(usize),
    /// An invariant's chain member, at this depth.
    InvariantGuard(usize, usize),
    ActionGuard,
}

/// A name's base and its chain depth, by this file's own stripping.
fn base_of(name: &str) -> (&str, usize) {
    let mut here = name;
    let mut depth = 0;
    while let Some(inner) = here.strip_suffix("#defined") {
        if inner.is_empty() {
            break;
        }
        here = inner;
        depth += 1;
    }
    (here, depth)
}

/// Which spec declaration a model predicate is.
fn subject_of(spec: &Spec, name: &str) -> Subject {
    let (base, depth) = base_of(name);
    if depth == 0 {
        return Subject::Invariant(
            spec.invs
                .iter()
                .position(|(n, _)| n == name)
                .expect("an invariant"),
        );
    }
    match spec.invs.iter().position(|(n, _)| n == base) {
        Some(i) if inv_chain_well_formed(spec, i) => Subject::InvariantGuard(i, depth),
        _ => Subject::ActionGuard,
    }
}

// ---------------------------------------------------------------------------
// comparison
// ---------------------------------------------------------------------------

fn same_undefined(model: &Model, got: &Undefined, want: &Expect, closed: bool, what: &str) {
    let Expect::Undefined {
        subject,
        action,
        state,
        depth,
        count,
    } = want
    else {
        panic!("{what}: engine says {got}, oracle says {want:?}");
    };
    assert_eq!(got.subject(), subject, "{what}: subject");
    assert_eq!(
        matches!(got.read(), Guarded::Action),
        *action,
        "{what}: kind"
    );
    assert_eq!(&decode(model, got.state()), state, "{what}: state");
    assert_eq!(got.depth(), *depth, "{what}: depth");
    assert_eq!(got.states(), *count, "{what}: count");
    match got.evidence().witness() {
        Some(path) => {
            assert!(closed, "{what}: a bounded exploration offers no path");
            assert_eq!(path.len(), *depth, "{what}: witness length");
            assert_eq!(path.target(), got.state(), "{what}: witness end");
        }
        None => assert!(!closed, "{what}: a closed exploration offers a path"),
    }
}

fn compare_outcome(model: &Model, got: &CheckOutcome, want: &Expect, closed: bool, what: &str) {
    match (got, want) {
        (CheckOutcome::Undefined(u), _) => same_undefined(model, u, want, closed, what),
        (
            CheckOutcome::Violated {
                state,
                depth,
                violations,
                evidence,
            },
            Expect::Violated {
                state: s,
                depth: d,
                count,
            },
        ) => {
            assert_eq!(&decode(model, state), s, "{what}: state");
            assert_eq!(depth, d, "{what}: depth");
            assert_eq!(violations, count, "{what}: count");
            if closed {
                let path = evidence.witness().expect("a closed run has a path");
                assert_eq!(path.len(), *d, "{what}: witness length");
                assert_eq!(path.target(), state, "{what}: witness end");
            }
        }
        (CheckOutcome::Holds { .. }, Expect::Holds) => assert!(closed, "{what}"),
        (CheckOutcome::Inconclusive(_), Expect::Holds) => assert!(!closed, "{what}"),
        _ => panic!("{what}: engine says {got}, oracle says {want:?}"),
    }
}

#[derive(Debug, Default)]
struct Tally {
    cases: usize,
    undefined_action: usize,
    undefined_invariant: usize,
    violated: usize,
    holds: usize,
    refused_emissions: usize,
    kernel_verified: usize,
    bounded: usize,
    bounded_undefined: usize,
    guard_emissions: usize,
    nested_decisive: usize,
    collision_decisive: usize,
}

fn envelope() -> ClaimEnvelope<'static> {
    ClaimEnvelope {
        model_digest: "blake3:definedness-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "blake3:definedness-property",
        scope_digest: "blake3:definedness-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    }
}

/// Run one case through every checking path and compare with the oracle.
fn one_case(seed: u64, tally: &mut Tally) {
    let spec = gen_spec(seed);
    let model = lower(&spec, &PLAIN);
    let space = oracle_explore(&spec);
    let what = |s: &str| format!("seed {seed}: {s}");

    // the reachable set and its depths
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let Exploration::Complete(reachable) = &exploration else {
        panic!("{}", what("the space closes"));
    };
    let got: BTreeMap<OState, usize> = reachable
        .states()
        .iter()
        .zip(reachable.depths())
        .map(|(s, d)| (decode(&model, s), *d))
        .collect();
    assert_eq!(got, space.depth, "{}", what("reachable set"));
    let states: Vec<(OState, usize)> = space.depth.iter().map(|(s, d)| (*s, *d)).collect();

    // checking, every predicate
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    )
    .expect("checks");
    for result in report.invariants() {
        let name = result.name().as_str();
        let subject = subject_of(&spec, name);
        let want = expect(&spec, &model, &states, Some(&subject));
        compare_outcome(&model, result.outcome(), &want, true, &what(name));
        if let Subject::Invariant(_) = subject {
            // Anti-vacuity for cr-pt5h3a: the outcome is undefined only because of a
            // nested or orphan guard, which a scan of `X#defined` alone would miss.
            if matches!(want, Expect::Undefined { .. }) {
                let bare = Spec {
                    metas: Vec::new(),
                    ..spec.clone()
                };
                if !matches!(
                    expect(&bare, &model, &states, Some(&subject)),
                    Expect::Undefined { .. }
                ) {
                    tally.nested_decisive += 1;
                }
                let apart = Spec {
                    collisions: Vec::new(),
                    ..spec.clone()
                };
                if matches!(want, Expect::Undefined { action: true, .. })
                    && !matches!(
                        expect(&apart, &model, &states, Some(&subject)),
                        Expect::Undefined { action: true, .. }
                    )
                {
                    tally.collision_decisive += 1;
                }
            }
            match &want {
                Expect::Undefined { action: true, .. } => tally.undefined_action += 1,
                Expect::Undefined { action: false, .. } => tally.undefined_invariant += 1,
                Expect::Violated { .. } => tally.violated += 1,
                Expect::Holds => tally.holds += 1,
            }
        }
    }

    // deadlock
    let want = expect(&spec, &model, &states, None);
    match (report.deadlock(), &want) {
        (DeadlockOutcome::Undefined(u), _) => {
            same_undefined(&model, u, &want, true, &what("deadlock"));
        }
        (DeadlockOutcome::Deadlocked { states: found }, Expect::Holds) => {
            let got: BTreeSet<OState> = found.iter().map(|d| decode(&model, d.state())).collect();
            let terminal: BTreeSet<OState> = space
                .fires
                .iter()
                .filter(|(_, fires)| !**fires)
                .map(|(s, _)| *s)
                .collect();
            assert_eq!(got, terminal, "{}", what("deadlocks"));
        }
        (DeadlockOutcome::Free { .. }, Expect::Holds) => {
            assert!(
                space.fires.values().all(|f| *f),
                "{}",
                what("deadlock free")
            );
        }
        (got, want) => panic!("{}: engine {got}, oracle {want:?}", what("deadlock")),
    }
    assert_eq!(
        report.undefined().is_some(),
        report
            .invariants()
            .iter()
            .any(|r| matches!(r.outcome(), CheckOutcome::Undefined(_)))
            || matches!(report.deadlock(), DeadlockOutcome::Undefined(_)),
        "{}",
        what("report.undefined")
    );

    // witnesses: `Fails(I)` and `Holds(I)` end only where I's reads are defined
    for (i, (name, body)) in spec.invs.iter().enumerate() {
        let index = model.predicate_index(name).unwrap();
        for (target, wanted) in [(Target::Fails(index), false), (Target::Holds(index), true)] {
            let hits: Vec<(OState, usize)> = states
                .iter()
                .filter(|(s, _)| ev_b(body, s) == Some(wanted) && !meta_false(&spec, name, 2, s))
                .copied()
                .collect();
            match (
                witness::shortest(&model, &exploration, &target),
                first_of(&model, &hits),
            ) {
                (Ok(path), Some((state, depth))) => {
                    assert_eq!(
                        decode(&model, path.target()),
                        state,
                        "{}",
                        what("witness end")
                    );
                    assert_eq!(path.len(), depth, "{}", what("witness length"));
                }
                (Err(NoWitness::Unreached { .. }), None) => {}
                (got, want) => panic!("{}: {got:?} vs {want:?} (inv {i})", what("witness")),
            }
        }
    }

    // emission: nothing is certified over an undefined read
    let closed = ClosedSet::of(&exploration).expect("closed");
    let env = envelope();
    let finite = certificate::emit_finite_closure(&model, closed, &env);
    match (&finite, &want) {
        (
            Err(EmissionError::Undefined { predicate, state }),
            Expect::Undefined {
                subject, state: s, ..
            },
        ) => {
            let (base, depth) = base_of(predicate);
            assert_eq!(base, subject, "{}", what("emit"));
            assert!(depth >= 1, "{}", what("emit"));
            assert_eq!(&decode(&model, state), s, "{}", what("emit state"));
            tally.refused_emissions += 1;
        }
        (Ok(bytes), Expect::Holds) => {
            assert!(matches!(
                check_certificate(bytes),
                KernelVerdict::Verified(_)
            ));
            tally.kernel_verified += 1;
        }
        (got, want) => panic!("{}: {got:?} vs {want:?}", what("finite closure")),
    }
    // an invariant closure of a definedness predicate: it is refused where the
    // predicate is false, and verified where it holds everywhere
    for predicate in model.predicates() {
        let name = predicate.name().as_str();
        if !name.ends_with("#defined") {
            continue;
        }
        let want = expect(&spec, &model, &states, Some(&subject_of(&spec, name)));
        match (
            certificate::emit_invariant_closure(&model, closed, &env, name),
            &want,
        ) {
            (
                Err(EmissionError::Undefined { predicate, state }),
                Expect::Undefined {
                    subject, state: s, ..
                },
            ) => {
                let (base, depth) = base_of(&predicate);
                assert_eq!(base, subject, "{}", what("emit guard"));
                assert!(depth >= 1, "{}", what("emit guard"));
                assert_eq!(&decode(&model, &state), s, "{}", what("emit guard state"));
                tally.guard_emissions += 1;
            }
            (Ok(bytes), Expect::Holds) => {
                assert!(
                    matches!(check_certificate(&bytes), KernelVerdict::Verified(_)),
                    "{}",
                    what("kernel verifies a guard that holds")
                );
                tally.guard_emissions += 1;
            }
            (got, want) => panic!("{}: {got:?} vs {want:?}", what("guard closure")),
        }
    }
    for (i, (name, _)) in spec.invs.iter().enumerate() {
        let want = expect(&spec, &model, &states, Some(&Subject::Invariant(i)));
        let emitted = certificate::emit_invariant_closure(&model, closed, &env, name);
        match (&emitted, &want) {
            (
                Err(EmissionError::Undefined { predicate, state }),
                Expect::Undefined {
                    subject, state: s, ..
                },
            ) => {
                let (base, depth) = base_of(predicate);
                assert_eq!(base, subject, "{}", what("emit inv"));
                assert!(depth >= 1, "{}", what("emit inv"));
                assert_eq!(&decode(&model, state), s, "{}", what("emit inv state"));
            }
            (Ok(bytes), Expect::Holds) => {
                assert!(
                    matches!(check_certificate(bytes), KernelVerdict::Verified(_)),
                    "{}",
                    what("kernel verifies a true invariant")
                );
            }
            (Ok(bytes), Expect::Violated { .. }) => {
                assert!(
                    matches!(check_certificate(bytes), KernelVerdict::Rejected(_)),
                    "{}",
                    what("kernel rejects a false invariant")
                );
            }
            (got, want) => panic!("{}: {got:?} vs {want:?}", what("invariant closure")),
        }

        // liveness: an undefined read is reported before any fair-cycle search
        let index = model.predicate_index(name).unwrap();
        let live = check_liveness(
            &model,
            &exploration,
            Goal::Eventually(index),
            Stuttering::Everywhere,
        )
        .expect("runs");
        match (&live, &want) {
            (LivenessOutcome::Undefined(u), Expect::Undefined { .. }) => {
                same_undefined(&model, u, &want, true, &what("liveness"));
            }
            (LivenessOutcome::Undefined(u), _) => panic!("{}: {u}", what("liveness")),
            (_, Expect::Undefined { .. }) => panic!("{}: {live:?}", what("liveness")),
            _ => {}
        }
    }

    // a bounded exploration: an undefined read found in the prefix is reported
    if space.depth.len() >= 3 {
        let bound = space.depth.len() / 2;
        if bound >= spec.init.len() {
            let bounded =
                bfs::explore(&model, Bounds::new(bound, 1 << 20, 1 << 20)).expect("explores");
            if let Exploration::Exhausted(_) = &bounded {
                tally.bounded += 1;
                let prefix = bounded.reachable();
                let prefix: Vec<(OState, usize)> = prefix
                    .states()
                    .iter()
                    .zip(prefix.depths())
                    .map(|(s, d)| (decode(&model, s), *d))
                    .collect();
                let report = checking::check(
                    &model,
                    &bounded,
                    &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
                )
                .expect("checks");
                for result in report.invariants() {
                    let name = result.name().as_str();
                    let subject = subject_of(&spec, name);
                    let want = expect(&spec, &model, &prefix, Some(&subject));
                    if matches!(want, Expect::Undefined { .. }) {
                        tally.bounded_undefined += 1;
                    }
                    compare_outcome(&model, result.outcome(), &want, false, &what(name));
                    // liveness over the same prefix: undefined, else inconclusive
                    if let Subject::Invariant(_) = subject {
                        let index = model.predicate_index(name).unwrap();
                        let live = check_liveness(
                            &model,
                            &bounded,
                            Goal::Recurrence(index),
                            Stuttering::Never,
                        )
                        .expect("runs");
                        match (&live, &want) {
                            (LivenessOutcome::Undefined(u), Expect::Undefined { .. }) => {
                                same_undefined(&model, u, &want, false, &what("bounded liveness"));
                            }
                            (
                                LivenessOutcome::Inconclusive(_),
                                Expect::Violated { .. } | Expect::Holds,
                            ) => {}
                            _ => panic!("{}: {live:?} vs {want:?}", what("bounded liveness")),
                        }
                    }
                }
                // the deadlock question over the prefix
                let want = expect(&spec, &model, &prefix, None);
                let terminal: BTreeSet<OState> = prefix
                    .iter()
                    .filter(|(s, _)| !space.fires[s])
                    .map(|(s, _)| *s)
                    .collect();
                match (report.deadlock(), &want) {
                    (DeadlockOutcome::Undefined(u), _) => {
                        same_undefined(&model, u, &want, false, &what("bounded deadlock"));
                    }
                    (DeadlockOutcome::Deadlocked { states: found }, Expect::Holds) => {
                        let got: BTreeSet<OState> =
                            found.iter().map(|d| decode(&model, d.state())).collect();
                        assert_eq!(got, terminal, "{}", what("bounded deadlocks"));
                    }
                    (DeadlockOutcome::Inconclusive(_), Expect::Holds) => {
                        assert!(terminal.is_empty(), "{}", what("bounded deadlock"));
                    }
                    (got, want) => panic!("{}: {got} vs {want:?}", what("bounded deadlock")),
                }
            }
        }
    }
    tally.cases += 1;
}

/// The corpus: 6000 seeded specifications through every checking path, against the
/// independent oracle. Anti-vacuity: every outcome class occurs many times.
#[test]
fn the_reference_engine_agrees_with_an_independent_partial_map_evaluator() {
    let mut tally = Tally::default();
    for seed in 0..6000 {
        one_case(0x2_4a5c_0000 + seed, &mut tally);
    }
    println!("{tally:?}");
    assert_eq!(tally.cases, 6000);
    assert!(tally.undefined_action >= 2000, "{tally:?}");
    assert!(tally.undefined_invariant >= 150, "{tally:?}");
    assert!(tally.violated >= 120, "{tally:?}");
    assert!(tally.holds >= 100, "{tally:?}");
    assert!(tally.refused_emissions >= 2000, "{tally:?}");
    assert!(tally.kernel_verified >= 250, "{tally:?}");
    assert!(tally.bounded >= 600, "{tally:?}");
    assert!(tally.bounded_undefined >= 1500, "{tally:?}");
    assert!(tally.guard_emissions >= 5000, "{tally:?}");
    assert!(tally.nested_decisive >= 100, "{tally:?}");
    assert!(tally.collision_decisive >= 40, "{tally:?}");
}

// ---------------------------------------------------------------------------
// metamorphic relations
// ---------------------------------------------------------------------------

/// The outcomes of one report with every name replaced by its position, so reports
/// over renamed models compare.
fn shape(model: &Model) -> Vec<String> {
    let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("explores");
    let report = checking::check(
        model,
        &exploration,
        &Obligations::every_predicate(model, DeadlockPolicy::Defect),
    )
    .expect("checks");
    let mut out: Vec<String> = report
        .invariants()
        .iter()
        .filter(|r| !r.name().as_str().ends_with("defined"))
        .map(|r| {
            let kind = match r.outcome() {
                CheckOutcome::Undefined(u) => undefined_shape(u),
                other => other.to_string(),
            };
            format!("{} {kind}", unname(r.name().as_str()))
        })
        .collect();
    out.sort();
    out.push(match report.deadlock() {
        DeadlockOutcome::Undefined(u) => format!("deadlock {}", undefined_shape(u)),
        other => other.verdict().to_string(),
    });
    out
}

/// An undefined read with its names mapped back. The action is left out: when several
/// actions are undefined at the reported state, the first in predicate (name) order is
/// named, so a renaming that reorders names may name another of them.
fn undefined_shape(u: &Undefined) -> String {
    let who = match u.read() {
        Guarded::Action => "action".to_owned(),
        Guarded::Predicate(_) => unname(u.subject()),
    };
    format!(
        "undefined {who} {} depth={} states={}",
        u.state(),
        u.depth(),
        u.states()
    )
}

/// Metamorphic relations "stable reordering of declarations" and "alpha-renaming"
/// (docs/19 §3): reversing the declaration order (which the builder canonicalises), and
/// renaming every action and invariant together with its `#defined` predicate so that
/// the name order reverses, preserve every outcome. The
/// non-preservation control: renaming only the definedness predicates (to `X_defined`)
/// detaches them, and every case with an undefined read changes its outcome.
#[test]
fn reordering_and_alpha_renaming_preserve_definedness_outcomes() {
    let mut detached_changed = 0;
    for seed in 0..200 {
        let spec = gen_spec(0x2_4a5c_1000 + seed);
        let base = shape(&lower(&spec, &PLAIN));
        let reordered = shape(&lower(
            &spec,
            &Naming {
                rename: false,
                detach_guards: false,
                reverse: true,
            },
        ));
        assert_eq!(
            base, reordered,
            "seed {seed}: stable reordering of declarations"
        );
        let renamed = shape(&lower(
            &spec,
            &Naming {
                rename: true,
                detach_guards: false,
                reverse: false,
            },
        ));
        assert_eq!(base, renamed, "seed {seed}: alpha-renaming");
        if base.iter().any(|line| line.contains("undefined")) {
            let detached = lower(
                &spec,
                &Naming {
                    rename: false,
                    detach_guards: true,
                    reverse: false,
                },
            );
            assert_ne!(base, shape(&detached), "seed {seed}: detached guards");
            detached_changed += 1;
        }
    }
    assert!(detached_changed >= 50, "{detached_changed}");
}

/// A definedness predicate whose subject names no declared predicate is read as an
/// action's (fail closed): its falsity dominates every obligation.
#[test]
fn an_orphan_definedness_predicate_is_read_as_an_action_read() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![("x", IntExpr::constant(1))],
        ))
        .predicate("Ok", BoolExpr::constant(true))
        .predicate(
            "Ghost#defined",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
        )
        .initial_state(&[("x", 0)])
        .build()
        .expect("builds");
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
    let ok = model.predicate_index("Ok").unwrap();
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(ok),
    )
    .expect("checks");
    let CheckOutcome::Undefined(u) = report.invariant(ok).unwrap().outcome() else {
        panic!("{report}");
    };
    assert_eq!(u.subject(), "Ghost");
    assert_eq!(u.read(), Guarded::Action);
    assert_eq!(u.depth(), 1);
    assert!(matches!(report.deadlock(), DeadlockOutcome::Undefined(_)));
    assert_eq!(report.verdict(), checking::Verdict::Inconclusive);
}

/// A hand-built model may give an action and a predicate one name, which CML's single
/// namespace forbids. A definedness predicate whose subject names both, or names a
/// schema whose instances are `X(…)`, is read as the action's (fail closed), never as
/// the predicate's.
#[test]
fn a_subject_that_names_an_action_and_a_predicate_is_read_as_the_action() {
    let x0 = || BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0));
    for (action, guard) in [("Inc", "Inc#defined"), ("Inc(n=a)", "Inc#defined")] {
        let model = ModelBuilder::new()
            .variable("x", 0, 1)
            .action(ActionDecl::deterministic(
                action,
                x0(),
                vec![("x", IntExpr::constant(1))],
            ))
            .predicate("Inc", BoolExpr::constant(true))
            .predicate(guard, x0())
            .initial_state(&[("x", 0)])
            .build()
            .expect("builds");
        let definedness = continuum_engine_reference::Definedness::of(&model);
        let g = model.predicate_index(guard).unwrap();
        assert_eq!(definedness.guards(g), Some(Guarded::Action), "{action}");
        assert_eq!(
            definedness.action_chains().collect::<Vec<_>>(),
            vec![&[g][..]]
        );
        assert_eq!(
            definedness.guards_of(model.predicate_index("Inc").unwrap()),
            &[g],
            "a witness to `Inc` respects the action chain it shares a name with"
        );
    }
}
