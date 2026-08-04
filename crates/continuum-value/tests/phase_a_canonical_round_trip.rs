//! **Canonical round trips** — the CVNF-1 property suite (`bn-221j`).
//!
//! # The law
//!
//! > Strict by construction: every non-canonical spelling of a value is an error, never a
//! > repair. […] Two byte strings therefore never decode to one value, which is what makes
//! > the encoding usable as an identity.
//! >
//! > — [`Value::decode`]'s own contract, `crates/continuum-value/src/value.rs`
//!
//! and the reason it matters, one level up:
//!
//! > Canonical structural encodings define identity. Hashes index and partition; collisions
//! > resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`
//!
//! "Round trip" is therefore two claims, not one, and only the pair is worth anything:
//!
//! - **`decode ∘ encode = id`** — nothing is lost on the way out. A suite that stops here
//!   is satisfied by any self-consistent pair of functions, including a permissive parser.
//! - **`encode ∘ decode = id` on canonical bytes** — nothing is *gained* on the way back.
//!   Together with the first, this says the encoding is a bijection onto its image, which is
//!   what lets a byte string *be* an identity.
//!
//! and the operational form of injectivity, which is what a permissive parser actually
//! fails: **no perturbation of a canonical encoding decodes to the value it came from.**
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | `decode(encode(v)) == v` | [`positive_decoding_an_encoding_returns_the_value`] |
//! | `encode(decode(b)) == b` for canonical `b` | [`positive_re_encoding_a_decoded_value_reproduces_its_bytes`] |
//! | a content identity is the canonical bytes, both directions | [`positive_content_identity_round_trips_through_its_canonical_bytes`] |
//! | `encode` is injective: equal bytes iff equal values | [`positive_the_encoding_is_injective_on_values`] |
//! | no single-byte mutation decodes back to the same value | [`adversarial_no_single_byte_mutation_decodes_to_the_same_value`] |
//! | no proper prefix and no extension decodes at all | [`adversarial_no_truncation_or_extension_decodes`] |
//! | the depth and width limits round trip, and past them is refused | [`boundary_the_declared_depth_and_width_limits_round_trip`] |
//! | **the suite can fail**: an encoder that spells one kind as another | [`falsification_a_kind_confusing_encoder_is_caught_and_shrinks_to_nat_zero`] |
//! | the shrunk counterexample is retained and replays on its own | [`falsification_the_retained_counterexample_replays_deterministically`] |
//!
//! # Determinism
//!
//! Every generated corpus is a function of a `seed` written into a [`Plan`] in this file and
//! of nothing else (INV-005). The adversarial cases are constructed linearly — one element
//! or one level per step — never by doubling.

#[path = "support/property.rs"]
mod property;
#[path = "support/value_domain.rs"]
mod value_domain;

use continuum_value::identity::ContentIdentity;
use continuum_value::value::{MAX_DEPTH, Value, ValueError};

use property::{Case, Domain, Pair, Pairs, Plan, Prng, check, expect_replay_refutes};
use value_domain::{ValueCase, Values, nested, wide_set};

/// Cases per law. Large enough that every kind and every alphabet entry is drawn many
/// times over, small enough that the suite is instant.
const CASES: usize = 512;

// --- the laws -------------------------------------------------------------------------

/// `decode(encode(v)) == v`.
fn decodes_back(case: &ValueCase) -> Result<(), String> {
    match Value::decode(&case.encoded()) {
        Ok(decoded) if decoded == case.0 => Ok(()),
        Ok(decoded) => Err(format!(
            "decoded to a different value: {decoded:?} != {:?}",
            case.0
        )),
        Err(error) => Err(format!("its own encoding did not decode: {error}")),
    }
}

/// `encode(decode(b)) == b` for `b = encode(v)`.
fn re_encodes_identically(case: &ValueCase) -> Result<(), String> {
    let bytes = case.encoded();
    let decoded = Value::decode(&bytes).map_err(|error| format!("did not decode: {error}"))?;
    let again = decoded.encode();
    if again == bytes {
        Ok(())
    } else {
        Err(format!(
            "re-encoding changed the bytes: {} -> {}",
            hex(&bytes),
            hex(&again)
        ))
    }
}

