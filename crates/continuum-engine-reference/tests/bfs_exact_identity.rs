//! C006 evidence: the exact finite explorer has no fingerprint unsoundness
//! (`notes/plan/docs/18_CLAIMS_MATRIX.md`, row C006; ADR-0013).
//!
//! Required evidence is "exact equality and collision injection". This file carries
//! both halves against the real explorer, [`bfs::explore`]:
//!
//! 1. **Exact equality.** The visited map is keyed by the full [`State`] value and
//!    compared with its derived `Ord`, which is lexicographic over the whole state
//!    vector. [`state_identity_is_the_full_state_vector_by_source`] pins that in the
//!    source, so a later edit that swaps in a fingerprint key fails here.
//! 2. **Collision injection.** A fingerprint is injected through a test-only
//!    [`Hasher`] fed by `State`'s own derived `Hash`: [`CollideAll`] maps every state
//!    to one value, and [`Fnv1aBits`] truncates FNV-1a to a few bits so that the
//!    pigeonhole principle forces collisions. The real explorer must keep every
//!    colliding state, and its reachable-state count must equal an exact count that
//!    this file computes independently (the Cartesian product of the domains).
//! 3. **Anti-vacuity.** A fingerprint-only deduplication mutant ([`walk`] keyed by a
//!    fingerprint) must lose states under the same injected collisions, and the same
//!    exactness check that accepts the real explorer must reject the mutant. An exact
//!    twin of the harness (keyed by `State`) must agree with the real explorer, so a
//!    lost state is caused by the fingerprint and not by the harness.
//!
//! The mutant was also applied once to the real explorer (bn-286s). In `src/bfs.rs`
//! the dedup test `!visited.contains_key(step.target()) && ...` was replaced by a
//! component-sum fingerprint comparison over the visited and discovered keys. Seven
//! of the eleven tests here failed: every real-explorer exactness test, the
//! exact-twin agreement, the bucketed-agreement test, and the source guard. The four
//! that passed exercise only the harness mutant or the initial-state path, which that
//! edit does not touch.
//!
//! The ADR-0013 design that remains legal — a hash *indexes* a bucket and collisions
//! resolve by exact comparison — is exercised too ([`walk_bucketed`]), and it is
//! immune to the total collision.
//!
//! Scope: `continuum-engine-reference`'s `bfs::explore`. The optimized engine
//! (`continuum-engine-explicit`, "state hashing" in its crate header) is a scaffold
//! today, so nothing here covers it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::hash::{Hash, Hasher};

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder, State};

// ---------------------------------------------------------------------------
// the injected fingerprints
// ---------------------------------------------------------------------------

/// A hasher that ignores its input: every state has fingerprint `0`.
///
/// This is total collision injection. Any deduplication that trusts this value alone
/// collapses the reachable set to one state per BFS layer's first discovery.
#[derive(Default)]
struct CollideAll;

impl Hasher for CollideAll {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _bytes: &[u8]) {}
}

/// FNV-1a over the bytes `State`'s derived `Hash` writes, truncated to `BITS` bits.
///
/// Deterministic (no seed, no platform input). With more than `2^BITS` reachable
/// states the pigeonhole principle guarantees a collision, so the test does not rely
/// on a lucky pair.
struct Fnv1aBits<const BITS: u32>(u64);

