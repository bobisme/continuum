//! **Total-order laws** — the property suite for [`Ord`] over [`Value`] and over
//! [`AssuranceLevel`] (`bn-221j`).
//!
//! # The law, and the reason it is not just "`Ord` is an `Ord`"
//!
//! `continuum-value` does not merely *have* an order; it declares one and says where it
//! comes from:
//!
//! > So [`Ord`] for [`Value`] is *defined* as lexicographic order on CVNF-1 encodings, and
//! > `Value::cmp` is the structural computation of that order — it never allocates a byte.
//! >
//! > — `crates/continuum-value/src/value.rs`, "The encoding is primary; the order is
//! >   derived"
//!
//! That makes four separate obligations, and a suite that checks only the first three is
//! checking that Rust's `derive` works:
//!
//! 1. **the order laws** — irreflexivity of `<`, duality (`a.cmp(b) == b.cmp(a).reverse()`),
//!    transitivity, and totality;
//! 2. **agreement with equality** — `cmp` answers `Equal` exactly when `PartialEq` does.
//!    This is the clause that a hash-shaped or a projection-shaped comparator fails, and it
//!    is the one ADR-0013 cares about: an order that merges two values has merged two
//!    identities;
//! 3. **the definition** — `a.cmp(b)` equals `a.encode().cmp(b.encode())`, on every pair,
//!    not on a sample. An order that disagrees with the encoding "can put a `BTreeSet` in a
//!    state its own serialization rejects" (that module's own words);
//! 4. **the consequence** — sorting by the order and sorting by the bytes produce the same
//!    sequence, which is what every canonical container in the workspace leans on.
//!
//! [`AssuranceLevel`] is the second subject, and it is checked *exhaustively* rather than
//! by generation: five variants make 25 pairs and 125 triples, so a sample would be a
//! weaker statement about a smaller space. Its order is RFC 0031's assurance ladder, and
//! `AssuranceLevel::ALL` is the declared sequence the derived `Ord` must agree with.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | `cmp` is reflexive-equal and `<` is irreflexive | [`positive_the_order_is_reflexive_on_equality_and_irreflexive_on_less`] |
//! | `a.cmp(b) == b.cmp(a).reverse()`, and `Equal` iff `==` | [`positive_the_order_is_antisymmetric_and_agrees_with_equality`] |
//! | `a <= b && b <= c` implies `a <= c` | [`positive_the_order_is_transitive`] |
//! | exactly one of `<`, `==`, `>` holds | [`positive_the_order_is_total`] |
//! | `cmp` **is** lexicographic order on CVNF-1 encodings | [`positive_the_order_is_lexicographic_order_on_encodings`] |
//! | sorting by the order equals sorting by the bytes | [`positive_sorting_by_the_order_agrees_with_sorting_by_the_bytes`] |
//! | shortlex, not `str` order: `"z" < "aa"`, at every kind | [`boundary_length_ordered_kinds_compare_shortlex`] |
//! | `AssuranceLevel`'s derived order is RFC 0031's ladder, exhaustively | [`positive_the_assurance_ladder_is_a_total_order_exhaustively`] |
//! | **the suite can fail**: a comparator that merges distinct values | [`falsification_a_length_only_comparator_is_caught_and_shrinks_to_a_two_byte_pair`] |
//! | **the suite can fail**: a cyclic comparator breaks transitivity | [`falsification_a_cyclic_comparator_is_caught_by_the_transitivity_law`] |
//! | both shrunk counterexamples are retained and replay on their own | [`falsification_the_retained_counterexamples_replay_deterministically`] |

#[path = "support/property.rs"]
mod property;
#[path = "support/value_domain.rs"]
mod value_domain;

use core::cmp::Ordering;

use continuum_value::assurance::AssuranceLevel;
use continuum_value::value::Value;

use property::{
    Case, Domain, Pair, Pairs, Plan, Prng, Triple, Triples, check, expect_replay_refutes,
};
use value_domain::{ValueCase, Values};

/// Cases per law.
const CASES: usize = 512;

