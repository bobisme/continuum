//! The seeded corpus generator.
//!
//! A corpus is a function of its seed range and nothing else (INV-005): the stream is
//! an explicit SplitMix64 on the seed, and no clock, entropy source, or hash-ordered
//! collection is read. Every generated fixture builds: domains are small, and every
//! update is clamped into its variable's domain with `min`/`max`, so no reachable
//! state faults. Guards, predicates, multi-outcome (nondeterministic) actions, several
//! initial states, and deadlocking states all occur across a corpus, so each compared
//! field has something to disagree about.
//!
//! Decision records: ADR-0003 (seeded, explicit randomness) and RFC 0013 ("Generated
//! finite universes").

use continuum_model_core::expr::{BoolExpr, CmpOp, IntExpr};

use super::fixture::{Fixture, FixtureAction};

/// SplitMix64 (Steele, Lea and Flood, OOPSLA 2014), written out so the stream is part
/// of this crate's source.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A stream seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..bound`; `0` when `bound` is `0`.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64().checked_rem(bound).unwrap_or(0)
    }

    fn small(&mut self, bound: u64) -> i64 {
        i64::try_from(self.below(bound)).unwrap_or(0)
    }

    fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.below(u64::try_from(len).unwrap_or(0))).unwrap_or(0)
    }
}

const NAMES: [&str; 3] = ["a", "b", "c"];

fn name(index: usize) -> &'static str {
    NAMES.get(index).copied().unwrap_or("a")
}

fn clamp(expr: IntExpr, lo: i64, hi: i64) -> IntExpr {
    IntExpr::min(
        IntExpr::max(expr, IntExpr::constant(lo)),
        IntExpr::constant(hi),
    )
}

fn comparison(rng: &mut SplitMix64, vars: &[(String, i64, i64)]) -> BoolExpr {
    let (var, lo, hi) = vars
        .get(rng.index(vars.len()))
        .cloned()
        .unwrap_or_else(|| ("a".to_owned(), 0, 1));
    let span = u64::try_from(hi.saturating_sub(lo).saturating_add(1)).unwrap_or(1);
    let value = lo.saturating_add(rng.small(span));
    let op = match rng.below(4) {
        0 => CmpOp::Lt,
        1 => CmpOp::Le,
        2 => CmpOp::Ne,
        _ => CmpOp::Ge,
    };
    BoolExpr::compare(op, IntExpr::var(&var), IntExpr::constant(value))
}

fn update(rng: &mut SplitMix64, vars: &[(String, i64, i64)]) -> Vec<(String, IntExpr)> {
    let mut out: Vec<(String, IntExpr)> = Vec::new();
    for (var, lo, hi) in vars {
        if rng.below(2) == 0 && !out.is_empty() {
            continue;
        }
        let value = match rng.below(4) {
            0 => IntExpr::plus(IntExpr::var(var), IntExpr::constant(1)),
            1 => IntExpr::minus(IntExpr::var(var), IntExpr::constant(1)),
            2 => {
                let other = vars
                    .get(rng.index(vars.len()))
                    .map_or_else(|| var.clone(), |(name, _, _)| name.clone());
                IntExpr::plus(IntExpr::var(var), IntExpr::var(&other))
            }
            _ => {
                let span = u64::try_from(hi.saturating_sub(*lo).saturating_add(1)).unwrap_or(1);
                IntExpr::constant(lo.saturating_add(rng.small(span)))
            }
        };
        out.push((var.clone(), clamp(value, *lo, *hi)));
    }
    out
}

/// The fixture for `seed`. Deterministic: one seed, one fixture, on every host.
#[must_use]
pub fn generate(seed: u64) -> Fixture {
    let mut rng = SplitMix64::new(seed);
    let count = usize::try_from(rng.below(3)).unwrap_or(0).saturating_add(1);
    let variables: Vec<(String, i64, i64)> = (0..count)
        .map(|index| (name(index).to_owned(), 0, rng.small(3).saturating_add(1)))
        .collect();

    let action_count = rng.below(4).saturating_add(1);
    let mut actions = Vec::new();
    for index in 0..action_count {
        let guard = match rng.below(3) {
            0 => BoolExpr::constant(true),
            1 => comparison(&mut rng, &variables),
            _ => BoolExpr::and(
                comparison(&mut rng, &variables),
                comparison(&mut rng, &variables),
            ),
        };
        let outcomes = if rng.below(4) == 0 {
            vec![update(&mut rng, &variables), update(&mut rng, &variables)]
        } else {
            vec![update(&mut rng, &variables)]
        };
        actions.push(FixtureAction {
            name: format!("act{index}"),
            guard,
            outcomes,
        });
    }

    let mut initial_states = Vec::new();
    for _ in 0..rng.below(2).saturating_add(1) {
        let state: Vec<(String, i64)> = variables
            .iter()
            .map(|(var, lo, hi)| {
                let span = u64::try_from(hi.saturating_sub(*lo).saturating_add(1)).unwrap_or(1);
                (var.clone(), lo.saturating_add(rng.small(span)))
            })
            .collect();
        // The builder refuses a repeated initial state; a repeat adds nothing.
        if !initial_states.contains(&state) {
            initial_states.push(state);
        }
    }

    let predicates = (0..rng.below(2).saturating_add(1))
        .map(|index| {
            let body = if rng.below(2) == 0 {
                comparison(&mut rng, &variables)
            } else {
                BoolExpr::or(
                    comparison(&mut rng, &variables),
                    comparison(&mut rng, &variables),
                )
            };
            (format!("inv{index}"), body)
        })
        .collect();

    Fixture {
        label: format!("gen-{seed}"),
        variables,
        actions,
        initial_states,
        predicates,
        fairness: Vec::new(),
    }
}

/// A fixture from [`generate`] with definedness predicates added (RFC 0003,
/// "Definedness"; `continuum_model_core::definedness`): `act0#defined` guards the first
/// action's reads (and is conjoined to its guard, as the lowering does),
/// `inv0#defined` the first invariant's, or both, by seed. Each is a
/// random comparison, so across seeds some reached states make it false and some do
/// not. The corpus needs these for the undefined-read category to be compared at all.
#[must_use]
pub fn generate_with_definedness(seed: u64) -> Fixture {
    let mut fixture = generate(seed);
    let mut rng = SplitMix64::new(seed ^ 0x5eed_def1_0ed0_0001);
    fixture.label = format!("def-{seed}");
    let which = seed.checked_rem(3).unwrap_or(0);
    let action = fixture.actions.first().map(|a| a.name.clone());
    let invariant = fixture.predicates.first().map(|(name, _)| name.clone());
    if which != 1 {
        if let Some(action) = action {
            let body = comparison(&mut rng, &fixture.variables);
            // As the lowering does: the action's guard is conjoined with its
            // definedness, so no successor is computed from an undefined read, and the
            // action looks disabled exactly where its read is undefined.
            if let Some(first) = fixture.actions.first_mut() {
                first.guard = BoolExpr::and(body.clone(), first.guard.clone());
            }
            fixture.predicates.push((format!("{action}#defined"), body));
        }
    }
    if which != 0 {
        if let Some(invariant) = invariant {
            let body = comparison(&mut rng, &fixture.variables);
            fixture
                .predicates
                .push((format!("{invariant}#defined"), body));
        }
    }
    fixture
}
