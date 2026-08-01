//! Die Hard, the Phase A corpus model, as a [`Model`].
//!
//! # Provenance
//!
//! A line-for-line transcription of
//! `notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`, whose header records
//! that its syntax is "normative intent, not yet parsed" (`DieHard.ctm:1`). Until the
//! CML front end exists, this module *is* the port: the corpus file states the model
//! and this file states the same model in the only vocabulary that can currently be
//! evaluated.
//!
//! | Corpus line | Here |
//! |---|---|
//! | `DieHard.ctm:4` `big: Nat where big <= 5` | `variable("big", 0, 5)` |
//! | `DieHard.ctm:5` `small: Nat where small <= 3` | `variable("small", 0, 3)` |
//! | `DieHard.ctm:8` `init Init { big == 0 && small == 0 }` | `initial_state(&[("big", 0), ("small", 0)])` |
//! | `DieHard.ctm:10` `action FillSmall { big' == big && small' == 3 }` | [`FILL_SMALL`] |
//! | `DieHard.ctm:11` `action FillBig { big' == 5 && small' == small }` | [`FILL_BIG`] |
//! | `DieHard.ctm:12` `action EmptySmall { big' == big && small' == 0 }` | [`EMPTY_SMALL`] |
//! | `DieHard.ctm:13` `action EmptyBig { big' == 0 && small' == small }` | [`EMPTY_BIG`] |
//! | `DieHard.ctm:15-19` `action SmallToBig { … }` | [`SMALL_TO_BIG`] |
//! | `DieHard.ctm:21-25` `action BigToSmall { … }` | [`BIG_TO_SMALL`] |
//! | `DieHard.ctm:30` `invariant TypeOK { big in 0..5 && small in 0..3 }` | [`TYPE_OK`] |
//! | `DieHard.ctm:31` `invariant NotSolved { big != 4 }` | [`NOT_SOLVED`] |
//!
//! Two lines are deliberately *not* transcribed. `DieHard.ctm:27`
//! (`action Next = FillSmall | … | BigToSmall`) is the disjunction of all six actions,
//! which is what [`Model::successors`] already computes; and `DieHard.ctm:29`
//! (`behavior Spec = Init && always(step(Next) || stutter(state))`) is a temporal
//! formula, which this layer does not represent — PR 8 is the finite *state space*,
//! not the behaviour language.
//!
//! `TypeOK` is transcribed even though it is a tautology over this model's declared
//! domains: the Lean port makes the same choice and says why — "The capacities are
//! *not* encoded in the types, so `TypeOK` below is a genuine invariant proved by a
//! certificate rather than a type fact" (`lean/Continuum/Examples/DieHard.lean:5-6`).
//! Here the capacities *are* the domains, so the interesting content moves into the
//! claim that every *reachable* state is well-typed, which is what a state-type
//! certificate carries.
//!
//! # Frozen facts
//!
//! The Revision 2 spike froze four numbers for this model, restated identically in
//! four places — `notes/plan/spikes/SPIKE_REPORT.md:9-12`,
//! `notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:24-27`,
//! `notes/plan/corpus/tla-examples/ports/TV-009/port.json:31-35`, and
//! `crates/continuum-kernel-core/src/fixture.rs:11-14`: **16** reachable states, **96**
//! labelled transitions, a shortest `big == 4` witness at depth **6**, and an
//! independently accepted certificate. The first three are asserted against this model
//! by `tests/diehard_evidence.rs`; the fourth is the certificate bone's.
//!
//! # Guards
//!
//! All six guards are `true`. That is not a simplification — the corpus actions carry
//! no `require` clause, and the frozen edge count depends on it: every action is
//! enabled in every state, so several successors are self-loops and 16 states carry
//! 6 transitions each, which is where 96 comes from
//! (`crates/continuum-kernel-core/src/fixture.rs:262-263`). An engine that "helpfully"
//! guarded `FillBig` on `big < 5` would produce a different, smaller transition
//! relation and disagree with every frozen artifact.
//!
//! # Action names
//!
//! The corpus spellings are kept (`FillSmall`, not `fill-small`). Under the wire
//! form's byte order they sort to
//! `BigToSmall < EmptyBig < EmptySmall < FillBig < FillSmall < SmallToBig`, which is
//! the same *relative* order — and therefore the same action indices 0…5 — as the
//! kebab-case names `continuum-kernel-core`'s test fixture uses
//! (`crates/continuum-kernel-core/src/fixture.rs:247-256`). The two agree on indices
//! while disagreeing on spelling, so a certificate emitted from this model is
//! row-comparable with the kernel's fixture without either side renaming anything.

use crate::expr::{BoolExpr, CmpOp, IntExpr};
use crate::model::{ActionDecl, Model, ModelBuilder, ModelError};

