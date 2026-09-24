//! PR-15 / IMPL-04, cr-37bshu (bn-1oj6): the replicated-register Intent Contract adopts
//! the `-v1` pack profiles through a revision, never by an edit of the names it already
//! cites.
//!
//! # Why a revision, and what it is
//!
//! A pack profile's name identifies its canonical bytes (RFC 0002 correction 1). The
//! bn-1oj6 profiles that add declared unsupported cases and assumptions are therefore
//! published as `network/adversarial-v1` and `storage/append-log-v1`, and the packs keep
//! `-v0` byte for byte (each pack's `pr15-impl04-hon-03` registry test). The contract
//! names `-v0`, so its meaning does not move when the packs change. It reaches `-v1`
//! only by a revision, which this file runs through the library path:
//! `continuum_semantic_diff::artifact::assemble` classifies all fifteen fields (RFC 0031)
//! and computes `PolicyTable::verdict` against the before contract's own policy (RFC 0037
//! P1–P6), and `IntentContract::check` decides W8 against the profiles the packs declare.
//!
//! # The daemon path is not exercised, and why
//!
//! `intent.propose_revision` on the wire classifies nothing today. It checks for a
//! locked field and then refuses with `UnsupportedSemanticFeature`, so it never mints a
//! `proposed` record. `crates/continuum-intent/tests/inv001_protected_intent_evidence.rs`
//! pins that gap as "the daemon propose lane". The revision evidence here is therefore
//! library-level, as INV-001's classifier evidence already is. The `revise-intent`
//! capability that an acceptance needs (RFC 0037 correction 11: all fifteen fields
//! "require `revise-intent` to accept") is decided by the daemon's admission, not by
//! this library, and is not exercised here.
//!
//! # What RFC 0031 and RFC 0037 compute for an added profile
//!
//! RFC 0031 classifies `fault_model.profiles` by membership: "one record per class or
//! profile added or removed" (RFC 0031, section `faults`). It does not read the
//! profile's declared assumptions, so a profile whose assumption set is a superset is
//! one `added` record on `faults`, not an assumption strengthening. RFC 0037's verb table
//! gives `no-removal` the denied set {`removed`} and "ordinary" authority, and states that
//! "the closed set contains no verb that blocks `added`". So the revision below closes
//! to what [`adding_the_v1_profiles_is_two_added_fault_records_and_the_computed_verdict`]
//! pins. That is a property of the RFCs as written, and it is recorded as a gap in the
//! bn-1oj6 report, for a follow-up bone: the added profile's assumptions do not reach
//! the `assumptions` classification, so an agent acceptance of a profile whose
//! declared assumptions are a superset closes to `allow`.
//!
//! Two limits of that `allow`. First, it leaves `-v1` outside `trust_boundaries.trusted`,
//! which still names only `-v0`: trusting `-v1` as the contract trusts `-v0` grows
//! `trusted`, which the assembler, with no `trust_boundaries` classifier yet, fails
//! closed to `unknown` and `review`
//! ([`trusting_the_v1_profiles_too_is_an_unclassified_trust_change_and_never_allow`]). Second, the
//! revised contract names two profiles of one pack, and no RFC yet says which of them
//! governs a run; the two share every modelled row, so a run is the same under both.
//!
//! Evidence, by artifact id:
//!
//! | Id | Test | What it shows |
//! |---|---|---|
//! | `pr15-impl04-rev-01` | [`the_unrevised_contract_still_resolves_because_v0_is_kept`] | the contract naming `-v0` passes W8 against the packs, which keep `-v0` |
//! | `pr15-impl04-rev-02` | [`adding_the_v1_profiles_is_two_added_fault_records_and_the_computed_verdict`] | the revision's diff is exactly `faults[…-v1]:added` for each profile, and its verdict is what RFC 0037 computes |
//! | `pr15-impl04-rev-03` | [`the_revised_contract_resolves_against_the_packs_and_not_against_a_v0_only_snapshot`] | W8 accepts the revision against the packs and refuses it, typed, against a snapshot whose packs predate `-v1` |
//! | `pr15-impl04-rev-04` | [`swapping_v0_for_v1_is_removed_plus_added_and_blocks_under_no_removal`] | a swap is `removed` + `added` and `block`, with the named reason |
//! | `pr15-impl04-rev-06` | [`trusting_the_v1_profiles_too_is_an_unclassified_trust_change_and_never_allow`] | adding `-v1` to `trusted` as well fails closed: `trust_boundaries` `unknown`, verdict `review` |
//! | `pr15-impl04-rev-05` | [`a_contract_naming_an_undeclared_profile_is_refused_by_w8`] | `-v9` is refused as `UnresolvedProfile` |