/// A content identity is exactly the canonical encoding, in both directions.
fn identity_is_the_canonical_bytes(case: &ValueCase) -> Result<(), String> {
    let bytes = case.encoded();
    let identity = ContentIdentity::of(&case.0);
    if identity.canonical_bytes() != bytes.as_slice() {
        return Err("the identity is not the canonical encoding".to_owned());
    }
    let recovered = ContentIdentity::from_canonical_bytes(&bytes)
        .map_err(|error| format!("the identity did not decode: {error}"))?;
    if recovered != identity {
        return Err("recovering the identity from its bytes changed it".to_owned());
    }
    match identity.to_value() {
        Ok(value) if value == case.0 => Ok(()),
        Ok(value) => Err(format!("the identity denotes {value:?}, not {:?}", case.0)),
        Err(error) => Err(format!("the identity did not decode: {error}")),
    }
}

/// Equal encodings iff equal values.
fn encoding_is_injective(pair: &Pair<ValueCase>) -> Result<(), String> {
    let same_bytes = pair.left.encoded() == pair.right.encoded();
    let same_value = pair.left.0 == pair.right.0;
    if same_bytes == same_value {
        Ok(())
    } else if same_bytes {
        Err("two different values share one encoding".to_owned())
    } else {
        Err("one value has two encodings".to_owned())
    }
}

/// No single-byte perturbation decodes back to the value it came from.
///
/// Linear in the encoding: two mutations per byte position, one `push` each, no doubling.
fn no_mutation_decodes_to_the_same_value(case: &ValueCase) -> Result<(), String> {
    let bytes = case.encoded();
    for position in 0..bytes.len() {
        for delta in [0x01u8, 0x80] {
            let mut mutated = bytes.clone();
            mutated[position] ^= delta;
            if mutated == bytes {
                continue;
            }
            if Value::decode(&mutated) == Ok(case.0.clone()) {
                return Err(format!(
                    "flipping {delta:#04x} at byte {position} still decodes to the same value"
                ));
            }
        }
    }
    Ok(())
}

/// No proper prefix decodes, and no extension decodes.
fn no_truncation_or_extension_decodes(case: &ValueCase) -> Result<(), String> {
    let bytes = case.encoded();
    for cut in 0..bytes.len() {
        if let Ok(value) = Value::decode(&bytes[..cut]) {
            return Err(format!(
                "a {cut}-byte prefix of a {}-byte encoding decoded to {value:?}",
                bytes.len()
            ));
        }
    }
    for suffix in [0x00u8, 0x01, 0xff] {
        let mut extended = bytes.clone();
        extended.push(suffix);
        if let Ok(value) = Value::decode(&extended) {
            return Err(format!(
                "an encoding with a trailing {suffix:#04x} decoded to {value:?}"
            ));
        }
    }
    Ok(())
}

// --- the suite ------------------------------------------------------------------------

#[test]
fn positive_decoding_an_encoding_returns_the_value() {
    let plan = Plan::new("decode(encode(v)) == v", 0x2021_0221_0000_0001, CASES);
    let drawn = check(&plan, &Values::small(), decodes_back).expect_held(&plan);
    assert_eq!(drawn, CASES);
}

#[test]
fn positive_re_encoding_a_decoded_value_reproduces_its_bytes() {
    let plan = Plan::new("encode(decode(b)) == b", 0x2021_0221_0000_0002, CASES);
    check(&plan, &Values::small(), re_encodes_identically).expect_held(&plan);
}

#[test]
fn positive_content_identity_round_trips_through_its_canonical_bytes() {
    let plan = Plan::new("identity == canonical bytes", 0x2021_0221_0000_0003, CASES);
    check(&plan, &Values::small(), identity_is_the_canonical_bytes).expect_held(&plan);
}

#[test]
fn positive_the_encoding_is_injective_on_values() {
    let plan = Plan::new("encode is injective", 0x2021_0221_0000_0004, CASES);
    check(&plan, &Pairs(Values::small()), encoding_is_injective).expect_held(&plan);
}

