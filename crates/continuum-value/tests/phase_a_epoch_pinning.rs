//! **Epoch pinning** — the property suite for [`EpochSet`] and for the identity of a pinned
//! artifact (`bn-221j`).
//!
//! # The law
//!
//! > Snapshots are content-addressed and immutable.
//! >
//! > — `notes/plan/plan.md` §4.2 / ADR-0018
//!
//! > […] continuations resume "only under their pinned epoch" (plan §4.6); resume rejects
//! > mismatched snapshots or epochs, and a mismatch is `ContinuationEpochMismatch`, never a
//! > silent re-run.
//! >
//! > — [`EpochSet::first_mismatch`]'s own contract, quoting plan §9.6 and RFC 0026
//!
//! Put together, epoch pinning is one property with two faces:
//!
//! - **the identity face** — *a pinned artifact's identity is a function of its pinned
//!   epochs*. Two artifacts with the same content under different pins are two artifacts.
//!   An identity that ignored an epoch would let a result computed under one meaning of
//!   evaluation be served for a question asked under another;
//! - **the admission face** — [`EpochSet::first_mismatch`] admits a resume exactly when
//!   every epoch the continuation pins is pinned identically now, and names the *first*
//!   disagreement in [`EpochKind::ALL`] order when it does not. An epoch the continuation
//!   left unpinned constrains nothing; an epoch it pinned that is now absent is a mismatch,
//!   not a permission to proceed.
//!
//! The two faces are checked against the same generated pin descriptions, so they cannot
//! drift apart: the same six-slot description builds the set whose mismatches are tested and
//! the preimage whose identity is tested.
//!
//! # Why the identity model lives in this file
//!
//! `continuum-value` owns [`EpochSet`] and [`ContentIdentity`] but does not own an artifact
//! type that carries both — the workspace snapshot that does is `continuum-workspace`'s, and
//! it is PR 3's. [`pinned_identity`] is therefore a *model* of the composition, written here
//! and stated as such: a canonical record naming the content and all six epochs. It is not
//! production code and nothing depends on it. What it buys is that the law "identity is a
//! function of the pinned epochs" is testable now, generically, over every combination of
//! six pins rather than over whatever combination a snapshot fixture happens to carry — and
//! that the seeded violation below (an identity that quietly drops one epoch) is caught by
//! the law rather than by a fixture that happens to exercise that epoch.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | a set is a function of its description; all six kinds are always named | [`positive_a_pinned_set_is_a_function_of_its_description`] |
//! | a set always admits itself | [`positive_a_pinned_set_admits_itself`] |
//! | pinning nothing constrains nothing; pinning something constrains it | [`positive_an_unpinned_continuation_constrains_nothing`] |
//! | a mismatch is reported at the first disagreeing kind, in `ALL` order | [`positive_a_mismatch_is_the_first_disagreeing_kind`] |
//! | a pinned artifact's identity is injective in (content, pins) | [`positive_a_pinned_identity_is_a_function_of_its_pinned_epochs`] |
//! | the digest of a pinned identity agrees with the identity | [`positive_the_digest_of_a_pinned_identity_indexes_it_faithfully`] |
//! | all 64 pin combinations are distinct artifacts, exhaustively | [`boundary_every_one_of_the_sixty_four_pin_combinations_is_distinct`] |
//! | **the suite can fail**: an identity that ignores one epoch | [`falsification_an_identity_that_ignores_the_corpus_epoch_is_caught`] |
//! | **the suite can fail**: a mismatch check that skips one epoch | [`falsification_a_mismatch_check_that_skips_the_corpus_epoch_is_caught`] |
//! | both shrunk counterexamples are retained and replay on their own | [`falsification_the_retained_counterexamples_replay_deterministically`] |

#[path = "support/property.rs"]
mod property;
#[path = "support/value_domain.rs"]
mod value_domain;

use continuum_value::epoch::{
    CorpusEpoch, EpochBinding, EpochKind, EpochSet, EvidenceEpoch, IntentEpoch, ProofEpoch,
    ProtocolEpoch, SemanticEpoch,
};
use continuum_value::identity::{Blake3Hasher, ContentIdentity};
use continuum_value::value::Value;

