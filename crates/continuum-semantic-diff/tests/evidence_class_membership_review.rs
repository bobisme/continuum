//! RFC 0031 correction 20 on the real corpus contracts (bn-36luu).
//!
//! > Normative: an `assurance` record whose relation is `added` or `removed`
//! > contributes `review` under `no-downgrade` and under `review`, on either
//! > acceptance path. Under `proposal-only` it keeps P4's authority: `allow` only on
//! > a path where a human principal performs `intent.accept`, and `review`
//! > otherwise, because that human acceptance is the review the verb requires. It
//! > contributes `block` under `locked` and `allow` under `unlocked`. It never
//! > contributes `allow` under `no-downgrade`.
//! >
//! > — RFC 0031, "Corrections recorded by this RFC", correction 20
//!
//! bn-bpz0's C020 campaign found that `PolicyTable::verdict` allowed an agent
//! acceptance of an added accepted evidence class under `no-downgrade` (P4 read the
//! verb's ordinary authority). This file runs the production front door
//! (`IntentContract::decode`) and the production assembler
//! (`continuum_semantic_diff::artifact::assemble`) over the two committed contracts
//! whose `assurance` verb is `no-downgrade`, and pins the corrected verdict for an
//! added class, a removed class, and every verb W6 admits on `assurance`.
//!
//! The last test is the anti-vacuity mutant: a port of the pre-fix P4 rule, fed the
//! real records, returns `allow`, and this file's own check rejects it.

use std::path::{Path, PathBuf};

use continuum_intent::assurance_policy::AssuranceLevel;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyVerb, Relation,
};
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_semantic_diff::artifact::{DiffArtifact, DiffId, DiffRequest, SnapshotId, assemble};

/// The two committed contracts whose `assurance` verb is `no-downgrade`.
const CORPUS: [&str; 2] = [
    "crates/continuum-intent/tests/fixtures/die-hard-contract.json",
    "crates/continuum-intent/tests/fixtures/replicated-register-contract.json",
];

/// An evidence class that neither contract accepts (both accept `certificate` and
/// `finite-exact`; the register also accepts `dpor-complete` and `refinement-proof`).
const EXTRA_CLASS: &str = "sampled";
/// An evidence class that both contracts accept.
const HELD_CLASS: &str = "certificate";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root resolves")
}

fn load(path: &str) -> Json {
    let text = std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|e| panic!("{path} is readable: {e}"));
    Json::parse(text.trim_end().as_bytes()).unwrap_or_else(|e| panic!("{path} parses: {e:?}"))
}

fn decode(document: &Json) -> IntentContract {
    IntentContract::decode(&document.to_canonical_bytes())
        .unwrap_or_else(|e| panic!("the document decodes: {e:?}"))
}

fn object_mut<'a>(json: &'a mut Json, key: &str) -> &'a mut Json {
    match json {
        Json::Object(map) => map
            .get_mut(key)
            .unwrap_or_else(|| panic!("missing {key:?}")),
        _ => panic!("not an object"),
    }
}

fn accepted_classes_mut(document: &mut Json) -> &mut Vec<Json> {
    match object_mut(
        object_mut(document, "assurance"),
        "accepted_evidence_classes",
    ) {
        Json::Array(items) => items,
        _ => panic!("accepted_evidence_classes is an array"),
    }
}

fn with_class_added(document: &Json, class: &str) -> Json {
    let mut out = document.clone();
    let classes = accepted_classes_mut(&mut out);
    assert!(
        !classes.contains(&Json::String(class.to_owned())),
        "{class} is not already accepted"
    );
    classes.push(Json::String(class.to_owned()));
    out
}

fn with_class_removed(document: &Json, class: &str) -> Json {
    let mut out = document.clone();
    let classes = accepted_classes_mut(&mut out);
    let before = classes.len();
    classes.retain(|item| item != &Json::String(class.to_owned()));
    assert_eq!(
        classes.len() + 1,
        before,
        "{class} was accepted exactly once"
    );
    out
}

/// The before-document with `assurance` under `verb`. A `review` verb needs a named
/// principal (W7), so the sweep names one.
fn with_assurance_verb(document: &Json, verb: PolicyVerb) -> Json {
    let mut out = document.clone();
    *object_mut(object_mut(&mut out, "policy"), "assurance") = Json::String(verb.wire().to_owned());
    if verb == PolicyVerb::Review {
        if let Json::Object(reviewers) = object_mut(&mut out, "policy_reviewers") {
            reviewers.insert(
                "assurance".to_owned(),
                Json::Array(vec![Json::String("verification-lead".to_owned())]),
            );
        }
    }
    out
}

