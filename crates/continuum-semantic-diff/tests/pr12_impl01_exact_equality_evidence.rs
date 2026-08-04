//! PR-12 / IMPL-01 exit evidence — exact equality classification (`bn-b8ru`).
//!
//! # What this file is evidence for
//!
//! `src/equality.rs` implements RFC 0031's whole-contract `unchanged` shortcut
//! (plan §5.3's "unchanged intent", PR-12's "exact equality" bullet). This file
//! is the dedicated adversarial and clause-mapped evidence the PR-12 family's
//! evidence bar asks for, on top of that module's own unit tests (which stay the
//! source of truth for the basic decision and are not re-derived here):
//!
//! - a real-corpus fixture closed all the way into `PolicyTable::verdict` under
//!   the fixture's own policy verbs;
//! - an adversarial disguise sweep: cosmetic/serialization-variant differences
//!   never read as `Distinct`, and genuine (including governance-only) content
//!   differences never read as `Unchanged`;
//! - anti-vacuity mutants asserted in both directions;
//! - determinism.
//!
//! # Clause → test map
//!
//! | RFC 0031 / plan clause | Test |
//! |---|---|
//! | "the same `in_*` identity … `intent_changes` MUST be empty" (`unchanged` shortcut) | [`positive_the_real_corpus_fixture_decoded_twice_is_the_unchanged_shortcut`] |
//! | "equal identity entails every field unchanged" | [`the_shortcut_closes_into_allow_under_the_fixtures_own_policy_even_with_two_locked_fields`] |
//! | R2: "Textual match … MUST NOT be used" | [`adversarial_a_reordered_and_repadded_fixture_is_still_the_unchanged_shortcut`], [`negative_mutant_comparing_raw_input_bytes_would_wrongly_call_the_reformatted_fixture_distinct`] |
//! | ID2: "the policy table, `policy_reviewers` … are **in** the preimage" | [`adversarial_a_policy_only_edit_with_every_other_byte_identical_is_distinct`], [`negative_mutant_that_treats_policy_as_metadata_would_wrongly_call_the_policy_edit_unchanged`] |
//! | ID2: "`schema_epoch` … in the preimage" | [`schema_epoch_is_reachable_from_the_identity_preimage_confirming_id2s_inclusion_claim`] |
//! | ID2: "`name` … MUST NOT affect identity" | [`adversarial_a_renamed_fixture_is_still_the_unchanged_shortcut`] |
//! | "distinct identities MUST be classified field by field" (converse; this module asserts nothing further) | [`unchanged_classification_is_none_for_every_distinct_mutation_in_this_file`] |
//! | RFC 0031 "Determinism" | [`determinism_two_independent_calls_produce_the_same_fifteen_records`] |
//! | anti-vacuity: the positive assertions are not vacuously true | [`negative_mutant_that_treats_policy_as_metadata_would_wrongly_call_the_policy_edit_unchanged`], [`negative_mutant_comparing_raw_input_bytes_would_wrongly_call_the_reformatted_fixture_distinct`] |
//!
//! # House rules, inherited from the PR-12 evidence precedent (`bn-1sdp`, `bn-ycn6`)
//!
//! - `src/` is untouched by this file, and no existing test anywhere is edited.
//! - [`FIXTURE`] is a real, already-reviewed corpus document
//!   (`crates/continuum-intent/tests/fixtures/replicated-register-contract.json`,
//!   already used by `continuum-intent`'s own PR-4 exit and INV-012 evidence, and
//!   by this crate's PR-12 / IMPL-04/05/06 evidence), read at compile time via
//!   `include_str!`. Every mutation of it below is one bounded, linear
//!   `.replacen` on that fixed document, or a whole-document structural
//!   reformat driven by the typed `Json` tree (never raw-text reordering that
//!   could reach into a string literal's content) — never a loop of doublings,
//!   never a hand-authored contract.
//! - Positive, adversarial, and anti-vacuity evidence are each present and
//!   separately named, per this bone's brief.

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_intent::contract::IntentContract;
use continuum_semantic_diff::equality::{Equality, classify, unchanged_classification};

