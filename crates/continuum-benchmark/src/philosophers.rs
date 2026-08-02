//! Dining Philosophers, the second Phase A corpus model, as a [`Model`].
//!
//! # Why this model is declared here and not in the engine
//!
//! `continuum-engine-reference` declares Die Hard because PR 8's scope was TV-009 and
//! because "until the CML front end exists, this module *is* the port"
//! (`crates/continuum-engine-reference/src/diehard.rs`). TV-007 is a **Wave 1** corpus port
//! (`notes/plan/corpus/tla-examples/PORTING_WAVES.md`), and no PR before this one needed it.
//! PR 10 does: START_HERE's PR 10 stem is "compare against a shell-scraping baseline on Die
//! Hard **and Dining Philosophers** tasks", and plan §21's Phase A exit names the same two.
//!
//! So the model is declared the way `diehard` is declared — inert data in
//! `continuum-engine-reference`'s own vocabulary, no closures, no new engine — and it is
//! declared in the crate that needs it rather than pushed into the engine, because the
//! engine is the *evaluator* and this is a *benchmark task*. Moving it into
//! `continuum-engine-reference` later is a file move and no semantic change; inventing an
//! engine feature for it would have been neither.
//!
//! # Provenance
//!
//! A transcription of `notes/plan/spikes/models.py`'s `DiningPhilosophers`, the Revision-2
//! executable spike whose numbers are the frozen facts below, cross-read against the corpus
//! port `notes/plan/corpus/tla-examples/ports/TV-007/DiningPhilosophers.ctm` (whose header
//! records, like TV-009's, that its syntax is design intent and not yet parsed).
//!
//! | Spike | Here |
//! |---|---|
//! | `phase: tuple[Phase, ...]`, `Phase` in `0..3` | `phase_<i>` over `0..=3` |
//! | `fork_owner: tuple[int, ...]`, `-1` means free | `fork_<f>` over `-1..=n-1` |
//! | `BecomeHungry(i)` | [`become_hungry`] |
//! | `TakeLeft(i)` | [`take_left`] |
//! | `TakeRight(i)` | [`take_right`] |
//! | `Release(i)` | [`release`] |
//! | `type_ok` | [`FORK_OWNERSHIP`] |
//!
//! The acquisition order is left-then-right for every philosopher — the deliberately
//! deadlock-prone protocol the spike names in its own docstring — and the deadlock is the
//! point: every philosopher holding its left fork and waiting for its right is a state with
//! no enabled action.
//!
//! # Frozen facts
//!
//! `notes/plan/spikes/SPIKE_REPORT.md:16-22`, `notes/plan/spikes/results/spike-results.json`
//! (`dining_philosophers_5`), `notes/plan/corpus/tla-examples/ports/TV-007/README.md`, and
//! `notes/plan/archive/README-revision-2.md:54` state the same three numbers for `N = 5`:
//! **573** reachable states, **2,365** labelled transitions, shortest deadlock depth **10**.
//! `tests/philosophers_evidence.rs` asserts all three against this model, plus the exact
//! deadlock state the spike recorded — five philosophers in `HAS_LEFT`, fork `f` owned by
//! philosopher `f`.
//!
//! # The invariant, and why it is written as a static conjunction
//!
//! The spike's `type_ok` indexes `phase` by a *fork owner read out of the state*, which this
//! expression language deliberately cannot do: it has no indexing, no quantifier, and no
//! recursion (`continuum_engine_reference::expr`, "the cost is expressiveness, and it is
//! paid deliberately"). The same predicate is therefore written as the finite conjunction it
//! denotes — one clause per `(philosopher, fork)` pair, all `n²` of them decided at
//! declaration time, which is exactly what a quantifier over a finite domain unfolds to.
//! [`conjoin`] folds the clauses into a *balanced* tree so the result stays inside
//! [`MAX_EXPR_DEPTH`](continuum_engine_reference::expr::MAX_EXPR_DEPTH) rather than nesting
//! once per clause.
//!
//! `ForkOwnership` is established on the reachable set, and the deadlock is refuted: one
//! model, two different questions, which is precisely what the spike report says this corpus
//! entry exists to show ("the reference kernel can distinguish ordinary invariant closure
//! from deadlock reachability").

use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder, ModelError};