fn diff(before: &IntentContract, after: &IntentContract, path: AcceptancePath) -> DiffArtifact {
    assemble(&DiffRequest {
        diff_id: DiffId::new("diff_bn36luu_evidence_class").expect("well formed"),
        before_snapshot: SnapshotId::new("ws_bn36luu_before").expect("well formed"),
        after_snapshot: SnapshotId::new("ws_bn36luu_after").expect("well formed"),
        before_intent: IntentId::new("in_bn36luu_before").expect("well formed"),
        after_intent: IntentId::new("in_bn36luu_after").expect("well formed"),
        before,
        after,
        requested_assurance: AssuranceLevel::Bounded,
        path,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    })
    .expect("the diff assembles")
}

/// What correction 20 requires for one membership change under `verb` on `path`.
const fn required(verb: PolicyVerb, path: AcceptancePath) -> PolicyDecision {
    match (verb, path) {
        (PolicyVerb::Unlocked, _) | (PolicyVerb::ProposalOnly, AcceptancePath::HumanAccept) => {
            PolicyDecision::Allow
        }
        (PolicyVerb::Locked, _) => PolicyDecision::Block,
        _ => PolicyDecision::Review,
    }
}

/// This file's check: the diff carries exactly the one membership record, and the
/// verdict is what correction 20 requires under `verb` on acceptance path `path`.
fn check(
    records: &[(PolicyField, Option<String>, Relation)],
    decision: PolicyDecision,
    class: &str,
    relation: Relation,
    verb: PolicyVerb,
    path: AcceptancePath,
) -> Result<(), String> {
    let expected = [(PolicyField::Assurance, Some(class.to_owned()), relation)];
    if records != expected {
        return Err(format!("records {records:?}, expected {expected:?}"));
    }
    if decision != required(verb, path) {
        return Err(format!(
            "assurance[{class}]:{relation} under {verb} on {path:?} decided {decision}, correction 20 requires {}",
            required(verb, path)
        ));
    }
    Ok(())
}

fn wire_records(artifact: &DiffArtifact) -> Vec<(PolicyField, Option<String>, Relation)> {
    artifact
        .intent_changes()
        .iter()
        .map(|record| {
            (
                record.field(),
                record.unit().map(str::to_owned),
                record.relation(),
            )
        })
        .collect()
}

#[test]
fn the_corpus_contracts_govern_assurance_with_no_downgrade() {
    // The regression is about `no-downgrade`; if a fixture's verb moved, the tests
    // below would silently measure a different verb.
    for path in CORPUS {
        assert_eq!(
            decode(&load(path)).policy().verb(PolicyField::Assurance),
            PolicyVerb::NoDowngrade,
            "{path}"
        );
    }
}

#[test]
fn an_added_evidence_class_under_no_downgrade_reaches_review_on_both_paths() {
    for path in CORPUS {
        let document = load(path);
        let before = decode(&document);
        let after = decode(&with_class_added(&document, EXTRA_CLASS));
        for acceptance in [AcceptancePath::AgentAccept, AcceptancePath::HumanAccept] {
            let artifact = diff(&before, &after, acceptance);
            check(
                &wire_records(&artifact),
                artifact.decision(),
                EXTRA_CLASS,
                Relation::Added,
                PolicyVerb::NoDowngrade,
                acceptance,
            )
            .unwrap_or_else(|e| panic!("{path} {acceptance:?}: {e}"));
            // P6: the reason names the field, the relation, and the verb.
            let reasons = artifact.verdict().reasons();
            assert_eq!(reasons.len(), 1, "{path}");
            assert_eq!(reasons[0].field(), PolicyField::Assurance);
            assert_eq!(reasons[0].relation(), Relation::Added);
            assert_eq!(reasons[0].verb(), PolicyVerb::NoDowngrade);
            assert_eq!(reasons[0].contribution(), PolicyDecision::Review);
        }
    }
}

