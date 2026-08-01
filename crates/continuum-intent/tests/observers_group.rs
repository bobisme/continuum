//! The `observers` field group: one canonical spelling, one identity, and a
//! comparison surface a coarsening cannot slip through (PR-4 / IMPL-03).
//!
//! # What this file is evidence for
//!
//! RFC 0031 classifies `observers` by componentwise set inclusion over four sets, and
//! it names the attack the classification exists to catch:
//!
//! > Dropping an event family or a projection element is the "hide observer events"
//! > attack (plan §19.5) and MUST classify `coarsened` even when other components
//! > grow — the mixed case blocks under the fail-closed rule regardless.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`observers`"
//!
//! A classifier can only do that if the types under it are honest about three things,
//! and each has a test below:
//!
//! 1. one observer has exactly one canonical spelling, so a re-authored contract is
//!    not a change (ID7) and a *real* change cannot hide inside a re-authoring;
//! 2. an absent projection and an empty projection are one meaning with one spelling,
//!    so deleting a key is not a quieter way to drop a set (RFC 0037, "Absence, null,
//!    and defaults");
//! 3. dropping one string from one set moves the identity and shows up as a set
//!    difference, not as a diff of two renderings (ID7).
//!
//! The schema half is validated where schemas are validated:
//! `notes/plan/tools/validate_dossier.py` checks
//! `schemas/examples/intent-contract.example.json` against
//! `schemas/intent-contract.schema.json` on every `just check`, and
//! [`the_schemas_own_validated_example_decodes`] reads that same file through
//! `include_str!` and decodes its `observers` array with this crate's decoder. Neither
//! half can drift without the other failing (INV-003 — the schema decides shape).

use std::collections::BTreeSet;

use continuum_intent::ast::{Formula, Identifier};
use continuum_intent::canonical_json::Json;
use continuum_intent::observers::{
    Observer, ObserverError, ObserverId, ObserverSet, ProjectionKind,
};
use continuum_intent::property::{Claim, ClaimKind, ClaimSet, PropertyExpression, UnitKey};

/// The dossier's validated Intent Contract example, included at compile time.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// One observer in canonical artifact form.
const GOOD: &str = concat!(
    r#"{"events":["ReplyPublished","RequestReceived"],"id":"client","#,
    r#""knowledge_projection":[],"security_projection":[],"#,
    r#""state_projection":["acknowledged"]}"#,
);

fn id(text: &str) -> ObserverId {
    ObserverId::new(text).expect("a test observer id is non-empty")
}

fn observer(name: &str, events: &[&str], state: &[&str]) -> Observer {
    Observer::new(
        id(name),
        [
            (
                ProjectionKind::Events,
                events.iter().map(|e| (*e).to_owned()).collect(),
            ),
            (
                ProjectionKind::State,
                state.iter().map(|s| (*s).to_owned()).collect(),
            ),
        ],
    )
    .expect("a test observer is well formed")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
}

// --- round trip ---------------------------------------------------------------------------

#[test]
fn the_baseline_round_trips_byte_for_byte() {
    let decoded = Observer::decode(GOOD.as_bytes()).expect("the baseline decodes");
    assert_eq!(utf8(&decoded.to_artifact_bytes()), GOOD);
    assert_eq!(decoded.locator(), "client");
    assert_eq!(decoded.events().len(), 2);
    assert_eq!(
        decoded.projection(ProjectionKind::State),
        &BTreeSet::from(["acknowledged".to_owned()])
    );
}

#[test]
fn a_constructed_observer_and_a_decoded_one_are_the_same_value() {
    let built = observer(
        "client",
        &["ReplyPublished", "RequestReceived"],
        &["acknowledged"],
    );
    let decoded = Observer::decode(GOOD.as_bytes()).expect("decodes");
    assert_eq!(built, decoded);
    assert_eq!(built.identity(), decoded.identity());
    assert_eq!(built.to_artifact_bytes(), decoded.to_artifact_bytes());
}

#[test]
fn a_set_round_trips_and_iterates_in_id_order() {
    let set = ObserverSet::from_observers([
        observer("z-late", &["B"], &[]),
        observer("a-early", &["A"], &[]),
    ])
    .expect("two distinct ids");
    let bytes = set.to_artifact_bytes();
    let decoded = ObserverSet::decode(&bytes).expect("round trips");
    assert_eq!(decoded, set);
    assert_eq!(decoded.identity(), set.identity());
    assert_eq!(decoded.to_artifact_bytes(), bytes);
    assert_eq!(
        decoded.iter().map(Observer::locator).collect::<Vec<_>>(),
        vec!["a-early", "z-late"]
    );
}

// --- canonical-spelling uniqueness -----------------------------------------------------------