/// The Phase A instance: five philosophers, five forks.
pub const N: usize = 5;

/// `notes/plan/spikes/SPIKE_REPORT.md:18` — reachable states at `N = 5`.
pub const FROZEN_STATES: u64 = 573;
/// `notes/plan/spikes/SPIKE_REPORT.md:19` — labelled transitions at `N = 5`.
pub const FROZEN_TRANSITIONS: u64 = 2_365;
/// `notes/plan/spikes/SPIKE_REPORT.md:20` — shortest deadlock depth at `N = 5`.
pub const FROZEN_DEADLOCK_DEPTH: usize = 10;

/// `Phase.THINKING`.
pub const THINKING: i64 = 0;
/// `Phase.WANTS`.
pub const WANTS: i64 = 1;
/// `Phase.HAS_LEFT`.
pub const HAS_LEFT: i64 = 2;
/// `Phase.EATING`.
pub const EATING: i64 = 3;
/// The spike's `-1`: nobody holds this fork.
pub const FREE: i64 = -1;

/// The declared name of philosopher `i`'s phase variable.
#[must_use]
pub fn phase_var(index: usize) -> String {
    format!("phase_{index}")
}

/// The declared name of fork `index`'s owner variable.
#[must_use]
pub fn fork_var(index: usize) -> String {
    format!("fork_{index}")
}

/// The fork philosopher `i` picks up first.
#[must_use]
pub const fn left(index: usize) -> usize {
    index
}

/// The fork philosopher `i` picks up second.
#[must_use]
pub const fn right(index: usize, n: usize) -> usize {
    (index + 1) % n
}

/// `phase_<i> == value`.
fn phase_is(index: usize, value: i64) -> BoolExpr {
    BoolExpr::compare(
        CmpOp::Eq,
        IntExpr::var(&phase_var(index)),
        IntExpr::Const(value),
    )
}

/// `fork_<f> == owner`.
fn fork_is(fork: usize, owner: i64) -> BoolExpr {
    BoolExpr::compare(
        CmpOp::Eq,
        IntExpr::var(&fork_var(fork)),
        IntExpr::Const(owner),
    )
}

/// Fold `clauses` into a balanced conjunction.
///
/// Balanced rather than right-nested because the expression language declares a depth bound
/// and enforces it at the builder: `n²` clauses folded linearly would nest `n²` deep, and at
/// `N = 5` that is 25 — inside the bound today and outside it at `N = 6`. A balanced fold
/// makes the depth logarithmic in the clause count, so the same declaration is legal for
/// every `N` the variable budget admits.
///
/// The empty conjunction is `true`, which is the identity the fold needs and also the honest
/// reading: a predicate with no clauses forbids nothing.
#[must_use]
pub fn conjoin(mut clauses: Vec<BoolExpr>) -> BoolExpr {
    if clauses.is_empty() {
        return BoolExpr::constant(true);
    }
    while clauses.len() > 1 {
        let mut folded = Vec::with_capacity(clauses.len().div_ceil(2));
        let mut rest = clauses.into_iter();
        while let Some(left) = rest.next() {
            match rest.next() {
                Some(right) => folded.push(BoolExpr::and(left, right)),
                None => folded.push(left),
            }
        }
        clauses = folded;
    }
    clauses.pop().expect("a non-empty clause list folds to one")
}

/// `BecomeHungry(i)`: a thinking philosopher decides to eat.
#[must_use]
pub fn become_hungry(index: usize) -> ActionDecl {
    let phase = phase_var(index);
    ActionDecl::deterministic(
        &format!("BecomeHungry({index})"),
        phase_is(index, THINKING),
        vec![(&phase, IntExpr::Const(WANTS))],
    )
}

/// `TakeLeft(i)`: a hungry philosopher takes the fork on its left, when it is free.
#[must_use]
pub fn take_left(index: usize) -> ActionDecl {
    let phase = phase_var(index);
    let fork = fork_var(left(index));
    ActionDecl::deterministic(
        &format!("TakeLeft({index})"),
        BoolExpr::and(phase_is(index, WANTS), fork_is(left(index), FREE)),
        vec![
            (&fork, IntExpr::Const(index as i64)),
            (&phase, IntExpr::Const(HAS_LEFT)),
        ],
    )
}

