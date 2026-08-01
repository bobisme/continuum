//! T02 reachability evidence — "Hash collision changes reachability"
//! (`docs/09_THREAT_MODEL.md:92`, bone `bn-2res`).
//!
//! # The question T02 asks
//!
//! > Can an adversarial hash collision cause a value/state to be wrongly
//! > deduplicated, skipped, or aliased such that analysis reachability differs
//! > from truth?
//! >
//! > — bn-2res
//!
//! `continuum_value::identity`'s collision-injection suite (`src/identity.rs`)
//! proves this at the level of one dedup structure: an [`IdentityIndex`] never
//! suppresses a distinct [`ContentIdentity`] and never reports a false hit, no
//! matter how an adversarial [`ContentHasher`] partitions it.
//! `crates/continuum-workspace/tests/pr2_exit_evidence.rs` and
//! `dx13_falsification.rs` prove the same claim one layer up, at the publication
//! store, including under concurrency and GC pressure.
//!
//! Neither file runs an actual *reachability computation* — a search that starts
//! from one state, follows transitions, and asks "what is the full reachable
//! set" the way an explicit-state engine (`continuum-engine-explicit`,
//! `continuum-engine-dpor` — currently PR-1 scaffolds, docs/01 §7.2) eventually
//! will. This file is that computation: a small adversarial state-transition
//! corpus, explored with an [`IdentityIndex`]-backed visited set standing in for
//! that engine's frontier/closure, under adversarial hashers, compared against a
//! hash-free ground truth. It also runs the *same* corpus through a visited set
//! that is deliberately built the wrong way — keyed on the digest alone — to show
//! the threat T02 names is real when control 1 is dropped, and that the
//! system's actual mechanism does not have that failure.
//!
//! # Control-by-control evidence map
//!
//! bn-2res names four controls, taken verbatim from `docs/09_THREAT_MODEL.md`:
//!
//! ## 1. "fingerprints are indexes, not identity"
//!
//! **Existing coverage** (not duplicated here):
//! `continuum_value::identity::tests::certified_identity_equality_cannot_consult_a_hash`,
//! `an_index_under_a_maximally_colliding_hash_still_separates_every_value`,
//! `a_colliding_lookup_does_not_report_a_false_hit` — [`ContentIdentity`]'s
//! equality never consults a digest, and [`IdentityIndex`] never mistakes a
//! colliding-but-absent value for a hit.
//!
//! **New coverage, this file**: [`positive_reachable_sets_match_ground_truth_under_every_adversarial_hash`]
//! lifts that claim from "one dedup structure" to "the reachable set a search
//! over it would compute". [`negative_a_fingerprint_only_visited_set_loses_reachable_states_under_total_collision`]
//! is the contrapositive: a visited set keyed on the digest alone — i.e. a
//! structure that violates this exact control — measurably loses reachable
//! states under the same attack the positive test survives, so the control is
//! shown to be load-bearing, not merely descriptive.
//!
//! ## 2. "exact canonical comparison on collision"
//!
//! **Existing coverage**: `identity::tests::a_collision_is_counted_rather_than_absorbed`,
//! `equal_values_get_one_identity_even_when_everything_collides`,
//! `every_injected_hash_gives_the_same_certified_answers` (value layer);
//! `continuum_workspace::publication::tests::an_identity_collision_aborts_rather_than_conflating_two_artifacts`,
//! `pr2_exit_evidence::negative_artificial_hash_collision_between_distinct_payloads_is_refused_not_conflated`,
//! `dx13_falsification::concurrent_publication_of_colliding_distinct_values_never_conflates_them`
//! (store layer), and `dx13_mutation_campaign::check_exact_comparison`, which
//! shows a mutant that skips the comparison is caught by nothing else.
//!
//! **New coverage, this file**: the same three positive-test fixtures exercise
//! canonical comparison as the thing that lets a *search*, not just a single
//! `insert`, converge on the right answer — including a diamond-shaped fixture
//! ([`bool_pair_diamond`]) where two different paths legitimately arrive at the
//! same state, so the machinery must distinguish "same state, different path"
//! (a merge — correct) from "different state, same digest" (a collision —
//! must not merge) under one adversarial hash in one run.
//!
//! ## 3. "cryptographic digests for artifact identity"
//!
//! **Existing coverage**: `identity.rs`'s "The hash seam" module-doc section and
//! `identity::tests::the_placeholder_declares_itself_non_cryptographic`,
//! `every_algorithm_token_is_well_formed`,
//! `a_non_certified_lane_cannot_be_entered_unlabeled` — [`Fnv1aPlaceholder`] is
//! the only [`ContentHasher`] this workspace ships outside test doubles, its
//! `ALGORITHM.is_cryptographic()` is pinned `false`, and its algorithm token
//! contains the word "placeholder", so nothing in the workspace can present it
//! as a cryptographic digest without visibly lying.
//!
//! **Finding, not a violation** (reported for the lead, `src/` is out of this
//! bone's scope regardless): no *cryptographic* [`ContentHasher`] or
//! [`continuum_workspace::publication::ContentIdentifier`] is vendored anywhere
//! in `crates/*/src/` today — a repo-wide search finds `impl ContentIdentifier`
//! only inside test files, and the only `HashAlgorithm::cryptographic(...)` call
//! site is an illustrative literal inside `identity.rs`'s own test module. This
//! is a pre-existing, explicitly documented deferral (identity.rs, "Seams left
//! open on purpose": "The release hash is not here" — the vendor decision needs
//! its own review), not something this bone introduces or can fix from a
//! test-only workspace fence. It is *not* a T02 reachability defect: this
//! file's own positive test proves reachability is computed identically under
//! [`Fnv1aPlaceholder`], under a total collider, and under a partial collider —
//! the deferred cryptographic choice can only change how many canonical
//! comparisons a bucket does, never which states are found. Vendoring the real
//! hash later is therefore a performance change, not a correctness one, and
//! that invariance is exactly what [`positive_reachable_sets_match_ground_truth_under_every_adversarial_hash`]
//! pins.
//!
//! ## 4. "adversarial collision corpus"
//!
//! **Existing coverage**: `identity.rs`'s `sample_values`/`sample_identities`
//! fixture (one of every [`Value`] kind, several same-kind pairs) run against
//! three independently constructed adversarial hashers (`ConstantHash`,
//! `KindTagHash`, `LengthHash`); `dx13_falsification.rs`'s twelve-attack,
//! up-to-48-thread campaign (bn-21dd) against
//! [`continuum_workspace::publication::ContentIdentifier`] under contention,
//! abandonment, fault injection, and GC races; `dx13_mutation_campaign.rs`'s
//! mutant-store campaign.
//!
//! **New coverage, this file**: a second, independent adversarial corpus at the
//! reachability layer — five state-transition fixtures ([`fixtures`]: a
//! no-successor state, a self-loop, a multi-state cycle, a converging DAG, and a
//! mixed-kind relay between [`Value::Nat`] and [`Value::Record`]) explored under
//! two adversarial hashers defined in this file ([`TotalCollider`],
//! [`KindByteCollider`], mirroring `identity.rs`'s `ConstantHash`/`KindTagHash`
//! idiom for exactly the reason its module doc gives: a total collision and a
//! multi-bucket collision catch different bug classes) plus the shipped
//! [`Fnv1aPlaceholder`], all checked against a hash-free ground truth.
//!
//! # House rules, inherited from the falsification campaign
//!
//! - **`src/` is not touched.** Every type this file exercises is `pub` already;
//!   nothing here edits, weakens, or moves an existing test.
//! - **Positive, negative, and boundary evidence, each a named test**:
//!   [`positive_reachable_sets_match_ground_truth_under_every_adversarial_hash`],
//!   [`negative_a_fingerprint_only_visited_set_loses_reachable_states_under_total_collision`],
//!   [`boundary_single_state_and_self_loop_graphs_do_not_over_or_under_count_under_collision`].