// --- the laws -------------------------------------------------------------------------

fn reflexive_and_irreflexive(case: &ValueCase) -> Result<(), String> {
    let value = &case.0;
    // A separately built copy rather than `value` twice: the law is about two occurrences
    // of one value, and comparing a binding with itself is a tautology the compiler can see
    // through.
    let copy = value.clone();
    if value.cmp(&copy) != Ordering::Equal {
        return Err("a value does not compare equal to a copy of itself".to_owned());
    }
    if *value < copy {
        return Err("a value is strictly less than a copy of itself".to_owned());
    }
    if *value != copy {
        return Err("a value is not equal to a copy of itself".to_owned());
    }
    // A clone must also encode identically: the order is the encoding, so a `Clone` that
    // changed a byte would change an identity.
    if value.encode() != copy.encode() {
        return Err("a copy of a value does not share its encoding".to_owned());
    }
    Ok(())
}

fn antisymmetric_and_agrees_with_equality(pair: &Pair<ValueCase>) -> Result<(), String> {
    let (left, right) = (&pair.left.0, &pair.right.0);
    let forward = left.cmp(right);
    let backward = right.cmp(left);
    if forward != backward.reverse() {
        return Err(format!(
            "the order is not antisymmetric: {forward:?} one way, {backward:?} the other"
        ));
    }
    match (forward == Ordering::Equal, left == right) {
        (true, true) | (false, false) => {}
        (true, false) => {
            return Err(
                "two distinct values compare Equal: the order merges identities".to_owned(),
            );
        }
        (false, true) => {
            return Err("two equal values do not compare Equal".to_owned());
        }
    }
    // `PartialOrd` must not disagree with `Ord`.
    if left.partial_cmp(right) != Some(forward) {
        return Err("partial_cmp disagrees with cmp".to_owned());
    }
    Ok(())
}

/// Every one of the six arrangements of three values, so the law does not depend on the
/// order the generator happened to draw them in.
const ARRANGEMENTS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn transitive(triple: &Triple<ValueCase>) -> Result<(), String> {
    let values = [&triple.first.0, &triple.second.0, &triple.third.0];
    for [i, j, k] in ARRANGEMENTS {
        let (a, b, c) = (values[i], values[j], values[k]);
        if a <= b && b <= c && !(a <= c) {
            return Err("a <= b and b <= c but not a <= c".to_owned());
        }
        if a < b && b < c && !(a < c) {
            return Err("a < b and b < c but not a < c".to_owned());
        }
        if a == b && b == c && a != c {
            return Err("equality is not transitive".to_owned());
        }
    }
    Ok(())
}

fn total(pair: &Pair<ValueCase>) -> Result<(), String> {
    let (left, right) = (&pair.left.0, &pair.right.0);
    let less = left < right;
    let equal = left == right;
    let greater = left > right;
    match usize::from(less) + usize::from(equal) + usize::from(greater) {
        1 => Ok(()),
        count => Err(format!(
            "exactly one of <, ==, > must hold; {count} did (less={less}, equal={equal}, \
             greater={greater})"
        )),
    }
}

/// The definition: the order **is** lexicographic order on canonical encodings.
fn order_is_lexicographic_on_encodings(pair: &Pair<ValueCase>) -> Result<(), String> {
    let structural = pair.left.0.cmp(&pair.right.0);
    let by_bytes = pair.left.encoded().cmp(&pair.right.encoded());
    if structural == by_bytes {
        Ok(())
    } else {
        Err(format!(
            "the structural order says {structural:?} but the encodings say {by_bytes:?}"
        ))
    }
}

// --- the suite ------------------------------------------------------------------------

#[test]
fn positive_the_order_is_reflexive_on_equality_and_irreflexive_on_less() {
    let plan = Plan::new("reflexive and irreflexive", 0x2021_0221_0001_0001, CASES);
    check(&plan, &Values::small(), reflexive_and_irreflexive).expect_held(&plan);
}