/// `TakeRight(i)`: a philosopher holding its left fork takes its right, when it is free.
#[must_use]
pub fn take_right(index: usize, n: usize) -> ActionDecl {
    let phase = phase_var(index);
    let fork = fork_var(right(index, n));
    ActionDecl::deterministic(
        &format!("TakeRight({index})"),
        BoolExpr::and(phase_is(index, HAS_LEFT), fork_is(right(index, n), FREE)),
        vec![
            (&fork, IntExpr::Const(index as i64)),
            (&phase, IntExpr::Const(EATING)),
        ],
    )
}

/// `Release(i)`: an eating philosopher puts both forks down and returns to thinking.
#[must_use]
pub fn release(index: usize, n: usize) -> ActionDecl {
    let phase = phase_var(index);
    let left_fork = fork_var(left(index));
    let right_fork = fork_var(right(index, n));
    ActionDecl::deterministic(
        &format!("Release({index})"),
        phase_is(index, EATING),
        vec![
            (&left_fork, IntExpr::Const(FREE)),
            (&right_fork, IntExpr::Const(FREE)),
            (&phase, IntExpr::Const(THINKING)),
        ],
    )
}

/// The declared name of the ownership invariant.
pub const FORK_OWNERSHIP: &str = "ForkOwnership";

/// The spike's `type_ok`, unfolded over the finite `(philosopher, fork)` grid.
///
/// Three clause shapes, and the grid decides which applies to each pair:
///
/// - a fork that is neither of philosopher `p`'s two forks is never owned by `p`;
/// - `p`'s **left** fork owned by `p` means `p` has at least taken it (`HAS_LEFT` or
///   `EATING`);
/// - `p`'s **right** fork owned by `p` means `p` took it, which only `EATING` follows.
///
/// Together these are exactly what the spike's loop checks, with the dynamic index resolved
/// at declaration time.
#[must_use]
pub fn fork_ownership(n: usize) -> BoolExpr {
    let mut clauses = Vec::new();
    for owner in 0..n {
        for fork in 0..n {
            let held = fork_is(fork, owner as i64);
            if fork == left(owner) {
                clauses.push(BoolExpr::implies(
                    held,
                    BoolExpr::compare(
                        CmpOp::Ge,
                        IntExpr::var(&phase_var(owner)),
                        IntExpr::Const(HAS_LEFT),
                    ),
                ));
            } else if fork == right(owner, n) {
                clauses.push(BoolExpr::implies(held, phase_is(owner, EATING)));
            } else {
                clauses.push(BoolExpr::negate(held));
            }
        }
    }
    conjoin(clauses)
}

/// The Phase A model: [`N`] philosophers.
///
/// # Errors
///
/// [`ModelError`] when the declaration is rejected — a name, a domain, a limit, or an
/// expression depth. It cannot happen for [`N`] and is returned rather than unwrapped so
/// that `philosophers(n)` stays honest at every `n`.
pub fn model() -> Result<Model, ModelError> {
    philosophers(N)
}

/// Dining Philosophers with `n` philosophers.
///
/// # Errors
///
/// [`ModelError`] when the declaration is rejected. `n < 2` produces a model whose left and
/// right forks coincide, which is a different problem; the corpus port's own constraint is
/// `N >= 2` (`DiningPhilosophers.ctm:2`) and callers below it get
/// [`ModelError`] from the builder rather than a silently different model.
pub fn philosophers(n: usize) -> Result<Model, ModelError> {
    let mut builder = ModelBuilder::new();
    for index in 0..n {
        builder = builder.variable(&phase_var(index), THINKING, EATING);
    }
    for index in 0..n {
        builder = builder.variable(&fork_var(index), FREE, (n as i64) - 1);
    }
    for index in 0..n {
        builder = builder
            .action(become_hungry(index))
            .action(take_left(index))
            .action(take_right(index, n))
            .action(release(index, n));
    }
    let initial: Vec<(String, i64)> = (0..n)
        .map(|index| (phase_var(index), THINKING))
        .chain((0..n).map(|index| (fork_var(index), FREE)))
        .collect();
    let initial: Vec<(&str, i64)> = initial
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    builder
        .initial_state(&initial)
        .predicate(FORK_OWNERSHIP, fork_ownership(n))
        .build()
}