use std::collections::BTreeSet;

use continuum_intent::assurance_policy::level_from_wire;
use continuum_intent::change_policy::{AcceptancePath, PolicyDecision, PolicyField, Relation};
use continuum_intent::contract::{
    CheckEnvironment, ContractFinding, IntentContract, IntentId, WellFormednessRule,
};
use continuum_semantic_diff::artifact::{DiffArtifact, DiffId, DiffRequest, SnapshotId, assemble};

/// The replicated-register Intent Contract, as accepted: it names the `-v0` profiles.
const FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The packs' profile declarations. This crate may not depend on the packs, so the
/// names are read from their declarations (the only lexical step in this file).
const NETWORK_PROFILE: &str = include_str!("../../continuum-effects-network/src/profile.rs");
const STORAGE_PROFILE: &str = include_str!("../../continuum-effects-storage/src/profile.rs");

/// The fixture's `fault_model.profiles` as it is spelled in the canonical document.
const V0_PROFILES: &str = r#""profiles":["network/adversarial-v0","storage/append-log-v0"]"#;

fn declared(source: &str, constant: &str) -> String {
    let prefix = format!("pub const {constant}: &str = \"");
    let line = source
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("the pack declares {constant}"));
    line.strip_suffix("\";")
        .expect("one string literal")
        .to_owned()
}

/// The profile names the current packs declare: `-v0` and `-v1` of each.
fn pack_profiles() -> BTreeSet<String> {
    let names: BTreeSet<String> = [
        declared(NETWORK_PROFILE, "PROFILE_NAME_V0"),
        declared(NETWORK_PROFILE, "PROFILE_NAME"),
        declared(STORAGE_PROFILE, "PROFILE_NAME_V0"),
        declared(STORAGE_PROFILE, "PROFILE_NAME"),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        names,
        [
            "network/adversarial-v0",
            "network/adversarial-v1",
            "storage/append-log-v0",
            "storage/append-log-v1",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect()
    );
    names
}

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the document decodes")
}

fn with_profiles(profiles: &[&str]) -> String {
    let list: Vec<String> = profiles.iter().map(|p| format!("\"{p}\"")).collect();
    let replacement = format!("\"profiles\":[{}]", list.join(","));
    assert_eq!(FIXTURE.matches(V0_PROFILES).count(), 1);
    FIXTURE.replacen(V0_PROFILES, &replacement, 1)
}

/// The revision: the fixture with both `-v1` profiles added beside `-v0`.
fn revised() -> IntentContract {
    decode(&with_profiles(&[
        "network/adversarial-v0",
        "network/adversarial-v1",
        "storage/append-log-v0",
        "storage/append-log-v1",
    ]))
}

fn diff(before: &IntentContract, after: &IntentContract, path: AcceptancePath) -> DiffArtifact {
    assemble(&DiffRequest {
        diff_id: DiffId::new("diff_pr15_impl04_profiles").expect("well formed"),
        before_snapshot: SnapshotId::new("ws_before1").expect("well formed"),
        after_snapshot: SnapshotId::new("ws_after1").expect("well formed"),
        before_intent: IntentId::new("in_replicated_register_v1").expect("well formed"),
        after_intent: IntentId::new("in_replicated_register_v2").expect("well formed"),
        before,
        after,
        requested_assurance: level_from_wire("validated").expect("a closed-set level"),
        path,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    })
    .expect("the classification completes")
}