/// `DieHard.ctm:4` — the five-gallon jug.
pub const BIG: &str = "big";
/// `DieHard.ctm:5` — the three-gallon jug.
pub const SMALL: &str = "small";

/// `DieHard.ctm:11` — fill the big jug.
pub const FILL_BIG: &str = "FillBig";
/// `DieHard.ctm:10` — fill the small jug.
pub const FILL_SMALL: &str = "FillSmall";
/// `DieHard.ctm:13` — empty the big jug.
pub const EMPTY_BIG: &str = "EmptyBig";
/// `DieHard.ctm:12` — empty the small jug.
pub const EMPTY_SMALL: &str = "EmptySmall";
/// `DieHard.ctm:15-19` — pour the small jug into the big one.
pub const SMALL_TO_BIG: &str = "SmallToBig";
/// `DieHard.ctm:21-25` — pour the big jug into the small one.
pub const BIG_TO_SMALL: &str = "BigToSmall";

/// `DieHard.ctm:30` — `big in 0..5 && small in 0..3`.
pub const TYPE_OK: &str = "TypeOK";
/// `DieHard.ctm:31` — `big != 4`. Intentionally false somewhere; its shortest
/// counterexample is the film's solution.
pub const NOT_SOLVED: &str = "NotSolved";

/// The capacity of the big jug (`DieHard.ctm:4`).
pub const BIG_CAPACITY: i64 = 5;
/// The capacity of the small jug (`DieHard.ctm:5`).
pub const SMALL_CAPACITY: i64 = 3;

/// The number of gallons the puzzle asks for, which `NotSolved` denies is reachable.
pub const TARGET_GALLONS: i64 = 4;

/// The Die Hard transition model.
///
/// # Errors
///
/// [`ModelError`], only if this module's transcription is itself invalid — which
/// `tests/diehard_evidence.rs` exists to prevent. It is returned rather than unwrapped
/// so that no path in this crate turns a broken fixture into a panic; a bad
/// declaration is reportable data here like anywhere else.
pub fn model() -> Result<Model, ModelError> {
    let big = || IntExpr::var(BIG);
    let small = || IntExpr::var(SMALL);
    let always = || BoolExpr::Const(true);

    // `DieHard.ctm:16` — `let next_big = min(big + small, 5)`
    let next_big = || {
        IntExpr::min(
            IntExpr::plus(big(), small()),
            IntExpr::constant(BIG_CAPACITY),
        )
    };
    // `DieHard.ctm:22` — `let next_small = min(big + small, 3)`
    let next_small = || {
        IntExpr::min(
            IntExpr::plus(big(), small()),
            IntExpr::constant(SMALL_CAPACITY),
        )
    };

    ModelBuilder::new()
        .variable(BIG, 0, BIG_CAPACITY)
        .variable(SMALL, 0, SMALL_CAPACITY)
        .initial_state(&[(BIG, 0), (SMALL, 0)])
        // `small' == 3`. `big' == big` is the frame rule, so it is not written.
        .action(ActionDecl::deterministic(
            FILL_SMALL,
            always(),
            vec![(SMALL, IntExpr::constant(SMALL_CAPACITY))],
        ))
        // `big' == 5`
        .action(ActionDecl::deterministic(
            FILL_BIG,
            always(),
            vec![(BIG, IntExpr::constant(BIG_CAPACITY))],
        ))
        // `small' == 0`
        .action(ActionDecl::deterministic(
            EMPTY_SMALL,
            always(),
            vec![(SMALL, IntExpr::constant(0))],
        ))
        // `big' == 0`
        .action(ActionDecl::deterministic(
            EMPTY_BIG,
            always(),
            vec![(BIG, IntExpr::constant(0))],
        ))
        // `big' == next_big` and `small' == small - (next_big - big)`.
        .action(ActionDecl::deterministic(
            SMALL_TO_BIG,
            always(),
            vec![
                (BIG, next_big()),
                (
                    SMALL,
                    IntExpr::minus(small(), IntExpr::minus(next_big(), big())),
                ),
            ],
        ))
        // `small' == next_small` and `big' == big - (next_small - small)`.
        .action(ActionDecl::deterministic(
            BIG_TO_SMALL,
            always(),
            vec![
                (SMALL, next_small()),
                (
                    BIG,
                    IntExpr::minus(big(), IntExpr::minus(next_small(), small())),
                ),
            ],
        ))
        .predicate(
            TYPE_OK,
            BoolExpr::and(
                BoolExpr::in_range(big(), 0, BIG_CAPACITY),
                BoolExpr::in_range(small(), 0, SMALL_CAPACITY),
            ),
        )
        .predicate(
            NOT_SOLVED,
            BoolExpr::compare(CmpOp::Ne, big(), IntExpr::constant(TARGET_GALLONS)),
        )
        .build()
}
