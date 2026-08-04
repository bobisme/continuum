//! **Idempotency** — the property suite for content-derived handles: `context.expand`'s
//! `ctx_*` (`bn-221j`).
//!
//! # The law
//!
//! > [`ExpansionHandle::derive`] makes it a property of the *question* […] The preimage is
//! > the ID5 canonical encoding of `{"anchor", "depth", "parent", "relation"}` — the
//! > question's three terms plus the parent's own handle — and the identity is `H`'s digest
//! > of it […] One canonical object, so no pair of inputs can produce another pair's
//! > preimage by concatenation.
//! >
//! > — `crates/continuum-context/src/expansion.rs`
//!
//! "Idempotent" is a claim with two halves, and the second is the one that carries the
//! weight:
//!
//! - **repetition changes nothing.** Deriving the same handle twice, eight times, or in two
//!   daemons yields one `ctx_*`. Nothing is minted, nothing is counted, nothing is drawn.
//!   This is the half a stateful or entropy-drawing implementation fails.
//! - **only the question changes it.** Two *different* questions never derive one handle.
//!   Idempotency without this is a constant function, which is idempotent and useless. This
//!   is the half an implementation that forgets a field fails — and forgetting `depth` is
//!   the realistic version, because `depth` is the term a caller most often leaves absent.
//!
//! Both halves are checked here over generated questions, and both have a seeded violation
//! below.
//!
//! # The two seams
//!
//! [`ExpansionQuestion::of`] is the *canonical rendering* of a question — the pack's
//! `question` field, an ID5 canonical-JSON object — and [`ExpansionHandle::derive`] is the
//! identity that rendering plus the parent handle determines. They are tested together
//! because RFC 0028 requires them to agree: a pack's `question` is what a reader compares
//! byte for byte, and the handle is what a store resolves; if either could move without the
//! other, one pack would answer two questions.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | deriving the same request any number of times gives one handle | [`positive_deriving_the_same_request_repeatedly_gives_one_handle`] |
//! | a handle is a function of the request, not of how it was built | [`positive_a_handle_is_a_function_of_the_request_not_of_its_construction`] |
//! | different requests never share a handle | [`positive_different_requests_never_share_a_handle`] |
//! | the canonical question is byte-stable and agrees with the handle | [`positive_the_canonical_question_is_byte_stable_and_agrees_with_the_handle`] |
//! | changing the hasher changes the handle and nothing else | [`positive_the_hasher_indexes_the_question_it_does_not_define_it`] |
//! | a parent that is not a pack is a typed refusal, for every class | [`negative_deriving_from_something_that_is_not_a_pack_is_refused`] |
//! | every relation and a linear depth sweep derive distinct handles | [`boundary_every_relation_and_a_linear_depth_sweep_stay_distinct`] |
//! | **the suite can fail**: a hasher that drifts between calls | [`falsification_a_drifting_hasher_breaks_idempotency_and_is_caught`] |
//! | **the suite can fail**: a derivation that forgets `depth` | [`falsification_a_depth_blind_derivation_is_caught`] |
//! | both shrunk counterexamples are retained and replay on their own | [`falsification_the_retained_counterexamples_replay_deterministically`] |

#[path = "../../continuum-value/tests/support/property.rs"]
mod property;

use std::cell::Cell;

use continuum_context::expansion::{
    Depth, ExpansionError, ExpansionHandle, ExpansionQuery, ExpansionQuestion, ExpansionRelation,
};
use continuum_value::identity::{
    Blake3Hasher, ContentHasher, DIGEST_LEN, Digest256, Fnv1aPlaceholder, HashAlgorithm,
};
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use property::{
    Case, Domain, NearPairs, Pair, Pairs, Plan, Prng, Sexp, check, expect_replay_refutes,
};

/// Cases per law.
const CASES: usize = 512;

/// How many times a request is re-derived before the suite believes it is idempotent.
///
/// Eight rather than two: a seam that drifts every *other* call, or every fourth, is a real
/// shape (a cache line, an alternating buffer) that two calls cannot see.
const REPEATS: usize = 8;

// --- the case -----------------------------------------------------------------------------

