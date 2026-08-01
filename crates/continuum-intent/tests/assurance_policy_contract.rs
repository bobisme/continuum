//! The `assurance` field group against the schema, the landed ladder, and RFC 0031's
//! movement rules (PR-4 / IMPL-07).
//!
//! # What this file is evidence for
//!
//! Three documents have to agree about assurance, and RFC 0037 says so outright:
//!
//! > The `assurance.minimum` enum and its order are shared with RFC 0031 and with
//! > `crates/continuum-value/src/assurance.rs`. Reordering or renaming any member is a
//! > coordinated breaking change across this RFC, RFC 0031, the contract schema, and
//! > that crate.
//!
//! So the first section reads the normative schema — through this crate's own
//! canonical-JSON reader, at compile time — and compares its two enums against
//! `continuum-value`'s landed `AssuranceLevel::ALL` and `EvidenceClass::ALL`. If the
//! schema and the crate ever disagree in membership, naming, or order, this test names
//! the difference instead of leaving a checker to guess which one is right.
//!
//! The rest is the group's own obligations: one canonical spelling per meaning, an
//! identity that moves with every semantic part and with nothing else, the 5×5×2×2
//! movement matrix RFC 0031's acceptance criteria ask for, and the guard that a
//! demanded level is validated against the closed five.

use continuum_intent::assurance_policy::{
    AssuranceComparisonError, AssurancePolicy, evidence_class_from_wire, level_from_wire,
};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::Relation;
use continuum_value::assurance::{AssuranceChange, AssuranceLevel, EvidenceClass};
use continuum_value::identity::{ContentHasher, Fnv1aPlaceholder};

