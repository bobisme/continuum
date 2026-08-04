//! The property harness's own evidence: seeded reproducibility, a lossless counterexample
//! text, a shrinker that reaches the exact boundary, and termination against a hostile
//! `Domain` (`bn-221j`).
//!
//! # Why a harness needs its own suite
//!
//! Every Phase A property suite makes two claims that are *about the harness*, not about
//! the code under test:
//!
//! 1. a refuted law shrinks to a **minimal** counterexample, and
//! 2. that counterexample is **retained** and **replays deterministically**.
//!
//! A suite can only demonstrate those against its own subject, where "minimal" is a
//! judgement about a `Value` or a Merkle tree and the reader has to take the shrinker's word
//! for it. Here the subject is arithmetic, the true minimum is known in advance by
//! inspection, and the assertion is an equality against it. So this file is where
//! `shrink` is shown to *land on* the boundary rather than merely near it, and where the
//! termination guarantee is exercised against a `Domain` that is actively trying to loop.
//!
//! It lives in `continuum-value` because that is where the shared harness file lives; it
//! links nothing from the crate under test, and `cargo test -p continuum-value --locked`
//! runs it beside the three suites that do.
//!
//! | Claim | Test |
//! |---|---|
//! | a seed reproduces its draw, and different seeds differ | [`positive_a_seed_reproduces_its_draw_exactly`] |
//! | the counterexample text round trips, and rejects every near miss | [`positive_the_counterexample_text_round_trips_and_is_strict`] |
//! | a true law holds; a false one shrinks to the exact boundary | [`positive_a_true_law_holds_and_a_false_one_shrinks_to_the_boundary`] |
//! | the same plan produces the same refutation twice | [`positive_a_refutation_is_a_function_of_the_plan`] |
//! | a retained counterexample replays with no generator at all | [`positive_a_retained_counterexample_replays_without_the_generator`] |
//! | pairs and triples shrink jointly, down to their true minimum | [`positive_pairs_and_triples_shrink_to_their_component_minimum`] |
//! | a `Domain` whose candidates never shrink still terminates | [`adversarial_the_shrinker_terminates_when_a_domain_never_gets_smaller`] |
//! | a law that cannot fail is reported as such, not passed over | [`negative_a_law_that_cannot_be_refuted_is_a_failure_not_a_pass`] |

#[path = "support/property.rs"]
mod property;

use property::{
    Case, Domain, Pair, Pairs, Plan, Prng, Report, Sexp, Triple, Triples, check,
    expect_replay_refutes, replay,
};

// --- a subject whose minimum is known by inspection -----------------------------------

/// A number, as a case. The text form is the decimal spelling's bytes, so `11` renders as
/// `#3131` — hex of ASCII `1`, `1`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Number(u128);

impl Case for Number {
    fn to_sexp(&self) -> Sexp {
        Sexp::number(self.0)
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        sexp.as_number().map(Self)
    }
}

/// Numbers below a thousand, shrinking towards zero.
#[derive(Debug, Clone, Copy)]
struct Numbers;

impl Domain for Numbers {
    type Item = Number;

    fn generate(&self, rng: &mut Prng) -> Number {
        Number(u128::try_from(rng.below(1000)).unwrap_or(0))
    }

    fn shrink(&self, item: &Number) -> Vec<Number> {
        if item.0 == 0 {
            return Vec::new();
        }
        // Zero first (the most aggressive reduction), then a halving, then a decrement —
        // so a law refuted only above a threshold walks down to exactly the threshold.
        vec![Number(0), Number(item.0 / 2), Number(item.0 - 1)]
    }

    fn size(&self, item: &Number) -> usize {
        usize::try_from(item.0).unwrap_or(usize::MAX)
    }
}

/// Refuted exactly above ten, so `11` is the unique minimum.
fn above_ten(case: &Number) -> Result<(), String> {
    if case.0 <= 10 {
        Ok(())
    } else {
        Err(format!("{} is above ten", case.0))
    }
}