#[test]
fn every_spelling_of_one_observer_reaches_one_canonical_form() {
    // Object keys reordered, array elements reordered, and the two empty projections
    // omitted entirely. All three are legal documents under the schema and none of
    // them is a change: ID7 says "Renaming the contract, reformatting it […] or
    // reordering keys MUST NOT change [the identity]".
    let spellings = [
        GOOD.to_owned(),
        concat!(
            r#"{"state_projection":["acknowledged"],"id":"client","#,
            r#""security_projection":[],"knowledge_projection":[],"#,
            r#""events":["RequestReceived","ReplyPublished"]}"#,
        )
        .to_owned(),
        concat!(
            r#"{"events":["RequestReceived","ReplyPublished"],"id":"client","#,
            r#""state_projection":["acknowledged"]}"#,
        )
        .to_owned(),
        // Insignificant whitespace is not part of the document.
        format!(" {GOOD}\n"),
    ];
    let baseline = Observer::decode(GOOD.as_bytes()).expect("baseline");
    for spelling in spellings {
        let decoded = Observer::decode(spelling.as_bytes()).expect("a legal spelling decodes");
        assert_eq!(decoded, baseline, "{spelling}");
        assert_eq!(decoded.identity(), baseline.identity(), "{spelling}");
        assert_eq!(utf8(&decoded.to_artifact_bytes()), GOOD, "{spelling}");
    }
}