fn environment(profiles: BTreeSet<String>) -> CheckEnvironment {
    CheckEnvironment::new()
        .with_domain_pack_profiles(profiles)
        .with_correspondence_maps(["map_runtime_to_abstract_v1".to_owned()])
}

/// The W8 findings of `contract` against `profiles`. W9 (the declared handle against a
/// minted one) is the registry's to decide when it materializes the revision (RFC 0037
/// R2) and is not what these tests are about, so the other rules are asserted clean
/// and W9 is left out.
fn w8(contract: &IntentContract, profiles: BTreeSet<String>) -> Vec<ContractFinding> {
    let verdict = contract.check(&environment(profiles));
    for rule in [
        WellFormednessRule::W2,
        WellFormednessRule::W3,
        WellFormednessRule::W7,
    ] {
        assert_eq!(verdict.findings_for(rule).count(), 0, "{verdict}");
    }
    verdict
        .findings_for(WellFormednessRule::W8)
        .cloned()
        .collect()
}

fn relations(artifact: &DiffArtifact) -> Vec<(PolicyField, Option<String>, Relation)> {
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

/// `pr15-impl04-rev-01`. The packs keep `-v0`, so the contract as accepted still
/// resolves: publishing `-v1` changed nothing the contract names.
#[test]
fn the_unrevised_contract_still_resolves_because_v0_is_kept() {
    assert_eq!(w8(&decode(FIXTURE), pack_profiles()), Vec::new());
}

/// `pr15-impl04-rev-02`. Adding the `-v1` profiles is exactly two `faults` records, each
/// `added`, and no other field moves. The verdict is what RFC 0037 computes for them
/// under the fixture's `faults = "no-removal"`: `added` is not in the denied set
/// {`removed`}, the authority is "ordinary", and "the closed set contains no verb that
/// blocks `added`" (RFC 0037, "Verb applicability"). So both acceptance paths close to
/// `allow`, with no reason. Acceptance still needs `revise-intent` (correction 11),
/// which the daemon's admission decides.
#[test]
fn adding_the_v1_profiles_is_two_added_fault_records_and_the_computed_verdict() {
    let before = decode(FIXTURE);
    let after = revised();
    for path in [AcceptancePath::HumanAccept, AcceptancePath::AgentAccept] {
        let artifact = diff(&before, &after, path);
        assert_eq!(
            relations(&artifact),
            vec![
                (
                    PolicyField::Faults,
                    Some("network/adversarial-v1".to_owned()),
                    Relation::Added
                ),
                (
                    PolicyField::Faults,
                    Some("storage/append-log-v1".to_owned()),
                    Relation::Added
                ),
            ],
            "{path:?}"
        );
        assert_eq!(artifact.decision(), PolicyDecision::Allow, "{path:?}");
        assert!(artifact.verdict().reasons().is_empty());
    }
}

/// `pr15-impl04-rev-03`. The revised contract resolves against the packs' profiles. It
/// is refused, with one typed `UnresolvedProfile` per `-v1` name, against a snapshot
/// whose packs predate `-v1`: a contract never resolves a name its snapshot's packs do
/// not declare.
#[test]
fn the_revised_contract_resolves_against_the_packs_and_not_against_a_v0_only_snapshot() {
    let after = revised();
    assert_eq!(w8(&after, pack_profiles()), Vec::new());
    let old_snapshot: BTreeSet<String> = ["network/adversarial-v0", "storage/append-log-v0"]
        .map(str::to_owned)
        .into_iter()
        .collect();
    // W8 reports the first unresolved profile in token order.
    assert_eq!(
        w8(&after, old_snapshot),
        vec![ContractFinding::UnresolvedProfile {
            name: "network/adversarial-v1".to_owned()
        }]
    );
}

/// `pr15-impl04-rev-04`. Swapping `-v0` for `-v1` is two `removed` and two `added`
/// records, never an unchanged rename, and the fixture's `no-removal` blocks it with a
/// reason naming `faults` and `removed`.
#[test]
fn swapping_v0_for_v1_is_removed_plus_added_and_blocks_under_no_removal() {
    let before = decode(FIXTURE);
    let after = decode(&with_profiles(&[
        "network/adversarial-v1",
        "storage/append-log-v1",
    ]));
    let artifact = diff(&before, &after, AcceptancePath::HumanAccept);
    let moved: BTreeSet<(Option<String>, Relation)> = relations(&artifact)
        .into_iter()
        .map(|(field, unit, relation)| {
            assert_eq!(field, PolicyField::Faults);
            (unit, relation)
        })
        .collect();
    assert_eq!(
        moved,
        [
            ("network/adversarial-v0", Relation::Removed),
            ("network/adversarial-v1", Relation::Added),
            ("storage/append-log-v0", Relation::Removed),
            ("storage/append-log-v1", Relation::Added),
        ]
        .map(|(unit, relation)| (Some(unit.to_owned()), relation))
        .into_iter()
        .collect()
    );
    assert_eq!(artifact.decision(), PolicyDecision::Block);
    assert!(
        artifact
            .verdict()
            .reasons()
            .iter()
            .any(|r| r.field() == PolicyField::Faults && r.relation() == Relation::Removed),
        "{:?}",
        artifact.verdict().reasons()
    );
}

/// `pr15-impl04-rev-05`. A contract naming a profile no pack declares, `-v9`, is refused
/// by W8 as a typed `UnresolvedProfile`, at acceptance time, never deferred.
#[test]
fn a_contract_naming_an_undeclared_profile_is_refused_by_w8() {
    let named = decode(&with_profiles(&[
        "network/adversarial-v0",
        "network/adversarial-v9",
        "storage/append-log-v0",
    ]));
    assert_eq!(
        w8(&named, pack_profiles()),
        vec![ContractFinding::UnresolvedProfile {
            name: "network/adversarial-v9".to_owned()
        }]
    );
}

/// The fixture's trusted set as the canonical document spells it.
const V0_TRUSTED: &str = r#""trusted":["network/adversarial-v0","storage/append-log-v0"]"#;

/// `pr15-impl04-rev-06`. Adopting `-v1` fully — naming it in `fault_model.profiles` and
/// trusting it in `trust_boundaries.trusted` as `-v0` is trusted — grows the trusted
/// set. The assembler has no `trust_boundaries` classifier yet and fails closed:
/// `src/artifact.rs` `classify_fail_closed` records `trust_boundaries` as `unknown`,
/// and RFC 0037 P2 makes an `unknown` record contribute at least `review` under any
/// verb. So the full adoption is never `allow`: the `allow` of `pr15-impl04-rev-02`
/// holds only while `-v1` stays untrusted.
#[test]
fn trusting_the_v1_profiles_too_is_an_unclassified_trust_change_and_never_allow() {
    let profiles = with_profiles(&[
        "network/adversarial-v0",
        "network/adversarial-v1",
        "storage/append-log-v0",
        "storage/append-log-v1",
    ]);
    assert_eq!(profiles.matches(V0_TRUSTED).count(), 1);
    let trusted = profiles.replacen(
        V0_TRUSTED,
        r#""trusted":["network/adversarial-v0","network/adversarial-v1","storage/append-log-v0","storage/append-log-v1"]"#,
        1,
    );
    let artifact = diff(
        &decode(FIXTURE),
        &decode(&trusted),
        AcceptancePath::HumanAccept,
    );
    assert!(
        relations(&artifact).contains(&(PolicyField::TrustBoundaries, None, Relation::Unknown)),
        "{:?}",
        relations(&artifact)
    );
    assert_eq!(artifact.decision(), PolicyDecision::Review);
    assert!(
        artifact.verdict().reasons().iter().any(|r| {
            r.field() == PolicyField::TrustBoundaries && r.relation() == Relation::Unknown
        }),
        "{:?}",
        artifact.verdict().reasons()
    );
}