/// Parent pack identities. Distinct handles, one of which is a prefix of another, so that a
/// derivation that concatenated rather than canonicalized its preimage would show.
const PARENTS: [&str; 3] = ["root", "root0", "other"];

/// Anchors: a shortlex pair and a name that is a prefix of another, for the same reason.
const ANCHORS: [&str; 4] = ["z", "aa", "item", "item0"];

/// Depths, including the default (one hop) and the two either side of a decimal digit
/// boundary, where a non-canonical integer rendering would show.
const DEPTHS: [u32; 5] = [1, 2, 9, 10, 4_294_967_295];

/// One `context.expand` request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuestionCase {
    parent: usize,
    relation: usize,
    anchor: usize,
    depth: usize,
}

impl QuestionCase {
    fn parent_handle(&self) -> ArtifactHandle {
        ArtifactHandle::new(
            ArtifactClass::ContextPack,
            PARENTS[self.parent % PARENTS.len()],
        )
        .expect("the parent alphabet is legal identity characters")
    }

    fn query(&self) -> ExpansionQuery {
        ExpansionQuery::new(
            ExpansionRelation::ALL[self.relation % ExpansionRelation::ALL.len()],
            Name::new(ANCHORS[self.anchor % ANCHORS.len()]).expect("the anchors are ASCII"),
        )
    }

    fn depth(&self) -> Depth {
        Depth::new(DEPTHS[self.depth % DEPTHS.len()]).expect("the depth alphabet is non-zero")
    }

    /// The handle this request derives under `H`.
    fn derive<H: ContentHasher>(&self) -> ExpansionHandle {
        ExpansionHandle::derive::<H>(&self.parent_handle(), &self.query(), self.depth())
            .expect("the parent is a context pack")
    }
}

impl Case for QuestionCase {
    fn to_sexp(&self) -> Sexp {
        Sexp::list([
            Sexp::number(self.parent as u128),
            Sexp::number(self.relation as u128),
            Sexp::number(self.anchor as u128),
            Sexp::number(self.depth as u128),
        ])
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        let [parent, relation, anchor, depth] = sexp.as_tuple::<4>()?;
        Some(Self {
            parent: usize::try_from(parent.as_number()?).ok()?,
            relation: usize::try_from(relation.as_number()?).ok()?,
            anchor: usize::try_from(anchor.as_number()?).ok()?,
            depth: usize::try_from(depth.as_number()?).ok()?,
        })
    }
}

/// Generated requests.
#[derive(Debug, Clone, Copy)]
struct Questions;

impl Domain for Questions {
    type Item = QuestionCase;

    fn generate(&self, rng: &mut Prng) -> QuestionCase {
        QuestionCase {
            parent: rng.below(PARENTS.len()),
            relation: rng.below(ExpansionRelation::ALL.len()),
            anchor: rng.below(ANCHORS.len()),
            depth: rng.below(DEPTHS.len()),
        }
    }

    fn shrink(&self, item: &QuestionCase) -> Vec<QuestionCase> {
        // One field at a time, towards the first entry of each alphabet: first the jump to
        // zero, then a single step down. The jump is the aggressive reduction; the step is
        // what lets a *pair* whose two halves must stay different walk all the way to
        // adjacent alphabet entries instead of stopping as soon as one half hits zero.
        let mut out = Vec::new();
        for jump in [true, false] {
            for (index, current) in [item.parent, item.relation, item.anchor, item.depth]
                .into_iter()
                .enumerate()
            {
                if current == 0 || (jump && current == 1) {
                    continue;
                }
                let mut reduced = item.clone();
                let slot = match index {
                    0 => &mut reduced.parent,
                    1 => &mut reduced.relation,
                    2 => &mut reduced.anchor,
                    _ => &mut reduced.depth,
                };
                *slot = if jump { 0 } else { current - 1 };
                out.push(reduced);
            }
        }
        out
    }

    fn size(&self, item: &QuestionCase) -> usize {
        item.parent + item.relation + item.anchor + item.depth
    }
}

// --- the laws -----------------------------------------------------------------------------

