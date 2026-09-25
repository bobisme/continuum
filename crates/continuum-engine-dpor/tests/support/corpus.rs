//! The C005 generated corpus: small random concurrent models, seeded and
//! deterministic.
//!
//! Shared by `tests/c005_differential.rs` and the crate's mutation campaign
//! (`src/mutation.rs`, which includes this file by path). It depends on
//! `continuum-model-core` alone, so it is the same generator in both places.
//!
//! # Shape
//!
//! Each model is two or three *processes* over one or two *shared* variables:
//!
//! - process `i` has a program counter `pc_i` in `0..=m_i` (`m_i` in 1..=3) and, half
//!   the time, a local `l_i` in `0..=2`;
//! - the process's step at program counter `p` is an action guarded by `pc_i == p`,
//!   sometimes also by a condition on a shared or local variable (so a step can be
//!   *disabled* by another process: the enabling relation the persistent-set closure
//!   must follow), and it advances `pc_i`; at the last program counter the process
//!   either stops (a terminal state, so deadlocks occur) or wraps to 0 (a cycle, so
//!   the proviso matters);
//! - a step often also writes a shared or local variable, with a constant, a copy of
//!   another variable, or a clamped increment; rarely an unclamped increment, which
//!   can leave its domain (an evaluation fault the reduction must not hide);
//! - a step is sometimes nondeterministic: two outcomes writing different values;
//! - sometimes an *environment* action with no program counter resets or bumps a
//!   shared variable;
//! - one to three invariants read a few variables each: a mutual-exclusion pair of
//!   program counters, a bound on a shared variable, or a relation between a local
//!   and a shared variable. The variables they read are the visible ones.
//!
//! Two families share that shape. A *tangled* model (two thirds of the seeds) guards
//! and writes shared variables often and may name program counters in an invariant.
//! A *loose* model (one third) gives every process a local, writes shared variables
//! and guards on them rarely, and keeps program counters out of its invariants: most
//! steps are invisible and independent, which is where a persistent set is smaller
//! than the enabled set.
//!
//! A fifth of the models (drawn from a separate stream, so the rest of the model is
//! unchanged) also declare definedness predicates (RFC 0003, bn-24a5c): an action's
//! chain `P0s…#defined`, sometimes nested one level, and an invariant's chain
//! `Inv0#defined`, sometimes nested. An action's predicate usually does not match the
//! action's guard, which a lowered model would conjoin: the corpus tests the reading,
//! not the lowering.
//!
//! The generator is splitmix64 over the seed: no ambient randomness (INV-005).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    dead_code
)]

use continuum_model_core::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_model_core::model::{ActionDecl, Model, ModelBuilder};

/// splitmix64.
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// True with probability `percent`/100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).unwrap()]
    }
}

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn int(value: i64) -> IntExpr {
    IntExpr::constant(value)
}

fn cmp(op: CmpOp, left: IntExpr, right: IntExpr) -> BoolExpr {
    BoolExpr::compare(op, left, right)
}

/// One generated model and the facts about its shape the evidence records.
pub struct Generated {
    pub seed: u64,
    pub loose: bool,
    pub model: Model,
    pub processes: usize,
    pub wraps: bool,
    pub nondeterministic: bool,
    pub environment: bool,
    pub unclamped: bool,
    /// How many definedness predicates the model declares.
    pub definedness: usize,
}