#[test]
fn positive_the_order_is_antisymmetric_and_agrees_with_equality() {
    let plan = Plan::new("antisymmetric, Equal iff ==", 0x2021_0221_0001_0002, CASES);
    check(
        &plan,
        &Pairs(Values::small()),
        antisymmetric_and_agrees_with_equality,
    )
    .expect_held(&plan);
}

#[test]
fn positive_the_order_is_transitive() {
    let plan = Plan::new("transitive", 0x2021_0221_0001_0003, CASES);
    check(&plan, &Triples(Values::small()), transitive).expect_held(&plan);
}

#[test]
fn positive_the_order_is_total() {
    let plan = Plan::new("trichotomy", 0x2021_0221_0001_0004, CASES);
    check(&plan, &Pairs(Values::small()), total).expect_held(&plan);
}

#[test]
fn positive_the_order_is_lexicographic_order_on_encodings() {
    let plan = Plan::new("cmp == bytes.cmp", 0x2021_0221_0001_0005, CASES);
    check(
        &plan,
        &Pairs(Values::small()),
        order_is_lexicographic_on_encodings,
    )
    .expect_held(&plan);
}

#[test]
fn positive_sorting_by_the_order_agrees_with_sorting_by_the_bytes() {
    // A corpus, not a pair: the pairwise law is what makes this hold, and this is the form
    // every canonical container in the workspace actually relies on.
    let plan = Plan::new("sorting agrees", 0x2021_0221_0001_0006, 64);
    let domain = Values::small();
    let mut rng = Prng::new(plan.seed);
    for round in 0..plan.cases {
        let mut values: Vec<Value> = Vec::with_capacity(32);
        for _ in 0..32 {
            values.push(domain.generate(&mut rng).0);
        }

        let mut by_order = values.clone();
        by_order.sort();

        let mut by_bytes = values.clone();
        by_bytes.sort_by_key(Value::encode);

        assert_eq!(
            by_order, by_bytes,
            "round {round}: sorting by Ord and by CVNF-1 bytes disagreed"
        );

        // And the sorted sequence's encodings are themselves ascending, which is the fact a
        // canonical `Set`/`Map` encoding depends on.
        let encodings: Vec<Vec<u8>> = by_order.iter().map(Value::encode).collect();
        assert!(
            encodings.windows(2).all(|pair| pair[0] <= pair[1]),
            "round {round}: the sorted encodings are not ascending"
        );
    }
}

#[test]
fn boundary_length_ordered_kinds_compare_shortlex() {
    // The module documentation calls this out as the surprising consequence: length first,
    // contents second, so `"z" < "aa"` — which is *not* what `str`'s `Ord` says. Pinned at
    // every kind whose payload is a blob, in both the order and the encoding.
    assert!("z" > "aa", "the premise: Rust's str order disagrees");

    let pairs: [(Value, Value); 4] = [
        (Value::text("z"), Value::text("aa")),
        (
            Value::bytes(b"\xff".to_vec()),
            Value::bytes(b"\x00\x00".to_vec()),
        ),
        (
            Value::Symbol(value_domain::name("z")),
            Value::Symbol(value_domain::name("aa")),
        ),
        (
            Value::seq([Value::nat(u128::MAX)]).expect("one element"),
            Value::seq([Value::nat(0), Value::nat(0)]).expect("two elements"),
        ),
    ];
    for (short, long) in pairs {
        assert!(short < long, "shortlex: {short:?} must precede {long:?}");
        assert!(
            short.encode() < long.encode(),
            "and the encodings must agree: {short:?} before {long:?}"
        );
    }
}