/// Repetition changes nothing, over whichever hasher is supplied.
fn idempotent_under<H: ContentHasher>(case: &QuestionCase) -> Result<(), String> {
    let first = case.derive::<H>();
    for repeat in 1..REPEATS {
        let again = case.derive::<H>();
        if again != first {
            return Err(format!(
                "derivation {repeat} produced {again}, not {first}: the handle is minted, \
                 not derived"
            ));
        }
    }
    // The canonical question is likewise a function of the request alone.
    let question = ExpansionQuestion::of(&case.query(), case.depth());
    for _ in 1..REPEATS {
        if ExpansionQuestion::of(&case.query(), case.depth()) != question {
            return Err("the canonical question changed between renderings".to_owned());
        }
    }
    Ok(())
}

fn idempotent(case: &QuestionCase) -> Result<(), String> {
    idempotent_under::<Blake3Hasher>(case)
}

/// A handle is a function of the request, not of the objects it was built from.
fn a_handle_ignores_its_construction(case: &QuestionCase) -> Result<(), String> {
    // Two independently constructed parents, queries, and depths that happen to be equal.
    let one =
        ExpansionHandle::derive::<Blake3Hasher>(&case.parent_handle(), &case.query(), case.depth())
            .expect("the parent is a context pack");
    let two =
        ExpansionHandle::derive::<Blake3Hasher>(&case.parent_handle(), &case.query(), case.depth())
            .expect("the parent is a context pack");
    if one != two {
        return Err("two equal requests built separately derived two handles".to_owned());
    }
    // And the handle is a `ctx_*`, because an expansion of a pack is a pack.
    if one.handle().class() != ArtifactClass::ContextPack {
        return Err(format!(
            "the derived handle is a {:?}, not a context pack",
            one.handle().class()
        ));
    }
    Ok(())
}

/// Only the question changes the handle, over whichever derivation is supplied.
fn injective_under(
    pair: &Pair<QuestionCase>,
    derive: impl Fn(&QuestionCase) -> ExpansionHandle,
) -> Result<(), String> {
    let same_handle = derive(&pair.left) == derive(&pair.right);
    let same_request = pair.left.parent_handle() == pair.right.parent_handle()
        && pair.left.query() == pair.right.query()
        && pair.left.depth() == pair.right.depth();
    match (same_handle, same_request) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(
            "two different questions derive one handle: the handle is not a function of \
             the whole question"
                .to_owned(),
        ),
        (false, true) => Err("one question derives two handles".to_owned()),
    }
}

fn different_requests_never_share_a_handle(pair: &Pair<QuestionCase>) -> Result<(), String> {
    injective_under(pair, QuestionCase::derive::<Blake3Hasher>)
}

// --- the suite ------------------------------------------------------------------------------

#[test]
fn positive_deriving_the_same_request_repeatedly_gives_one_handle() {
    let plan = Plan::new("derive is idempotent", 0x2021_0221_0004_0001, CASES);
    check(&plan, &Questions, idempotent).expect_held(&plan);
}

#[test]
fn positive_a_handle_is_a_function_of_the_request_not_of_its_construction() {
    let plan = Plan::new("derive ignores construction", 0x2021_0221_0004_0002, CASES);
    check(&plan, &Questions, a_handle_ignores_its_construction).expect_held(&plan);
}

#[test]
fn positive_different_requests_never_share_a_handle() {
    // Adjacent requests first — one field changed — because that is where a derivation that
    // forgets a field hides.
    let plan = Plan::new(
        "derive is injective, adjacent",
        0x2021_0221_0004_0003,
        CASES,
    );
    check(
        &plan,
        &NearPairs(Questions),
        different_requests_never_share_a_handle,
    )
    .expect_held(&plan);

    let plan = Plan::new(
        "derive is injective, independent",
        0x2021_0221_0004_0013,
        CASES,
    );
    check(
        &plan,
        &Pairs(Questions),
        different_requests_never_share_a_handle,
    )
    .expect_held(&plan);
}