use property::{
    Case, Domain, NearPairs, Pair, Pairs, Plan, Prng, Sexp, check, expect_replay_refutes,
};
use value_domain::name;

/// Cases per law.
const CASES: usize = 512;

// --- the pin description ---------------------------------------------------------------

/// How many tokens each epoch kind can be pinned to in this domain.
///
/// Three: enough that "pinned differently" is a case distinct from "pinned versus unpinned",
/// and small enough that the whole space stays enumerable for the exhaustive boundary test.
const TOKENS_PER_KIND: usize = 3;

/// The content an artifact carries, as an index into a small alphabet.
///
/// Content is not what this suite is about — the canonical-round-trip suite covers the value
/// algebra — so three contents suffice to separate "the pins changed" from "the content
/// changed".
const CONTENTS: [&str; 3] = ["alpha", "beta", "gamma"];

/// Six slots, in [`EpochKind::ALL`] order: `None` is unpinned, `Some(i)` is the `i`-th token
/// of that kind.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PinCase {
    content: usize,
    slots: [Option<usize>; 6],
}

impl PinCase {
    /// The content value this artifact carries.
    fn content_value(&self) -> Value {
        Value::text(CONTENTS[self.content % CONTENTS.len()])
    }

    /// The canonical token this case pins `kind` to, if it pins it.
    fn token(&self, kind: EpochKind) -> Option<String> {
        let index = self.slots[kind_index(kind)]? % TOKENS_PER_KIND;
        Some(match kind {
            // A protocol epoch's identity is its `major.minor` spelling, so its alphabet is
            // built from components rather than from a token string.
            EpochKind::Protocol => ProtocolEpoch::new(3, u32::try_from(index).unwrap_or(0))
                .identity()
                .as_str()
                .to_owned(),
            other => format!("{}/{index}", other.as_str()),
        })
    }

    /// The set this description denotes.
    fn epoch_set(&self) -> EpochSet {
        let mut set = EpochSet::unpinned();
        for kind in EpochKind::ALL {
            let Some(index) = self.slots[kind_index(kind)].map(|i| i % TOKENS_PER_KIND) else {
                continue;
            };
            let token = self.token(kind).expect("the slot is pinned");
            set = match kind {
                EpochKind::Protocol => {
                    set.with_protocol(ProtocolEpoch::new(3, u32::try_from(index).unwrap_or(0)))
                }
                EpochKind::Semantic => {
                    set.with_semantic(SemanticEpoch::new(&token).expect("a canonical token"))
                }
                EpochKind::Intent => {
                    set.with_intent(IntentEpoch::new(&token).expect("a canonical token"))
                }
                EpochKind::Evidence => {
                    set.with_evidence(EvidenceEpoch::new(&token).expect("a canonical token"))
                }
                EpochKind::Proof => {
                    set.with_proof(ProofEpoch::new(&token).expect("a canonical token"))
                }
                EpochKind::Corpus => {
                    set.with_corpus(CorpusEpoch::new(&token).expect("a canonical token"))
                }
            };
        }
        set
    }
}

fn kind_index(kind: EpochKind) -> usize {
    EpochKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .expect("`EpochKind::ALL` names every kind")
}

impl Case for PinCase {
    fn to_sexp(&self) -> Sexp {
        let mut items = vec![Sexp::number(self.content as u128)];
        for slot in self.slots {
            items.push(slot.map_or_else(
                || Sexp::Atom(Vec::new()),
                |index| Sexp::number(index as u128),
            ));
        }
        Sexp::list(items)
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        let items = sexp.as_list()?;
        let [content, rest @ ..] = items else {
            return None;
        };
        let rest: &[Sexp; 6] = rest.try_into().ok()?;
        let mut slots = [None; 6];
        for (slot, item) in slots.iter_mut().zip(rest) {
            *slot = match item.as_atom()? {
                [] => None,
                _ => Some(usize::try_from(item.as_number()?).ok()?),
            };
        }
        Some(Self {
            content: usize::try_from(content.as_number()?).ok()?,
            slots,
        })
    }
}

/// Generated pin descriptions.
#[derive(Debug, Clone, Copy)]
struct Pins;

impl Domain for Pins {
    type Item = PinCase;