/// The real corpus fixture: the replicated-register Intent Contract, also used
/// by `continuum-intent`'s own PR-4 exit and INV-012 evidence, and by this
/// crate's other PR-12 evidence files. Its `policy.properties` and
/// `policy.completion_policy` are both `"locked"` — the strictest verb in RFC
/// 0037's closed set — which is exactly what makes the positive `verdict` test
/// below load-bearing rather than a `PolicyTable::all_unlocked()` softball.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the fixture decodes")
}

fn fixture_policy(document: &str) -> PolicyTable {
    let parsed =
        Json::parse(document.trim_end().as_bytes()).expect("the fixture is canonical JSON");
    let policy = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("policy")
        .expect("the fixture declares a policy table");
    PolicyTable::from_json(policy).expect("the fixture's policy table decodes")
}

fn fixture_reviewers(document: &str) -> PolicyReviewers {
    let parsed =
        Json::parse(document.trim_end().as_bytes()).expect("the fixture is canonical JSON");
    let reviewers = parsed
        .as_object()
        .expect("the fixture is an object")
        .get("policy_reviewers")
        .expect("the fixture declares policy_reviewers");
    PolicyReviewers::from_json(reviewers).expect("the fixture's reviewer map decodes")
}

// --- positive: the real fixture is its own shortcut ------------------------------------------

#[test]
fn positive_the_real_corpus_fixture_decoded_twice_is_the_unchanged_shortcut() {
    let a = decode(FIXTURE);
    let b = decode(FIXTURE);
    assert_eq!(classify(a.identity(), b.identity()), Equality::Unchanged);
}

