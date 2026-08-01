//! PR-4 exit evidence, properties half: "Die Hard […] intents serialize canonically"
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 4).
//!
//! # What this file is evidence for
//!
//! PR 4's exit has two clauses. This file discharges the first one for the
//! `properties` field group, and does it against a corpus fixture rather than an
//! invented one:
//!
//! > **Exit:** Die Hard and replicated-register intents serialize canonically;
//! > ordinary operations cannot mutate them.
//!
//! The Die Hard properties are not written for this test. They are transcribed from
//! `notes/plan/corpus/tla-examples/ports/TV-009/`, the workspace's first corpus
//! fixture (RFC 0019, docs/32):
//!
//! | Claim | Source |
//! |---|---|
//! | `TypeOK` | `DieHard.ctm`: `invariant TypeOK { big in 0..5 && small in 0..3 }` |
//! | `NotSolved` | `DieHard.ctm`: `invariant NotSolved { big != 4 }` — "intentionally false; shortest witness is the solution" |
//! | `SolutionReachesFourGallons` | `port.json`'s `lean.theorems`: `Continuum.Examples.DieHard.solution_ends_with_four_gallons`, which `lean/Continuum/Examples/DieHard.lean` proves by exhibiting a 6-step solution ending at `⟨4, 3⟩` |
//!
//! `TypeOK` is authored the way a person writes it — `0 ≤ big` as a `ge`, four
//! conjuncts in source order — so the fixture exercises N6's `gt`/`ge` elimination
//! and N4's ordering on the way to its canonical bytes rather than being handed
//! them.
//!
//! The corpus records that a default run of this model reaches 16 states over 96
//! labeled transitions and refutes `NotSolved` at depth 6. None of that is checked
//! here — a search engine is PR 6+ and this crate is in the smallest trust base.
//! What is checked is the thing the engine's answer will be *about*: that the
//! question has one canonical spelling and one identity, so that an answer produced
//! under it cannot later be reported for a different question.
//!
//! # Why a checked-in fixture, and what validates the schema side
//!
//! `tests/fixtures/die-hard-claims.json` is the byte-for-byte expected artifact
//! form. A test that only round-trips its own output cannot catch an encoder that
//! changed: encode and decode would move together and agree. The fixture is the
//! external witness — a golden identity vector, which RFC 0037's acceptance criteria
//! ask for by name ("Golden identity vectors are part of the acceptance suite").
//!
//! The *schema* half is validated where schemas are validated:
//! `notes/plan/tools/validate_dossier.py` checks
//! `schemas/examples/intent-contract.example.json` against
//! `schemas/intent-contract.schema.json` on every `just check`. This file meets that
//! validation from the other side — [`the_schemas_own_validated_example_decodes`]
//! reads the *same* example through `include_str!` and decodes its `claims` array
//! with this crate's decoder. Neither half can drift without the other failing: if
//! the example changes shape the dossier validator has already agreed the new shape
//! is legal, and this test then says whether the types admit it (INV-003 — the
//! schema decides shape, and this is where the code finds out).
//!
//! No test here shells out, reads a clock, or touches a network.
//! `include_str!` is compile-time, so the coupling is a build-time fact rather than
//! a runtime path.

use std::collections::BTreeSet;

use continuum_intent::ast::{ComparisonOperator, Formula, Fragment, Identifier, Literal, Term};
use continuum_intent::canonical_json::Json;
use continuum_intent::property::{Claim, ClaimKind, ClaimSet, PropertyExpression, UnitKey};

/// The dossier's validated Intent Contract example, included at compile time.
///
/// `notes/plan/tools/validate_dossier.py` validates this exact file against
/// `schemas/intent-contract.schema.json`.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The golden artifact form of the Die Hard claim set.
const DIE_HARD_FIXTURE: &str = include_str!("fixtures/die-hard-claims.json");

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("a corpus identifier is well formed")
}

fn unit(id: &str) -> UnitKey {
    UnitKey::new(id).expect("a corpus unit key is non-empty")
}

fn state(name: &str) -> Term {
    Term::State {
        name: ident(name),
        indices: Vec::new(),
    }
}

fn integer(value: i64) -> Term {
    Term::Literal {
        value: Literal::Integer(value),
    }
}

fn compare(op: ComparisonOperator, left: Term, right: Term) -> Formula {
    Formula::compare(op, left, right)
}

/// `invariant TypeOK { big in 0..5 && small in 0..3 }`, as a person writes it.
///
/// The range membership is spelled as four comparisons, two of them `ge`, in source
/// order. N6 rewrites the `ge`s and N4 reorders the conjuncts, so the canonical form
/// is reached by the rules rather than by the transcription.
fn type_ok() -> Formula {
    Formula::always(
        Formula::and(vec![
            compare(ComparisonOperator::Ge, state("big"), integer(0)),
            compare(ComparisonOperator::Le, state("big"), integer(5)),
            compare(ComparisonOperator::Ge, state("small"), integer(0)),
            compare(ComparisonOperator::Le, state("small"), integer(3)),
        ])
        .expect("four conjuncts"),
    )
}

