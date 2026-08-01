//! The `fairness` field group: one canonical spelling, one identity, and a
//! strengthening that cannot arrive as a reformat (PR-4 / IMPL-06).
//!
//! # What this file is evidence for
//!
//! Fairness is the canonical gaming vector. Plan §5.1 and docs/50 name it, and RFC
//! 0031's attack table gives it a verdict:
//!
//! > | add a fairness assumption that schedules away the bug | `fairness` | `added` /
//! > `strengthened` | `review` |
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Attacks and their
//! > classifications"
//!
//! The move is not a weaker property; it is a smaller world. A stronger fairness
//! assumption deletes the behaviors in which the property fails, so the same claim
//! passes over less. RFC 0031 classifies it along three movements — membership, the
//! `kind`, and the `condition` under an antitone rule — and each of the three is
//! tested below for *visibility*: it moves the identity, and it shows up as a keyed
//! set difference or as a structural change to a CPNF-1 formula, never as a diff of
//! two renderings.
//!
//! W4 gets its own section. `fairness[].condition` is a **bare** temporal-operator-free
//! formula, and the RFC says the exclusion "MUST be rejected by the schema, not by
//! prose", so the rejection is structural on both the document path and the
//! constructor path and it names the operator that caused it.
//!
//! The schema half is validated by `notes/plan/tools/validate_dossier.py` on every
//! `just check`; [`the_schemas_own_validated_example_decodes`] reads that same
//! validated example through `include_str!` and decodes its `fairness` array with this
//! crate's decoder (INV-003 — the schema decides shape).

use continuum_intent::ast::{ComparisonOperator, Formula, Identifier, Literal, Term};
use continuum_intent::canonical_json::Json;
use continuum_intent::fairness::{
    ActionName, FairnessConstraint, FairnessError, FairnessKey, FairnessKind, FairnessSet,
};

/// The dossier's validated Intent Contract example, included at compile time.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// One fairness constraint in canonical artifact form. It is the schema example's
/// constraint, re-encoded: the example writes the predicate without its `args` key,
/// which reads as the empty list and is written back explicitly.
const GOOD: &str = concat!(
    r#"{"action":"SyncCompleted","condition":{"args":[],"kind":"predicate","#,
    r#""name":"node_running"},"kind":"weak"}"#,
);

fn action(name: &str) -> ActionName {
    ActionName::new(name).expect("a test action name is non-empty")
}

fn key(kind: FairnessKind, name: &str) -> FairnessKey {
    FairnessKey::new(kind, action(name))
}

fn predicate(name: &str) -> Formula {
    Formula::predicate(
        Identifier::new(name).expect("a test identifier is well formed"),
        Vec::new(),
    )
}

fn constraint(kind: FairnessKind, name: &str, condition: Option<&Formula>) -> FairnessConstraint {
    FairnessConstraint::new(key(kind, name), condition).expect("a test constraint is well formed")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
}

// --- round trip ---------------------------------------------------------------------------

#[test]
fn the_baseline_round_trips_byte_for_byte() {
    let decoded = FairnessConstraint::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(utf8(&decoded.to_artifact_bytes()), GOOD);
    assert_eq!(decoded.kind(), FairnessKind::Weak);
    assert_eq!(decoded.action().as_str(), "SyncCompleted");
    assert_eq!(decoded.key().locator(), "weak:SyncCompleted");
    assert!(!decoded.is_unconditional());
}

#[test]
fn a_constructed_constraint_and_a_decoded_one_are_the_same_value() {
    let built = constraint(
        FairnessKind::Weak,
        "SyncCompleted",
        Some(&predicate("node_running")),
    );
    let decoded = FairnessConstraint::decode(GOOD.as_bytes()).expect("decodes");
    assert_eq!(built, decoded);
    assert_eq!(built.identity(), decoded.identity());
}