#[test]
fn adversarial_no_single_byte_mutation_decodes_to_the_same_value() {
    let plan = Plan::new(
        "no mutation is a second spelling",
        0x2021_0221_0000_0005,
        CASES,
    );
    check(
        &plan,
        &Values::small(),
        no_mutation_decodes_to_the_same_value,
    )
    .expect_held(&plan);
}

#[test]
fn adversarial_no_truncation_or_extension_decodes() {
    let plan = Plan::new(
        "the encoding is self-delimiting",
        0x2021_0221_0000_0006,
        CASES,
    );
    check(&plan, &Values::small(), no_truncation_or_extension_decodes).expect_held(&plan);
}

#[test]
fn boundary_the_declared_depth_and_width_limits_round_trip() {
    // Depth: exactly at the bound, built by counting to it.
    let deepest = ValueCase(nested(MAX_DEPTH));
    assert_eq!(deepest.0.depth(), MAX_DEPTH);
    assert_eq!(decodes_back(&deepest), Ok(()));
    assert_eq!(re_encodes_identically(&deepest), Ok(()));

    // One past the bound is refused at construction rather than encoded and rejected later.
    let too_deep = Value::seq([nested(MAX_DEPTH)]);
    assert_eq!(
        too_deep,
        Err(ValueError::DepthExceeded {
            depth: MAX_DEPTH + 1,
            max: MAX_DEPTH,
        })
    );

    // And a byte string that *claims* one level past the bound is refused by the decoder,
    // which is the direction that matters for untrusted input. Built linearly: one `seq`
    // header per level, then the innermost scalar.
    let mut hostile: Vec<u8> = Vec::new();
    for _ in 0..MAX_DEPTH {
        hostile.push(0x0a); // seq
        hostile.push(0x01); // nat(1): one item
        hostile.push(0x01);
    }
    hostile.push(0x01); // null
    assert!(
        Value::decode(&hostile).is_err(),
        "the decoder must bound its own recursion on untrusted bytes"
    );

    // Width: a length byte carries far more than a byte's worth of elements, so a wide
    // collection is a round-trip case rather than a limit. Linear construction.
    for width in [0usize, 1, 255, 256, 1_024] {
        let wide = ValueCase(wide_set(width));
        assert_eq!(decodes_back(&wide), Ok(()), "a {width}-element set");
        assert_eq!(
            re_encodes_identically(&wide),
            Ok(()),
            "a {width}-element set"
        );
    }
}

// --- anti-vacuity: the suite can fail --------------------------------------------------

/// An encoder with one bug: a `nat` is spelled with the `int` kind's tag and payload.
///
/// This is the shape of a real defect rather than a strawman — `Value::Nat`'s own
/// documentation names it as *the* hazard ("silently promoting `Nat(3)` to `Int(3)` would
/// make identity depend on the coercion direction, which is exactly the ambiguity ADR-0013
/// exists to remove"). Nothing in `src/` is touched: the broken seam is this function, and
/// the suite's own law is run against it unchanged.
fn kind_confusing_encode(value: &Value) -> Vec<u8> {
    match value {
        Value::Nat(magnitude) => i128::try_from(*magnitude)
            .map_or_else(|_| value.encode(), |as_int| Value::int(as_int).encode()),
        other => other.encode(),
    }
}

/// The suite's own round-trip law, over the broken encoder.
fn decodes_back_through_the_broken_encoder(case: &ValueCase) -> Result<(), String> {
    match Value::decode(&kind_confusing_encode(&case.0)) {
        Ok(decoded) if decoded == case.0 => Ok(()),
        Ok(decoded) => Err(format!(
            "decoded to a different value: {decoded:?} != {:?}",
            case.0
        )),
        Err(error) => Err(format!("its own encoding did not decode: {error}")),
    }
}