// --- evidence -------------------------------------------------------------------------

#[test]
fn positive_a_seed_reproduces_its_draw_exactly() {
    let draw = |seed: u64| {
        let mut rng = Prng::new(seed);
        (0..16).map(|_| rng.next_u64()).collect::<Vec<_>>()
    };
    for seed in [0u64, 1, 7, 0x2211_dead_beef, u64::MAX] {
        assert_eq!(draw(seed), draw(seed), "seed {seed} did not reproduce");
    }
    // Zero is splitmix64's ordinary seed, not a fixed point: the reason this harness does
    // not use the xorshift `deterministic_ordering.rs` has to special-case.
    assert_ne!(draw(0), draw(1));
    assert!(draw(0).iter().all(|word| *word != 0));
}

#[test]
fn positive_the_counterexample_text_round_trips_and_is_strict() {
    let cases = [
        Sexp::Atom(Vec::new()),
        Sexp::List(Vec::new()),
        Sexp::atom(vec![0x00, 0xff, 0x7f]),
        Sexp::text("hello"),
        Sexp::number(11),
        Sexp::list([
            Sexp::text("a"),
            Sexp::list([Sexp::number(42), Sexp::Atom(vec![])]),
        ]),
    ];
    for case in cases {
        let text = case.render();
        assert_eq!(
            Sexp::parse(&text).as_ref(),
            Some(&case),
            "did not round trip: {text}"
        );
    }

    // One text, one tree: every near miss of the canonical rendering is refused rather
    // than repaired, which is the discipline the encodings under test are held to.
    for hostile in [
        "#0",         // an odd hex run
        "#0g",        // a non-hex digit
        "#00 ",       // a trailing separator
        "#00#01",     // two values, one text
        "(",          // unterminated
        "(#00",       // unbalanced
        "(#00  #01)", // a doubled separator
        "( #00)",     // a leading separator
        "(#00 )",     // a trailing separator inside a list
        "#0A",        // uppercase hex is a second spelling
        "",           // empty
        " ",          // whitespace is not a document
    ] {
        assert_eq!(
            Sexp::parse(hostile),
            None,
            "accepted a non-canonical text: {hostile:?}"
        );
    }

    // Nesting past the bound is refused rather than recursed into: linear input, bounded
    // parser. Built by counting up, never by doubling.
    let deep_enough = format!("{}{}", "(".repeat(60), ")".repeat(60));
    assert!(
        Sexp::parse(&deep_enough).is_some(),
        "60 deep is inside the bound"
    );
    let too_deep = format!("{}{}", "(".repeat(200), ")".repeat(200));
    assert_eq!(Sexp::parse(&too_deep), None, "200 deep is past the bound");
}

#[test]
fn positive_a_true_law_holds_and_a_false_one_shrinks_to_the_boundary() {
    let plan = Plan::new("every drawn number is below a thousand", 7, 256);
    let cases = check(&plan, &Numbers, |case: &Number| {
        if case.0 < 1000 {
            Ok(())
        } else {
            Err(format!("{} is not below a thousand", case.0))
        }
    })
    .expect_held(&plan);
    assert_eq!(cases, 256);

    let refuted = check(&plan, &Numbers, above_ten).expect_refuted(&plan);
    assert_eq!(
        refuted.minimal,
        Number(11),
        "the shrinker must land on the boundary, not near it"
    );
    assert_eq!(refuted.minimal_repr, "#3131");
    assert_eq!(refuted.reason, "11 is above ten");
    assert!(
        refuted.shrink_steps > 0,
        "the drawn case was not already minimal"
    );
    assert!(
        refuted.minimal_size < refuted.generated_size,
        "shrinking must strictly reduce size: {} -> {}",
        refuted.generated_size,
        refuted.minimal_size
    );
}