use std::collections::BTreeSet;

use continuum_value::identity::{
    ContentHasher, ContentIdentity, DIGEST_LEN, Digest256, Fnv1aPlaceholder, HashAlgorithm,
    IdentityIndex,
};
use continuum_value::value::{Name, Value};

// --- adversarial hashers -----------------------------------------------------------------
//
// Re-declared locally rather than imported: `identity.rs`'s `ConstantHash`/`KindTagHash` are
// private to its own `#[cfg(test)]` module. These mirror them so this file's collisions are
// injected the same way, for the same stated reason (`identity.rs`'s "adversarial hashes"
// section) — a total collision and a multi-bucket collision catch different bug classes.

/// Every input hashes to the same digest. The worst hash that exists.
#[derive(Debug, Clone, Copy)]
struct TotalCollider;

impl ContentHasher for TotalCollider {
    const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-t02-total-collider");

    fn hash(_bytes: &[u8]) -> Digest256 {
        Digest256::from_bytes([0x42; DIGEST_LEN])
    }
}

/// Hashes only the first encoded byte (the value's kind tag), so every two
/// states of the same [`Value`] kind collide but states of different kinds do
/// not — a multi-bucket collision, unlike [`TotalCollider`]'s single bucket.
#[derive(Debug, Clone, Copy)]
struct KindByteCollider;

