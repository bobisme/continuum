//! The `optimization` field group against the schema, the two policy keys, and
//! INV-012's non-vacuity obligation (PR-4 / IMPL-08).
//!
//! # What this file is evidence for
//!
//! One contract group, two policy verbs. RFC 0037 calls the split "required, not
//! cosmetic", and gives the reason:
//!
//! > `non_vacuity = "no-removal"` (INV-012) is unenforceable if a removed non-vacuity
//! > behavior is reported as an `optimization` change (RFC 0031).
//!
//! So the load-bearing test here is not that the group encodes — it is that a removed
//! non-vacuity behavior can only ever be reported under `non_vacuity`, and that the
//! type system is what makes that true rather than a convention.
//!
//! The second load-bearing test is the vacuity guard. A system that admits no behavior
//! satisfies every safety claim, so the non-vacuity set is what stops "safety by
//! disabling the system" from counting as a repair (INV-012, plan §14.5, and the
//! G0-DX-07 Forge question). The schema admits an empty set, so the obligation is a
//! caller-invoked check and not a decode-time rejection; both halves are tested.

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyVerb, Relation};
use continuum_intent::optimization::{
    NonVacuityObligation, Objective, ObjectiveRole, Optimization, OptimizationError,
    OptimizationUnit,
};
use continuum_value::identity::{ContentHasher, Fnv1aPlaceholder};

const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");
const EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The example contract's `optimization` block in canonical form — the golden vector.
const EXAMPLE_OPTIMIZATION: &str = concat!(
    r#"{"hard":["progress"],"non_vacuity":["synced request eventually acknowledged"],"#,
    r#""soft":["stable_writes"]}"#,
);

fn parsed(text: &str) -> Json {
    Json::parse(text.as_bytes()).expect("a normative dossier document parses")
}

fn at<'a>(json: &'a Json, path: &[&str]) -> &'a Json {
    let mut cursor = json;
    for key in path {
        cursor = cursor
            .as_object()
            .unwrap_or_else(|| panic!("{key} is reached through an object"))
            .get(*key)
            .unwrap_or_else(|| panic!("the schema declares {key}"));
    }
    cursor
}

fn objective(text: &str) -> Objective {
    Objective::new(text).expect("a test objective is non-empty")
}

fn behavior(text: &str) -> NonVacuityObligation {
    NonVacuityObligation::new(text).expect("a test behavior is non-empty")
}

// --- provenance --------------------------------------------------------------------------

#[test]
fn the_group_is_three_unique_string_sets_and_no_required_key() {
    let schema = parsed(SCHEMA);
    let properties = at(&schema, &["properties", "optimization", "properties"])
        .as_object()
        .expect("the optimization object declares properties");
    let declared: Vec<&str> = properties.keys().map(String::as_str).collect();
    assert_eq!(declared, ["hard", "non_vacuity", "soft"]);
    for key in declared {
        assert_eq!(
            at(
                &schema,
                &[
                    "properties",
                    "optimization",
                    "properties",
                    key,
                    "uniqueItems"
                ]
            )
            .as_bool(),
            Some(true),
            "{key} is a set"
        );
        assert_eq!(
            at(
                &schema,
                &[
                    "properties",
                    "optimization",
                    "properties",
                    key,
                    "items",
                    "type"
                ]
            )
            .as_str(),
            Some("string"),
            "{key} carries strings today"
        );
    }
    // No `required` key: the group MAY be empty, which is why the non-vacuity
    // obligation is a caller-invoked check rather than a decoder rejection (INV-003).
    assert!(
        at(&schema, &["properties", "optimization"])
            .as_object()
            .expect("object")
            .get("required")
            .is_none()
    );
    // The group itself is required at the contract level, though: an absent
    // `optimization` key is not a legal contract.
    let required: Vec<&str> = at(&schema, &["required"])
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string"))
        .collect();
    assert!(required.contains(&"optimization"));
}