    fn generate(&self, rng: &mut Prng) -> PinCase {
        let mut slots = [None; 6];
        for slot in &mut slots {
            // Unpinned is drawn as often as any single token, so "pins nothing", "pins
            // some", and "pins all" are all common shapes.
            *slot = match rng.below(TOKENS_PER_KIND + 1) {
                0 => None,
                index => Some(index - 1),
            };
        }
        PinCase {
            content: rng.below(CONTENTS.len()),
            slots,
        }
    }

    fn shrink(&self, item: &PinCase) -> Vec<PinCase> {
        let mut out = Vec::new();
        if item.content != 0 {
            out.push(PinCase {
                content: 0,
                ..item.clone()
            });
        }
        // Unpin, left to right: the most aggressive reduction.
        for index in 0..6 {
            if item.slots[index].is_some() {
                let mut slots = item.slots;
                slots[index] = None;
                out.push(PinCase {
                    content: item.content,
                    slots,
                });
            }
        }
        // Then move a pin to the first token of its kind.
        for index in 0..6 {
            if item.slots[index].is_some_and(|token| token != 0) {
                let mut slots = item.slots;
                slots[index] = Some(0);
                out.push(PinCase {
                    content: item.content,
                    slots,
                });
            }
        }
        out
    }

    fn size(&self, item: &PinCase) -> usize {
        item.content
            + item
                .slots
                .iter()
                .map(|slot| slot.map_or(0, |token| 2 + token))
                .sum::<usize>()
    }
}

// --- the identity model -----------------------------------------------------------------

/// The identity of an artifact carrying `content` under the pins in `set`.
///
/// A model of the composition plan §4.6 requires, written as a canonical record so the
/// identity is a [`ContentIdentity`] — the canonical bytes themselves — rather than a
/// digest. All six kinds appear whether pinned or not, and an unpinned kind is a named
/// [`Value::Null`] rather than an absent field: "pinning nothing" is a statement about an
/// artifact, and INV-007 wants it named rather than omitted.
fn pinned_identity(content: &Value, set: &EpochSet) -> ContentIdentity {
    ContentIdentity::of(&pinned_preimage(content, set, &EpochKind::ALL))
}

/// The preimage [`pinned_identity`] is the encoding of, over exactly the named kinds.
///
/// Taking the kind list as an argument is what makes the seeded violation below a
/// *one-argument* change rather than a second implementation: an identity that forgets an
/// epoch is this function with one kind missing from the list.
fn pinned_preimage(content: &Value, set: &EpochSet, kinds: &[EpochKind]) -> Value {
    let epochs = Value::record(kinds.iter().map(|kind| {
        let binding = match set.binding(*kind) {
            EpochBinding::Pinned(identity) => Value::text(identity.as_str()),
            EpochBinding::Unpinned => Value::Null,
        };
        (name(kind.as_str()), binding)
    }))
    .expect("the six kind names are distinct");
    Value::record([(name("content"), content.clone()), (name("epochs"), epochs)])
        .expect("two distinct field names")
}

// --- the laws ---------------------------------------------------------------------------

fn a_set_is_a_function_of_its_description(case: &PinCase) -> Result<(), String> {
    let first = case.epoch_set();
    let second = case.epoch_set();
    if first != second {
        return Err("building the same description twice gave two sets".to_owned());
    }
    let entries = first.entries();
    if entries.len() != 6 {
        return Err(format!("a set named {} epochs, not six", entries.len()));
    }
    for (position, entry) in entries.iter().enumerate() {
        if entry.kind != EpochKind::ALL[position] {
            return Err(format!(
                "entry {position} is {:?}, not {:?}",
                entry.kind,
                EpochKind::ALL[position]
            ));
        }
        let expected = case.token(entry.kind);
        match (entry.binding, expected) {
            (EpochBinding::Pinned(identity), Some(token)) if identity.as_str() == token => {}
            (EpochBinding::Unpinned, None) => {}
            (binding, expected) => {
                return Err(format!(
                    "{:?} reads {binding:?} but the description says {expected:?}",
                    entry.kind
                ));
            }
        }
    }
    // The unpinned list is the complement of the pinned one, in `ALL` order.
    let unpinned = first.unpinned_kinds();
    let expected: Vec<EpochKind> = EpochKind::ALL
        .into_iter()
        .filter(|kind| case.token(*kind).is_none())
        .collect();
    if unpinned != expected {
        return Err(format!(
            "unpinned_kinds says {unpinned:?}, not {expected:?}"
        ));
    }
    Ok(())
}