/// Generate the model for `seed`.
pub fn generate(seed: u64) -> Generated {
    let mut rng = Rng::new(seed ^ 0xC005_C005_C005_C005);
    let loose = rng.below(3) == 0;
    let (guard_percent, write_percent, local_percent) =
        if loose { (15, 70, 80) } else { (40, 65, 35) };
    let processes = 2 + usize::try_from(rng.below(2)).unwrap();
    let shared = 1 + usize::try_from(rng.below(2)).unwrap();
    let mut builder = ModelBuilder::new();
    let mut initial: Vec<(String, i64)> = Vec::new();
    let mut pcs: Vec<(String, i64)> = Vec::new();
    let mut locals: Vec<Option<String>> = Vec::new();
    let shared_names: Vec<String> = (0..shared).map(|j| format!("g{j}")).collect();
    for name in &shared_names {
        builder = builder.variable(name, 0, 2);
        initial.push((name.clone(), 0));
    }
    let mut wraps = false;
    let mut nondeterministic = false;
    let mut environment = false;
    let mut unclamped = false;

    for i in 0..processes {
        let top = 1 + i64::try_from(rng.below(3)).unwrap();
        let pc = format!("pc{i}");
        builder = builder.variable(&pc, 0, top);
        initial.push((pc.clone(), 0));
        pcs.push((pc.clone(), top));
        let local = if loose || rng.chance(50) {
            let name = format!("l{i}");
            builder = builder.variable(&name, 0, 2);
            initial.push((name.clone(), 0));
            Some(name)
        } else {
            None
        };
        locals.push(local.clone());

        let wrap = rng.chance(35);
        wraps |= wrap;
        let last = if wrap { top } else { top - 1 };
        for p in 0..=last {
            let next = if p == top { 0 } else { p + 1 };
            let mut guard = cmp(CmpOp::Eq, var(&pc), int(p));
            if rng.chance(guard_percent) {
                let target = if local.is_some() && rng.chance(30) {
                    local.clone().unwrap()
                } else {
                    rng.pick(&shared_names).clone()
                };
                let op = *rng.pick(&[CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Ge]);
                let bound = i64::try_from(rng.below(3)).unwrap();
                guard = BoolExpr::and(guard, cmp(op, var(&target), int(bound)));
            }
            let write = if rng.chance(write_percent) {
                let target = if local.is_some() && rng.chance(local_percent) {
                    local.clone().unwrap()
                } else {
                    rng.pick(&shared_names).clone()
                };
                Some(target)
            } else {
                None
            };
            let value = |rng: &mut Rng, target: &str, unclamped: &mut bool| -> IntExpr {
                match rng.below(10) {
                    0..=3 => int(i64::try_from(rng.below(3)).unwrap()),
                    4..=5 => {
                        let source = rng.pick(&shared_names).clone();
                        var(&source)
                    }
                    6..=8 => IntExpr::min(IntExpr::plus(var(target), int(1)), int(2)),
                    _ => {
                        if rng.chance(40) {
                            *unclamped = true;
                            IntExpr::plus(var(target), int(1))
                        } else {
                            IntExpr::max(IntExpr::minus(var(target), int(1)), int(0))
                        }
                    }
                }
            };
            let name = format!("P{i}s{p}");
            let decl = match &write {
                None => ActionDecl::deterministic(&name, guard, vec![(&pc, int(next))]),
                Some(target) => {
                    if rng.chance(20) {
                        nondeterministic = true;
                        let first = value(&mut rng, target, &mut unclamped);
                        let second = int(i64::try_from(rng.below(3)).unwrap());
                        ActionDecl::enumerated(
                            &name,
                            guard,
                            vec![
                                vec![(pc.as_str(), int(next)), (target.as_str(), first)],
                                vec![(pc.as_str(), int(next)), (target.as_str(), second)],
                            ],
                        )
                    } else {
                        let update = value(&mut rng, target, &mut unclamped);
                        ActionDecl::deterministic(
                            &name,
                            guard,
                            vec![(pc.as_str(), int(next)), (target.as_str(), update)],
                        )
                    }
                }
            };
            builder = builder.action(decl);
        }
    }

    if rng.chance(30) {
        environment = true;
        let target = rng.pick(&shared_names).clone();
        let trigger = i64::try_from(rng.below(3)).unwrap();
        let reset = i64::try_from(rng.below(3)).unwrap();
        builder = builder.action(ActionDecl::deterministic(
            "Env",
            cmp(CmpOp::Eq, var(&target), int(trigger)),
            vec![(target.as_str(), int(reset))],
        ));
    }

    let invariants = 1 + rng.below(3);
    for k in 0..invariants {
        let kind = if loose {
            1 + rng.below(2)
        } else {
            rng.below(3)
        };
        let body = match kind {
            0 => {
                let (a, top_a) = pcs[0].clone();
                let (b, top_b) = pcs[1].clone();
                let x = i64::try_from(rng.below(u64::try_from(top_a).unwrap() + 1)).unwrap();
                let y = i64::try_from(rng.below(u64::try_from(top_b).unwrap() + 1)).unwrap();
                BoolExpr::negate(BoolExpr::and(
                    cmp(CmpOp::Eq, var(&a), int(x)),
                    cmp(CmpOp::Eq, var(&b), int(y)),
                ))
            }
            1 => {
                let target = rng.pick(&shared_names).clone();
                cmp(
                    CmpOp::Le,
                    var(&target),
                    int(i64::try_from(rng.below(3)).unwrap()),
                )
            }
            _ => {
                let local = if loose {
                    locals
                        .get(usize::try_from(rng.below(locals.len() as u64)).unwrap())
                        .cloned()
                        .flatten()
                } else {
                    locals.iter().flatten().next().cloned()
                };
                let left = local.unwrap_or_else(|| pcs[0].0.clone());
                let right = rng.pick(&shared_names).clone();
                let total = i64::try_from(rng.below(5)).unwrap();
                cmp(
                    CmpOp::Ne,
                    IntExpr::plus(var(&left), var(&right)),
                    int(total),
                )
            }
        };
        builder = builder.predicate(&format!("Inv{k}"), body);
    }

    // Definedness predicates (RFC 0003, model-core's `Definedness`): drawn from a
    // separate stream so the models above are those of the corpus before bn-24a5c.
    let mut defined = Rng::new(seed ^ 0xDEF1_4ED0_DEF1_4ED0);
    let mut definedness = 0_usize;
    // A guard reads a process's local where one exists — written only by that
    // process's steps, which are independent of the others' and invisible to an
    // invariant over shared variables — so a reduction that hid the guard's reads could
    // postpone the step that makes it false (the `hidden-undefined` hazard).
    let local_names: Vec<String> = locals.iter().flatten().cloned().collect();
    let guard_over = |rng: &mut Rng| -> BoolExpr {
        let target = if !local_names.is_empty() && rng.chance(70) {
            rng.pick(&local_names).clone()
        } else {
            rng.pick(&shared_names).clone()
        };
        let op = *rng.pick(&[CmpOp::Ne, CmpOp::Lt, CmpOp::Le]);
        cmp(
            op,
            var(&target),
            int(1 + i64::try_from(rng.below(2)).unwrap()),
        )
    };
    if defined.chance(15) {
        // An action's chain: `P0s0#defined`, and sometimes `P0s0#defined#defined`.
        let action = format!(
            "P0s{}",
            defined.below(u64::try_from(pcs[0].1).unwrap().max(1))
        );
        builder = builder.predicate(&format!("{action}#defined"), guard_over(&mut defined));
        definedness += 1;
        if defined.chance(30) {
            builder = builder.predicate(
                &format!("{action}#defined#defined"),
                guard_over(&mut defined),
            );
            definedness += 1;
        }
    }
    if defined.chance(25) {
        // An invariant's chain, depth one or two.
        builder = builder.predicate("Inv0#defined", guard_over(&mut defined));
        definedness += 1;
        if defined.chance(40) {
            builder = builder.predicate("Inv0#defined#defined", guard_over(&mut defined));
            definedness += 1;
        }
    }

    let bindings: Vec<(&str, i64)> = initial.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    builder = builder.initial_state(&bindings);
    if rng.chance(20) {
        let alternative: Vec<(&str, i64)> = initial
            .iter()
            .map(|(n, v)| (n.as_str(), if n == "g0" { 1 } else { *v }))
            .collect();
        builder = builder.initial_state(&alternative);
    }

    Generated {
        seed,
        loose,
        model: builder.build().expect("a generated model is well formed"),
        processes,
        wraps,
        nondeterministic,
        environment,
        unclamped,
        definedness,
    }
}