#[test]
fn positive_the_canonical_question_is_byte_stable_and_agrees_with_the_handle() {
    let plan = Plan::new("question is byte-stable", 0x2021_0221_0004_0004, CASES);
    check(&plan, &NearPairs(Questions), |pair: &Pair<QuestionCase>| {
        let left = ExpansionQuestion::of(&pair.left.query(), pair.left.depth());
        let right = ExpansionQuestion::of(&pair.right.query(), pair.right.depth());

        // The rendering is ID5 canonical JSON: no whitespace, keys ascending by code point.
        for question in [&left, &right] {
            let text = question.as_str();
            if text.contains(' ') || text.contains('\n') {
                return Err(format!("the canonical question carries whitespace: {text}"));
            }
            let expected = r#"{"anchor":"#;
            if !text.starts_with(expected) {
                return Err(format!("the canonical question is not ID5-ordered: {text}"));
            }
        }

        // Two questions agree exactly when their (query, depth) pairs do — and when they
        // agree, the handles derived under one parent agree too.
        let same_question = left == right;
        let same_terms =
            pair.left.query() == pair.right.query() && pair.left.depth() == pair.right.depth();
        if same_question != same_terms {
            return Err(format!(
                "the canonical question does not separate the terms: {} vs {}",
                left.as_str(),
                right.as_str()
            ));
        }
        let under_one_parent = |case: &QuestionCase| {
            ExpansionHandle::derive::<Blake3Hasher>(
                &QuestionCase {
                    parent: 0,
                    ..case.clone()
                }
                .parent_handle(),
                &case.query(),
                case.depth(),
            )
            .expect("the parent is a context pack")
        };
        if same_question != (under_one_parent(&pair.left) == under_one_parent(&pair.right)) {
            return Err("the handle and the canonical question disagree".to_owned());
        }
        Ok(())
    })
    .expect_held(&plan);
}

#[test]
fn positive_the_hasher_indexes_the_question_it_does_not_define_it() {
    // ADR-0013 again, one level down: the digest is an index. Swapping the hasher must move
    // every handle (a different index) while moving no *question* (the same preimage), and
    // both hashers must stay internally idempotent and injective.
    let plan = Plan::new("the hasher is an index", 0x2021_0221_0004_0005, CASES);
    check(&plan, &NearPairs(Questions), |pair: &Pair<QuestionCase>| {
        for case in [&pair.left, &pair.right] {
            idempotent_under::<Fnv1aPlaceholder>(case)?;
            if case.derive::<Blake3Hasher>() == case.derive::<Fnv1aPlaceholder>() {
                return Err("two different hashers produced one handle".to_owned());
            }
        }
        // Injectivity holds under each hasher separately, and the *question* — the thing
        // the digest indexes — is the same either way.
        injective_under(pair, QuestionCase::derive::<Fnv1aPlaceholder>)?;
        let left = ExpansionQuestion::of(&pair.left.query(), pair.left.depth());
        let right = ExpansionQuestion::of(&pair.right.query(), pair.right.depth());
        if (left == right)
            != (pair.left.query() == pair.right.query() && pair.left.depth() == pair.right.depth())
        {
            return Err("the question depends on something other than its terms".to_owned());
        }
        Ok(())
    })
    .expect_held(&plan);
}

#[test]
fn negative_deriving_from_something_that_is_not_a_pack_is_refused() {
    // Exhaustive over the plan §4.4 class list: an expansion of something that is not a
    // Context Pack has no meaning, and deriving a `ctx_*` for it would invent a lineage.
    let query = ExpansionQuery::new(ExpansionRelation::ALL[0], Name::new("item").expect("ASCII"));
    let mut refused = 0usize;
    for class in ArtifactClass::ALL {
        let parent = ArtifactHandle::new(class, "identity").expect("legal identity characters");
        let derived = ExpansionHandle::derive::<Blake3Hasher>(&parent, &query, Depth::DEFAULT);
        if class == ArtifactClass::ContextPack {
            assert!(derived.is_ok(), "a pack must be expandable");
        } else {
            assert_eq!(
                derived,
                Err(ExpansionError::NotAPack { class }),
                "{class:?} must be a typed refusal, not a derived handle"
            );
            refused += 1;
        }
    }
    assert_eq!(refused, ArtifactClass::ALL.len() - 1);

    // Zero depth is likewise a typed refusal rather than a default or "no limit".
    assert_eq!(Depth::new(0), Err(ExpansionError::ZeroDepth));
}