fn a_set_admits_itself(case: &PinCase) -> Result<(), String> {
    let set = case.epoch_set();
    match set.first_mismatch(&set) {
        None => Ok(()),
        Some(kind) => Err(format!("a set disagreed with itself at {kind:?}")),
    }
}

fn an_unpinned_continuation_constrains_nothing(case: &PinCase) -> Result<(), String> {
    let set = case.epoch_set();
    let nothing = EpochSet::unpinned();
    if let Some(kind) = nothing.first_mismatch(&set) {
        return Err(format!(
            "a continuation that pins nothing was refused at {kind:?}"
        ));
    }
    // The other direction: every kind this case pins is a kind an unpinned `current` cannot
    // satisfy, and the first such kind in `ALL` order is what is reported.
    let expected = EpochKind::ALL
        .into_iter()
        .find(|kind| case.token(*kind).is_some());
    let reported = set.first_mismatch(&nothing);
    if reported != expected {
        return Err(format!(
            "against an unpinned current, the mismatch is {reported:?}, expected {expected:?}"
        ));
    }
    Ok(())
}

/// The first disagreement in `ALL` order, computed independently of the implementation.
fn expected_mismatch(left: &PinCase, right: &PinCase) -> Option<EpochKind> {
    EpochKind::ALL
        .into_iter()
        .find(|kind| match left.token(*kind) {
            None => false,
            Some(pinned) => right.token(*kind) != Some(pinned),
        })
}

fn a_mismatch_is_the_first_disagreeing_kind(pair: &Pair<PinCase>) -> Result<(), String> {
    let reported = pair
        .left
        .epoch_set()
        .first_mismatch(&pair.right.epoch_set());
    let expected = expected_mismatch(&pair.left, &pair.right);
    if reported == expected {
        Ok(())
    } else {
        Err(format!(
            "first_mismatch says {reported:?}, the description says {expected:?}"
        ))
    }
}

fn a_pinned_identity_is_injective(pair: &Pair<PinCase>) -> Result<(), String> {
    injective_under(pair, &EpochKind::ALL)
}

/// The injectivity law, over an identity that names exactly `kinds`.
fn injective_under(pair: &Pair<PinCase>, kinds: &[EpochKind]) -> Result<(), String> {
    let left = ContentIdentity::of(&pinned_preimage(
        &pair.left.content_value(),
        &pair.left.epoch_set(),
        kinds,
    ));
    let right = ContentIdentity::of(&pinned_preimage(
        &pair.right.content_value(),
        &pair.right.epoch_set(),
        kinds,
    ));
    let same_identity = left == right;
    let same_artifact = pair.left.content_value() == pair.right.content_value()
        && pair.left.epoch_set() == pair.right.epoch_set();
    match (same_identity, same_artifact) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(
            "two artifacts pinned to different epochs share one identity: the identity is \
             not a function of its pinned epochs"
                .to_owned(),
        ),
        (false, true) => Err("one artifact under one set of pins has two identities".to_owned()),
    }
}

// --- the suite ---------------------------------------------------------------------------

#[test]
fn positive_a_pinned_set_is_a_function_of_its_description() {
    let plan = Plan::new("a set is its description", 0x2021_0221_0002_0001, CASES);
    check(&plan, &Pins, a_set_is_a_function_of_its_description).expect_held(&plan);
}

#[test]
fn positive_a_pinned_set_admits_itself() {
    let plan = Plan::new("reflexive admission", 0x2021_0221_0002_0002, CASES);
    check(&plan, &Pins, a_set_admits_itself).expect_held(&plan);
}

#[test]
fn positive_an_unpinned_continuation_constrains_nothing() {
    let plan = Plan::new("unpinned constrains nothing", 0x2021_0221_0002_0003, CASES);
    check(&plan, &Pins, an_unpinned_continuation_constrains_nothing).expect_held(&plan);
}