#[test]
fn a_removed_evidence_class_under_no_downgrade_reaches_review_too() {
    // The set is unordered and MUST NOT be ranked, so a removal is not read as the
    // safe direction (correction 20's removal decision).
    for path in CORPUS {
        let document = load(path);
        let before = decode(&document);
        let after = decode(&with_class_removed(&document, HELD_CLASS));
        let artifact = diff(&before, &after, AcceptancePath::AgentAccept);
        check(
            &wire_records(&artifact),
            artifact.decision(),
            HELD_CLASS,
            Relation::Removed,
            PolicyVerb::NoDowngrade,
            AcceptancePath::AgentAccept,
        )
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    }
}

#[test]
fn every_admissible_assurance_verb_decides_a_membership_change_as_correction_20_says() {
    // `proposal-only` keeps P4's path condition: the human acceptance is the review
    // the verb requires, so it allows there and reviews on the agent path.
    assert_eq!(
        required(PolicyVerb::ProposalOnly, AcceptancePath::HumanAccept),
        PolicyDecision::Allow
    );
    assert_eq!(
        required(PolicyVerb::ProposalOnly, AcceptancePath::AgentAccept),
        PolicyDecision::Review
    );
    let verbs: Vec<PolicyVerb> = PolicyVerb::ALL
        .into_iter()
        .filter(|verb| verb.is_admissible_on(PolicyField::Assurance))
        .collect();
    assert_eq!(
        verbs,
        [
            PolicyVerb::Unlocked,
            PolicyVerb::ProposalOnly,
            PolicyVerb::Review,
            PolicyVerb::Locked,
            PolicyVerb::NoDowngrade,
        ],
        "W6's assurance row"
    );
    for path in CORPUS {
        let document = load(path);
        for verb in &verbs {
            let before_document = with_assurance_verb(&document, *verb);
            let before = decode(&before_document);
            for (after_document, class, relation) in [
                (
                    with_class_added(&before_document, EXTRA_CLASS),
                    EXTRA_CLASS,
                    Relation::Added,
                ),
                (
                    with_class_removed(&before_document, HELD_CLASS),
                    HELD_CLASS,
                    Relation::Removed,
                ),
            ] {
                let after = decode(&after_document);
                for acceptance in [AcceptancePath::AgentAccept, AcceptancePath::HumanAccept] {
                    let artifact = diff(&before, &after, acceptance);
                    check(
                        &wire_records(&artifact),
                        artifact.decision(),
                        class,
                        relation,
                        *verb,
                        acceptance,
                    )
                    .unwrap_or_else(|e| panic!("{path} {acceptance:?}: {e}"));
                }
            }
        }
    }
}

/// Metamorphic relation "set/map insertion order" (docs/19 §3):
/// `accepted_evidence_classes` is a set, so where the added class sits in the array,
/// and the order of the classes already accepted, change neither the records nor
/// the verdict.
#[test]
fn metamorphic_set_insertion_order_of_evidence_classes_preserves_the_verdict() {
    for path in CORPUS {
        let document = load(path);
        let before = decode(&document);
        let appended = with_class_added(&document, EXTRA_CLASS);
        let mut permuted = appended.clone();
        {
            let classes = accepted_classes_mut(&mut permuted);
            classes.rotate_right(1); // the new class moves to the front
            classes[1..].reverse(); // and the held classes reverse
        }
        assert_ne!(appended, permuted, "{path}: the permutation is a real edit");
        let a = diff(&before, &decode(&appended), AcceptancePath::AgentAccept);
        let b = diff(&before, &decode(&permuted), AcceptancePath::AgentAccept);
        assert_eq!(wire_records(&a), wire_records(&b), "{path}");
        assert_eq!(a.decision(), b.decision(), "{path}");
        assert_eq!(a.decision(), PolicyDecision::Review, "{path}");
    }
}