#[test]
fn the_shortcut_closes_into_allow_under_the_fixtures_own_policy_even_with_two_locked_fields() {
    // RFC 0037's "Change policy" table makes `locked` the top of the verb
    // lattice ("no change without policy amendment … denies every non-
    // `unchanged` relation"), and the fixture uses it twice (`properties`,
    // `completion_policy`). If the shortcut's fifteen `Unchanged` records
    // closed into anything but `allow` here, the shortcut would be unsound —
    // this is the load-bearing case, not an easy one.
    let policy = fixture_policy(FIXTURE);
    assert_eq!(policy.verb(PolicyField::Properties), PolicyVerb::Locked);
    assert_eq!(
        policy.verb(PolicyField::CompletionPolicy),
        PolicyVerb::Locked
    );

    let a = decode(FIXTURE);
    let b = decode(FIXTURE);
    let records =
        unchanged_classification(a.identity(), b.identity()).expect("identities are equal");

    let verdict = policy
        .verdict(
            &records,
            &fixture_reviewers(FIXTURE),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes");
    assert_eq!(verdict.decision(), PolicyDecision::Allow);
    assert!(
        verdict.reasons().is_empty(),
        "an all-`unchanged` classification forbids nothing, so P6 names no reasons: {:?}",
        verdict.reasons()
    );
}

// --- adversarial: cosmetic differences never read as Distinct ------------------------------

#[test]
fn adversarial_a_renamed_fixture_is_still_the_unchanged_shortcut() {
    // ID2: "`name`; … MUST NOT affect identity."
    let renamed = FIXTURE.replacen(
        "\"name\":\"Cancel-Correct Replicated Register\"",
        "\"name\":\"A Totally Different Display Name\"",
        1,
    );
    assert_ne!(renamed, FIXTURE, "the replacement must actually fire");
    let a = decode(FIXTURE);
    let b = decode(&renamed);
    assert_ne!(a.name(), b.name());
    assert_eq!(classify(a.identity(), b.identity()), Equality::Unchanged);
}

#[test]
fn adversarial_a_reordered_and_repadded_fixture_is_still_the_unchanged_shortcut() {
    let contract = decode(FIXTURE);
    let spelled_differently = reformat(&contract.artifact_json());
    assert_ne!(
        spelled_differently.as_bytes(),
        contract.to_artifact_bytes().as_slice(),
        "the reformat must actually change the bytes, or this test proves nothing"
    );
    let reparsed = decode(&spelled_differently);
    assert_eq!(
        classify(contract.identity(), reparsed.identity()),
        Equality::Unchanged
    );
}

// --- adversarial: governance-only differences (no field's *value* moves) are Distinct ------

#[test]
fn adversarial_a_policy_only_edit_with_every_other_byte_identical_is_distinct() {
    // ID2, verbatim: "Nothing else is excluded — in particular the `policy`
    // table, `policy_reviewers`, `schema_id`, and `schema_epoch` are **in** the
    // preimage." Loosen `bounds`' own verb from `no-decrease` to `unlocked`:
    // no field's *content* changes, only its governance — and RFC 0031's
    // shortcut still MUST NOT fire, because a governance downgrade slipping
    // through as "unchanged" is exactly the class of gap INV-001 exists to
    // close.
    let loosened = FIXTURE.replacen(r#""bounds":"no-decrease""#, r#""bounds":"unlocked""#, 1);
    assert_ne!(loosened, FIXTURE, "the replacement must actually fire");
    let a = decode(FIXTURE);
    let b = decode(&loosened);
    // Confirm the edit really did land only on governance: every content field
    // the diff would otherwise classify is untouched.
    assert_eq!(a.bounds().values(), b.bounds().values());
    assert_eq!(a.bounds().nodes(), b.bounds().nodes());
    assert_eq!(a.bounds().faults(), b.bounds().faults());
    assert_eq!(a.bounds().depth(), b.bounds().depth());
    assert_ne!(
        a.policy().verb(PolicyField::Bounds),
        b.policy().verb(PolicyField::Bounds)
    );

    assert_eq!(classify(a.identity(), b.identity()), Equality::Distinct);
}

#[test]
fn schema_epoch_is_reachable_from_the_identity_preimage_confirming_id2s_inclusion_claim() {
    // ID2 names `schema_epoch` among the preimage members explicitly, in the
    // same sentence as `policy` and `policy_reviewers`. A live two-`schema_
    // epoch` comparison is not exercisable through the public `decode` API
    // today — W10 rejects any `schema_epoch` other than the schema's own
    // `const` outright ("HeaderMismatch"), before identity is ever computed —
    // so the strongest fact checkable here is the structural one: `schema_
    // epoch` really is present, with its value, in the bytes `classify`
    // compares, exactly as ID2 says and exactly like `policy` above.
    let a = decode(FIXTURE);
    let preimage = a.identity_preimage_json();
    let schema_epoch = preimage
        .as_object()
        .expect("the preimage is an object")
        .get("schema_epoch")
        .expect("ID2 puts schema_epoch in the preimage");
    assert_eq!(schema_epoch.as_integer(), Some(1));
}

// --- unsupported-shortcut converse: every distinct case above yields no records ------------

#[test]
fn unchanged_classification_is_none_for_every_distinct_mutation_in_this_file() {
    let a = decode(FIXTURE);
    for mutant in [
        FIXTURE.replacen(r#""bounds":"no-decrease""#, r#""bounds":"unlocked""#, 1),
        FIXTURE.replacen(r#""nodes":3"#, r#""nodes":2"#, 1),
    ] {
        let b = decode(&mutant);
        assert_eq!(classify(a.identity(), b.identity()), Equality::Distinct);
        assert!(
            unchanged_classification(a.identity(), b.identity()).is_none(),
            "RFC 0031: distinct identities MUST NOT be reported as a whole-contract change"
        );
    }
}

// --- anti-vacuity mutants: the positive assertions are not vacuously true ------------------
//
// Each mutant below is a plausible, *wrong* comparator — the kind of bug this file's
// positive/adversarial tests exist to catch. Run the same scenarios through it and show it
// gets the wrong answer where `classify` gets the right one; that is the proof this file's
// assertions are load-bearing rather than tautological.

/// Mutant 1: treats `policy` and `policy_reviewers` as ungoverned metadata —
/// exactly the mistake `name` (a *real* metadata exclusion) makes plausible —
/// and drops both from the comparison. The single worst outcome for this
/// module: a governance-only edit (see
/// `adversarial_a_policy_only_edit_with_every_other_byte_identical_is_distinct`)
/// reads as the `unchanged` shortcut and licenses `allow` on every field
/// without a single one having been reviewed.
fn mutant_ignoring_policy_and_reviewers(contract: &IntentContract) -> Vec<u8> {
    let mut fields: std::collections::BTreeMap<String, Json> = contract
        .identity_preimage_json()
        .as_object()
        .expect("the preimage is an object")
        .clone();
    fields.remove("policy");
    fields.remove("policy_reviewers");
    Json::object(fields)
        .expect("the filtered preimage has no duplicate keys")
        .to_canonical_bytes()
}

#[test]
fn negative_mutant_that_treats_policy_as_metadata_would_wrongly_call_the_policy_edit_unchanged() {
    let loosened = FIXTURE.replacen(r#""bounds":"no-decrease""#, r#""bounds":"unlocked""#, 1);
    let a = decode(FIXTURE);
    let b = decode(&loosened);

    let real = classify(a.identity(), b.identity());
    assert_eq!(real, Equality::Distinct);

    let mutant_a = mutant_ignoring_policy_and_reviewers(&a);
    let mutant_b = mutant_ignoring_policy_and_reviewers(&b);
    assert_eq!(
        mutant_a, mutant_b,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
}

/// Mutant 2: compares the two sides' *raw input bytes* — the spelling a caller
/// happened to hand in — instead of decoding to canonical form first. R2
/// forbids exactly this: "Textual match, partial structural match, and
/// label/comment equality MUST NOT be used." The single cost of this bug is
/// the safe direction (spurious review, never a spurious `allow`), but it is
/// still a wrong answer this module exists to avoid, and it is the direct
/// mirror of Mutant 1.
fn mutant_comparing_raw_bytes(a: &str, b: &str) -> bool {
    a.trim_end().as_bytes() == b.trim_end().as_bytes()
}

#[test]
fn negative_mutant_comparing_raw_input_bytes_would_wrongly_call_the_reformatted_fixture_distinct() {
    let contract = decode(FIXTURE);
    let spelled_differently = reformat(&contract.artifact_json());

    let real = classify(contract.identity(), decode(&spelled_differently).identity());
    assert_eq!(real, Equality::Unchanged);

    let mutant_says_equal = mutant_comparing_raw_bytes(FIXTURE, &spelled_differently);
    assert!(
        !mutant_says_equal,
        "the mutant must actually get this wrong, or it is not exercising the bug"
    );
}

// --- determinism -----------------------------------------------------------------------------

#[test]
fn determinism_two_independent_calls_produce_the_same_fifteen_records() {
    let a = decode(FIXTURE);
    let b = decode(FIXTURE);
    let first = unchanged_classification(a.identity(), b.identity()).expect("identities are equal");
    let second =
        unchanged_classification(a.identity(), b.identity()).expect("identities are equal");
    let as_pairs = |records: &[ClassificationRecord; 15]| {
        records
            .iter()
            .map(|record| (record.field(), record.relation()))
            .collect::<Vec<_>>()
    };
    assert_eq!(as_pairs(&first), as_pairs(&second));
    assert!(
        first
            .iter()
            .all(|record| record.relation() == Relation::Unchanged)
    );
}

// --- test-local helper: reformat a `Json` document without touching leaf content ------------
//
// Adapted from `continuum-intent`'s own `tests/pr4_exit_evidence.rs` (`reformat`/
// `render_reversed`), which established this exact technique for this exact fixture
// family: reverse object-key iteration order and array-item order everywhere *except*
// inside a property/fairness expression's `ast`/`condition` subtree, where item order is
// semantically significant and is preserved verbatim. Operating on the typed `Json` tree
// rather than on raw text means no leaf string's *content* is ever touched — unlike a
// blind text-level whitespace or punctuation substitution, which this fixture's own
// `optimization.non_vacuity`/`optimization.soft` free-text entries (both inside the
// identity preimage, both containing commas) would make unsafe.

fn reformat(json: &Json) -> String {
    let mut out = String::new();
    render_reversed(json, false, &mut out);
    out
}

fn render_reversed(json: &Json, inside_ast: bool, out: &mut String) {
    match json {
        Json::Object(fields) => {
            out.push('{');
            for (index, (key, value)) in fields.iter().rev().enumerate() {
                if index > 0 {
                    out.push_str(",  ");
                }
                out.push_str(&Json::String(key.clone()).to_string());
                out.push_str(": ");
                let descend = inside_ast || key == "expression" || key == "condition";
                render_reversed(value, descend, out);
            }
            out.push('}');
        }
        Json::Array(items) => {
            out.push('[');
            let ordered: Vec<&Json> = if inside_ast {
                items.iter().collect()
            } else {
                items.iter().rev().collect()
            };
            for (index, item) in ordered.into_iter().enumerate() {
                if index > 0 {
                    out.push_str(",  ");
                }
                render_reversed(item, inside_ast, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}