const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");
const EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The example contract's `assurance` block in canonical form — the golden vector.
const EXAMPLE_ASSURANCE: &str = concat!(
    r#"{"accepted_evidence_classes":[],"clean_recompute":true,"independent_checker":true,"#,
    r#""minimum":"bounded"}"#,
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

fn policy(
    minimum: AssuranceLevel,
    independent: Option<bool>,
    clean: Option<bool>,
    classes: impl IntoIterator<Item = EvidenceClass>,
) -> AssurancePolicy {
    AssurancePolicy::new(minimum, independent, clean, classes).expect("no duplicate class")
}

// --- provenance: schema, crate, and RFC agree ---------------------------------------------

#[test]
fn the_five_levels_are_the_schemas_enum_and_the_landed_ladder() {
    let schema = parsed(SCHEMA);
    let declared = tokens(at(
        &schema,
        &["properties", "assurance", "properties", "minimum", "enum"],
    ));
    let landed: Vec<&str> = AssuranceLevel::ALL
        .iter()
        .map(|level| level.as_str())
        .collect();
    assert_eq!(
        declared, landed,
        "the schema's assurance.minimum enum and continuum-value's AssuranceLevel disagree"
    );
    assert_eq!(
        declared,
        ["observed", "sampled", "bounded", "validated", "proved"]
    );
    // The declaration order *is* the total order, so the derived `Ord` is RFC 0031's.
    for window in AssuranceLevel::ALL.windows(2) {
        assert!(
            window[0] < window[1],
            "{:?} is not below {:?}",
            window[0],
            window[1]
        );
    }
    // And `minimum` is the group's one required key (RFC 0037 correction 6).
    assert_eq!(
        tokens(at(&schema, &["properties", "assurance", "required"])),
        ["minimum"]
    );
}

#[test]
fn the_thirteen_evidence_classes_are_the_schemas_enum_and_the_landed_set() {
    let schema = parsed(SCHEMA);
    let declared = tokens(at(
        &schema,
        &[
            "properties",
            "assurance",
            "properties",
            "accepted_evidence_classes",
            "items",
            "enum",
        ],
    ));
    let landed: Vec<&str> = EvidenceClass::ALL
        .iter()
        .map(|class| class.as_str())
        .collect();
    assert_eq!(declared, landed);
    assert_eq!(declared.len(), 13);
    // The set is a set: the schema says so, and the type rejects a repeat.
    assert_eq!(
        at(
            &schema,
            &[
                "properties",
                "assurance",
                "properties",
                "accepted_evidence_classes",
                "uniqueItems",
            ],
        )
        .as_bool(),
        Some(true)
    );
}

#[test]
fn the_two_vocabularies_share_one_token_and_nothing_else() {
    // RFC 0037: "different vocabularies that share the token `sampled`. A level is what
    // the intent *demands*; an evidence class is what an artifact *is*."
    let levels: Vec<&str> = AssuranceLevel::ALL.iter().map(|l| l.as_str()).collect();
    let classes: Vec<&str> = EvidenceClass::ALL.iter().map(|c| c.as_str()).collect();
    let shared: Vec<&&str> = levels
        .iter()
        .filter(|token| classes.contains(token))
        .collect();
    assert_eq!(shared, [&"sampled"]);
    // And the shared token resolves in both directions, to two different things.
    assert_eq!(level_from_wire("sampled"), Some(AssuranceLevel::Sampled));
    assert_eq!(
        evidence_class_from_wire("sampled"),
        Some(EvidenceClass::Sampled)
    );
    // Nothing else crosses: a level token is not a class and a class token is not a
    // level, so neither vocabulary can be derived from the other by accident.
    for level in AssuranceLevel::ALL {
        if level != AssuranceLevel::Sampled {
            assert_eq!(evidence_class_from_wire(level.as_str()), None, "{level}");
        }
    }
    for class in EvidenceClass::ALL {
        if class != EvidenceClass::Sampled {
            assert_eq!(level_from_wire(class.as_str()), None, "{class}");
        }
    }
}

// --- the artifact form ----------------------------------------------------------------------

#[test]
fn the_dossiers_example_assurance_block_round_trips_byte_for_byte() {
    let example = parsed(EXAMPLE);
    let group =
        AssurancePolicy::from_json(at(&example, &["assurance"])).expect("the example decodes");
    assert_eq!(group.to_artifact_bytes(), EXAMPLE_ASSURANCE.as_bytes());
    assert_eq!(
        AssurancePolicy::decode(EXAMPLE_ASSURANCE.as_bytes()).expect("the golden form decodes"),
        group
    );
    assert_eq!(group.minimum(), AssuranceLevel::Bounded);
    assert_eq!(group.demanded_of_every_claim(), AssuranceLevel::Bounded);
    assert_eq!(group.independent_checker(), Some(true));
    assert_eq!(group.clean_recompute(), Some(true));
    assert_eq!(group.accepted_len(), 0);
}

#[test]
fn an_absent_accepted_set_and_an_explicit_empty_one_are_one_meaning_with_one_spelling() {
    // RFC 0037: an absent optional set "MUST be read as the empty set […] and MUST be
    // encoded explicitly as the empty set when the identity preimage is built (ID5), so
    // that 'absent' and 'empty' cannot yield two identities for one meaning."
    let absent = AssurancePolicy::decode(br#"{"minimum":"bounded"}"#).expect("absent is legal");
    let empty = AssurancePolicy::decode(br#"{"accepted_evidence_classes":[],"minimum":"bounded"}"#)
        .expect("empty is legal");
    assert_eq!(absent, empty);
    assert_eq!(absent.identity(), empty.identity());
    assert_eq!(absent.to_artifact_bytes(), empty.to_artifact_bytes());
    assert_eq!(
        String::from_utf8(absent.to_artifact_bytes()).expect("utf-8"),
        r#"{"accepted_evidence_classes":[],"minimum":"bounded"}"#
    );
}

#[test]
fn an_undeclared_checker_flag_survives_the_encoding_and_is_not_read_as_false() {
    // The two flags are the one place in this group where absence is a *distinct*
    // meaning, because RFC 0037 gives optional sets a default and optional scalars
    // none: "A checker MUST NOT invent a value for a key the schema leaves optional."
    let undeclared = AssurancePolicy::decode(br#"{"minimum":"proved"}"#).expect("legal");
    let off = AssurancePolicy::decode(
        br#"{"clean_recompute":false,"independent_checker":false,"minimum":"proved"}"#,
    )
    .expect("legal");
    assert_ne!(undeclared, off);
    assert_ne!(undeclared.identity(), off.identity());
    assert_eq!(undeclared.independent_checker(), None);
    assert_eq!(off.independent_checker(), Some(false));
    assert!(undeclared.requirement().is_none());
    assert!(off.requirement().is_some());
    // And the encoding preserves the distinction rather than normalizing it away.
    assert_eq!(
        String::from_utf8(undeclared.to_artifact_bytes()).expect("utf-8"),
        r#"{"accepted_evidence_classes":[],"minimum":"proved"}"#
    );
}

#[test]
fn the_accepted_set_has_one_spelling_whatever_order_it_arrives_in() {
    let authored = br#"{"accepted_evidence_classes":["inductive","certificate","finite-exact"],"minimum":"validated"}"#;
    let group = AssurancePolicy::decode(authored).expect("legal");
    assert_eq!(
        String::from_utf8(group.to_artifact_bytes()).expect("utf-8"),
        concat!(
            r#"{"accepted_evidence_classes":["certificate","finite-exact","inductive"],"#,
            r#""minimum":"validated"}"#
        )
    );
    // The order is the encoding's — code-point order over the tokens — and carries no
    // strength claim: `EvidenceClass` derives no `Ord`, so none can be written.
    let classes: Vec<EvidenceClass> = group.accepted_evidence_classes().collect();
    assert_eq!(
        classes,
        vec![
            EvidenceClass::Certificate,
            EvidenceClass::FiniteExact,
            EvidenceClass::Inductive
        ]
    );
    assert!(group.accepts(EvidenceClass::Inductive));
    assert!(!group.accepts(EvidenceClass::Example));
}

#[test]
fn every_semantic_part_moves_the_identity() {
    // ID7's checkable list, restricted to this group: "Changing any […] assurance
    // requirement […] MUST change [the identity]."
    let base = policy(AssuranceLevel::Bounded, Some(true), Some(true), []);
    let mutations = [
        policy(AssuranceLevel::Sampled, Some(true), Some(true), []),
        policy(AssuranceLevel::Bounded, Some(false), Some(true), []),
        policy(AssuranceLevel::Bounded, Some(true), Some(false), []),
        policy(AssuranceLevel::Bounded, None, Some(true), []),
        policy(
            AssuranceLevel::Bounded,
            Some(true),
            Some(true),
            [EvidenceClass::Certificate],
        ),
    ];
    for mutated in mutations {
        assert_ne!(mutated.identity(), base.identity());
        assert_ne!(mutated, base);
    }
    // The identity *is* the preimage bytes, and a digest can only index them.
    assert_eq!(
        base.identity().canonical_bytes(),
        base.identity().to_string().as_bytes()
    );
    assert_eq!(
        base.identity().digest::<Fnv1aPlaceholder>(),
        Fnv1aPlaceholder::hash(base.identity().canonical_bytes())
    );
}

// --- movement -------------------------------------------------------------------------------

#[test]
fn all_twenty_five_minimum_movements_classify_by_the_landed_ladder() {
    // RFC 0031's acceptance criteria: "Assurance tests mirroring
    // `crates/continuum-value/src/assurance.rs`: all 5×5 minimum movements plus both
    // checker flags, including every mixed case classifying `downgraded`, and
    // `incomparable` never emitted."
    for before in AssuranceLevel::ALL {
        for after in AssuranceLevel::ALL {
            let change = policy(before, Some(true), Some(true), [])
                .change_to(&policy(after, Some(true), Some(true), []))
                .expect("both sides declare both flags");
            let expected = match before.cmp(&after) {
                core::cmp::Ordering::Less => AssuranceChange::Upgraded,
                core::cmp::Ordering::Equal => AssuranceChange::Unchanged,
                core::cmp::Ordering::Greater => AssuranceChange::Downgraded,
            };
            assert_eq!(change, expected, "{before} → {after}");
        }
    }
}

#[test]
fn a_checker_flag_turning_off_is_a_downgrade_and_a_mixed_movement_is_too() {
    // "Checker requirements (`independent_checker`, `clean_recompute`) turning off is
    // also `downgraded`" — and the trade the `no-downgrade` verb exists to refuse:
    // a raised minimum bought with a dropped checker.
    let strict = policy(AssuranceLevel::Bounded, Some(true), Some(true), []);
    let no_checker = policy(AssuranceLevel::Bounded, Some(false), Some(true), []);
    let no_recompute = policy(AssuranceLevel::Bounded, Some(true), Some(false), []);
    assert_eq!(
        strict.change_to(&no_checker),
        Ok(AssuranceChange::Downgraded)
    );
    assert_eq!(
        strict.change_to(&no_recompute),
        Ok(AssuranceChange::Downgraded)
    );
    assert_eq!(no_checker.change_to(&strict), Ok(AssuranceChange::Upgraded));
    let traded = policy(AssuranceLevel::Proved, Some(false), Some(true), []);
    assert_eq!(
        strict.change_to(&traded),
        Ok(AssuranceChange::Downgraded),
        "a net direction MUST NOT be computed"
    );
    // And what the verb blocks is exactly `downgraded`.
    assert!(AssuranceChange::Downgraded.blocked_by_no_downgrade());
    assert!(!AssuranceChange::Upgraded.blocked_by_no_downgrade());
    assert!(!AssuranceChange::Unchanged.blocked_by_no_downgrade());
}

#[test]
fn two_undeclared_flags_compare_and_a_declaredness_change_refuses() {
    let before = policy(AssuranceLevel::Bounded, None, None, []);
    let after = policy(AssuranceLevel::Validated, None, None, []);
    assert_eq!(before.change_to(&after), Ok(AssuranceChange::Upgraded));
    assert_eq!(before.change_to(&before), Ok(AssuranceChange::Unchanged));
    // Declaring a flag that was undeclared, in either direction, has no relation in
    // the three-member assurance set, so it is refused for the classifier to record as
    // `unknown` and fail closed on.
    let declared = policy(AssuranceLevel::Bounded, Some(true), None, []);
    assert_eq!(
        before.change_to(&declared),
        Err(AssuranceComparisonError::CheckerDeclarednessChanged {
            flag: "independent_checker"
        })
    );
    assert_eq!(
        declared.change_to(&before),
        Err(AssuranceComparisonError::CheckerDeclarednessChanged {
            flag: "independent_checker"
        })
    );
    assert_eq!(
        policy(AssuranceLevel::Bounded, Some(true), Some(true), []).change_to(&policy(
            AssuranceLevel::Bounded,
            Some(true),
            None,
            []
        )),
        Err(AssuranceComparisonError::CheckerDeclarednessChanged {
            flag: "clean_recompute"
        })
    );
    assert!(
        before
            .change_to(&declared)
            .expect_err("refused")
            .to_string()
            .contains("unknown")
    );
}

#[test]
fn an_evidence_class_membership_change_carries_no_direction() {
    let before = policy(
        AssuranceLevel::Bounded,
        Some(true),
        Some(true),
        [EvidenceClass::Sampled, EvidenceClass::FiniteExact],
    );
    let after = policy(
        AssuranceLevel::Bounded,
        Some(true),
        Some(true),
        [EvidenceClass::FiniteExact, EvidenceClass::LivenessProof],
    );
    let changes = before.evidence_class_changes(&after);
    assert_eq!(
        changes,
        vec![
            (EvidenceClass::LivenessProof, Relation::Added),
            (EvidenceClass::Sampled, Relation::Removed),
        ]
    );
    // Only `added` and `removed`, ever: "It MUST NOT contribute an `upgraded` or
    // `downgraded` relation, and MUST NOT be ranked."
    for (_, relation) in changes {
        assert!(matches!(relation, Relation::Added | Relation::Removed));
    }
    // Dropping a proof class while adding an observation class is two records, not a
    // ranked one — the type cannot express a comparison between them.
    assert_eq!(before.change_to(&after), Ok(AssuranceChange::Unchanged));
    assert!(before.evidence_class_changes(&before).is_empty());
}

// --- the guard the bone asks for ------------------------------------------------------------

#[test]
fn the_level_a_claim_demands_is_validated_against_the_closed_five() {
    // Every legal token decodes to exactly one level, and the demand every claim
    // inherits is that level.
    for level in AssuranceLevel::ALL {
        let text = format!(r#"{{"minimum":"{}"}}"#, level.as_str());
        let group = AssurancePolicy::decode(text.as_bytes()).expect("a closed-set token");
        assert_eq!(group.demanded_of_every_claim(), level);
        assert!(group.meets_minimum(level));
    }
    // A sixth rung is not a level, and it is not rounded to a neighbouring one.
    for outside in [
        "exhaustive",
        "Proved",
        "PROVED",
        "verified",
        "finite-exact",
        "",
    ] {
        let text = format!(r#"{{"minimum":"{outside}"}}"#);
        AssurancePolicy::decode(text.as_bytes())
            .expect_err("a token outside the closed five must not decode");
    }
    // The ladder is a ladder: only levels at or above the minimum meet it.
    let group = policy(AssuranceLevel::Bounded, None, None, []);
    assert!(!group.meets_minimum(AssuranceLevel::Observed));
    assert!(!group.meets_minimum(AssuranceLevel::Sampled));
    assert!(group.meets_minimum(AssuranceLevel::Bounded));
    assert!(group.meets_minimum(AssuranceLevel::Validated));
    assert!(group.meets_minimum(AssuranceLevel::Proved));
}
