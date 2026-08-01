//! The `faults` field group: one canonical spelling, one identity, and a removal that
//! cannot be summarized away (PR-4 / IMPL-05).
//!
//! # What this file is evidence for
//!
//! `faults` is one of four fields whose applicable-verb set contains `no-removal`, and
//! RFC 0031 names the attack it blocks:
//!
//! > | remove crash-after-submit | `faults` | `removed` | `no-removal` |
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Attacks and their
//! > classifications"
//!
//! A verb that blocks `removed` is only enforceable if a removal is *individually*
//! visible, which is why RFC 0031 insists the relation is per member — "one record per
//! class or profile added or removed" — and why plan §5.3's "fault envelope
//! contraction" is explicitly demoted to an informal alias. The tests below say that
//! this crate's representation keeps that promise: a removal is a set difference over
//! stable member identities, it moves the artifact identity (ID7), and no spelling of
//! the same model produces two identities or one model two spellings.
//!
//! The schema half is validated by `notes/plan/tools/validate_dossier.py` on every
//! `just check`; [`the_schemas_own_validated_example_decodes`] reads that same
//! validated example through `include_str!` and decodes its `fault_model` with this
//! crate's decoder (INV-003 — the schema decides shape).

use std::collections::BTreeSet;

use continuum_intent::canonical_json::Json;
use continuum_intent::faults::{
    FaultClass, FaultModel, FaultUnit, FaultWellFormednessError, ProfileName,
};

/// The dossier's validated Intent Contract example, included at compile time.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// A fault model in canonical artifact form.
const GOOD: &str = r#"{"enabled":["crash","recovery"],"profiles":["storage-posix-v1"]}"#;

fn profile(name: &str) -> ProfileName {
    ProfileName::new(name).expect("a test profile name is non-empty")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
}

// --- round trip ---------------------------------------------------------------------------

#[test]
fn the_baseline_round_trips_byte_for_byte() {
    let model = FaultModel::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(utf8(&model.to_artifact_bytes()), GOOD);
    assert_eq!(
        model.enabled(),
        &BTreeSet::from([FaultClass::Crash, FaultClass::Recovery])
    );
    assert_eq!(model.profiles().len(), 1);
}

#[test]
fn a_constructed_model_and_a_decoded_one_are_the_same_value() {
    let built = FaultModel::new(
        [FaultClass::Crash, FaultClass::Recovery],
        [profile("storage-posix-v1")],
    )
    .expect("well formed");
    let decoded = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    assert_eq!(built, decoded);
    assert_eq!(built.identity(), decoded.identity());
}

// --- canonical-spelling uniqueness -----------------------------------------------------------

#[test]
fn every_spelling_of_one_fault_model_reaches_one_canonical_form() {
    let spellings = [
        GOOD.to_owned(),
        // Object keys reordered.
        r#"{"profiles":["storage-posix-v1"],"enabled":["crash","recovery"]}"#.to_owned(),
        // Array members reordered: both arrays are `uniqueItems` sets, so their order
        // is not part of the document's meaning.
        r#"{"enabled":["recovery","crash"],"profiles":["storage-posix-v1"]}"#.to_owned(),
        // Insignificant whitespace.
        format!("  {GOOD}\t\n"),
    ];
    let baseline = FaultModel::decode(GOOD.as_bytes()).expect("baseline");
    for spelling in spellings {
        let decoded = FaultModel::decode(spelling.as_bytes()).expect("a legal spelling decodes");
        assert_eq!(decoded, baseline, "{spelling}");
        assert_eq!(decoded.identity(), baseline.identity(), "{spelling}");
        assert_eq!(utf8(&decoded.to_artifact_bytes()), GOOD, "{spelling}");
    }
}