/// `invariant NotSolved { big != 4 }`.
fn not_solved() -> Formula {
    Formula::always(compare(ComparisonOperator::Ne, state("big"), integer(4)))
}

/// The Lean theorem's content: the puzzle is solvable, so `big == 4` is reachable.
fn solution_reaches_four_gallons() -> Formula {
    Formula::eventually(compare(ComparisonOperator::Eq, state("big"), integer(4)))
}

fn die_hard_claims() -> ClaimSet {
    ClaimSet::from_claims([
        Claim::new(
            unit("TypeOK"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &type_ok(),
                Some(Fragment::Finite),
                Some("big in 0..5 && small in 0..3".to_owned()),
            )
            .expect("normalizes"),
            None,
        ),
        Claim::new(
            unit("NotSolved"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &not_solved(),
                Some(Fragment::Finite),
                Some("big != 4".to_owned()),
            )
            .expect("normalizes"),
            None,
        ),
        Claim::new(
            unit("SolutionReachesFourGallons"),
            ClaimKind::Liveness,
            PropertyExpression::normalized(
                &solution_reaches_four_gallons(),
                Some(Fragment::Finite),
                Some("eventually (big == 4)".to_owned()),
            )
            .expect("normalizes"),
            None,
        ),
    ])
    .expect("three distinct unit keys")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
}

/// The conjuncts of `TypeOK`'s normalized AST, as canonical JSON documents.
fn type_ok_operands() -> Vec<Json> {
    let claims = die_hard_claims();
    let type_ok = claims.get(&unit("TypeOK")).expect("the TypeOK claim");
    let ast = type_ok.expression().ast().to_json();
    let operand = ast
        .as_object()
        .expect("`always` is an object")
        .get("operand")
        .expect("`always` carries an operand");
    operand
        .as_object()
        .expect("`and` is an object")
        .get("operands")
        .expect("`and` carries operands")
        .as_array()
        .expect("operands is an array")
        .to_vec()
}

// --- the golden vector -----------------------------------------------------------------

#[test]
fn die_hards_properties_serialize_to_their_golden_bytes() {
    let encoded = utf8(&die_hard_claims().to_artifact_bytes());
    assert_eq!(
        encoded,
        DIE_HARD_FIXTURE.trim_end(),
        "the Die Hard claim set no longer serializes to its checked-in golden form; if the \
         change is intended, RFC 0037 makes it an encoding change and therefore an identity \
         change for every stored contract (see the CPNF-1 versioning rules)"
    );
}

#[test]
fn the_golden_bytes_decode_back_to_the_same_claim_set() {
    let decoded = ClaimSet::decode(DIE_HARD_FIXTURE.trim_end().as_bytes()).expect("decodes");
    let built = die_hard_claims();
    assert_eq!(decoded, built);
    assert_eq!(decoded.identity(), built.identity());
    assert_eq!(decoded.to_artifact_bytes(), built.to_artifact_bytes());
}