#[test]
fn boundary_every_relation_and_a_linear_depth_sweep_stay_distinct() {
    let parent = ArtifactHandle::new(ArtifactClass::ContextPack, "root").expect("legal");
    let anchor = Name::new("item").expect("ASCII");

    // Every one of the eleven relations, under one anchor and one depth.
    let mut handles: Vec<String> = ExpansionRelation::ALL
        .into_iter()
        .map(|relation| {
            let query = ExpansionQuery::new(relation, anchor.clone());
            ExpansionHandle::derive::<Blake3Hasher>(&parent, &query, Depth::DEFAULT)
                .expect("a pack")
                .to_string()
        })
        .collect();
    let before = handles.len();
    handles.sort();
    handles.dedup();
    assert_eq!(handles.len(), before, "two relations derived one handle");

    // A linear depth sweep: one increment at a time, up to 512, plus the extremes. Never
    // doubled — the bound to target is `u32`'s, and it is reached by naming it, not by
    // growing towards it.
    let query = ExpansionQuery::new(ExpansionRelation::ALL[0], anchor);
    let mut sweep: Vec<String> = Vec::with_capacity(515);
    for depth in 1..=512u32 {
        sweep.push(
            ExpansionHandle::derive::<Blake3Hasher>(
                &parent,
                &query,
                Depth::new(depth).expect("non-zero"),
            )
            .expect("a pack")
            .to_string(),
        );
    }
    for depth in [u32::MAX - 1, u32::MAX] {
        sweep.push(
            ExpansionHandle::derive::<Blake3Hasher>(
                &parent,
                &query,
                Depth::new(depth).expect("non-zero"),
            )
            .expect("a pack")
            .to_string(),
        );
    }
    let before = sweep.len();
    sweep.sort();
    sweep.dedup();
    assert_eq!(sweep.len(), before, "two depths derived one handle");
}

// --- anti-vacuity: the suite can fail --------------------------------------------------------

thread_local! {
    /// How many times [`DriftingHasher`] has been called on this thread.
    ///
    /// Thread-local, not global: `cargo test` runs each test on its own thread, so the
    /// counter is a per-test quantity and the seeded violation stays deterministic under
    /// parallel execution. It is also explicit state a test owns, not an ambient capability
    /// reached through the environment — INV-005 is about what *controlled code* may reach
    /// for, and this is a deliberately broken seam being held up as an example.
    static DRIFT: Cell<u64> = const { Cell::new(0) };
}

/// A hasher whose answer depends on how many times it has been asked.
///
/// The realistic shape: a seam that carries interior state — a counter, a salt, a reused
/// buffer — instead of being a pure function of its bytes. `ContentHasher`'s own
/// documentation forbids exactly this ("Implementations are zero-sized types […] because
/// docs/19 §7 requires the artifacts derived from it to be identical across processes,
/// machines, and restarts"), and this is what a suite that could not tell would let through.
#[derive(Debug, Clone, Copy)]
struct DriftingHasher;

impl ContentHasher for DriftingHasher {
    const ALGORITHM: HashAlgorithm = HashAlgorithm::non_cryptographic("drifting-test-double");

    fn hash(bytes: &[u8]) -> Digest256 {
        let drift = DRIFT.with(|calls| {
            let next = calls.get().wrapping_add(1);
            calls.set(next);
            next
        });
        let mut digest = [0u8; DIGEST_LEN];
        let base = Blake3Hasher::hash(bytes);
        digest.copy_from_slice(base.as_bytes());
        for (slot, byte) in digest.iter_mut().zip(drift.to_be_bytes()) {
            *slot ^= byte;
        }
        Digest256::from_bytes(digest)
    }
}

/// A derivation that forgets one term of the question: `depth` is replaced by its default.
///
/// The realistic shape of the *other* failure: an argument added to a request and not added
/// to the preimage it is supposed to name. It is perfectly idempotent — repetition changes
/// nothing — and it is still wrong, which is why an idempotency suite needs an injectivity
/// law beside it.
fn depth_blind_derive(case: &QuestionCase) -> ExpansionHandle {
    ExpansionHandle::derive::<Blake3Hasher>(&case.parent_handle(), &case.query(), Depth::DEFAULT)
        .expect("the parent is a context pack")
}