#[test]
fn a_fault_entrys_identity_is_stable_under_key_reordering() {
    // The semantic guard the group turns on: whatever order the author wrote the two
    // keys and the two arrays in, each *entry* keeps the same locator and the model
    // keeps the same identity. A classifier that keys `added`/`removed` records by
    // these locators therefore cannot be steered by a reformat.
    let reordered = r#"{"profiles":["storage-posix-v1"],"enabled":["recovery","crash"]}"#;
    let one = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    let other = FaultModel::decode(reordered.as_bytes()).expect("decodes");
    assert_eq!(
        one.units()
            .map(|unit| unit.locator().to_owned())
            .collect::<Vec<_>>(),
        other
            .units()
            .map(|unit| unit.locator().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

#[test]
fn an_absent_profiles_key_and_an_empty_one_are_one_meaning_with_one_spelling() {
    let absent = FaultModel::decode(br#"{"enabled":["crash"]}"#).expect("absent is legal");
    let explicit =
        FaultModel::decode(br#"{"enabled":["crash"],"profiles":[]}"#).expect("explicit is legal");
    assert_eq!(absent, explicit);
    assert_eq!(absent.identity(), explicit.identity());
    assert!(utf8(&absent.to_artifact_bytes()).contains(r#""profiles":[]"#));
}

#[test]
fn the_encoded_order_is_token_order_and_not_declaration_order() {
    let model = FaultModel::new(FaultClass::ALL, []).expect("well formed");
    assert_eq!(
        utf8(&model.to_artifact_bytes()),
        r#"{"enabled":["crash","delay","duplication","loss","partition","recovery"],"profiles":[]}"#,
        "every member array sorts by the RFC 0031 unit locator"
    );
    // `ALL` keeps the RFC's declaration order, which is a different thing and stays
    // available to a caller that wants to enumerate the vocabulary as written.
    assert_eq!(FaultClass::ALL[1], FaultClass::Recovery);
}

// --- the protected weakening ------------------------------------------------------------------

#[test]
fn removing_a_fault_class_moves_the_identity_and_is_a_set_difference() {
    let before = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    // The attack: drop `crash`, leave everything else alone.
    let after = FaultModel::decode(br#"{"enabled":["recovery"],"profiles":["storage-posix-v1"]}"#)
        .expect("decodes");

    // ID7: "Changing any claim, assumption, observer, bound, fault […] MUST change
    // [the identity]."
    assert_ne!(before.identity(), after.identity());
    assert_ne!(before, after);

    // The removal is a membership fact about one named unit, which is exactly the
    // record RFC 0031 emits and `no-removal` blocks. Nothing here classifies.
    let removed: Vec<String> = before
        .units()
        .filter(|unit| !after.contains(unit))
        .map(|unit| unit.locator().to_owned())
        .collect();
    assert_eq!(removed, vec!["crash".to_owned()]);
    assert!(after.units().all(|unit| before.contains(&unit)));
}

#[test]
fn removing_a_profile_is_as_visible_as_removing_a_class() {
    let before = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    let after = FaultModel::decode(br#"{"enabled":["crash","recovery"]}"#).expect("decodes");
    assert_ne!(before.identity(), after.identity());
    let removed: Vec<String> = before
        .units()
        .filter(|unit| !after.contains(unit))
        .map(|unit| unit.locator().to_owned())
        .collect();
    assert_eq!(removed, vec!["storage-posix-v1".to_owned()]);
}

#[test]
fn a_fault_model_is_never_summarized_into_an_envelope() {
    // Two models with the same *number* of enabled classes and different members are
    // different models. A count, a bitmask width, or an "envelope" scalar would make
    // them equal, and `no-removal` would then be unenforceable.
    let one = FaultModel::new([FaultClass::Crash, FaultClass::Delay], []).expect("well formed");
    let other = FaultModel::new([FaultClass::Loss, FaultClass::Delay], []).expect("well formed");
    assert_eq!(one.enabled().len(), other.enabled().len());
    assert_ne!(one, other);
    assert_ne!(one.identity(), other.identity());
}

#[test]
fn there_is_no_way_to_remove_a_fault_in_place() {
    // The compile-time half of INV-001/INV-011: `FaultModel` exposes no `&mut self`
    // method and no setter, so the only path to a smaller fault model is a new value
    // with a new identity.
    let before = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    let after = FaultModel::new([FaultClass::Recovery], [profile("storage-posix-v1")])
        .expect("well formed");
    assert_ne!(before.identity(), after.identity());
}

// --- unit identity --------------------------------------------------------------------------

#[test]
fn a_unit_locator_is_the_spelling_the_diff_schema_fixes() {
    let model = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    let units: Vec<FaultUnit> = model.units().collect();
    assert_eq!(
        units.iter().map(FaultUnit::locator).collect::<Vec<_>>(),
        vec!["crash", "recovery", "storage-posix-v1"],
        "semantic-diff.schema.json: `faults` by the member of `enabled` or `profiles`"
    );
    // The locator alone does not record which set a member came from — the flag the
    // module raises — so the type carries the qualification beside it.
    assert_eq!(units[0].field(), "fault_model.enabled");
    assert_eq!(units[2].field(), "fault_model.profiles");
    assert_ne!(
        FaultUnit::Class(FaultClass::Crash),
        FaultUnit::Profile(profile("crash")),
        "a class and a profile with the same token are different units"
    );
}

// --- the cross-group seam ----------------------------------------------------------------------

#[test]
fn w8s_fault_half_takes_the_snapshots_available_profiles() {
    let model = FaultModel::decode(GOOD.as_bytes()).expect("decodes");
    let mut available = BTreeSet::new();
    assert_eq!(
        model.check_profiles(&available),
        Err(FaultWellFormednessError::UnresolvedProfile {
            name: "storage-posix-v1".to_owned()
        }),
        "an unresolvable profile is rejected at acceptance time, never deferred into \
         an `unknown` classification"
    );
    available.insert("storage-posix-v1".to_owned());
    assert_eq!(model.check_profiles(&available), Ok(()));
}

#[test]
fn the_schemas_own_validated_example_decodes() {
    let example = Json::parse(SCHEMA_EXAMPLE.as_bytes()).expect("the example is canonical JSON");
    let fault_model = example
        .as_object()
        .expect("the contract is an object")
        .get("fault_model")
        .expect("the contract declares a fault model");
    let model = FaultModel::from_json(fault_model).expect("the schema's example decodes");
    assert_eq!(
        model.enabled(),
        &BTreeSet::from([FaultClass::Crash, FaultClass::Recovery])
    );
    assert_eq!(
        model
            .units()
            .map(|u| u.locator().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "crash".to_owned(),
            "recovery".to_owned(),
            "storage-posix-v1".to_owned()
        ]
    );
    assert_eq!(utf8(&model.to_artifact_bytes()), GOOD);
}