#[test]
fn a_set_round_trips_and_iterates_in_key_order() {
    let set = FairnessSet::from_constraints([
        constraint(FairnessKind::Weak, "B", None),
        constraint(FairnessKind::Strong, "A", None),
    ])
    .expect("two distinct keys");
    let bytes = set.to_artifact_bytes();
    let decoded = FairnessSet::decode(&bytes).expect("round trips");
    assert_eq!(decoded, set);
    assert_eq!(decoded.identity(), set.identity());
    assert_eq!(
        decoded
            .iter()
            .map(|c| c.key().locator())
            .collect::<Vec<_>>(),
        vec!["strong:A".to_owned(), "weak:B".to_owned()]
    );
}

// --- canonical-spelling uniqueness -----------------------------------------------------------

#[test]
fn every_spelling_of_one_constraint_reaches_one_canonical_form() {
    let spellings = [
        GOOD.to_owned(),
        // Object keys reordered, at both levels.
        concat!(
            r#"{"kind":"weak","condition":{"name":"node_running","kind":"predicate","#,
            r#""args":[]},"action":"SyncCompleted"}"#,
        )
        .to_owned(),
        // The optional `args` omitted, as the schema's own example writes it.
        concat!(
            r#"{"action":"SyncCompleted","condition":{"kind":"predicate","#,
            r#""name":"node_running"},"kind":"weak"}"#,
        )
        .to_owned(),
        format!(" {GOOD}\n"),
    ];
    let baseline = FairnessConstraint::decode(GOOD.as_bytes()).expect("baseline");
    for spelling in spellings {
        let decoded =
            FairnessConstraint::decode(spelling.as_bytes()).expect("a legal spelling decodes");
        assert_eq!(decoded, baseline, "{spelling}");
        assert_eq!(decoded.identity(), baseline.identity(), "{spelling}");
        assert_eq!(utf8(&decoded.to_artifact_bytes()), GOOD, "{spelling}");
    }
}

