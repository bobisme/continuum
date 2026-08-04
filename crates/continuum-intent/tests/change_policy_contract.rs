//! The field-level change policy against its two normative sources (PR-4 / IMPL-09).
//!
//! # What this file is evidence for
//!
//! RFC 0037 states the policy vocabulary as prose tables, and
//! `notes/plan/schemas/intent-contract.schema.json` states it structurally. INV-003
//! makes the schema the shape authority, so the code must implement the schema's
//! reading and the two must agree. This file asserts the agreement *mechanically*:
//! it parses the normative schema with this crate's own canonical-JSON reader and
//! compares the enums it finds against [`continuum_intent::change_policy`]'s types.
//!
//! A transcription that is only checked by eye drifts. A transcription checked
//! against the document it came from cannot: if the schema's `policy_verb` enum grows
//! a ninth member, or a `$defs/policy_verb_*` restriction moves a verb from one row of
//! the W6 matrix to another, the test below fails and names the difference.
//!
//! # The `policy` table is the enforcement surface
//!
//! The second half of the file is the P1–P6 verdict computation, exercised over every
//! field, every verb, and the eight gaming moves plan §5.1 lists — each of which must
//! forbid `allow` through the verb RFC 0031's table names for it.
//!
//! Nothing here shells out, reads a clock, or touches a network; `include_str!` is
//! compile-time, so the coupling to the schema is a build-time fact.

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptanceAuthority, AcceptancePath, ChangePolicyError, ClassificationRecord, EnforcementError,
    PolicyDecision, PolicyField, PolicyReviewers, PolicyTable, PolicyVerb, Relation,
    WellFormednessError,
};

/// The normative contract schema, included at compile time.
///
/// `notes/plan/tools/validate_dossier.py` validates the dossier's example instance
/// against this exact file on every `just check`; this test reads the same file from
/// the other side.
const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