#[test]
fn positive_the_assurance_ladder_is_a_total_order_exhaustively() {
    // Five variants: 25 pairs and 125 triples is the whole space, so this is exhaustive
    // rather than sampled — a stronger statement than any generated corpus could make.
    let ladder = AssuranceLevel::ALL;
    assert_eq!(ladder.len(), 5);

    // The derived order is the declaration order, which is RFC 0031's ladder.
    for (index, level) in ladder.iter().enumerate() {
        for (other_index, other) in ladder.iter().enumerate() {
            assert_eq!(
                level.cmp(other),
                index.cmp(&other_index),
                "{level} vs {other} disagrees with the declared ladder"
            );
            assert_eq!(level.cmp(other), other.cmp(level).reverse());
            assert_eq!((level == other), (index == other_index));
            assert_eq!(
                usize::from(level < other)
                    + usize::from(level == other)
                    + usize::from(level > other),
                1,
                "trichotomy fails at {level} vs {other}"
            );
        }
    }
    for a in ladder {
        for b in ladder {
            for c in ladder {
                if a <= b && b <= c {
                    assert!(a <= c, "transitivity fails at {a} <= {b} <= {c}");
                }
            }
        }
    }
    // The weakest and strongest are the ends, so "at least `minimum`" is a real filter.
    assert_eq!(ladder.iter().min(), Some(&AssuranceLevel::Observed));
    assert_eq!(ladder.iter().max(), Some(&AssuranceLevel::Proved));
}

// --- anti-vacuity: the suite can fail --------------------------------------------------

/// A comparator that looks only at the *length* of an encoding.
///
/// The shape of a real defect: it is reflexive, antisymmetric, transitive and total — every
/// law a careless suite checks — and it is still wrong, because it answers `Equal` for two
/// values that are not equal. That is the identity merge ADR-0013 exists to prevent, and it
/// is exactly what clause 2 of this suite is for.
fn length_only_cmp(left: &Value, right: &Value) -> Ordering {
    left.encode().len().cmp(&right.encode().len())
}

/// A comparator with no transitivity at all: a rock-paper-scissors tournament over the kind
/// tags, which is reflexive, antisymmetric and total and still not an order.
///
/// `Value` has sixteen kinds, so "b is within the next seven tags, cyclically" gives
/// `text < record` (tags 7 and 13), `record < null` (13 and 1, wrapping), and
/// `null < text` (1 and 7). Every *pairwise* law holds; only the triple law can see it.
/// That is why [`positive_the_order_is_transitive`] draws triples rather than trusting the
/// pairwise ones. The antipodal step of exactly eight falls back to the real order, which is
/// what keeps antisymmetry — a broken comparator that also failed a pairwise law would
/// prove less.
fn cyclic_cmp(left: &Value, right: &Value) -> Ordering {
    let step = i32::from(right.kind().tag()) - i32::from(left.kind().tag());
    match step.rem_euclid(16) {
        0 | 8 => left.cmp(right),
        1..=7 => Ordering::Less,
        _ => Ordering::Greater,
    }
}

/// This suite's own equality-agreement law, over the length-only comparator.
fn agrees_with_equality_under_length_only(pair: &Pair<ValueCase>) -> Result<(), String> {
    let (left, right) = (&pair.left.0, &pair.right.0);
    match (
        length_only_cmp(left, right) == Ordering::Equal,
        left == right,
    ) {
        (true, false) => {
            Err("two distinct values compare Equal: the order merges identities".to_owned())
        }
        (false, true) => Err("two equal values do not compare Equal".to_owned()),
        _ => Ok(()),
    }
}

/// This suite's own transitivity law, over the cyclic comparator.
fn transitive_under_cyclic(triple: &Triple<ValueCase>) -> Result<(), String> {
    let (a, b, c) = (&triple.first.0, &triple.second.0, &triple.third.0);
    let le = |x: &Value, y: &Value| cyclic_cmp(x, y) != Ordering::Greater;
    if le(a, b) && le(b, c) && !le(a, c) {
        return Err("a <= b and b <= c but not a <= c".to_owned());
    }
    Ok(())
}

/// `(Set{}, Tuple())`: the empty set and the empty tuple, whose encodings are `0b 00` and
/// `09 00` — two bytes each, and different values.
///
/// Minimal by construction as well as by search: `Null` is the only one-byte encoding, so
/// no pair of *distinct* values can be smaller than two bytes each.
const RETAINED_LENGTH_ONLY: &str = "(#0b00 #0900)";