fn idempotent_under_a_drifting_hasher(case: &QuestionCase) -> Result<(), String> {
    idempotent_under::<DriftingHasher>(case)
}

fn injective_under_a_depth_blind_derivation(pair: &Pair<QuestionCase>) -> Result<(), String> {
    injective_under(pair, depth_blind_derive)
}

/// The first request of every alphabet: parent `root`, the first relation, anchor `z`,
/// depth 1. Nothing about the request matters to a hasher that drifts, so the minimum is
/// the smallest request there is.
const RETAINED_DRIFTING_HASHER: &str = "(#30 #30 #30 #30)";

/// Two requests identical but for `depth` — the term the blind derivation drops — and
/// adjacent in the depth alphabet: `2` against `1`, the two smallest depths there are.
const RETAINED_DEPTH_BLIND: &str = "((#30 #30 #30 #31) (#30 #30 #30 #30))";

#[test]
fn falsification_a_drifting_hasher_breaks_idempotency_and_is_caught() {
    let plan = Plan::new(
        "derive is idempotent, over a hasher that drifts between calls",
        0x2021_0221_0004_0001,
        CASES,
    );
    let refuted =
        check(&plan, &Questions, idempotent_under_a_drifting_hasher).expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_DRIFTING_HASHER);
    assert!(
        refuted.reason.contains("minted, not derived"),
        "{}",
        refuted.reason
    );

    // The real hasher is idempotent on the very case that refutes the drifting one.
    assert_eq!(idempotent(&refuted.minimal), Ok(()));
}

#[test]
fn falsification_a_depth_blind_derivation_is_caught() {
    let plan = Plan::new(
        "derive is injective, over a derivation that forgets `depth`",
        0x2021_0221_0004_0003,
        CASES,
    );
    let refuted = check(
        &plan,
        &NearPairs(Questions),
        injective_under_a_depth_blind_derivation,
    )
    .expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_DEPTH_BLIND);
    assert!(
        refuted
            .reason
            .contains("not a function of the whole question"),
        "{}",
        refuted.reason
    );
    // The two requests differ in `depth` and in nothing else.
    assert_eq!(refuted.minimal.left.parent, refuted.minimal.right.parent);
    assert_eq!(
        refuted.minimal.left.relation,
        refuted.minimal.right.relation
    );
    assert_eq!(refuted.minimal.left.anchor, refuted.minimal.right.anchor);
    assert_ne!(refuted.minimal.left.depth, refuted.minimal.right.depth);

    // The blind derivation is *idempotent*, which is exactly why idempotency alone is not
    // evidence that a handle is content-derived.
    assert_eq!(
        depth_blind_derive(&refuted.minimal.left),
        depth_blind_derive(&refuted.minimal.left)
    );
    // And the real derivation separates the pair.
    assert_eq!(
        different_requests_never_share_a_handle(&refuted.minimal),
        Ok(())
    );
}

#[test]
fn falsification_the_retained_counterexamples_replay_deterministically() {
    let reason = expect_replay_refutes::<QuestionCase, _>(
        RETAINED_DRIFTING_HASHER,
        idempotent_under_a_drifting_hasher,
    );
    assert!(reason.contains("minted, not derived"), "{reason}");

    let reason = expect_replay_refutes::<Pair<QuestionCase>, _>(
        RETAINED_DEPTH_BLIND,
        injective_under_a_depth_blind_derivation,
    );
    assert!(reason.contains("derive one handle"), "{reason}");

    let case = QuestionCase::from_repr(RETAINED_DRIFTING_HASHER).expect("parses");
    assert_eq!(case.repr(), RETAINED_DRIFTING_HASHER);
    assert_eq!(idempotent(&case), Ok(()));

    let pair = Pair::<QuestionCase>::from_repr(RETAINED_DEPTH_BLIND).expect("parses");
    assert_eq!(pair.repr(), RETAINED_DEPTH_BLIND);
    assert_eq!(different_requests_never_share_a_handle(&pair), Ok(()));
}