#[test]
fn positive_a_mismatch_is_the_first_disagreeing_kind() {
    let plan = Plan::new("first mismatch in ALL order", 0x2021_0221_0002_0004, CASES);
    check(
        &plan,
        &NearPairs(Pins),
        a_mismatch_is_the_first_disagreeing_kind,
    )
    .expect_held(&plan);
    let plan = Plan::new("first mismatch in ALL order", 0x2021_0221_0002_0014, CASES);
    check(
        &plan,
        &Pairs(Pins),
        a_mismatch_is_the_first_disagreeing_kind,
    )
    .expect_held(&plan);
}

#[test]
fn positive_a_pinned_identity_is_a_function_of_its_pinned_epochs() {
    let plan = Plan::new("pinned identity is injective", 0x2021_0221_0002_0005, CASES);
    check(&plan, &NearPairs(Pins), a_pinned_identity_is_injective).expect_held(&plan);
    // And over independent pairs, which is a different corpus and a different risk.
    let plan = Plan::new("pinned identity is injective", 0x2021_0221_0002_0015, CASES);
    check(&plan, &Pairs(Pins), a_pinned_identity_is_injective).expect_held(&plan);
}

#[test]
fn positive_the_digest_of_a_pinned_identity_indexes_it_faithfully() {
    // ADR-0013: the digest indexes, the canonical bytes decide. So equal identities must
    // agree on their digest — the direction an index depends on — while the identity
    // relation itself never consults one.
    let plan = Plan::new("digest agrees with identity", 0x2021_0221_0002_0006, CASES);
    check(&plan, &NearPairs(Pins), |pair: &Pair<PinCase>| {
        let left = pinned_identity(&pair.left.content_value(), &pair.left.epoch_set());
        let right = pinned_identity(&pair.right.content_value(), &pair.right.epoch_set());
        if left == right && left.digest::<Blake3Hasher>() != right.digest::<Blake3Hasher>() {
            return Err("one identity has two digests".to_owned());
        }
        if left.digest::<Blake3Hasher>() == right.digest::<Blake3Hasher>() && left != right {
            return Err(
                "two identities share a digest: a collision, or a truncated \
                        preimage"
                    .to_owned(),
            );
        }
        Ok(())
    })
    .expect_held(&plan);
}

#[test]
fn boundary_every_one_of_the_sixty_four_pin_combinations_is_distinct() {
    // Six kinds, pinned or not, is 64 combinations — the whole space, enumerated by
    // counting to it rather than sampled. Every one must be a different artifact under the
    // same content, which is the identity face of pinning at its sharpest.
    let mut identities = Vec::with_capacity(64);
    for mask in 0u32..64 {
        let mut slots = [None; 6];
        for (index, slot) in slots.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                *slot = Some(0);
            }
        }
        let case = PinCase { content: 0, slots };
        identities.push(pinned_identity(&case.content_value(), &case.epoch_set()));
    }
    identities.sort();
    let before = identities.len();
    identities.dedup();
    assert_eq!(
        identities.len(),
        before,
        "two of the 64 pin combinations produced one identity"
    );

    // And changing the *token* a kind is pinned to, with the combination fixed, is likewise
    // a different artifact — at every kind, one at a time.
    for kind in EpochKind::ALL {
        let mut seen = Vec::with_capacity(TOKENS_PER_KIND);
        for token in 0..TOKENS_PER_KIND {
            let mut slots = [None; 6];
            slots[kind_index(kind)] = Some(token);
            let case = PinCase { content: 0, slots };
            seen.push(pinned_identity(&case.content_value(), &case.epoch_set()));
        }
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "{kind:?} tokens collapsed to one identity"
        );
    }
}

// --- anti-vacuity: the suite can fail ----------------------------------------------------

/// Every epoch kind except `corpus`.
///
/// The seeded violation: an identity, or an admission check, that names five of the six
/// epochs. This is the realistic defect shape — an epoch added to the vocabulary and not
/// added to a preimage — rather than an invented one, and it is invisible to any test whose
/// fixtures never pin that epoch.
const ALL_BUT_CORPUS: [EpochKind; 5] = [
    EpochKind::Protocol,
    EpochKind::Semantic,
    EpochKind::Intent,
    EpochKind::Evidence,
    EpochKind::Proof,
];

/// This suite's own injectivity law, over an identity that forgets the corpus epoch.
fn injective_under_a_leaky_identity(pair: &Pair<PinCase>) -> Result<(), String> {
    injective_under(pair, &ALL_BUT_CORPUS)
}