/// The dossier's validated Intent Contract example.
const EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The example's `policy` object in canonical form — the golden vector for this group.
const EXAMPLE_POLICY: &str = concat!(
    r#"{"abstraction_maps":"review","assumptions":"review","assurance":"no-downgrade","#,
    r#""bounds":"no-decrease","completion_policy":"review","fairness":"review","#,
    r#""faults":"no-removal","non_vacuity":"no-removal","nondeterminism":"review","#,
    r#""observers":"review","optimization":"review","properties":"locked","scope":"review","#,
    r#""security_policy":"review","trust_boundaries":"no-expansion"}"#,
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

fn tokens(json: &Json) -> Vec<&str> {
    json.as_array()
        .expect("an enum is an array")
        .iter()
        .map(|item| item.as_str().expect("an enum member is a string"))
        .collect()
}

/// A full table carrying `verb` on `field` and `unlocked` everywhere else.
fn table_with(field: PolicyField, verb: PolicyVerb) -> PolicyTable {
    PolicyTable::new(PolicyField::ALL.map(|candidate| {
        if candidate == field {
            (candidate, verb)
        } else {
            (candidate, PolicyVerb::Unlocked)
        }
    }))
    .expect("the verb is admissible on its field")
}

/// A complete classification: `unchanged` on all fifteen but `relation` on `field`.
fn records_with(field: PolicyField, relation: Relation) -> Vec<ClassificationRecord> {
    PolicyField::ALL
        .into_iter()
        .map(|candidate| {
            let relation = if candidate == field {
                relation
            } else {
                Relation::Unchanged
            };
            ClassificationRecord::new(candidate, relation).expect("the relation is admissible")
        })
        .collect()
}

/// A reviewer map naming one principal for every field, so W7 never interferes.
fn every_field_reviewed() -> PolicyReviewers {
    PolicyReviewers::new(
        PolicyField::ALL
            .map(|field| (field, BTreeSet::from(["did:continuum:reviewer".to_owned()]))),
    )
    .expect("no field is named twice and no list is empty")
}

// --- provenance: the code is the schema's reading ---------------------------------------

#[test]
fn the_fifteen_policy_keys_are_the_schemas_policy_object_keys() {
    let schema = parsed(SCHEMA);
    let properties = at(&schema, &["properties", "policy", "properties"])
        .as_object()
        .expect("the policy object declares properties");
    let declared: Vec<&str> = properties.keys().map(String::as_str).collect();
    let ours: Vec<&str> = PolicyField::ALL.iter().map(|field| field.wire()).collect();
    assert_eq!(declared, ours);
    // And the schema requires all fifteen, so a table is never partial.
    let required: BTreeSet<&str> = tokens(at(&schema, &["properties", "policy", "required"]))
        .into_iter()
        .collect();
    assert_eq!(required, ours.iter().copied().collect::<BTreeSet<_>>());
    assert_eq!(required.len(), 15);
}

#[test]
fn the_eight_verbs_are_the_schemas_policy_verb_enum() {
    let schema = parsed(SCHEMA);
    let declared = tokens(at(&schema, &["$defs", "policy_verb", "enum"]));
    let ours: Vec<&str> = PolicyVerb::ALL.iter().map(|verb| verb.wire()).collect();
    assert_eq!(declared, ours);
    assert_eq!(declared.len(), 8);
}

#[test]
fn the_w6_matrix_is_the_schemas_five_per_field_restrictions() {
    // The provenance test for the verb → field table. For every one of the fifteen
    // keys, follow the `$ref` the schema's `policy` object gives it, read the
    // restriction's own `enum`, and compare with `PolicyField::admissible_verbs`.
    let schema = parsed(SCHEMA);
    let properties = at(&schema, &["properties", "policy", "properties"])
        .as_object()
        .expect("properties");
    let mut rows: BTreeMap<&str, Vec<PolicyField>> = BTreeMap::new();
    for field in PolicyField::ALL {
        let reference = at(
            &schema,
            &["properties", "policy", "properties", field.wire()],
        )
        .as_object()
        .expect("a per-field restriction")
        .get("$ref")
        .and_then(Json::as_str)
        .expect("every policy key is a $ref to one of the five restrictions");
        let def = reference
            .strip_prefix("#/$defs/")
            .expect("the restrictions live in $defs");
        let declared = tokens(at(&schema, &["$defs", def, "enum"]));
        let ours: Vec<&str> = field
            .admissible_verbs()
            .iter()
            .map(|verb| verb.wire())
            .collect();
        assert_eq!(
            declared, ours,
            "the W6 row for {field} disagrees with the schema's {def}"
        );
        rows.entry(def).or_default().push(field);
    }
    assert_eq!(properties.len(), 15);
    // Five restrictions, and the eight-field base row is the largest — the RFC's
    // table shape, recovered from the schema rather than asserted beside it.
    assert_eq!(rows.len(), 5);
    assert_eq!(rows["policy_verb_base"].len(), 8);
    assert_eq!(rows["policy_verb_bounds"], vec![PolicyField::Bounds]);
    assert_eq!(rows["policy_verb_assurance"], vec![PolicyField::Assurance]);
    assert_eq!(
        rows["policy_verb_trust_boundaries"],
        vec![PolicyField::TrustBoundaries]
    );
    assert_eq!(rows["policy_verb_removal"].len(), 4);
}

#[test]
fn the_contract_path_function_is_the_schemas_x_policy_field_map() {
    let schema = parsed(SCHEMA);
    let map = at(&schema, &["x-policy-field-map"])
        .as_object()
        .expect("the normative policy ↦ contract-path map");
    for field in PolicyField::ALL {
        let declared = map
            .get(field.wire())
            .and_then(Json::as_str)
            .unwrap_or_else(|| panic!("the map names {field}"));
        assert_eq!(declared, field.contract_path(), "{field}");
    }
    // Sixteen keys would mean a policy key with no contract group, or the reverse.
    assert_eq!(map.len(), 16, "fifteen fields plus the map's own $comment");
}

#[test]
fn the_reviewer_maps_keys_are_the_fifteen_policy_keys() {
    let schema = parsed(SCHEMA);
    let declared: BTreeSet<&str> = tokens(at(
        &schema,
        &["properties", "policy_reviewers", "propertyNames", "enum"],
    ))
    .into_iter()
    .collect();
    let ours: BTreeSet<&str> = PolicyField::ALL.iter().map(|field| field.wire()).collect();
    assert_eq!(declared, ours);
    // `minItems: 1` is W7's first half, and it is the reason an empty principal list
    // cannot be held.
    assert_eq!(
        at(
            &schema,
            &[
                "properties",
                "policy_reviewers",
                "additionalProperties",
                "minItems"
            ]
        )
        .as_integer(),
        Some(1)
    );
}

#[test]
fn the_relation_vocabulary_is_the_semantic_diff_schemas() {
    // RFC 0031's sixteen relations, transcribed from the sibling schema's enum. The
    // file is read through the same reader, so a drift in either direction fails.
    const DIFF_SCHEMA: &str = include_str!("../../../notes/plan/schemas/semantic-diff.schema.json");
    let schema = parsed(DIFF_SCHEMA);
    let declared = tokens(at(
        &schema,
        &[
            "properties",
            "intent_changes",
            "items",
            "properties",
            "relation",
            "enum",
        ],
    ));
    let ours: Vec<&str> = Relation::ALL
        .iter()
        .map(|relation| relation.wire())
        .collect();
    assert_eq!(declared, ours);
    // And the diff's `field` enum is the same fifteen policy keys, in the RFC's
    // protected-set presentation order rather than code-point order.
    let fields: BTreeSet<&str> = tokens(at(
        &schema,
        &[
            "properties",
            "intent_changes",
            "items",
            "properties",
            "field",
            "enum",
        ],
    ))
    .into_iter()
    .collect();
    assert_eq!(
        fields,
        PolicyField::ALL
            .iter()
            .map(|field| field.wire())
            .collect::<BTreeSet<_>>()
    );
    // The three-value decision, with no fourth `unknown`.
    assert_eq!(
        tokens(at(
            &schema,
            &["properties", "policy", "properties", "decision", "enum"],
        )),
        PolicyDecision::ALL
            .iter()
            .map(|decision| decision.wire())
            .collect::<Vec<_>>()
    );
}

// --- the artifact form -------------------------------------------------------------------

#[test]
fn the_dossiers_example_policy_table_round_trips_byte_for_byte() {
    let example = parsed(EXAMPLE);
    let table = PolicyTable::from_json(at(&example, &["policy"])).expect("the example decodes");
    assert_eq!(table.to_artifact_bytes(), EXAMPLE_POLICY.as_bytes());
    assert_eq!(
        PolicyTable::decode(EXAMPLE_POLICY.as_bytes()).expect("the golden form decodes"),
        table
    );
    // Spot-check the verbs the plan §5.4 example is famous for.
    assert_eq!(table.verb(PolicyField::Properties), PolicyVerb::Locked);
    assert_eq!(table.verb(PolicyField::Bounds), PolicyVerb::NoDecrease);
    assert_eq!(table.verb(PolicyField::Assurance), PolicyVerb::NoDowngrade);
    assert_eq!(table.verb(PolicyField::NonVacuity), PolicyVerb::NoRemoval);
    assert_eq!(
        table.verb(PolicyField::TrustBoundaries),
        PolicyVerb::NoExpansion
    );
}

#[test]
fn the_dossiers_example_names_reviewers_for_its_nine_review_verbs() {
    // The example instance is schema-valid, and — since the bn-16pyr correction pass
    // fixed the W7 flag this test used to pin — it is also W7-well-formed: nine
    // fields carry `review` and `policy_reviewers` names a principal for each of
    // them. W7 is a checker rule the schema cannot state (RFC 0037 F3), which is
    // exactly why the checker exists; the empty-reviewers half below keeps the
    // unenforceable-block rejection itself pinned.
    let example = parsed(EXAMPLE);
    let reviewers = PolicyReviewers::from_json(
        example
            .as_object()
            .expect("object")
            .get("policy_reviewers")
            .expect("the example names reviewers for its review verbs (bn-16pyr)"),
    )
    .expect("decodes");
    let table = PolicyTable::from_json(at(&example, &["policy"])).expect("decodes");
    let reviewing = table
        .iter()
        .filter(|(_, verb)| *verb == PolicyVerb::Review)
        .count();
    assert_eq!(reviewing, 9);
    assert_eq!(table.check_reviewers(&reviewers), Ok(()));
    // The rejection W7 exists for is still pinned: strip the reviewers and the
    // first review-verb field (in field order) is an unenforceable block.
    assert_eq!(
        table.check_reviewers(&PolicyReviewers::empty()),
        Err(WellFormednessError::UnenforceableReview {
            field: PolicyField::AbstractionMaps
        })
    );
}

#[test]
fn key_order_is_normalized_away_and_the_identity_is_the_canonical_bytes() {
    let reordered = format!(
        "{{{}}}",
        PolicyField::ALL
            .iter()
            .rev()
            .map(|field| format!(r#""{}":"unlocked""#, field.wire()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let table = PolicyTable::decode(reordered.as_bytes()).expect("JSON objects are unordered");
    assert_eq!(table, PolicyTable::all_unlocked());
    assert_eq!(
        table.to_artifact_bytes(),
        PolicyTable::all_unlocked().to_artifact_bytes()
    );
    // The identity *is* the bytes, readable as the canonical JSON itself (ADR-0013).
    assert_eq!(
        table.identity().canonical_bytes(),
        table.identity().to_string().as_bytes()
    );
    assert_eq!(
        table.identity().canonical_bytes(),
        table.to_artifact_bytes()
    );
}

#[test]
fn changing_one_verb_moves_the_identity_and_nothing_else_does() {
    // ID7: "Changing any […] policy verb, or reviewer list […] MUST change [the
    // identity]", and ID3 records the consequence: `intent.lock` mints a successor.
    let base = PolicyTable::all_unlocked();
    for field in PolicyField::ALL {
        let locked = table_with(field, PolicyVerb::Locked);
        assert_ne!(locked.identity(), base.identity(), "{field}");
        assert_ne!(locked, base, "{field}");
    }
    // Two tables built from differently ordered inputs are one table with one identity.
    let forwards =
        PolicyTable::new(PolicyField::ALL.map(|f| (f, PolicyVerb::Unlocked))).expect("admissible");
    let mut backwards: Vec<_> = PolicyField::ALL.map(|f| (f, PolicyVerb::Unlocked)).to_vec();
    backwards.reverse();
    let backwards = PolicyTable::new(backwards).expect("admissible");
    assert_eq!(forwards.identity(), backwards.identity());
}

#[test]
fn a_reviewer_list_is_a_set_with_one_spelling() {
    // The schema declares `minItems` but no `uniqueItems` on the principal arrays, so
    // a repeat is schema-legal; `policy_reviewers` is in the identity preimage, so it
    // must have one spelling. The reader is liberal, the writer is not.
    let repeated = r#"{"assumptions":["b","a","b"]}"#;
    let reviewers = PolicyReviewers::decode(repeated.as_bytes()).expect("schema-legal input");
    assert_eq!(
        String::from_utf8(reviewers.to_artifact_bytes()).expect("utf-8"),
        r#"{"assumptions":["a","b"]}"#
    );
    assert_eq!(
        reviewers
            .principals(PolicyField::Assumptions)
            .expect("named")
            .len(),
        2
    );
    assert_eq!(reviewers.len(), 1);
    assert!(!reviewers.is_empty());
    assert!(PolicyReviewers::empty().is_empty());
}

// --- W6 and W7 ----------------------------------------------------------------------------

#[test]
fn w6_rejects_the_two_inapplicable_verbs_the_rfc_names() {
    // RFC 0037's acceptance list asks for exactly this vector: "an inapplicable verb
    // (`no-decrease` on `properties`)".
    assert_eq!(
        PolicyTable::new(PolicyField::ALL.map(|field| {
            if field == PolicyField::Properties {
                (field, PolicyVerb::NoDecrease)
            } else {
                (field, PolicyVerb::Unlocked)
            }
        })),
        Err(ChangePolicyError::InapplicableVerb {
            field: PolicyField::Properties,
            verb: PolicyVerb::NoDecrease
        })
    );
    // "`no-downgrade` outside `assurance`, which is the only field that emits
    // `downgraded`."
    for field in PolicyField::ALL {
        let admissible = PolicyVerb::NoDowngrade.is_admissible_on(field);
        assert_eq!(admissible, field == PolicyField::Assurance, "{field}");
    }
    // `no-decrease` on `trust_boundaries` "would block the *safe* direction […] and is
    // ill-formed there".
    assert!(!PolicyVerb::NoDecrease.is_admissible_on(PolicyField::TrustBoundaries));
    assert!(PolicyVerb::NoExpansion.is_admissible_on(PolicyField::TrustBoundaries));
}

#[test]
fn every_field_admits_the_four_non_directional_verbs_and_at_most_one_directional() {
    for field in PolicyField::ALL {
        let verbs = field.admissible_verbs();
        for base in [
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
        ] {
            assert!(verbs.contains(&base), "{field} does not admit {base}");
        }
        let directional = verbs
            .iter()
            .filter(|verb| !verb.denied_relations().is_empty() && **verb != PolicyVerb::Locked)
            .count();
        assert!(
            directional <= 1,
            "{field} admits {directional} directional verbs"
        );
        // A directional verb is admissible only where the relation it denies is one
        // the field's own order produces.
        for verb in verbs {
            for denied in verb.denied_relations() {
                if *verb != PolicyVerb::Locked {
                    assert!(
                        field.classified_relations().contains(denied),
                        "{field} admits {verb}, which denies {denied}, a relation the field never \
                         produces"
                    );
                }
            }
        }
    }
}

#[test]
fn w7_is_structural_where_it_can_be_and_relational_where_it_cannot() {
    // First half: a key outside the fifteen cannot be held, and an empty list cannot.
    assert_eq!(
        PolicyReviewers::new([(PolicyField::Fairness, BTreeSet::new())]),
        Err(ChangePolicyError::EmptyReviewerList {
            field: PolicyField::Fairness
        })
    );
    // Second half: the relation between the two objects.
    let table = table_with(PolicyField::Fairness, PolicyVerb::Review);
    assert_eq!(
        table.check_reviewers(&PolicyReviewers::empty()),
        Err(WellFormednessError::UnenforceableReview {
            field: PolicyField::Fairness
        })
    );
    let named = PolicyReviewers::new([(
        PolicyField::Fairness,
        BTreeSet::from(["did:continuum:alice".to_owned()]),
    )])
    .expect("one non-empty list");
    assert_eq!(table.check_reviewers(&named), Ok(()));
    // A reviewer named for a field that does not need one is not an error: W7 requires
    // reviewers where `review` is set, and says nothing about the converse.
    assert_eq!(PolicyTable::all_unlocked().check_reviewers(&named), Ok(()));
}

// --- P1 through P6 -------------------------------------------------------------------------

#[test]
fn p1_a_partial_classification_cannot_reach_a_verdict() {
    let table = PolicyTable::all_unlocked();
    let mut records = records_with(PolicyField::Bounds, Relation::Unchanged);
    records.retain(|record| record.field() != PolicyField::Faults);
    assert_eq!(
        table.verdict(
            &records,
            &PolicyReviewers::empty(),
            AcceptancePath::HumanAccept
        ),
        Err(EnforcementError::IncompleteClassification {
            field: PolicyField::Faults
        })
    );
    // An empty classification is the degenerate partial one.
    assert!(matches!(
        table.verdict(&[], &PolicyReviewers::empty(), AcceptancePath::HumanAccept),
        Err(EnforcementError::IncompleteClassification { .. })
    ));
}

#[test]
fn p2_a_non_affirmative_relation_forbids_allow_on_every_field_under_every_verb() {
    // "This holds for every field and every verb — the fail-closed rule is not a
    // property of the directional verbs alone."
    //
    // One combination is excluded from the sweep: `assurance` × `incomparable`. RFC
    // 0031 correction 13's parenthetical excludes exactly that pair from `assurance`'s
    // admissible relations (bn-2sngz, closing the over-admission bn-3vxp pinned), so
    // `records_with` cannot construct it — `ClassificationRecord::new` now returns
    // `InadmissibleRelation` there, checked separately below rather than folded into
    // this loop's uniform `.expect`.
    let reviewers = every_field_reviewed();
    for field in PolicyField::ALL {
        for verb in field.admissible_verbs() {
            for relation in [
                Relation::Unknown,
                Relation::Unsupported,
                Relation::Incomparable,
            ] {
                if field == PolicyField::Assurance && relation == Relation::Incomparable {
                    continue;
                }
                let table = table_with(field, *verb);
                let verdict = table
                    .verdict(
                        &records_with(field, relation),
                        &reviewers,
                        AcceptancePath::HumanAccept,
                    )
                    .expect("a complete classification");
                let expected = if *verb == PolicyVerb::Locked {
                    PolicyDecision::Block
                } else {
                    PolicyDecision::Review
                };
                assert_eq!(
                    verdict.decision(),
                    expected,
                    "{field} classified {relation} under {verb}"
                );
                assert_eq!(verdict.reasons().len(), 1);
                assert_eq!(verdict.reasons()[0].field(), field);
                assert_eq!(verdict.reasons()[0].relation(), relation);
            }
        }
    }
}

#[test]
fn p2_excludes_assurance_incomparable_per_correction_13_rather_than_silently_skipping_it() {
    // The gap in the sweep above is a typed rejection, not an untested corner:
    // `assurance` refuses to record `incomparable` at all (RFC 0031 correction 13's
    // parenthetical), so there is no record for P2 to ever apply the fail-closed rule
    // to on that pair.
    assert_eq!(
        ClassificationRecord::new(PolicyField::Assurance, Relation::Incomparable),
        Err(ChangePolicyError::InadmissibleRelation {
            field: PolicyField::Assurance,
            relation: Relation::Incomparable
        })
    );
}

#[test]
fn p3_each_directional_verb_blocks_exactly_the_relation_it_denies() {
    let reviewers = every_field_reviewed();
    for (field, verb, denied, permitted) in [
        (
            PolicyField::Bounds,
            PolicyVerb::NoDecrease,
            Relation::Contracted,
            Relation::Expanded,
        ),
        (
            PolicyField::Faults,
            PolicyVerb::NoRemoval,
            Relation::Removed,
            Relation::Added,
        ),
        (
            PolicyField::Assurance,
            PolicyVerb::NoDowngrade,
            Relation::Downgraded,
            Relation::Upgraded,
        ),
        (
            PolicyField::TrustBoundaries,
            PolicyVerb::NoExpansion,
            Relation::Expanded,
            Relation::Contracted,
        ),
    ] {
        let table = table_with(field, verb);
        let blocked = table
            .verdict(
                &records_with(field, denied),
                &reviewers,
                AcceptancePath::HumanAccept,
            )
            .expect("complete");
        assert_eq!(
            blocked.decision(),
            PolicyDecision::Block,
            "{field}/{denied}"
        );
        assert_eq!(blocked.reasons()[0].verb(), verb);
        let allowed = table
            .verdict(
                &records_with(field, permitted),
                &reviewers,
                AcceptancePath::HumanAccept,
            )
            .expect("complete");
        assert_eq!(
            allowed.decision(),
            PolicyDecision::Allow,
            "{field}/{permitted} is the safe direction and the verb does not block it"
        );
        assert!(allowed.reasons().is_empty());
    }
}

#[test]
fn p4_the_acceptance_authority_applies_when_the_relation_is_not_denied() {
    let reviewers = every_field_reviewed();
    let field = PolicyField::Scope;
    let changed = Relation::Added;

    // `unlocked` contributes allow.
    let unlocked = table_with(field, PolicyVerb::Unlocked);
    assert_eq!(
        unlocked
            .verdict(
                &records_with(field, changed),
                &reviewers,
                AcceptancePath::AgentAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Allow
    );

    // `proposal-only` contributes allow only on a human-acceptance path.
    let proposal = table_with(field, PolicyVerb::ProposalOnly);
    assert_eq!(
        proposal
            .verdict(
                &records_with(field, changed),
                &reviewers,
                AcceptancePath::HumanAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Allow
    );
    assert_eq!(
        proposal
            .verdict(
                &records_with(field, changed),
                &reviewers,
                AcceptancePath::AgentAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Review
    );

    // `review` contributes review for any non-`unchanged` relation, and names the
    // reviewers; `unchanged` still allows.
    let review = table_with(field, PolicyVerb::Review);
    let verdict = review
        .verdict(
            &records_with(field, changed),
            &reviewers,
            AcceptancePath::HumanAccept,
        )
        .expect("complete");
    assert_eq!(verdict.decision(), PolicyDecision::Review);
    assert!(
        verdict.reasons()[0]
            .reviewers()
            .contains("did:continuum:reviewer")
    );
    assert_eq!(
        review
            .verdict(
                &records_with(field, Relation::Unchanged),
                &reviewers,
                AcceptancePath::HumanAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Allow
    );

    // `locked` blocks any non-`unchanged` relation.
    let locked = table_with(field, PolicyVerb::Locked);
    assert_eq!(
        locked
            .verdict(
                &records_with(field, changed),
                &reviewers,
                AcceptancePath::HumanAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Block
    );
    assert_eq!(
        locked
            .verdict(
                &records_with(field, Relation::Unchanged),
                &reviewers,
                AcceptancePath::HumanAccept
            )
            .expect("complete")
            .decision(),
        PolicyDecision::Allow
    );
}

#[test]
fn p5_the_verdict_is_the_join_and_p6_names_every_record_that_forbade_allow() {
    let reviewers = every_field_reviewed();
    let table = PolicyTable::new(PolicyField::ALL.map(|field| match field {
        PolicyField::Bounds => (field, PolicyVerb::NoDecrease),
        PolicyField::Observers => (field, PolicyVerb::Review),
        _ => (field, PolicyVerb::Unlocked),
    }))
    .expect("admissible");
    let records: Vec<ClassificationRecord> = PolicyField::ALL
        .into_iter()
        .map(|field| {
            let relation = match field {
                // Blocks.
                PolicyField::Bounds => Relation::Contracted,
                // Reviews.
                PolicyField::Observers => Relation::Coarsened,
                // Allows: a change on an unlocked field still appears in the
                // classification, and still contributes nothing further.
                PolicyField::Scope => Relation::Added,
                _ => Relation::Unchanged,
            };
            ClassificationRecord::new(field, relation).expect("admissible")
        })
        .collect();
    let verdict = table
        .verdict(&records, &reviewers, AcceptancePath::HumanAccept)
        .expect("complete");
    // P5: block joins over review joins over allow.
    assert_eq!(verdict.decision(), PolicyDecision::Block);
    // P6: exactly the two records that forbade allow, each naming field and relation.
    assert_eq!(verdict.reasons().len(), 2);
    let rendered: Vec<String> = verdict
        .reasons()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    assert!(rendered.iter().any(|reason| reason.contains("bounds")
        && reason.contains("contracted")
        && reason.contains("block")));
    assert!(rendered.iter().any(|reason| reason.contains("observers")
        && reason.contains("coarsened")
        && reason.contains("review")));
    // The allowed `scope` change is in the classification and not in the reasons.
    assert!(!rendered.iter().any(|reason| reason.contains("scope")));
    // And the join is the total order, not a tally.
    assert_eq!(
        PolicyDecision::Allow.join(PolicyDecision::Review),
        PolicyDecision::Review
    );
    assert_eq!(
        PolicyDecision::Review.join(PolicyDecision::Block),
        PolicyDecision::Block
    );
    assert_eq!(
        PolicyDecision::Block.join(PolicyDecision::Allow),
        PolicyDecision::Block
    );
}

#[test]
fn every_plan_5_1_gaming_move_forbids_allow_through_the_verb_rfc_0031_names() {
    // RFC 0031, "Completeness guarantee": eight moves, eight fields, eight relations,
    // eight blocking verbs. Each row must forbid `allow` — four through a directional
    // block, four through the review path.
    let reviewers = every_field_reviewed();
    for (field, relation, verb, expected) in [
        (
            PolicyField::Observers,
            Relation::Coarsened,
            PolicyVerb::Review,
            PolicyDecision::Review,
        ),
        (
            PolicyField::Fairness,
            Relation::Added,
            PolicyVerb::Review,
            PolicyDecision::Review,
        ),
        (
            PolicyField::Bounds,
            Relation::Contracted,
            PolicyVerb::NoDecrease,
            PolicyDecision::Block,
        ),
        (
            PolicyField::Faults,
            Relation::Removed,
            PolicyVerb::NoRemoval,
            PolicyDecision::Block,
        ),
        (
            PolicyField::TrustBoundaries,
            Relation::Expanded,
            PolicyVerb::NoExpansion,
            PolicyDecision::Block,
        ),
        (
            PolicyField::AbstractionMaps,
            Relation::Merged,
            PolicyVerb::Review,
            PolicyDecision::Review,
        ),
        (
            PolicyField::Assurance,
            Relation::Downgraded,
            PolicyVerb::NoDowngrade,
            PolicyDecision::Block,
        ),
        (
            PolicyField::Properties,
            Relation::Weakened,
            PolicyVerb::Review,
            PolicyDecision::Review,
        ),
    ] {
        let verdict = table_with(field, verb)
            .verdict(
                &records_with(field, relation),
                &reviewers,
                AcceptancePath::HumanAccept,
            )
            .expect("complete");
        assert_eq!(
            verdict.decision(),
            expected,
            "{field} / {relation} / {verb}"
        );
        assert_ne!(verdict.decision(), PolicyDecision::Allow);
        // And the non-vacuity removal INV-012 protects, which the plan §5.1 table
        // omits but plan §5.4's example locks.
    }
    let verdict = table_with(PolicyField::NonVacuity, PolicyVerb::NoRemoval)
        .verdict(
            &records_with(PolicyField::NonVacuity, Relation::Removed),
            &reviewers,
            AcceptancePath::HumanAccept,
        )
        .expect("complete");
    assert_eq!(verdict.decision(), PolicyDecision::Block);
}

#[test]
fn a_verdict_is_refused_rather_than_computed_around_an_unenforceable_review() {
    let table = table_with(PolicyField::Fairness, PolicyVerb::Review);
    assert_eq!(
        table.verdict(
            &records_with(PolicyField::Fairness, Relation::Unchanged),
            &PolicyReviewers::empty(),
            AcceptancePath::HumanAccept
        ),
        Err(EnforcementError::UnenforceableReview {
            field: PolicyField::Fairness
        })
    );
}

#[test]
fn a_relation_the_fields_order_cannot_produce_is_a_typed_rejection() {
    assert_eq!(
        ClassificationRecord::new(PolicyField::Bounds, Relation::Downgraded),
        Err(ChangePolicyError::InadmissibleRelation {
            field: PolicyField::Bounds,
            relation: Relation::Downgraded
        })
    );
    assert_eq!(
        ClassificationRecord::new(PolicyField::CompletionPolicy, Relation::Strengthened),
        Err(ChangePolicyError::InadmissibleRelation {
            field: PolicyField::CompletionPolicy,
            relation: Relation::Strengthened
        })
    );
    // `incomparable` is also a relation `assurance`'s order cannot produce — RFC 0031
    // correction 13's parenthetical: "the non-affirmative three are admissible on
    // every row (with `incomparable` excluded from `assurance` alone, per its own
    // total-order rule)". `assurance`'s three affirmative relations (`unchanged`,
    // `upgraded`, `downgraded`) already dispose of every comparable pair, so there is
    // no "both inclusions refuted" case left for `incomparable` to name.
    assert_eq!(
        ClassificationRecord::new(PolicyField::Assurance, Relation::Incomparable),
        Err(ChangePolicyError::InadmissibleRelation {
            field: PolicyField::Assurance,
            relation: Relation::Incomparable
        })
    );
    // The non-affirmative three are admissible everywhere else, including on the five
    // membership-only fields RFC 0031's table omits them from — refusing them would
    // refuse the fail-closed answer.
    for field in PolicyField::ALL {
        for relation in [
            Relation::Unknown,
            Relation::Unsupported,
            Relation::Incomparable,
        ] {
            if field == PolicyField::Assurance && relation == Relation::Incomparable {
                continue;
            }
            assert!(
                ClassificationRecord::new(field, relation).is_ok(),
                "{field} cannot record {relation}"
            );
        }
    }
}

// --- policy joins ---------------------------------------------------------------------------

#[test]
fn a_representable_join_merges_and_an_unrepresentable_one_is_a_conflict() {
    let unlocked = PolicyTable::all_unlocked();
    let example = PolicyTable::decode(EXAMPLE_POLICY.as_bytes()).expect("the golden table");
    // One side `unlocked`: the join is the other side, on every field.
    let merged = unlocked.join(&example).expect("representable");
    assert_eq!(merged, example);
    assert_eq!(example.join(&unlocked).expect("representable"), example);
    // Two identical tables join to themselves.
    assert_eq!(example.join(&example).expect("representable"), example);
    // `no-decrease ⊔ review` on `bounds` is the RFC's own unrepresentable example.
    let reviewed_bounds = table_with(PolicyField::Bounds, PolicyVerb::Review);
    let no_decrease = table_with(PolicyField::Bounds, PolicyVerb::NoDecrease);
    let conflict = reviewed_bounds
        .join(&no_decrease)
        .expect_err("no closed-set verb names ({contracted}, named-reviewer)");
    assert_eq!(
        conflict.fields(),
        [(
            PolicyField::Bounds,
            PolicyVerb::Review,
            PolicyVerb::NoDecrease
        )]
    );
    assert!(conflict.to_string().contains("bounds"));
}

#[test]
fn a_merge_is_never_strictly_more_permissive_than_either_input() {
    // RFC 0037: "A merge MUST NOT produce a policy table strictly more permissive than
    // either input on any field." The join is the least upper bound, so this holds by
    // construction — and here it is checked over every representable pair.
    for field in PolicyField::ALL {
        for left in field.admissible_verbs() {
            for right in field.admissible_verbs() {
                if let Some(joined) = left.join(*right) {
                    assert!(left.is_weaker_or_equal(joined), "{left} ⊔ {right}");
                    assert!(right.is_weaker_or_equal(joined), "{left} ⊔ {right}");
                    // Least: no strictly smaller upper bound exists in the closed set.
                    for candidate in PolicyVerb::ALL {
                        if left.is_weaker_or_equal(candidate) && right.is_weaker_or_equal(candidate)
                        {
                            assert!(
                                joined.is_weaker_or_equal(candidate),
                                "{left} ⊔ {right} = {joined} is not least below {candidate}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_authority_ladder_is_the_rfcs_order() {
    assert_eq!(
        AcceptanceAuthority::ALL,
        [
            AcceptanceAuthority::Ordinary,
            AcceptanceAuthority::Proposal,
            AcceptanceAuthority::NamedReviewer,
            AcceptanceAuthority::PolicyAmendment,
        ]
    );
    assert!(AcceptanceAuthority::Ordinary < AcceptanceAuthority::Proposal);
    assert!(AcceptanceAuthority::Proposal < AcceptanceAuthority::NamedReviewer);
    assert!(AcceptanceAuthority::NamedReviewer < AcceptanceAuthority::PolicyAmendment);
    assert_eq!(
        PolicyVerb::Locked.authority(),
        AcceptanceAuthority::PolicyAmendment
    );
    assert_eq!(
        PolicyVerb::NoRemoval.authority(),
        AcceptanceAuthority::Ordinary
    );
    assert_eq!(
        AcceptanceAuthority::NamedReviewer.to_string(),
        "named-reviewer"
    );
}