#[test]
fn the_normalization_rules_fired_on_the_way_to_the_golden_form() {
    let encoded = utf8(&die_hard_claims().to_artifact_bytes());
    // N6: the two authored `ge` comparisons are gone, rewritten as `le` with the
    // operands swapped.
    assert!(!encoded.contains(r#""op":"ge""#), "{encoded}");
    assert_eq!(encoded.matches(r#""op":"le""#).count(), 4);
    // N4: the four conjuncts are in ascending N8 byte order, which is not the order
    // `DieHard.ctm` writes them in. Checked as the rule states it — each operand's
    // own encoding against its successor's — rather than by pinning a hand-picked
    // arrangement.
    let operands = type_ok_operands();
    let mut previous: Option<Vec<u8>> = None;
    for operand in &operands {
        let key = operand.to_canonical_bytes();
        if let Some(previous) = previous {
            assert!(
                previous < key,
                "N4 did not order the conjuncts ascending, and did not deduplicate them"
            );
        }
        previous = Some(key);
    }
    assert_eq!(operands.len(), 4);
    // Source order was `big ≥ 0`, `big ≤ 5`, `small ≥ 0`, `small ≤ 3`; the canonical
    // order is not that.
    assert_ne!(
        operands[0].to_canonical_bytes(),
        Formula::compare(ComparisonOperator::Le, integer(0), state("big"))
            .to_json()
            .to_canonical_bytes()
    );
    // The declaration is present and true, and `source` survived as display-only.
    assert_eq!(encoded.matches(r#""normal_form":"cpnf-1""#).count(), 3);
    assert!(encoded.contains(r#""source":"big != 4""#), "{encoded}");
}

#[test]
fn every_die_hard_claim_is_stable_under_rewriting_and_moves_on_weakening() {
    let claims = die_hard_claims();
    let not_solved = claims.get(&unit("NotSolved")).expect("present");

    // The same invariant written as a negated equality is the same claim.
    let rewritten = Claim::new(
        unit("NotSolved"),
        ClaimKind::Safety,
        PropertyExpression::normalized(
            &Formula::always(Formula::not(compare(
                ComparisonOperator::Eq,
                integer(4),
                state("big"),
            ))),
            Some(Fragment::Finite),
            // A different rendering of the same thing: display-only, out of the
            // preimage.
            Some("not (4 == big)".to_owned()),
        )
        .expect("normalizes"),
        None,
    );
    assert_eq!(rewritten.identity(), not_solved.identity());

    // Weakening `big != 4` to `big != 4 or guard` is the docs/50 "weaken property"
    // move and it moves the identity.
    let weakened = Claim::new(
        unit("NotSolved"),
        ClaimKind::Safety,
        PropertyExpression::normalized(
            &Formula::always(
                Formula::or(vec![
                    compare(ComparisonOperator::Ne, state("big"), integer(4)),
                    Formula::predicate(ident("under_test"), Vec::new()),
                ])
                .expect("two operands"),
            ),
            Some(Fragment::Finite),
            None,
        )
        .expect("normalizes"),
        None,
    );
    assert_ne!(weakened.identity(), not_solved.identity());

    // Narrowing the type invariant's range is a strengthening, and it moves too:
    // CPNF-1 supplies no direction (S2), only the fact that something changed.
    let narrowed = Claim::new(
        unit("TypeOK"),
        ClaimKind::Safety,
        PropertyExpression::normalized(
            &Formula::always(
                Formula::and(vec![
                    compare(ComparisonOperator::Ge, state("big"), integer(0)),
                    compare(ComparisonOperator::Le, state("big"), integer(4)),
                    compare(ComparisonOperator::Ge, state("small"), integer(0)),
                    compare(ComparisonOperator::Le, state("small"), integer(3)),
                ])
                .expect("four conjuncts"),
            ),
            Some(Fragment::Finite),
            None,
        )
        .expect("normalizes"),
        None,
    );
    assert_ne!(
        narrowed.identity(),
        claims.get(&unit("TypeOK")).expect("present").identity()
    );
}

#[test]
fn deleting_a_die_hard_claim_moves_the_sets_identity() {
    // The crudest gaming move on a claim set: drop the invariant that fails.
    let full = die_hard_claims();
    let without = ClaimSet::from_claims(
        full.iter()
            .filter(|claim| claim.unit().as_str() != "NotSolved")
            .cloned(),
    )
    .expect("two remaining claims");
    assert_ne!(full.identity(), without.identity());
    assert_eq!(without.len(), 2);
}

#[test]
fn the_die_hard_claims_are_well_formed_against_the_ports_declared_fragment() {
    let claims = die_hard_claims();
    // `port.json` records `features: ["finite", …]` and the model is explored
    // exhaustively, so `Finite` is the declared fragment.
    assert!(
        claims
            .check_fragments(&BTreeSet::from([Fragment::Finite]))
            .is_ok()
    );
    // No claim binds an observer, so W2 holds against an empty observer set.
    assert!(claims.observer_references().is_empty());
    assert!(claims.check_observers(&BTreeSet::new()).is_ok());
}

// --- the schema cross-check ------------------------------------------------------------

#[test]
fn the_schemas_own_validated_example_decodes() {
    let document = Json::parse(SCHEMA_EXAMPLE.as_bytes()).expect("the example is JSON");
    let claims_json = document
        .as_object()
        .expect("the example is an object")
        .get("claims")
        .expect("the example carries a claims array");
    let claims = ClaimSet::from_json(claims_json).expect(
        "this crate's types must admit exactly what the schema's validated example carries \
         (INV-003)",
    );
    assert!(!claims.is_empty());
    // The example's claim is the RFC's worked example, and it decodes into the
    // normal form the RFC's own N8 line describes.
    let encoded = utf8(&claims.to_artifact_bytes());
    assert!(!encoded.contains(r#""kind":"implies""#), "{encoded}");
    assert!(encoded.contains(r#""variable":"v0""#), "{encoded}");
    // Round-trip through this crate's canonical form and back.
    let decoded = ClaimSet::decode(&claims.to_artifact_bytes()).expect("decodes");
    assert_eq!(decoded, claims);
    assert_eq!(decoded.identity(), claims.identity());
}

#[test]
fn the_schema_examples_claims_survive_a_reordering_of_their_json_keys() {
    // The example is authored with sorted keys. Re-emitting it through this crate's
    // canonical writer and decoding again must reach the same value, which is the
    // ID5 determinism claim applied to a file nobody here wrote.
    let document = Json::parse(SCHEMA_EXAMPLE.as_bytes()).expect("the example is JSON");
    let claims_json = document
        .as_object()
        .expect("object")
        .get("claims")
        .expect("claims");
    let once = ClaimSet::from_json(claims_json).expect("decodes");
    let twice = ClaimSet::decode(&once.to_artifact_bytes()).expect("re-decodes");
    assert_eq!(once.to_artifact_bytes(), twice.to_artifact_bytes());
    assert_eq!(once.identity(), twice.identity());
}