#[test]
fn an_absent_projection_and_an_empty_one_are_one_meaning_with_one_spelling() {
    let absent = Observer::decode(br#"{"events":["E"],"id":"o"}"#).expect("absent is legal");
    let explicit = Observer::decode(
        br#"{"events":["E"],"id":"o","knowledge_projection":[],"security_projection":[],"state_projection":[]}"#,
    )
    .expect("explicit is legal");
    assert_eq!(absent, explicit);
    assert_eq!(absent.identity(), explicit.identity());
    // And the one spelling writes every set, so "absent" is not reachable on output.
    let encoded = utf8(&absent.to_artifact_bytes());
    for kind in ProjectionKind::ALL {
        assert!(encoded.contains(kind.wire()), "{encoded}");
    }
}

#[test]
fn authored_array_order_does_not_reach_the_identity() {
    let a = observer("a", &["X"], &[]);
    let b = observer("b", &["Y"], &[]);
    let one = ObserverSet::from_observers([a.clone(), b.clone()]).expect("distinct ids");
    let other = ObserverSet::from_observers([b, a]).expect("distinct ids");
    assert_eq!(one.identity(), other.identity());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
}

// --- the protected weakening ------------------------------------------------------------------

#[test]
fn dropping_an_event_moves_the_identity_and_is_a_set_difference() {
    let before = observer(
        "client",
        &["ReplyPublished", "RequestReceived"],
        &["acknowledged"],
    );
    // The attack: one string removed from one set, everything else untouched.
    let after = observer("client", &["RequestReceived"], &["acknowledged"]);

    // ID7: "Changing any claim, assumption, observer […] MUST change [the identity]."
    assert_ne!(before.identity(), after.identity());
    assert_ne!(before, after);
    // The unit key is unchanged, so the change is an edit to *this* unit rather than a
    // removal paired with an addition a classifier could mismatch.
    assert_eq!(before.id(), after.id());

    // And the movement is componentwise set inclusion, which is what RFC 0031 asks of
    // the surface. Nothing here classifies; this is the arithmetic a classifier does.
    let strictly_smaller = before
        .components()
        .iter()
        .zip(after.components())
        .any(|((_, old), (_, new))| new.is_subset(old) && new.len() < old.len());
    let nothing_grew = before
        .components()
        .iter()
        .zip(after.components())
        .all(|((_, old), (_, new))| new.is_subset(old));
    assert!(strictly_smaller && nothing_grew);
}

#[test]
fn coarsening_a_projection_is_as_visible_as_coarsening_events() {
    let before = observer("client", &["E"], &["acknowledged", "durable"]);
    let after = observer("client", &["E"], &["acknowledged"]);
    assert_ne!(before.identity(), after.identity());
    assert!(
        after
            .projection(ProjectionKind::State)
            .is_subset(before.projection(ProjectionKind::State))
    );
}

#[test]
fn deleting_a_projection_key_is_not_a_quieter_way_to_drop_it() {
    // The document form of the same attack: remove `state_projection` rather than
    // empty it. It must land on the same value as the explicit empty set, and must
    // differ from the observer that still declares the element.
    let dropped = Observer::decode(br#"{"events":["E"],"id":"client"}"#).expect("legal");
    let emptied = Observer::decode(br#"{"events":["E"],"id":"client","state_projection":[]}"#)
        .expect("legal");
    let kept =
        Observer::decode(br#"{"events":["E"],"id":"client","state_projection":["acknowledged"]}"#)
            .expect("legal");
    assert_eq!(dropped.identity(), emptied.identity());
    assert_ne!(dropped.identity(), kept.identity());
}

// --- semantic guards -------------------------------------------------------------------------

#[test]
fn observer_entries_with_duplicate_identities_are_rejected() {
    // Byte-identical entries. Deduplicating them would be a reader inventing a
    // document the author did not write; W1 rejects instead.
    let doubled = format!("[{GOOD},{GOOD}]");
    assert!(matches!(
        ObserverSet::decode(doubled.as_bytes()).expect_err("W1 rejects a repeated id"),
        continuum_intent::observers::ObserverDecodeError::Observer(
            ObserverError::DuplicateObserverId { .. }
        )
    ));
    // And the dangerous shape: one id, two different contents. Last-writer-wins would
    // let the *second* entry silently coarsen the first.
    let coarsened = GOOD.replace(
        r#"["ReplyPublished","RequestReceived"]"#,
        r#"["ReplyPublished"]"#,
    );
    assert_eq!(
        ObserverSet::decode(format!("[{GOOD},{coarsened}]").as_bytes())
            .expect_err("two entries, one key"),
        continuum_intent::observers::ObserverDecodeError::Observer(
            ObserverError::DuplicateObserverId {
                id: "client".to_owned()
            }
        )
    );
    // Identical *content* under different ids is not a duplicate: the unit key is the
    // id, so these are two observers.
    let renamed = GOOD.replace(r#""id":"client""#, r#""id":"server""#);
    assert_eq!(
        ObserverSet::decode(format!("[{GOOD},{renamed}]").as_bytes())
            .expect("distinct ids")
            .len(),
        2
    );
}

#[test]
fn an_empty_observer_set_is_a_declaration_and_not_an_omission() {
    let empty = ObserverSet::decode(b"[]").expect("an empty array is legal");
    assert!(empty.is_empty());
    assert_eq!(utf8(&empty.to_artifact_bytes()), "[]");
    assert_eq!(utf8(empty.identity().canonical_bytes()), "[]");
    assert!(empty.declared_ids().is_empty());
}

#[test]
fn there_is_no_way_to_coarsen_an_observer_in_place() {
    // The compile-time half of INV-001. `Observer` exposes no `&mut self` method, no
    // setter, and no builder; the only way to reach a coarsened observer is to build a
    // new one, which derives a new identity. This test documents the API surface that
    // makes that true — it is the type, not the discipline, that forbids it.
    let observer = observer("client", &["E"], &[]);
    let coarsened = Observer::new(observer.id().clone(), [(ProjectionKind::Events, vec![])])
        .expect("well formed");
    assert_ne!(observer.identity(), coarsened.identity());
}

// --- the cross-group seams ---------------------------------------------------------------------

#[test]
fn w2_closes_against_the_property_groups_check() {
    let observers =
        ObserverSet::from_observers([observer("client", &["E"], &[])]).expect("one observer");
    let bound = ClaimSet::from_claims([Claim::new(
        UnitKey::new("C-1").expect("non-empty"),
        ClaimKind::Safety,
        PropertyExpression::normalized(
            &Formula::predicate(
                Identifier::new("delivered").expect("a well-formed identifier"),
                Vec::new(),
            ),
            None,
            None,
        )
        .expect("normalizes"),
        Some("client".to_owned()),
    )])
    .expect("one claim");
    assert_eq!(bound.check_observers(&observers.declared_ids()), Ok(()));

    // The dangling case: the claim names an observer the set does not declare.
    let empty = ObserverSet::from_observers([]).expect("empty is legal");
    assert!(bound.check_observers(&empty.declared_ids()).is_err());
}

#[test]
fn the_schemas_own_validated_example_decodes() {
    let example = Json::parse(SCHEMA_EXAMPLE.as_bytes()).expect("the example is canonical JSON");
    let observers = example
        .as_object()
        .expect("the contract is an object")
        .get("observers")
        .expect("the contract declares observers");
    let set = ObserverSet::from_json(observers).expect("the schema's example decodes");
    assert_eq!(set.len(), 1);
    let client = set.get(&id("client")).expect("the example's observer");
    assert_eq!(
        client.events(),
        &BTreeSet::from(["ReplyPublished".to_owned()])
    );
    assert_eq!(
        client.projection(ProjectionKind::State),
        &BTreeSet::from(["acknowledged".to_owned()])
    );
    // The example omits two projections; they read as empty and re-encode explicitly.
    assert!(client.projection(ProjectionKind::Knowledge).is_empty());
    assert!(
        utf8(&client.to_artifact_bytes()).contains(r#""knowledge_projection":[]"#),
        "the encoder writes every set"
    );

    // The example's claim binds this observer, so W2 holds across the two groups.
    let claims = ClaimSet::from_json(
        example
            .as_object()
            .expect("object")
            .get("claims")
            .expect("the contract declares claims"),
    )
    .expect("the example's claims decode");
    assert_eq!(claims.check_observers(&set.declared_ids()), Ok(()));
}