/// The minimal counterexample the shrinker reaches, retained verbatim.
///
/// `#0300` is the CVNF-1 encoding of `Value::Nat(0)`: kind tag `0x03`, then `nat(0)` as a
/// zero-length magnitude. The retention is the identity of the counterexample, not a
/// rendering of it — see `tests/support/value_domain.rs`.
const RETAINED_KIND_CONFUSION: &str = "#0300";

#[test]
fn falsification_a_kind_confusing_encoder_is_caught_and_shrinks_to_nat_zero() {
    let plan = Plan::new(
        "decode(encode(v)) == v, over a kind-confusing encoder",
        0x2021_0221_0000_0001,
        CASES,
    );
    let refuted = check(
        &plan,
        &Values::small(),
        decodes_back_through_the_broken_encoder,
    )
    .expect_refuted(&plan);

    assert_eq!(
        refuted.minimal_repr, RETAINED_KIND_CONFUSION,
        "the shrinker must reach the smallest natural, and reach it every time"
    );
    assert_eq!(refuted.minimal.0, Value::nat(0));
    assert!(
        refuted.minimal_size < refuted.generated_size,
        "shrinking must strictly reduce the case: {} -> {}",
        refuted.generated_size,
        refuted.minimal_size
    );

    // The same plan, twice, is the same refutation: a counterexample is a fact about the
    // seed, not about the run.
    let again = check(
        &plan,
        &Values::small(),
        decodes_back_through_the_broken_encoder,
    )
    .expect_refuted(&plan);
    assert_eq!(again.minimal_repr, refuted.minimal_repr);
    assert_eq!(again.case_index, refuted.case_index);
    assert_eq!(again.shrink_steps, refuted.shrink_steps);

    // And the real encoder is not the broken one: the same case passes the real law.
    assert_eq!(decodes_back(&refuted.minimal), Ok(()));
}

#[test]
fn falsification_the_retained_counterexample_replays_deterministically() {
    // No plan, no seed, no generator: the pinned string, parsed through the strict decoder,
    // handed to the same law.
    let reason = expect_replay_refutes::<ValueCase, _>(
        RETAINED_KIND_CONFUSION,
        decodes_back_through_the_broken_encoder,
    );
    assert!(
        reason.contains("Int(0)"),
        "the retained case must still fail for its original reason: {reason}"
    );

    // The retained text is the value's own canonical encoding, so the retention cannot
    // drift from the case it names.
    let case = ValueCase::from_repr(RETAINED_KIND_CONFUSION).expect("the retained case parses");
    assert_eq!(case.0, Value::nat(0));
    assert_eq!(case.repr(), RETAINED_KIND_CONFUSION);
}

#[test]
fn falsification_a_generator_that_never_draws_the_broken_kind_is_reported_as_vacuous() {
    /// The [`Values`] domain with `Nat` unreachable — the shape of a corpus that would let
    /// the broken encoder above pass unnoticed.
    #[derive(Debug, Clone, Copy)]
    struct NoNaturals;

    impl Domain for NoNaturals {
        type Item = ValueCase;

        fn generate(&self, rng: &mut Prng) -> ValueCase {
            ValueCase(Value::int(
                i128::try_from(rng.below(1000)).unwrap_or_default(),
            ))
        }

        fn shrink(&self, _item: &ValueCase) -> Vec<ValueCase> {
            Vec::new()
        }

        fn size(&self, item: &ValueCase) -> usize {
            item.encoded().len()
        }
    }

    // The law is refutable and the seam is broken, but this corpus cannot reach the bug —
    // and `expect_refuted` says so rather than reading as a pass. That is the failure mode
    // a property suite is most likely to have and least likely to notice.
    let plan = Plan::new("a corpus that cannot reach the bug", 11, 256);
    let report = check(&plan, &NoNaturals, decodes_back_through_the_broken_encoder);
    let outcome = std::panic::catch_unwind(move || report.expect_refuted(&plan));
    let payload = outcome.expect_err("a corpus that cannot reach the bug must not read as a pass");
    let message = payload
        .downcast_ref::<String>()
        .expect("the harness panics with a String");
    assert!(message.contains("the suite cannot fail"), "{message}");
}

// --- helpers ---------------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