#[test]
fn the_two_policy_keys_govern_the_two_halves_of_one_group() {
    // The schema's `x-policy-field-map` is where the split is normative.
    let schema = parsed(SCHEMA);
    let map = at(&schema, &["x-policy-field-map"])
        .as_object()
        .expect("the normative map");
    assert_eq!(
        map.get("optimization").and_then(Json::as_str),
        Some("optimization")
    );
    assert_eq!(
        map.get("non_vacuity").and_then(Json::as_str),
        Some("optimization.non_vacuity")
    );
    assert_eq!(
        PolicyField::NonVacuity.contract_path(),
        "optimization.non_vacuity"
    );
    // Both keys admit `no-removal`, which is the verb INV-012 leans on.
    assert!(PolicyVerb::NoRemoval.is_admissible_on(PolicyField::NonVacuity));
    assert!(PolicyVerb::NoRemoval.is_admissible_on(PolicyField::Optimization));
    // And both are classified by membership only.
    assert_eq!(
        PolicyField::NonVacuity.classified_relations(),
        [Relation::Unchanged, Relation::Added, Relation::Removed]
    );
    assert_eq!(
        PolicyField::Optimization.classified_relations(),
        [Relation::Unchanged, Relation::Added, Relation::Removed]
    );
}

// --- the artifact form ---------------------------------------------------------------------

#[test]
fn the_dossiers_example_optimization_block_round_trips_byte_for_byte() {
    let example = parsed(EXAMPLE);
    let group =
        Optimization::from_json(at(&example, &["optimization"])).expect("the example decodes");
    assert_eq!(group.to_artifact_bytes(), EXAMPLE_OPTIMIZATION.as_bytes());
    assert_eq!(
        Optimization::decode(EXAMPLE_OPTIMIZATION.as_bytes()).expect("the golden form decodes"),
        group
    );
    assert_eq!(
        group.hard().map(Objective::as_str).collect::<Vec<_>>(),
        ["progress"]
    );
    assert_eq!(
        group.soft().map(Objective::as_str).collect::<Vec<_>>(),
        ["stable_writes"]
    );
    assert_eq!(
        group
            .non_vacuity()
            .map(NonVacuityObligation::as_str)
            .collect::<Vec<_>>(),
        ["synced request eventually acknowledged"]
    );
    // The example's own non-vacuity behavior is exactly the plan §14.5 shape: "A
    // request must be acknowledged in a failure-free fair run."
    assert!(group.require_non_vacuity().is_ok());
}