impl<const BITS: u32> Default for Fnv1aBits<BITS> {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl<const BITS: u32> Hasher for Fnv1aBits<BITS> {
    fn finish(&self) -> u64 {
        self.0 & ((1_u64 << BITS) - 1)
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// `state`'s fingerprint under hasher `H`, through `State`'s own `Hash` impl.
fn fingerprint<H: Hasher + Default>(state: &State) -> u64 {
    let mut hasher = H::default();
    state.hash(&mut hasher);
    hasher.finish()
}

/// A structural fingerprint that collides by design: the sum of the components.
/// `(1, 0)` and `(0, 1)` share it.
fn component_sum(state: &State) -> u64 {
    state.as_slice().iter().fold(0_u64, |acc, v| {
        acc.wrapping_add(u64::from_le_bytes(v.to_le_bytes()))
    })
}

// ---------------------------------------------------------------------------
// the models
// ---------------------------------------------------------------------------

/// Two counters `x, y` in `[0, n]`, each incremented by its own guarded action.
///
/// Every point of the grid is reachable, so the exact reachable count is `(n + 1)^2`,
/// and states on one anti-diagonal share [`component_sum`].
fn grid(n: i64) -> Model {
    let inc = |name: &str, var: &str| {
        ActionDecl::deterministic(
            name,
            BoolExpr::compare(CmpOp::Lt, IntExpr::var(var), IntExpr::constant(n)),
            vec![(var, IntExpr::plus(IntExpr::var(var), IntExpr::constant(1)))],
        )
    };
    ModelBuilder::new()
        .variable("x", 0, n)
        .variable("y", 0, n)
        .initial_state(&[("x", 0), ("y", 0)])
        .action(inc("IncX", "x"))
        .action(inc("IncY", "y"))
        .build()
        .expect("the grid is a valid model")
}

/// Two initial states that collide under [`component_sum`] and [`CollideAll`], each
/// a self-loop. Exact count: 2.
fn colliding_starts() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("y", 0, 1)
        .initial_state(&[("x", 1), ("y", 0)])
        .initial_state(&[("x", 0), ("y", 1)])
        .action(ActionDecl::deterministic(
            "Stay",
            BoolExpr::Const(true),
            vec![],
        ))
        .build()
        .expect("the colliding-starts model is a valid model")
}

/// The exact grid state set, computed without the model: the Cartesian product.
fn grid_exact(n: i64) -> BTreeSet<Vec<i64>> {
    let mut out = BTreeSet::new();
    for x in 0..=n {
        for y in 0..=n {
            out.insert(vec![x, y]);
        }
    }
    out
}

fn explore_closed(model: &Model) -> Vec<State> {
    match bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates everywhere") {
        Exploration::Complete(reachable) => reachable.states().to_vec(),
        Exploration::Exhausted(_) => panic!("the model is far inside the certifiable bounds"),
    }
}

// ---------------------------------------------------------------------------
// the harness: the explorer's walk, with a pluggable dedup key
// ---------------------------------------------------------------------------

/// Breadth-first walk over the real model primitives, deduplicating by `key`.
///
/// With `key = State::clone` this is an exact twin of `bfs::explore`. With a
/// fingerprint key it is the fingerprint-only dedup mutant: two states with equal
/// keys are treated as one, and the second is never explored.
fn walk<K: Ord>(model: &Model, key: impl Fn(&State) -> K) -> Vec<State> {
    let mut seen: BTreeSet<K> = BTreeSet::new();
    let mut kept: Vec<State> = Vec::new();
    let mut queue: VecDeque<State> = VecDeque::new();
    for state in model.initial_states() {
        if seen.insert(key(state)) {
            kept.push(state.clone());
            queue.push_back(state.clone());
        }
    }
    while let Some(here) = queue.pop_front() {
        for step in model
            .successors(&here)
            .expect("the model evaluates everywhere")
        {
            if seen.insert(key(step.target())) {
                kept.push(step.target().clone());
                queue.push_back(step.target().clone());
            }
        }
    }
    kept.sort();
    kept
}

/// The ADR-0013 design: a fingerprint indexes a bucket, and membership in the bucket
/// is decided by exact comparison. Collisions cost time, never states.
fn walk_bucketed(model: &Model, index: impl Fn(&State) -> u64) -> Vec<State> {
    let mut buckets: BTreeMap<u64, Vec<State>> = BTreeMap::new();
    let mut queue: VecDeque<State> = VecDeque::new();
    let mut admit = |state: &State, queue: &mut VecDeque<State>| {
        let bucket = buckets.entry(index(state)).or_default();
        if !bucket.iter().any(|member| member == state) {
            bucket.push(state.clone());
            queue.push_back(state.clone());
        }
    };
    for state in model.initial_states() {
        admit(state, &mut queue);
    }
    while let Some(here) = queue.pop_front() {
        for step in model
            .successors(&here)
            .expect("the model evaluates everywhere")
        {
            admit(step.target(), &mut queue);
        }
    }
    let mut kept: Vec<State> = buckets.into_values().flatten().collect();
    kept.sort();
    kept
}

/// The exactness oracle. `Ok` only when `explored` is exactly `expected`: same
/// count, no duplicate, no missing state, no extra state.
fn check_exact(explored: &[State], expected: &BTreeSet<Vec<i64>>) -> Result<(), String> {
    let got: BTreeSet<Vec<i64>> = explored.iter().map(|s| s.as_slice().to_vec()).collect();
    if got.len() != explored.len() {
        return Err(format!("{} states, {} distinct", explored.len(), got.len()));
    }
    if &got != expected {
        let missing: Vec<_> = expected.difference(&got).collect();
        let extra: Vec<_> = got.difference(expected).collect();
        return Err(format!(
            "{} states, exact count {}; missing {missing:?}, extra {extra:?}",
            got.len(),
            expected.len()
        ));
    }
    Ok(())
}

fn vec_of(states: &[State]) -> Vec<Vec<i64>> {
    states.iter().map(|s| s.as_slice().to_vec()).collect()
}

// ---------------------------------------------------------------------------
// exact equality: the real explorer under injected collisions
// ---------------------------------------------------------------------------

/// Two distinct reachable states that collide under every injected fingerprint are
/// both explored, and the count is the exact grid count.
#[test]
fn explorer_keeps_distinct_states_whose_fingerprints_collide() {
    let model = grid(3);
    let a = model.state(&[1, 0]).expect("(1, 0) is in the domain");
    let b = model.state(&[0, 1]).expect("(0, 1) is in the domain");

    // Preconditions: the pair is distinct, and the injection really collides it.
    assert_ne!(a, b);
    assert_eq!(fingerprint::<CollideAll>(&a), fingerprint::<CollideAll>(&b));
    assert_eq!(component_sum(&a), component_sum(&b));

    let explored = explore_closed(&model);
    assert!(explored.contains(&a), "(1, 0) must be explored");
    assert!(explored.contains(&b), "(0, 1) must be explored");
    assert_eq!(explored.len(), 16, "(3 + 1)^2 grid points");
    check_exact(&explored, &grid_exact(3)).expect("the real explorer is exact");
}

/// Total collision: every reachable state has the same injected fingerprint, and the
/// real explorer still returns the exact set.
#[test]
fn explorer_is_exact_when_every_state_collides() {
    for n in [1, 2, 4, 7] {
        let model = grid(n);
        let explored = explore_closed(&model);
        let prints: BTreeSet<u64> = explored.iter().map(fingerprint::<CollideAll>).collect();
        assert_eq!(
            prints.len(),
            1,
            "precondition: the injection collides every state"
        );
        check_exact(&explored, &grid_exact(n))
            .unwrap_or_else(|e| panic!("grid({n}) under total collision: {e}"));
    }
}

/// Pigeonhole collision: 49 states and a 4-bit fingerprint (16 values), so at least
/// 33 states share a fingerprint with another. The real explorer is still exact.
#[test]
fn explorer_is_exact_under_pigeonhole_collisions() {
    let model = grid(6);
    let explored = explore_closed(&model);
    let prints: BTreeSet<u64> = explored.iter().map(fingerprint::<Fnv1aBits<4>>).collect();
    assert!(prints.len() <= 16);
    assert!(
        explored.len() > prints.len(),
        "precondition: collisions exist"
    );
    check_exact(&explored, &grid_exact(6)).expect("the real explorer is exact");
}

/// Colliding *initial* states: both are kept at depth 0.
#[test]
fn explorer_keeps_colliding_initial_states() {
    let model = colliding_starts();
    let [a, b] = model.initial_states() else {
        panic!("the model declares two initial states")
    };
    assert_eq!(fingerprint::<CollideAll>(a), fingerprint::<CollideAll>(b));
    assert_eq!(component_sum(a), component_sum(b));

    let reachable = match bfs::explore(&model, Bounds::CERTIFIABLE).expect("evaluates") {
        Exploration::Complete(reachable) => reachable,
        Exploration::Exhausted(_) => panic!("two states fit every bound"),
    };
    assert_eq!(vec_of(reachable.states()), vec![vec![0, 1], vec![1, 0]]);
    assert_eq!(reachable.depths(), &[0, 0]);
}

// ---------------------------------------------------------------------------
// anti-vacuity: the fingerprint-only dedup mutant is caught
// ---------------------------------------------------------------------------

/// The harness itself is faithful: keyed by the full state it returns exactly what
/// the real explorer returns. So any loss below is the fingerprint's doing.
#[test]
fn exact_twin_harness_agrees_with_the_explorer() {
    for model in [grid(1), grid(3), grid(6), colliding_starts()] {
        assert_eq!(walk(&model, State::clone), explore_closed(&model));
    }
}

/// Under total collision the fingerprint-only mutant keeps one state, and the
/// exactness oracle that accepted the real explorer rejects it.
#[test]
fn fingerprint_only_mutant_loses_states_under_total_collision() {
    let model = grid(3);
    let mutant = walk(&model, fingerprint::<CollideAll>);
    assert_eq!(vec_of(&mutant), vec![vec![0, 0]]);
    assert!(check_exact(&mutant, &grid_exact(3)).is_err());
    assert!(check_exact(&explore_closed(&model), &grid_exact(3)).is_ok());
}

/// Under the anti-diagonal collision the mutant keeps one state per diagonal —
/// `2n + 1` of `(n + 1)^2` — and loses `(1, 0)` or `(0, 1)`.
#[test]
fn fingerprint_only_mutant_loses_the_colliding_pair() {
    let n = 3;
    let model = grid(n);
    let mutant = walk(&model, component_sum);
    assert_eq!(mutant.len(), 7, "2n + 1 anti-diagonals");
    let a = model.state(&[1, 0]).expect("in domain");
    let b = model.state(&[0, 1]).expect("in domain");
    assert!(
        !(mutant.contains(&a) && mutant.contains(&b)),
        "the mutant cannot keep both members of a colliding pair"
    );
    let err = check_exact(&mutant, &grid_exact(n)).expect_err("the oracle detects the mutant");
    assert!(err.contains("exact count 16"), "{err}");
}

/// Under pigeonhole collisions the mutant keeps at most 16 of 49 states: loss is
/// forced by counting, not by a chosen pair.
#[test]
fn fingerprint_only_mutant_loses_states_under_pigeonhole_collisions() {
    let model = grid(6);
    let mutant = walk(&model, fingerprint::<Fnv1aBits<4>>);
    assert!(
        mutant.len() <= 16,
        "at most one state per 4-bit fingerprint"
    );
    assert!(check_exact(&mutant, &grid_exact(6)).is_err());
}

/// Colliding initial states: the mutant drops one at depth 0.
#[test]
fn fingerprint_only_mutant_drops_a_colliding_initial_state() {
    let model = colliding_starts();
    let expected: BTreeSet<Vec<i64>> = [vec![0, 1], vec![1, 0]].into_iter().collect();
    assert_eq!(walk(&model, component_sum).len(), 1);
    assert!(check_exact(&walk(&model, component_sum), &expected).is_err());
    assert!(check_exact(&explore_closed(&model), &expected).is_ok());
}

/// The legal ADR-0013 design — hash as index, exact comparison on collision — is
/// immune to every injected collision.
#[test]
fn hash_indexed_exact_resolution_is_collision_immune() {
    for model in [grid(3), grid(6), colliding_starts()] {
        let exact = explore_closed(&model);
        assert_eq!(walk_bucketed(&model, fingerprint::<CollideAll>), exact);
        assert_eq!(walk_bucketed(&model, fingerprint::<Fnv1aBits<4>>), exact);
        assert_eq!(walk_bucketed(&model, component_sum), exact);
    }
}

// ---------------------------------------------------------------------------
// exact equality: pinned in the source
// ---------------------------------------------------------------------------

const BFS_SRC: &str = include_str!("../src/bfs.rs");
const MODEL_SRC: &str = include_str!("../src/model.rs");

/// Source lines that are code: comments and derive attributes removed.
fn code_lines(src: &str) -> impl Iterator<Item = (usize, &str)> {
    src.lines().enumerate().filter_map(|(i, line)| {
        let t = line.trim_start();
        (!t.starts_with("//") && !t.starts_with("#[derive(")).then_some((i + 1, line))
    })
}

/// The explorer's deduplication keys are the full `State`, and `State`'s equality
/// and order are the derived, component-wise ones over the whole vector. No hash,
/// hasher, or fingerprint appears in the explorer's code.
///
/// This is the drift guard for the collision tests above: they exercise the real
/// explorer, and this test keeps the reason they pass from changing silently.
#[test]
fn state_identity_is_the_full_state_vector_by_source() {
    for needle in [
        "let mut visited: BTreeMap<State, Visit> = BTreeMap::new();",
        "let mut discovered: BTreeMap<State, usize> = BTreeMap::new();",
        "if !visited.contains_key(step.target()) && !discovered.contains_key(step.target()) {",
        "if visited.insert(state.clone(), visit).is_none() {",
        "visited.insert(target.clone(), visit);",
    ] {
        assert!(
            BFS_SRC.contains(needle),
            "bfs.rs no longer contains `{needle}`"
        );
    }
    for (line, text) in code_lines(BFS_SRC) {
        let lower = text.to_lowercase();
        for banned in ["hash", "fingerprint", "digest"] {
            assert!(
                !lower.contains(banned),
                "bfs.rs:{line} uses `{banned}` in code: {text}"
            );
        }
    }

    assert!(
        MODEL_SRC.contains(
            "#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]\n\
             pub struct State {\n    values: Vec<i64>,\n}"
        ),
        "State must stay a single full vector with derived equality and order"
    );
    for manual in [
        "impl PartialEq for State",
        "impl Eq for State",
        "impl PartialOrd for State",
        "impl Ord for State",
    ] {
        assert!(
            !MODEL_SRC.contains(manual),
            "model.rs hand-writes `{manual}`"
        );
    }
}