impl ContentHasher for KindByteCollider {
    const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("test-t02-kind-collider");

    fn hash(bytes: &[u8]) -> Digest256 {
        let mut digest = [0u8; DIGEST_LEN];
        digest[0] = bytes.first().copied().unwrap_or(0);
        Digest256::from_bytes(digest)
    }
}

// --- state-transition fixtures ------------------------------------------------------------

/// One synthetic transition system: a name, an initial state, and a pure
/// successor function. Stands in for what an explicit-state engine's model
/// core would hand a search.
type Fixture = (&'static str, Value, Box<dyn Fn(&Value) -> Vec<Value>>);

/// A `record` field name, built once per call site.
fn field(text: &str) -> Name {
    Name::new(text).expect("fixture field name is canonical")
}

/// Fixture: one state, no successors. The degenerate case — a search that adds
/// anything beyond the seed, or drops the seed, is wrong regardless of hashing.
fn single_state_no_successors() -> Fixture {
    (
        "single state, no successors",
        Value::nat(0),
        Box::new(|_state: &Value| Vec::new()),
    )
}

/// Fixture: one state, one self-transition. Termination depends on recognizing
/// "already visited", not on running out of successors.
fn self_loop() -> Fixture {
    (
        "self-loop (single state, always transitions to itself)",
        Value::nat(0),
        Box::new(|_state: &Value| vec![Value::nat(0)]),
    )
}

/// Fixture: a five-state cycle over [`Value::Nat`], `k -> (k + 1) mod 5`.
/// Requires dedup for termination and produces several same-kind states, which
/// stresses [`KindByteCollider`].
fn counter_cycle() -> Fixture {
    const MODULUS: u128 = 5;
    (
        "five-state counter cycle",
        Value::nat(0),
        Box::new(|state: &Value| {
            let Value::Nat(k) = state else {
                unreachable!("counter_cycle only ever produces Nat states")
            };
            vec![Value::nat((k + 1) % MODULUS)]
        }),
    )
}

/// Fixture: the four-state Boolean lattice `{F,F} -> {T,F},{F,T} -> {T,T}`, a
/// diamond DAG where two distinct paths legitimately converge on one state.
/// Distinguishes "same state via two paths" (a correct merge) from "different
/// state, same digest" (a collision, must not merge) under one adversarial
/// hash in one run.
fn bool_pair_diamond() -> Fixture {
    (
        "bool-pair diamond",
        Value::tuple([Value::Bool(false), Value::Bool(false)]).expect("within depth"),
        Box::new(|state: &Value| {
            let Value::Tuple(items) = state else {
                unreachable!("bool_pair_diamond only ever produces 2-tuples")
            };
            let (Value::Bool(a), Value::Bool(b)) = (&items[0], &items[1]) else {
                unreachable!("bool_pair_diamond tuples hold two Bools")
            };
            let mut next = Vec::new();
            if !a {
                next.push(
                    Value::tuple([Value::Bool(true), Value::Bool(*b)]).expect("within depth"),
                );
            }
            if !b {
                next.push(
                    Value::tuple([Value::Bool(*a), Value::Bool(true)]).expect("within depth"),
                );
            }
            next
        }),
    )
}

/// Fixture: a relay between [`Value::Nat`] and [`Value::Record`] —
/// `nat(k) -> record{count: k} -> nat(k + 1)` for `k` in `0..3`, then stops.
/// Mixed kinds in one reachable set, stressing a hash that collides *within* a
/// kind ([`KindByteCollider`]) against a graph that is not single-kind.
fn mixed_kind_relay() -> Fixture {
    const ROUNDS: u128 = 3;
    (
        "mixed nat/record relay",
        Value::nat(0),
        Box::new(|state: &Value| match state {
            Value::Nat(k) if *k < ROUNDS => {
                vec![Value::record([(field("count"), Value::nat(*k))]).expect("within depth")]
            }
            Value::Nat(_) => Vec::new(),
            Value::Record(fields) => {
                let Some(Value::Nat(k)) = fields.get(&field("count")) else {
                    unreachable!("mixed_kind_relay records always carry a Nat 'count' field")
                };
                vec![Value::nat(k + 1)]
            }
            _ => unreachable!("mixed_kind_relay only ever produces Nat and Record states"),
        }),
    )
}

/// Every fixture this file's tests sweep over.
fn fixtures() -> Vec<Fixture> {
    vec![
        single_state_no_successors(),
        self_loop(),
        counter_cycle(),
        bool_pair_diamond(),
        mixed_kind_relay(),
    ]
}

// --- explorers -----------------------------------------------------------------------------

/// Ground truth: reachability computed with no hash anywhere, by canonical
/// (`==`) comparison alone. The standard every explorer below is checked
/// against.
fn reachable_ground_truth(initial: Value, successors: &dyn Fn(&Value) -> Vec<Value>) -> Vec<Value> {
    let mut visited: Vec<Value> = Vec::new();
    let mut frontier = vec![initial];
    while let Some(state) = frontier.pop() {
        if !visited.contains(&state) {
            frontier.extend(successors(&state));
            visited.push(state);
        }
    }
    visited.sort();
    visited
}

/// The system's actual mechanism: a search whose visited set is an
/// [`IdentityIndex`] — digest for bucket selection, canonical comparison for
/// membership (ADR-0013, controls 1 and 2). Returns the reachable set in
/// canonical order, independent of `H` if the controls hold.
fn reachable_via_identity_index<H: ContentHasher>(
    initial: Value,
    successors: &dyn Fn(&Value) -> Vec<Value>,
) -> Vec<ContentIdentity> {
    let mut index: IdentityIndex<H> = IdentityIndex::new();
    let mut frontier = vec![initial];
    while let Some(state) = frontier.pop() {
        let identity = ContentIdentity::of(&state);
        if index.insert(identity).is_fresh() {
            frontier.extend(successors(&state));
        }
    }
    index.sorted_identities().into_iter().cloned().collect()
}

/// The vulnerability control 1 forbids, built anyway so its consequence can be
/// measured: a visited set keyed on the digest *alone*, with no canonical
/// comparison ever consulted. Every value that ever collides with something
/// already marked "seen" is treated as already explored, whether or not it
/// actually is.
fn reachable_via_fingerprint_only<H: ContentHasher>(
    initial: Value,
    successors: &dyn Fn(&Value) -> Vec<Value>,
) -> Vec<Value> {
    let mut seen_digests: BTreeSet<Digest256> = BTreeSet::new();
    let mut visited: Vec<Value> = Vec::new();
    let mut frontier = vec![initial];
    while let Some(state) = frontier.pop() {
        let digest = H::hash(&state.encode());
        if seen_digests.insert(digest) {
            frontier.extend(successors(&state));
            visited.push(state);
        }
    }
    visited.sort();
    visited
}

/// Canonical identities of `values`, sorted — the form every explorer's output
/// is compared in.
fn identities_of(values: &[Value]) -> Vec<ContentIdentity> {
    let mut identities: Vec<ContentIdentity> = values.iter().map(ContentIdentity::of).collect();
    identities.sort();
    identities
}

// --- positive -------------------------------------------------------------------------------

/// **Positive.** For every fixture, the reachable set an [`IdentityIndex`]-backed
/// search computes is exactly the hash-free ground truth — under a total
/// collision, under a multi-bucket collision, and under the hash the workspace
/// actually ships. No state is dropped (a false "already visited"), duplicated,
/// or merged with a different state, and the diamond fixture confirms a
/// legitimate two-path merge still lands on one state, not two.
#[test]
fn positive_reachable_sets_match_ground_truth_under_every_adversarial_hash() {
    for (name, initial, successors) in fixtures() {
        let ground_truth = identities_of(&reachable_ground_truth(
            initial.clone(),
            successors.as_ref(),
        ));
        assert!(
            !ground_truth.is_empty(),
            "{name}: every fixture reaches at least its seed"
        );

        let via_total =
            reachable_via_identity_index::<TotalCollider>(initial.clone(), successors.as_ref());
        assert_eq!(
            via_total, ground_truth,
            "{name}: reachable set changed under a total hash collision"
        );

        let via_kind =
            reachable_via_identity_index::<KindByteCollider>(initial.clone(), successors.as_ref());
        assert_eq!(
            via_kind, ground_truth,
            "{name}: reachable set changed under a same-kind hash collision"
        );

        let via_honest =
            reachable_via_identity_index::<Fnv1aPlaceholder>(initial, successors.as_ref());
        assert_eq!(
            via_honest, ground_truth,
            "{name}: reachable set changed under the shipped placeholder hash"
        );
    }
}

// --- negative ------------------------------------------------------------------------------

/// **Negative.** The contrapositive of the positive test: drop control 1
/// (build the visited set from the digest alone) and the exact attack the
/// positive test survives now measurably changes reachability — every fixture
/// with more than one true state collapses to exactly the seed state, because
/// a total collision makes every subsequent state look "already visited"
/// under a fingerprint-only check. This is what T02 warns about, reproduced on
/// purpose, and it is why control 1 is load-bearing rather than decorative.
#[test]
fn negative_a_fingerprint_only_visited_set_loses_reachable_states_under_total_collision() {
    let mut exercised_a_real_loss = false;
    for (name, initial, successors) in fixtures() {
        let ground_truth = reachable_ground_truth(initial.clone(), successors.as_ref());
        let naive = reachable_via_fingerprint_only::<TotalCollider>(initial, successors.as_ref());

        assert_eq!(
            naive.len(),
            1,
            "{name}: a fingerprint-only visited set under a total collision must collapse to \
             exactly the seed state"
        );
        if ground_truth.len() > 1 {
            assert!(
                naive.len() < ground_truth.len(),
                "{name}: expected the fingerprint-only search to under-approximate reachability \
                 ({} states truly reachable, {} found), but it did not lose anything",
                ground_truth.len(),
                naive.len()
            );
            exercised_a_real_loss = true;
        }
    }
    assert!(
        exercised_a_real_loss,
        "no fixture had more than one true state, so the vulnerability was never actually \
         exercised — the fixture list, not the claim, would be at fault"
    );
}

// --- boundary --------------------------------------------------------------------------------

/// **Boundary.** The two degenerate fixtures — a state with no successors, and
/// a state whose only transition is to itself — each have exactly one true
/// state. A total collision must not turn that one state into zero (the seed
/// dropped) or into more than one (spurious duplication); both are collision
/// failure modes a sweep over larger fixtures could miss if an implementation
/// happened to special-case "the first insert" or "an empty frontier".
#[test]
fn boundary_single_state_and_self_loop_graphs_do_not_over_or_under_count_under_collision() {
    let boundary_fixture_names = [
        "single state, no successors",
        "self-loop (single state, always transitions to itself)",
    ];
    let mut checked = 0usize;
    for (name, initial, successors) in fixtures() {
        if !boundary_fixture_names.contains(&name) {
            continue;
        }
        checked += 1;

        let ground_truth = identities_of(&reachable_ground_truth(
            initial.clone(),
            successors.as_ref(),
        ));
        assert_eq!(
            ground_truth.len(),
            1,
            "{name}: boundary fixture must have exactly one true state"
        );

        let via_total = reachable_via_identity_index::<TotalCollider>(initial, successors.as_ref());
        assert_eq!(
            via_total, ground_truth,
            "{name}: a degenerate one-state graph must still yield exactly its one state under \
             a total collision"
        );
    }
    assert_eq!(
        checked,
        boundary_fixture_names.len(),
        "a boundary fixture was renamed or removed"
    );
}