#[test]
fn an_absent_set_and_an_explicit_empty_one_are_one_meaning_with_one_spelling() {
    let absent = Optimization::decode(br#"{"hard":["progress"]}"#).expect("absent is legal");
    let empty = Optimization::decode(br#"{"hard":["progress"],"non_vacuity":[],"soft":[]}"#)
        .expect("empty is legal");
    assert_eq!(absent, empty);
    assert_eq!(absent.identity(), empty.identity());
    assert_eq!(
        String::from_utf8(absent.to_artifact_bytes()).expect("utf-8"),
        r#"{"hard":["progress"],"non_vacuity":[],"soft":[]}"#
    );
    // The wholly empty group is a declaration that the sets are empty, and encodes as
    // one — "not 'unspecified'" (RFC 0037).
    let nothing = Optimization::decode(b"{}").expect("the group MAY be empty");
    assert_eq!(nothing, Optimization::empty());
    assert!(nothing.is_empty());
}

#[test]
fn members_have_one_spelling_whatever_order_they_arrive_in() {
    let authored = br#"{"soft":["b","a"],"hard":["z","y"],"non_vacuity":["q","p"]}"#;
    let group = Optimization::decode(authored).expect("legal");
    assert_eq!(
        String::from_utf8(group.to_artifact_bytes()).expect("utf-8"),
        r#"{"hard":["y","z"],"non_vacuity":["p","q"],"soft":["a","b"]}"#
    );
    // Reordering is a reformatting, and ID7 says reformatting does not move the
    // identity.
    let reversed =
        Optimization::decode(br#"{"hard":["y","z"],"non_vacuity":["q","p"],"soft":["b","a"]}"#)
            .expect("legal");
    assert_eq!(group.identity(), reversed.identity());
}

#[test]
fn every_member_moves_the_identity_and_the_set_it_moved_in_matters() {
    let base = Optimization::new([objective("progress")], [], []).expect("distinct");
    // The same string, in a different set, is a different contract.
    let as_soft = Optimization::new([], [objective("progress")], []).expect("distinct");
    let as_behavior = Optimization::new([], [], [behavior("progress")]).expect("distinct");
    assert_ne!(base.identity(), as_soft.identity());
    assert_ne!(base.identity(), as_behavior.identity());
    assert_ne!(as_soft.identity(), as_behavior.identity());
    // Adding a behavior moves it too.
    let with_behavior =
        Optimization::new([objective("progress")], [], [behavior("acked")]).expect("distinct");
    assert_ne!(base.identity(), with_behavior.identity());
    // The identity *is* the preimage bytes; a digest only indexes them (ADR-0013).
    assert_eq!(
        base.identity().canonical_bytes(),
        base.identity().to_string().as_bytes()
    );
    assert_eq!(
        base.identity().digest::<Fnv1aPlaceholder>(),
        Fnv1aPlaceholder::hash(base.identity().canonical_bytes())
    );
}

// --- the split -------------------------------------------------------------------------------

#[test]
fn a_unit_cannot_be_obtained_without_the_policy_key_that_governs_it() {
    let group = Optimization::new(
        [objective("progress")],
        [objective("stable_writes")],
        [behavior("synced request eventually acknowledged")],
    )
    .expect("distinct");
    let units: Vec<(PolicyField, &str)> = group
        .units()
        .map(|unit| (unit.policy_field(), unit.as_str()))
        .collect();
    assert_eq!(
        units,
        vec![
            (PolicyField::Optimization, "progress"),
            (PolicyField::Optimization, "stable_writes"),
            (
                PolicyField::NonVacuity,
                "synced request eventually acknowledged"
            ),
        ]
    );
    // And a unit renders with its field, so a log line cannot lose the distinction.
    let rendered: Vec<String> = group.units().map(|unit| unit.to_string()).collect();
    assert!(rendered[0].starts_with("optimization:"));
    assert!(rendered[2].starts_with("non_vacuity:"));
    // The role is retained inside the `optimization` half without changing the verb.
    let roles: Vec<ObjectiveRole> = group
        .units()
        .filter_map(|unit| match unit {
            OptimizationUnit::Objective { role, .. } => Some(role),
            OptimizationUnit::NonVacuity(_) => None,
        })
        .collect();
    assert_eq!(roles, vec![ObjectiveRole::Hard, ObjectiveRole::Soft]);
}

#[test]
fn a_removed_non_vacuity_behavior_is_reported_under_non_vacuity_and_nothing_else() {
    // The whole reason RFC 0037 splits the group: `non_vacuity = "no-removal"` is
    // unenforceable if this record carries the `optimization` field instead.
    let before = Optimization::new(
        [objective("progress")],
        [objective("stable_writes")],
        [
            behavior("a request is acknowledged in a failure-free fair run"),
            behavior("at least one write is admitted when capacity exists"),
        ],
    )
    .expect("distinct");
    let after = Optimization::new(
        [objective("progress")],
        [objective("stable_writes"), objective("low_latency")],
        [behavior(
            "a request is acknowledged in a failure-free fair run",
        )],
    )
    .expect("distinct");
    let changes = before.changes(&after);
    assert_eq!(
        changes,
        vec![
            (
                PolicyField::Optimization,
                "low_latency".to_owned(),
                Relation::Added
            ),
            (
                PolicyField::NonVacuity,
                "at least one write is admitted when capacity exists".to_owned(),
                Relation::Removed
            ),
        ]
    );
    // The removal is under `non_vacuity`, so a `no-removal` verb on that key blocks it
    // — and a table that locked only `optimization` would not have.
    let removal = changes
        .iter()
        .find(|(_, _, relation)| *relation == Relation::Removed)
        .expect("one removal");
    assert_eq!(removal.0, PolicyField::NonVacuity);
    assert!(PolicyVerb::NoRemoval.denies(removal.2));
    // And nothing was reported under the wrong key.
    assert!(
        !changes
            .iter()
            .any(|(field, _, relation)| *field == PolicyField::Optimization
                && *relation == Relation::Removed)
    );
}

#[test]
fn membership_is_the_whole_order_and_carries_no_direction() {
    let before = Optimization::new([objective("a")], [], [behavior("x")]).expect("distinct");
    let after = Optimization::new([objective("b")], [], [behavior("y")]).expect("distinct");
    for (_, _, relation) in before.changes(&after) {
        assert!(
            matches!(relation, Relation::Added | Relation::Removed),
            "membership produced {relation}"
        );
    }
    assert!(before.changes(&before).is_empty());
    // Editing a string is a removal plus an addition, never a `strengthened`: the
    // strings are opaque to this group, and RFC 0031 classifies them by membership only.
    assert_eq!(before.changes(&after).len(), 4);
}

// --- the non-vacuity guard ---------------------------------------------------------------

#[test]
fn an_empty_non_vacuity_set_is_schema_legal_and_fails_the_inv_012_obligation() {
    // Decoding accepts it, because `intent-contract.schema.json` does and the schema
    // decides shape (INV-003).
    let vacuous = Optimization::decode(br#"{"hard":["safety"],"soft":[]}"#)
        .expect("an empty non-vacuity set is schema-legal");
    assert_eq!(vacuous.non_vacuity_len(), 0);
    // The obligation is where the rejection lives.
    assert_eq!(
        vacuous.require_non_vacuity(),
        Err(OptimizationError::NoNonVacuityObligation)
    );
    let message = OptimizationError::NoNonVacuityObligation.to_string();
    assert!(message.contains("INV-012"), "{message}");
    assert!(message.contains("cannot fire"), "{message}");
    // A contract that states even one positive behavior discharges it. plan §14.5's
    // three examples, transcribed.
    for text in [
        "A request must be acknowledged in a failure-free fair run.",
        "At least one write must be admitted when capacity exists.",
        "Leadership must remain possible after recovery.",
    ] {
        let group = Optimization::new([], [], [behavior(text)]).expect("one behavior");
        assert_eq!(group.require_non_vacuity(), Ok(()));
    }
}

#[test]
fn a_unit_that_names_nothing_is_rejected() {
    // RFC 0031 makes "one optimization or non-vacuity string" the classified unit, and
    // the diff schema's unit locator has `minLength: 1`: a unit with no name cannot
    // appear in a record.
    assert_eq!(Objective::new(""), Err(OptimizationError::EmptyObjective));
    assert_eq!(
        NonVacuityObligation::new(""),
        Err(OptimizationError::EmptyObjective)
    );
    assert!(Objective::new("progress").is_ok());
}

#[test]
fn a_repeated_member_is_rejected_and_a_string_in_two_sets_is_not() {
    assert_eq!(
        Optimization::new([], [], [behavior("acked"), behavior("acked")]),
        Err(OptimizationError::DuplicateNonVacuity {
            behavior: "acked".to_owned()
        })
    );
    assert_eq!(
        Optimization::new([], [objective("p"), objective("p")], []),
        Err(OptimizationError::DuplicateObjective {
            role: ObjectiveRole::Soft,
            objective: "p".to_owned()
        })
    );
    // One string may be a hard constraint, a soft objective, and a non-vacuity
    // behavior at once: the schema constrains each array on its own, and the three ask
    // different questions.
    let group =
        Optimization::new([objective("p")], [objective("p")], [behavior("p")]).expect("legal");
    assert_eq!(group.units().count(), 3);
    assert_eq!(
        group
            .units()
            .filter(|unit| unit.policy_field() == PolicyField::NonVacuity)
            .count(),
        1
    );
}