#[test]
fn positive_a_refutation_is_a_function_of_the_plan() {
    let plan = Plan::new("nothing is above ten", 20_260_801, 128);
    let first = check(&plan, &Numbers, above_ten).expect_refuted(&plan);
    let second = check(&plan, &Numbers, above_ten).expect_refuted(&plan);
    assert_eq!(first.case_index, second.case_index);
    assert_eq!(first.generated, second.generated);
    assert_eq!(first.minimal_repr, second.minimal_repr);
    assert_eq!(first.shrink_steps, second.shrink_steps);
}

#[test]
fn positive_a_retained_counterexample_replays_without_the_generator() {
    // No `Plan`, no `Prng`, no `Domain`: a pinned string, parsed, and handed to the law.
    assert_eq!(
        expect_replay_refutes::<Number, _>("#3131", above_ten),
        "11 is above ten"
    );
    assert_eq!(replay::<Number, _>("#39", above_ten), Ok(()));
}

#[test]
fn positive_pairs_and_triples_shrink_to_their_component_minimum() {
    let plan = Plan::new("no pair sums above ten", 99, 256);
    let refuted = check(&plan, &Pairs(Numbers), |pair: &Pair<Number>| {
        if pair.left.0 + pair.right.0 <= 10 {
            Ok(())
        } else {
            Err(format!("{} + {} is above ten", pair.left.0, pair.right.0))
        }
    })
    .expect_refuted(&plan);
    assert_eq!(
        refuted.minimal_size, 11,
        "the sum is driven down to the boundary"
    );
    assert_eq!(
        Pair::<Number>::from_repr(&refuted.minimal_repr).as_ref(),
        Some(&refuted.minimal),
        "a pair's text form is lossless"
    );

    let plan = Plan::new("no triple sums above ten", 99, 256);
    let refuted = check(&plan, &Triples(Numbers), |triple: &Triple<Number>| {
        let sum = triple.first.0 + triple.second.0 + triple.third.0;
        if sum <= 10 {
            Ok(())
        } else {
            Err(format!("the sum {sum} is above ten"))
        }
    })
    .expect_refuted(&plan);
    assert_eq!(refuted.minimal_size, 11);
    assert_eq!(
        Triple::<Number>::from_repr(&refuted.minimal_repr).as_ref(),
        Some(&refuted.minimal),
    );
}

#[test]
fn adversarial_the_shrinker_terminates_when_a_domain_never_gets_smaller() {
    /// A `Domain` whose candidates are never smaller than what they replace, and one of
    /// which is the item itself — the shape that turns a naive greedy shrinker into an
    /// infinite loop.
    struct Stuck;

    impl Domain for Stuck {
        type Item = Number;

        fn generate(&self, rng: &mut Prng) -> Number {
            Number(rng.below(8) as u128)
        }

        fn shrink(&self, item: &Number) -> Vec<Number> {
            vec![item.clone(), Number(item.0 + 1), Number(item.0)]
        }

        fn size(&self, item: &Number) -> usize {
            item.0 as usize
        }
    }

    let plan = Plan::new("a hostile domain still terminates", 3, 4);
    let refuted = check(&plan, &Stuck, |_: &Number| Err("always".to_owned())).expect_refuted(&plan);
    assert_eq!(
        refuted.shrink_steps, 0,
        "no candidate is strictly smaller, so none is accepted"
    );
    assert_eq!(refuted.minimal, refuted.generated);
}

#[test]
fn negative_a_law_that_cannot_be_refuted_is_a_failure_not_a_pass() {
    // `expect_refuted` is what every suite's anti-vacuity test calls. If it were lenient,
    // a seeded violation the suite silently missed would read as evidence. It is not.
    let plan = Plan::new("this law is never refuted", 1, 8);
    let report: Report<Number> = check(&plan, &Numbers, |_: &Number| Ok(()));
    let outcome = std::panic::catch_unwind(move || report.expect_refuted(&plan));
    let payload = outcome.expect_err("a law that held must not satisfy `expect_refuted`");
    let message = payload
        .downcast_ref::<String>()
        .expect("the harness panics with a String");
    assert!(
        message.contains("the suite cannot fail"),
        "the report must say why a green run is not evidence: {message}"
    );
}