/// Differential: the oracle is `continuum_intent::change_policy::PolicyTable::verdict`
/// fed a hand-built, P1-complete classification (the one `assurance` membership
/// record plus fourteen `unchanged` records); the subject is
/// `continuum_semantic_diff::artifact::assemble` over the real documents. They must
/// reach the same decision and the same reasons.
#[test]
fn differential_the_policy_table_and_the_assembler_agree_on_a_membership_change() {
    for path in CORPUS {
        let document = load(path);
        let before = decode(&document);
        for (after_document, relation) in [
            (with_class_added(&document, EXTRA_CLASS), Relation::Added),
            (with_class_removed(&document, HELD_CLASS), Relation::Removed),
        ] {
            let after = decode(&after_document);
            for acceptance in [AcceptancePath::AgentAccept, AcceptancePath::HumanAccept] {
                let records: Vec<ClassificationRecord> = PolicyField::ALL
                    .into_iter()
                    .map(|field| {
                        let relation = if field == PolicyField::Assurance {
                            relation
                        } else {
                            Relation::Unchanged
                        };
                        ClassificationRecord::new(field, relation).expect("admissible")
                    })
                    .collect();
                let oracle = continuum_intent::change_policy::PolicyTable::verdict(
                    before.policy(),
                    &records,
                    before.policy_reviewers(),
                    acceptance,
                )
                .expect("the oracle verdict is computed");
                let subject = diff(&before, &after, acceptance);
                assert_eq!(
                    oracle.decision(),
                    subject.decision(),
                    "{path} {acceptance:?}"
                );
                assert_eq!(
                    oracle.reasons(),
                    subject.verdict().reasons(),
                    "{path} {acceptance:?}"
                );
                assert_eq!(
                    oracle.decision(),
                    PolicyDecision::Review,
                    "{path} {acceptance:?}"
                );
            }
        }
    }
}

// --- anti-vacuity: the check can fail ----------------------------------------------

/// The pre-fix P4 rule, ported verbatim from `PolicyTable::verdict` before
/// bn-36luu: a directional verb's ordinary authority allows every relation outside
/// its denied set. Non-affirmative relations are out of scope here (P2 is unchanged).
fn pre_fix_contribution(
    verb: PolicyVerb,
    relation: Relation,
    path: AcceptancePath,
) -> PolicyDecision {
    if verb.denies(relation) {
        return PolicyDecision::Block;
    }
    match verb {
        PolicyVerb::Unlocked
        | PolicyVerb::NoDecrease
        | PolicyVerb::NoRemoval
        | PolicyVerb::NoDowngrade
        | PolicyVerb::NoExpansion => PolicyDecision::Allow,
        PolicyVerb::ProposalOnly => match path {
            AcceptancePath::HumanAccept => PolicyDecision::Allow,
            AcceptancePath::AgentAccept => PolicyDecision::Review,
        },
        PolicyVerb::Review | PolicyVerb::Locked if relation.is_unchanged() => PolicyDecision::Allow,
        PolicyVerb::Review => PolicyDecision::Review,
        PolicyVerb::Locked => PolicyDecision::Block,
    }
}

#[test]
fn mutant_the_pre_fix_p4_rule_allows_the_added_class_and_the_check_rejects_it() {
    let document = load(CORPUS[0]);
    let before = decode(&document);
    let after = decode(&with_class_added(&document, EXTRA_CLASS));
    let artifact = diff(&before, &after, AcceptancePath::AgentAccept);
    let real = wire_records(&artifact);
    assert_eq!(artifact.decision(), PolicyDecision::Review);

    // The mutant folds the real records under the real before-table with the
    // pre-fix P4. Every field the diff did not record is `unchanged`, which the
    // pre-fix rule allows under every verb, so only the recorded ones matter.
    let mutant = real
        .iter()
        .map(|(field, _, relation)| {
            let record = ClassificationRecord::new(*field, *relation).expect("admissible");
            pre_fix_contribution(
                before.policy().verb(record.field()),
                record.relation(),
                AcceptancePath::AgentAccept,
            )
        })
        .fold(PolicyDecision::Allow, PolicyDecision::join);
    assert_eq!(
        mutant,
        PolicyDecision::Allow,
        "the mutant must actually restore allow, or it does not exercise the check"
    );
    let error = check(
        &real,
        mutant,
        EXTRA_CLASS,
        Relation::Added,
        PolicyVerb::NoDowngrade,
        AcceptancePath::AgentAccept,
    )
    .expect_err("the check must reject the pre-fix verdict");
    assert!(error.contains("correction 20 requires review"), "{error}");

    // And the live verdict is not the mutant's: the fix is what the check measures.
    check(
        &real,
        artifact.decision(),
        EXTRA_CLASS,
        Relation::Added,
        PolicyVerb::NoDowngrade,
        AcceptancePath::AgentAccept,
    )
    .expect("the fixed verdict passes");
}