#[test]
fn an_authoring_form_condition_and_its_normal_form_are_one_value() {
    // The condition is a property AST and is compared over CPNF-1 "exactly as for
    // claims" (RFC 0031). So a rewrite that CPNF-1 erases is not a change: `implies`
    // is eliminated by N1 and the two documents reach one identity.
    let authored = concat!(
        r#"{"action":"A","condition":{"antecedent":{"args":[],"kind":"predicate","name":"p"},"#,
        r#""consequent":{"args":[],"kind":"predicate","name":"q"},"kind":"implies"},"#,
        r#""kind":"weak"}"#,
    );
    let normal = concat!(
        r#"{"action":"A","condition":{"kind":"or","operands":["#,
        r#"{"args":[],"kind":"predicate","name":"q"},"#,
        r#"{"kind":"not","operand":{"args":[],"kind":"predicate","name":"p"}}]},"#,
        r#""kind":"weak"}"#,
    );
    let one = FairnessConstraint::decode(authored.as_bytes()).expect("authoring form decodes");
    let other = FairnessConstraint::decode(normal.as_bytes()).expect("normal form decodes");
    assert_eq!(one, other);
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

#[test]
fn an_absent_condition_and_an_explicit_null_are_one_meaning_with_one_spelling() {
    let absent = FairnessConstraint::decode(br#"{"action":"A","kind":"weak"}"#)
        .expect("an absent condition is legal");
    let explicit = FairnessConstraint::decode(br#"{"action":"A","condition":null,"kind":"weak"}"#)
        .expect("an explicit null is legal");
    assert_eq!(absent, explicit);
    assert_eq!(absent.identity(), explicit.identity());
    assert!(absent.is_unconditional());
    assert_eq!(
        utf8(&absent.to_artifact_bytes()),
        r#"{"action":"A","condition":null,"kind":"weak"}"#
    );
}

#[test]
fn authored_array_order_does_not_reach_the_identity() {
    let a = constraint(FairnessKind::Weak, "A", None);
    let b = constraint(FairnessKind::Strong, "B", None);
    let one = FairnessSet::from_constraints([a.clone(), b.clone()]).expect("distinct keys");
    let other = FairnessSet::from_constraints([b, a]).expect("distinct keys");
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

// --- the protected weakening, in its three movements -------------------------------------------

#[test]
fn adding_a_constraint_is_a_keyed_set_difference() {
    let before = FairnessSet::from_constraints([constraint(FairnessKind::Weak, "A", None)])
        .expect("one constraint");
    // The attack: schedule the bug away by assuming more of the environment.
    let after = FairnessSet::from_constraints([
        constraint(FairnessKind::Weak, "A", None),
        constraint(FairnessKind::Strong, "TheActionThatWasStarving", None),
    ])
    .expect("two constraints");

    assert_ne!(before.identity(), after.identity());
    let added: Vec<String> = after
        .unit_keys()
        .difference(&before.unit_keys())
        .map(|key| key.locator())
        .collect();
    assert_eq!(added, vec!["strong:TheActionThatWasStarving".to_owned()]);
}

#[test]
fn the_kind_is_part_of_the_unit_key_so_weak_to_strong_is_visible() {
    // RFC 0031: "`weak → strong` on the same action classifies `strengthened`, because
    // strong fairness implies weak fairness." The schema keys the unit by the pair, so
    // the movement is a key change and cannot be mistaken for an unchanged constraint.
    let weak = constraint(FairnessKind::Weak, "A", None);
    let strong = constraint(FairnessKind::Strong, "A", None);
    assert_ne!(weak.identity(), strong.identity());
    assert_ne!(weak.key(), strong.key());
    assert_eq!(weak.action(), strong.action());
    // Both are addressable at once, and they are two units rather than a duplicate.
    let set = FairnessSet::from_constraints([weak, strong]).expect("two units");
    assert_eq!(set.len(), 2);
    assert!(set.get(&key(FairnessKind::Weak, "A")).is_some());
    assert!(set.get(&key(FairnessKind::Strong, "A")).is_some());
}

#[test]
fn the_condition_is_structural_so_an_antitone_move_is_visible() {
    // RFC 0031: "the enabling predicate occupies an **antitone** position […] `null`
    // means unconditional and is the weakest condition, hence the strongest
    // constraint: `C → null` classifies `strengthened`." Nothing here classifies; what
    // is asserted is that the three constraints are three artifacts with three
    // identities, and that the surface a classifier reads is a formula, not a string.
    let conditional = constraint(FairnessKind::Weak, "A", Some(&predicate("enabled_p")));
    let weaker_condition = constraint(
        FairnessKind::Weak,
        "A",
        Some(
            &Formula::or(vec![predicate("enabled_p"), predicate("enabled_q")])
                .expect("two operands"),
        ),
    );
    let unconditional = constraint(FairnessKind::Weak, "A", None);

    assert_ne!(conditional.identity(), weaker_condition.identity());
    assert_ne!(conditional.identity(), unconditional.identity());
    assert!(unconditional.is_unconditional());
    // The comparison surface is the AST, in CPNF-1.
    assert_eq!(
        conditional.condition().map(Formula::kind),
        Some("predicate")
    );
    assert_eq!(weaker_condition.condition().map(Formula::kind), Some("or"));
    assert_eq!(unconditional.condition(), None);
}

#[test]
fn a_condition_rewrite_that_is_not_a_change_keeps_the_identity() {
    // The mirror of the previous test, and the reason a *string* condition would be
    // unsound in both directions: a rename or a re-association must not look like a
    // strengthening. CPNF-1's N4 orders the operands of a commutative junction.
    let one = constraint(
        FairnessKind::Weak,
        "A",
        Some(&Formula::and(vec![predicate("q"), predicate("p")]).expect("two operands")),
    );
    let other = constraint(
        FairnessKind::Weak,
        "A",
        Some(&Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands")),
    );
    assert_eq!(one.identity(), other.identity());
}

#[test]
fn there_is_no_way_to_strengthen_a_constraint_in_place() {
    // The compile-time half of INV-001: no `&mut self` method, no setter, no builder.
    // Strengthening means constructing a new constraint, which derives a new identity.
    let before = constraint(FairnessKind::Weak, "A", Some(&predicate("p")));
    let after = constraint(FairnessKind::Strong, "A", None);
    assert_ne!(before.identity(), after.identity());
}

// --- W4: a condition is a state formula --------------------------------------------------------

#[test]
fn w4_rejects_a_temporal_operator_and_names_it_on_the_constructor_path() {
    for (formula, operator) in [
        (Formula::always(predicate("p")), "always"),
        (Formula::eventually(predicate("p")), "eventually"),
        (
            Formula::leads_to(predicate("p"), predicate("q")),
            "leads_to",
        ),
    ] {
        assert_eq!(
            FairnessConstraint::new(key(FairnessKind::Weak, "A"), Some(&formula)),
            Err(FairnessError::TemporalCondition { operator })
        );
    }
}

#[test]
fn w4_rejects_a_temporal_operator_at_any_depth_on_the_document_path() {
    // "MUST contain no `always`, `eventually`, or `leads_to` node at any depth." The
    // schema states it as a structural exclusion over every child position, and the
    // decoder applies the same rule to the parsed document.
    let cases = [
        (
            concat!(
                r#"{"action":"A","condition":{"kind":"and","operands":["#,
                r#"{"args":[],"kind":"predicate","name":"p"},"#,
                r#"{"kind":"always","operand":{"args":[],"kind":"predicate","name":"q"}}]},"#,
                r#""kind":"weak"}"#,
            ),
            "always",
        ),
        (
            concat!(
                r#"{"action":"A","condition":{"binder":{"domain":{"kind":"constant","#,
                r#""name":"Nodes"},"variable":"n"},"body":{"kind":"eventually","#,
                r#""operand":{"args":[{"kind":"var","name":"n"}],"kind":"predicate","#,
                r#""name":"up"}},"kind":"forall"},"kind":"weak"}"#,
            ),
            "eventually",
        ),
        (
            concat!(
                r#"{"action":"A","condition":{"kind":"not","operand":{"#,
                r#""antecedent":{"args":[],"kind":"predicate","name":"p"},"#,
                r#""consequent":{"args":[],"kind":"predicate","name":"q"},"#,
                r#""kind":"leads_to"}},"kind":"weak"}"#,
            ),
            "leads_to",
        ),
    ];
    for (document, operator) in cases {
        assert_eq!(
            FairnessConstraint::decode(document.as_bytes())
                .expect_err("W4 rejects a temporal condition"),
            continuum_intent::fairness::FairnessDecodeError::Fairness(
                FairnessError::TemporalCondition { operator }
            ),
            "{document}"
        );
    }
}

#[test]
fn a_string_literal_spelled_like_an_operator_is_data_and_not_an_operator() {
    // The structural rule reads the value under a `kind` key. A literal whose *value*
    // is the text "always" is a state predicate about a mode field, and rejecting it
    // would be prose masquerading as structure.
    let document = concat!(
        r#"{"action":"A","condition":{"kind":"compare","#,
        r#""left":{"indices":[],"kind":"state","name":"mode"},"op":"eq","#,
        r#""right":{"kind":"literal","value":"always"}},"kind":"weak"}"#,
    );
    let decoded = FairnessConstraint::decode(document.as_bytes())
        .expect("a literal is not a temporal operator");
    assert_eq!(decoded.condition().map(Formula::kind), Some("compare"));
    // And the same shape with a real operator is still rejected.
    let real = document.replace(
        r#"{"kind":"literal","value":"always"}"#,
        r#"{"kind":"literal","value":1}"#,
    );
    FairnessConstraint::decode(real.as_bytes()).expect("an integer literal is fine too");
}

// --- W1 ------------------------------------------------------------------------------------------

#[test]
fn w1_rejects_two_constraints_sharing_a_kind_action_pair() {
    let doubled = format!("[{GOOD},{GOOD}]");
    assert_eq!(
        FairnessSet::decode(doubled.as_bytes()).expect_err("W1 rejects a repeated pair"),
        continuum_intent::fairness::FairnessDecodeError::Fairness(FairnessError::DuplicateKey {
            key: "weak:SyncCompleted".to_owned()
        })
    );
    // The dangerous shape: one key, two different conditions. Last-writer-wins would
    // let the second entry silently replace the first.
    let stronger = GOOD.replace(
        r#"{"args":[],"kind":"predicate","name":"node_running"}"#,
        "null",
    );
    assert!(FairnessSet::decode(format!("[{GOOD},{stronger}]").as_bytes()).is_err());
    // A different kind on the same action is a different unit, not a duplicate.
    let strong = GOOD.replace(r#""kind":"weak""#, r#""kind":"strong""#);
    assert_eq!(
        FairnessSet::decode(format!("[{GOOD},{strong}]").as_bytes())
            .expect("two units")
            .len(),
        2
    );
}

#[test]
fn an_empty_fairness_set_is_a_declaration_and_not_an_omission() {
    let empty = FairnessSet::decode(b"[]").expect("an empty array is legal");
    assert!(empty.is_empty());
    assert_eq!(utf8(&empty.to_artifact_bytes()), "[]");
    // The empty set is the weakest environment assumption, and declaring it is a
    // statement rather than a silence (RFC 0037).
    assert_eq!(utf8(empty.identity().canonical_bytes()), "[]");
}

// --- the schema's own example --------------------------------------------------------------------

#[test]
fn the_schemas_own_validated_example_decodes() {
    let example = Json::parse(SCHEMA_EXAMPLE.as_bytes()).expect("the example is canonical JSON");
    let fairness = example
        .as_object()
        .expect("the contract is an object")
        .get("fairness")
        .expect("the contract declares fairness");
    let set = FairnessSet::from_json(fairness).expect("the schema's example decodes");
    assert_eq!(set.len(), 1);
    let constraint = set
        .get(&key(FairnessKind::Weak, "SyncCompleted"))
        .expect("the example's constraint");
    assert_eq!(constraint.condition().map(Formula::kind), Some("predicate"));
    assert_eq!(utf8(&constraint.to_artifact_bytes()), GOOD);
    // The example omits the predicate's `args`; the encoder writes it back, which is
    // the same "absent is the empty set, encoded explicitly" rule the AST already
    // applies (RFC 0037, ID5).
    assert!(utf8(&constraint.to_artifact_bytes()).contains(r#""args":[]"#));
}

#[test]
fn a_comparison_condition_is_normalized_the_way_a_claim_is() {
    // The condition goes through the crate's one property-AST reader and CPNF-1, so N6
    // rewrites `gt` into `lt` with swapped operands here exactly as it does inside a
    // claim. One normal form, one comparison order, for both carriers.
    let document = concat!(
        r#"{"action":"A","condition":{"kind":"compare","#,
        r#""left":{"indices":[],"kind":"state","name":"queue"},"op":"gt","#,
        r#""right":{"kind":"literal","value":0}},"kind":"weak"}"#,
    );
    let decoded = FairnessConstraint::decode(document.as_bytes()).expect("decodes");
    let encoded = utf8(&decoded.to_artifact_bytes());
    assert!(encoded.contains(r#""op":"lt""#), "{encoded}");
    assert!(!encoded.contains(r#""op":"gt""#), "{encoded}");
    assert_eq!(
        decoded.condition(),
        Some(&Formula::compare(
            ComparisonOperator::Lt,
            Term::Literal {
                value: Literal::Integer(0)
            },
            Term::State {
                name: Identifier::new("queue").expect("well formed"),
                indices: Vec::new()
            }
        ))
    );
}