/// This suite's own mismatch law, over a check that skips the corpus epoch.
fn mismatch_under_a_leaky_check(pair: &Pair<PinCase>) -> Result<(), String> {
    let reported = ALL_BUT_CORPUS.into_iter().find(|kind| {
        match (
            pair.left.epoch_set().binding(*kind),
            pair.right.epoch_set().binding(*kind),
        ) {
            (EpochBinding::Unpinned, _) => false,
            (EpochBinding::Pinned(pinned), EpochBinding::Pinned(offered)) => pinned != offered,
            (EpochBinding::Pinned(_), EpochBinding::Unpinned) => true,
        }
    });
    let expected = expected_mismatch(&pair.left, &pair.right);
    if reported == expected {
        Ok(())
    } else {
        Err(format!(
            "first_mismatch says {reported:?}, the description says {expected:?}"
        ))
    }
}

/// Two artifacts identical but for the corpus epoch: the left pins it, the right does not.
const RETAINED_LEAKY_IDENTITY: &str = "((#30 # # # # # #30) (#30 # # # # # #))";

/// The same shape, caught by the admission face instead of the identity face.
const RETAINED_LEAKY_MISMATCH: &str = "((#30 # # # # # #30) (#30 # # # # # #))";

#[test]
fn falsification_an_identity_that_ignores_the_corpus_epoch_is_caught() {
    let plan = Plan::new(
        "pinned identity is injective, over an identity that forgets `corpus`",
        0x2021_0221_0002_0005,
        CASES,
    );
    let refuted =
        check(&plan, &NearPairs(Pins), injective_under_a_leaky_identity).expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_LEAKY_IDENTITY);
    assert!(
        refuted
            .reason
            .contains("not a function of its pinned epochs"),
        "{}",
        refuted.reason
    );
    // The two cases differ in the corpus slot and in nothing else, which is what makes this
    // a *minimal* witness of the dropped epoch rather than an incidental one.
    assert_eq!(
        refuted.minimal.left.slots[..5],
        refuted.minimal.right.slots[..5]
    );
    assert_ne!(
        refuted.minimal.left.slots[5],
        refuted.minimal.right.slots[5]
    );

    // The real identity separates exactly this pair.
    assert_eq!(a_pinned_identity_is_injective(&refuted.minimal), Ok(()));

    let again =
        check(&plan, &NearPairs(Pins), injective_under_a_leaky_identity).expect_refuted(&plan);
    assert_eq!(again.minimal_repr, refuted.minimal_repr);
    assert_eq!(again.shrink_steps, refuted.shrink_steps);
}

#[test]
fn falsification_a_mismatch_check_that_skips_the_corpus_epoch_is_caught() {
    let plan = Plan::new(
        "first mismatch in ALL order, over a check that skips `corpus`",
        0x2021_0221_0002_0004,
        CASES,
    );
    let refuted =
        check(&plan, &NearPairs(Pins), mismatch_under_a_leaky_check).expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_LEAKY_MISMATCH);
    assert_eq!(
        a_mismatch_is_the_first_disagreeing_kind(&refuted.minimal),
        Ok(())
    );
}

#[test]
fn falsification_the_retained_counterexamples_replay_deterministically() {
    let reason = expect_replay_refutes::<Pair<PinCase>, _>(
        RETAINED_LEAKY_IDENTITY,
        injective_under_a_leaky_identity,
    );
    assert!(reason.contains("share one identity"), "{reason}");

    let reason = expect_replay_refutes::<Pair<PinCase>, _>(
        RETAINED_LEAKY_MISMATCH,
        mismatch_under_a_leaky_check,
    );
    assert!(reason.contains("first_mismatch says"), "{reason}");

    // The retained texts round trip, so a retention cannot drift from the case it names.
    let pair = Pair::<PinCase>::from_repr(RETAINED_LEAKY_IDENTITY).expect("parses");
    assert_eq!(pair.repr(), RETAINED_LEAKY_IDENTITY);

    // And both retained cases pass the real laws they were built to refute.
    assert_eq!(a_pinned_identity_is_injective(&pair), Ok(()));
    assert_eq!(a_mismatch_is_the_first_disagreeing_kind(&pair), Ok(()));
}