/// `(Tuple(), Map{}, Null)`: kind tags 9, 15 and 1, which form a cycle under [`cyclic_cmp`]
/// — 9 precedes 15 (a cyclic step of 6) and 15 precedes 1 (a step of 2), while 9 against 1
/// is the antipodal step of 8 and falls back to the real order, which puts `Null` first. So
/// `a <= b` and `b <= c` and `c < a`, and no pairwise law can see it.
const RETAINED_CYCLIC: &str = "(#0900 #0f00 #01)";

#[test]
fn falsification_a_length_only_comparator_is_caught_and_shrinks_to_a_two_byte_pair() {
    let plan = Plan::new(
        "Equal iff ==, over a length-only comparator",
        0x2021_0221_0001_0002,
        CASES,
    );
    let refuted = check(
        &plan,
        &Pairs(Values::small()),
        agrees_with_equality_under_length_only,
    )
    .expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_LENGTH_ONLY);
    assert_eq!(refuted.minimal_size, 4, "two two-byte encodings");
    assert_ne!(refuted.minimal.left.0, refuted.minimal.right.0);
    assert_eq!(
        refuted.minimal.left.encoded().len(),
        refuted.minimal.right.encoded().len()
    );

    // The real comparator passes the very case that refutes the broken one.
    assert_eq!(
        antisymmetric_and_agrees_with_equality(&refuted.minimal),
        Ok(())
    );

    // Same plan, same refutation.
    let again = check(
        &plan,
        &Pairs(Values::small()),
        agrees_with_equality_under_length_only,
    )
    .expect_refuted(&plan);
    assert_eq!(again.minimal_repr, refuted.minimal_repr);
    assert_eq!(again.shrink_steps, refuted.shrink_steps);
}

#[test]
fn falsification_a_cyclic_comparator_is_caught_by_the_transitivity_law() {
    let plan = Plan::new(
        "transitive, over a cyclic comparator",
        0x2021_0221_0001_0003,
        CASES,
    );
    let refuted =
        check(&plan, &Triples(Values::small()), transitive_under_cyclic).expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_CYCLIC);
    assert_eq!(
        transitive(&refuted.minimal),
        Ok(()),
        "the real order is transitive here"
    );

    // Every *pairwise* law still holds of the cyclic comparator on this triple: the
    // counterexample is only visible at arity three, which is the point of drawing triples.
    let pairs = [
        (&refuted.minimal.first.0, &refuted.minimal.second.0),
        (&refuted.minimal.second.0, &refuted.minimal.third.0),
        (&refuted.minimal.third.0, &refuted.minimal.first.0),
    ];
    for (left, right) in pairs {
        assert_eq!(
            cyclic_cmp(left, right),
            cyclic_cmp(right, left).reverse(),
            "the cyclic comparator is still antisymmetric on {left:?} vs {right:?}"
        );
    }
}

#[test]
fn falsification_the_retained_counterexamples_replay_deterministically() {
    let reason = expect_replay_refutes::<Pair<ValueCase>, _>(
        RETAINED_LENGTH_ONLY,
        agrees_with_equality_under_length_only,
    );
    assert!(reason.contains("merges identities"), "{reason}");

    let reason =
        expect_replay_refutes::<Triple<ValueCase>, _>(RETAINED_CYCLIC, transitive_under_cyclic);
    assert!(reason.contains("not a <= c"), "{reason}");

    // The retained texts are the cases' own canonical encodings, so a retention cannot
    // drift from the case it names.
    let pair = Pair::<ValueCase>::from_repr(RETAINED_LENGTH_ONLY).expect("parses");
    assert_eq!(pair.repr(), RETAINED_LENGTH_ONLY);
    let triple = Triple::<ValueCase>::from_repr(RETAINED_CYCLIC).expect("parses");
    assert_eq!(triple.repr(), RETAINED_CYCLIC);

    // And both retained cases pass the *real* laws they were built to refute.
    assert_eq!(antisymmetric_and_agrees_with_equality(&pair), Ok(()));
    assert_eq!(transitive(&triple), Ok(()));
}
