//! The AO2 obligation's production caller, exercised against real corpus documents
//! (RFC 0037 correction 17 and flag F12, INV-012, bn-1dsih).
//!
//! # What this file proves, and what it deliberately leaves to `continuum-intent`
//!
//! `Optimization::require_non_vacuity` is already covered where it lives —
//! `continuum-intent`'s `src/optimization.rs` unit tests, `tests/optimization_contract.rs`,
//! `tests/optimization_adversarial.rs`, and the dedicated evidence map
//! `tests/inv012_nonvacuity_evidence.rs`. None of that is reimplemented here. What is
//! new is the *lane*: that a landed, non-test caller assembles a synthesis task only
//! after making the second call, refuses with a typed refusal naming INV-012 when the
//! obligation fails, and keeps the two calls' answers distinguishable.
//!
//! # Clause → test map
//!
//! - **"Every synthesis task includes positive behaviors or progress scenarios"**
//!   (plan §14.5) — [`positive_the_schema_example_assembles_and_the_task_carries_its_declared_behavior`],
//!   [`positive_the_die_hard_contract_assembles_with_zero_hard_and_zero_soft_objectives`].
//! - **"MUST refuse the task otherwise, with a typed refusal naming INV-012"**
//!   (RFC 0037 AO2) — [`negative_a_wholly_well_formed_contract_with_an_emptied_non_vacuity_set_is_refused`],
//!   [`negative_the_die_hard_mutant_is_refused_and_the_refusal_names_the_invariant_as_data`].
//! - **the mutant that skips the second call** —
//!   [`mutant_a_lane_that_made_only_the_first_call_would_accept_the_vacuous_contract`].
//!   The whole point of correction 17 is that a clean verdict is not an acceptance
//!   decision, so this test holds the first call's answer and the lane's answer side by
//!   side on the same bytes and requires them to disagree.
//! - **both calls are made, observably** —
//!   [`boundary_an_ill_formed_and_vacuous_contract_reports_both_grounds`] and
//!   [`boundary_an_ill_formed_but_non_vacuous_contract_reports_only_the_first`]. A lane
//!   that short-circuited on the verdict could not tell these two apart.
//! - **the guard counts behaviors, it does not judge them** (RFC 0037 AO3) —
//!   [`boundary_one_single_character_behavior_discharges_the_obligation`].
//! - **INV-008, typed** — [`boundary_the_two_refusals_are_distinct_types_not_one_flag`].
//! - **anti-drift** — [`the_disposition_this_lane_implements_is_the_one_rfc_0037_records`].
//!
//! House rules, inherited from the PR-4 evidence precedent: every fixture is a real,
//! already-reviewed corpus document read at compile time via `include_str!`; nothing
//! shells out, reads a clock, or touches a network; every mutant is exactly one
//! bounded, linear substring replacement on a fixed input under 8 KiB, pinned by
//! [`fixture_sizes_stay_well_under_the_len_guard`].

use continuum_forge::task::{TaskRefusal, assemble};
use continuum_intent::contract::{CheckEnvironment, IntentContract, WellFormednessRule};
use continuum_intent::optimization::NonVacuityObligation;

// --- fixtures and normative sources, read at compile time -----------------------------------

/// The dossier's own validated example contract — the one document in the tree that
/// `continuum-intent`'s `tests/pr4_exit_evidence.rs` shows produces a *clean* verdict on
/// all five cross-field rules, which is what makes it the right subject for the sharp
/// form of the second call: a contract that passes every W-rule and still may not become
/// a synthesis task.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The golden Die Hard Intent Contract — `optimization.hard = []`,
/// `optimization.soft = []`, and one non-vacuity behavior. The corpus's own instance of
/// the boundary a synthesis task must not over-reject.
const DIE_HARD_FIXTURE: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The one behavior `DIE_HARD_FIXTURE` declares.
const DIE_HARD_BEHAVIOR: &str =
    "a reachable state with big == 4 witnesses the NotSolved refutation";

/// The one behavior `SCHEMA_EXAMPLE` declares.
const SCHEMA_EXAMPLE_BEHAVIOR: &str = "synced request eventually acknowledged";

/// RFC 0037 — the normative home of AO2, the disposition this lane implements.
const RFC_0037: &str = include_str!("../../../notes/plan/rfcs/0037-intent-contract.md");

// --- helpers ----------------------------------------------------------------------------------

/// The environment under which `SCHEMA_EXAMPLE` is clean — the same three facts
/// `continuum-intent`'s `tests/pr4_exit_evidence.rs` supplies, rebuilt here because a
/// `tests/*.rs` file is its own crate and cannot `use` another crate's test module.
///
/// The minted handle is the example's own declared `in_ack_v1` as a literal, exactly as
/// that test supplies it, so W9 stays fixed across the mutations below and the only
/// variable between two verdicts is the non-vacuity set.
fn schema_example_environment() -> CheckEnvironment {
    CheckEnvironment::new()
        .with_domain_pack_profiles(["storage-posix-v1".to_owned()])
        .with_correspondence_maps(["map_register_abs_v1".to_owned()])
        .with_minted_intent_id("in_ack_v1")
}

/// The environment under which `DIE_HARD_FIXTURE` is clean: it names no domain-pack
/// profile and no abstraction map, so only W9's minted handle is owed.
fn die_hard_environment() -> CheckEnvironment {
    CheckEnvironment::new().with_minted_intent_id("in_die_hard_v1")
}

/// An environment that mints nothing — the fail-closed default, under which W9 fails and
/// the document is not well formed.
fn unminted_environment() -> CheckEnvironment {
    CheckEnvironment::new()
}

fn decode(text: &str) -> IntentContract {
    IntentContract::decode(text.trim_end().as_bytes()).expect("the document decodes")
}

/// `SCHEMA_EXAMPLE` with its one non-vacuity behavior emptied — the "safety by disabling
/// the system" mutant, still schema-legal and still decodable, INV-012-illegal once a
/// lane with standing invokes the obligation. The example is pretty-printed, so the
/// substring is the three-line block rather than a canonical one-liner.
fn vacuous_schema_example() -> String {
    let mutated = SCHEMA_EXAMPLE.replacen(
        "\"non_vacuity\": [\n      \"synced request eventually acknowledged\"\n    ],",
        "\"non_vacuity\": [],",
        1,
    );
    assert_ne!(
        mutated, SCHEMA_EXAMPLE,
        "the mutation must actually remove the behavior, not merely copy the example"
    );
    mutated
}

/// `DIE_HARD_FIXTURE` with its one non-vacuity behavior emptied — the same mutation
/// against the canonical one-line encoding.
fn vacuous_die_hard() -> String {
    let mutated = DIE_HARD_FIXTURE.replacen(
        &format!("\"non_vacuity\":[\"{DIE_HARD_BEHAVIOR}\"]"),
        "\"non_vacuity\":[]",
        1,
    );
    assert_ne!(
        mutated, DIE_HARD_FIXTURE,
        "the mutation must actually remove the behavior, not merely copy the fixture"
    );
    mutated
}

/// `DIE_HARD_FIXTURE` with its behavior replaced by the smallest legal one — a
/// one-character string. Syntactically present, semantically trivial.
fn trivially_non_vacuous_die_hard() -> String {
    let mutated = DIE_HARD_FIXTURE.replacen(DIE_HARD_BEHAVIOR, "x", 1);
    assert_ne!(mutated, DIE_HARD_FIXTURE);
    mutated
}

// --- sizes: the hard OOM rule, pinned ----------------------------------------------------------

#[test]
fn fixture_sizes_stay_well_under_the_len_guard() {
    assert!(SCHEMA_EXAMPLE.len() < 8192, "{}", SCHEMA_EXAMPLE.len());
    assert!(DIE_HARD_FIXTURE.len() < 8192, "{}", DIE_HARD_FIXTURE.len());
    assert!(vacuous_schema_example().len() < 8192);
    assert!(vacuous_die_hard().len() < 8192);
    assert!(trivially_non_vacuous_die_hard().len() < 8192);
}

// --- positive: a task is assembled, and it carries the declared behaviors ----------------------

#[test]
fn positive_the_schema_example_assembles_and_the_task_carries_its_declared_behavior() {
    let contract = decode(SCHEMA_EXAMPLE);
    let task = assemble(&contract, &schema_example_environment())
        .expect("a well-formed, non-vacuous contract assembles into a task");

    assert_eq!(task.intent_id().as_str(), "in_ack_v1");
    assert_eq!(
        task.positive_behaviors()
            .iter()
            .map(NonVacuityObligation::as_str)
            .collect::<Vec<_>>(),
        vec![SCHEMA_EXAMPLE_BEHAVIOR],
    );
    // The behaviors travel verbatim: this lane relays what the contract declared and
    // judges none of it (RFC 0037 AO3).
    assert_eq!(task.hard_objectives().len(), 1);
    assert_eq!(task.soft_objectives().len(), 1);
}

#[test]
fn positive_the_die_hard_contract_assembles_with_zero_hard_and_zero_soft_objectives() {
    // A synthesis task does not need a single hard constraint or soft objective; one
    // declared behavior suffices, which is exactly the corpus fixture's own shape.
    let contract = decode(DIE_HARD_FIXTURE);
    let task = assemble(&contract, &die_hard_environment())
        .expect("zero hard and zero soft objectives are not a defect");

    assert_eq!(task.intent_id().as_str(), "in_die_hard_v1");
    assert!(task.hard_objectives().is_empty());
    assert!(task.soft_objectives().is_empty());
    assert_eq!(
        task.positive_behaviors()
            .iter()
            .map(NonVacuityObligation::as_str)
            .collect::<Vec<_>>(),
        vec![DIE_HARD_BEHAVIOR],
    );
}

// --- negative: the second call refuses, with a typed refusal naming INV-012 ---------------------

#[test]
fn negative_a_wholly_well_formed_contract_with_an_emptied_non_vacuity_set_is_refused() {
    let mutant = vacuous_schema_example();
    let contract = decode(&mutant);
    // Still schema-legal and still decodable — a decoder that rejected it would reject
    // artifacts the shape authority admits (RFC 0037 AO1, INV-003).
    assert_eq!(contract.optimization().non_vacuity().count(), 0);

    let refusal = assemble(&contract, &schema_example_environment())
        .expect_err("a contract requiring no behavior to remain possible is not a task");

    let TaskRefusal::Vacuous(vacuous) = &refusal else {
        panic!("a well-formed contract may only be refused on INV-012 grounds: {refusal}");
    };
    assert_eq!(vacuous.invariant(), "INV-012");
    assert_eq!(vacuous.intent_id().as_str(), "in_ack_v1");
    assert_eq!(refusal.intent_id().as_str(), "in_ack_v1");
    assert_eq!(refusal.vacuous(), Some(vacuous));

    let message = refusal.to_string();
    assert!(message.contains("INV-012"), "{message}");
    assert!(
        message.contains("satisfies every safety claim"),
        "the refusal must name the vacuity attack, not merely report failure: {message}"
    );
}

#[test]
fn negative_the_die_hard_mutant_is_refused_and_the_refusal_names_the_invariant_as_data() {
    let mutant = vacuous_die_hard();
    let contract = decode(&mutant);
    let refusal = assemble(&contract, &die_hard_environment())
        .expect_err("emptying the one declared behavior refuses the task");

    // "Names INV-012" is a typed accessor, not a substring of a message a rewording
    // could drop — the refusal carries the invariant as data (RFC 0037 AO2, INV-008).
    let vacuous = refusal
        .vacuous()
        .expect("the refusal carries the INV-012 ground");
    assert_eq!(vacuous.invariant(), "INV-012");
    assert!(matches!(refusal, TaskRefusal::Vacuous(_)));
}

// --- the mutant that matters: a lane that skips the second call --------------------------------

#[test]
fn mutant_a_lane_that_made_only_the_first_call_would_accept_the_vacuous_contract() {
    // This is correction 17's whole argument, executed. The same bytes are handed to
    // the first call alone and to the lane; the first call says "well formed" and the
    // lane refuses. A Forge lane that dropped the second call would therefore assemble
    // a synthesis task against a contract that requires no behavior to remain possible,
    // which is precisely "safety by disabling the system" (INV-012).
    let mutant = vacuous_schema_example();
    let contract = decode(&mutant);
    let environment = schema_example_environment();

    let verdict = contract.check(&environment);
    assert!(
        verdict.is_well_formed(),
        "the vacuous mutant must still pass all five cross-field rules, or this test is \
         proving something else: {verdict}"
    );
    assert_eq!(
        verdict.to_string(),
        "the contract satisfies W2, W3, W7, W8, and W9"
    );

    assert!(
        assemble(&contract, &environment).is_err(),
        "the lane must refuse where the verdict alone accepts — that difference is the \
         second call"
    );

    // And the comparison is capable of failing: the unmutated example clears both.
    let good = decode(SCHEMA_EXAMPLE);
    assert!(good.check(&environment).is_well_formed());
    assert!(assemble(&good, &environment).is_ok());
}

// --- boundary: both calls run, and their answers stay distinguishable ---------------------------

#[test]
fn boundary_an_ill_formed_and_vacuous_contract_reports_both_grounds() {
    // W9 fails (nothing was minted) *and* the non-vacuity set is empty. The refusal
    // names the first call's verdict and still carries the second call's answer — which
    // a lane that short-circuited on the verdict could not have produced.
    let mutant = vacuous_die_hard();
    let contract = decode(&mutant);
    let refusal = assemble(&contract, &unminted_environment())
        .expect_err("an unminted, vacuous contract is refused");

    let TaskRefusal::NotWellFormed(not_well_formed) = &refusal else {
        panic!("the first call's failure decides the variant: {refusal}");
    };
    assert_eq!(not_well_formed.verdict().findings().len(), 1);
    assert_eq!(
        not_well_formed.verdict().findings()[0].rule(),
        WellFormednessRule::W9
    );
    let vacuous = not_well_formed
        .vacuous()
        .expect("the second call ran and failed, and the refusal says so");
    assert_eq!(vacuous.invariant(), "INV-012");
    assert_eq!(refusal.vacuous(), Some(vacuous));

    let message = refusal.to_string();
    assert!(message.contains("INV-012"), "{message}");
}

#[test]
fn boundary_an_ill_formed_but_non_vacuous_contract_reports_only_the_first() {
    // The control for the test above: the same unminted environment over the *unmutated*
    // fixture. W9 still fails; the second call now succeeds, so no INV-012 ground is
    // reported. If the lane fabricated the ground rather than running the call, this and
    // the previous test could not both hold.
    let contract = decode(DIE_HARD_FIXTURE);
    let refusal =
        assemble(&contract, &unminted_environment()).expect_err("an unminted contract fails W9");

    let TaskRefusal::NotWellFormed(not_well_formed) = &refusal else {
        panic!("expected the first call to decide: {refusal}");
    };
    assert_eq!(
        not_well_formed.verdict().findings()[0].rule(),
        WellFormednessRule::W9
    );
    assert_eq!(not_well_formed.vacuous(), None);
    assert_eq!(refusal.vacuous(), None);
    assert!(
        !refusal.to_string().contains("INV-012"),
        "a contract that met the obligation must not be told it did not: {refusal}"
    );
}

// --- boundary: presence, not content ------------------------------------------------------------

#[test]
fn boundary_one_single_character_behavior_discharges_the_obligation() {
    // RFC 0031 classifies `optimization.non_vacuity` by set membership only, so the
    // obligation is a presence requirement: a trivial behavior discharges it exactly as
    // a genuine one does. This is reported, not patched — judging content needs plan
    // §14.5's mutation challenges, which is search-time work this lane does not do and
    // PR 29 has not landed.
    let trivial = trivially_non_vacuous_die_hard();
    let contract = decode(&trivial);
    let task = assemble(&contract, &die_hard_environment())
        .expect("a syntactically present behavior discharges the obligation");
    assert_eq!(
        task.positive_behaviors()
            .iter()
            .map(NonVacuityObligation::as_str)
            .collect::<Vec<_>>(),
        vec!["x"],
    );
}

#[test]
fn boundary_the_two_refusals_are_distinct_types_not_one_flag() {
    // INV-008: "this is not a contract" and "this is a contract this lane may not act
    // on" are different outcomes, carried by different payload types, so a caller that
    // matched one arm cannot be holding the other.
    let vacuous_only = assemble(&decode(&vacuous_die_hard()), &die_hard_environment())
        .expect_err("well formed, obligation unmet");
    let ill_formed_only =
        assemble(&decode(DIE_HARD_FIXTURE), &unminted_environment()).expect_err("W9 fails");

    assert!(matches!(vacuous_only, TaskRefusal::Vacuous(_)));
    assert!(matches!(ill_formed_only, TaskRefusal::NotWellFormed(_)));
    assert_ne!(vacuous_only, ill_formed_only);
    // Neither refusal is a bare boolean or a bare string: both name the contract they
    // refused, and the INV-012 one names its invariant.
    assert_eq!(vacuous_only.intent_id(), ill_formed_only.intent_id());
    assert!(vacuous_only.vacuous().is_some());
    assert!(ill_formed_only.vacuous().is_none());
}

// --- anti-drift: the disposition this lane implements, held to its source -----------------------

#[test]
fn the_disposition_this_lane_implements_is_the_one_rfc_0037_records() {
    // A citation that is only prose rots silently. Each anchor is matched against the
    // live RFC and then shown capable of failing, per the house anti-drift discipline.
    const ANCHORS: [&str; 3] = [
        "**AO2 — The obligation belongs to the caller with standing, and that caller is named.**",
        "MUST refuse the task otherwise, with a typed refusal naming INV-012",
        "**F12 — No landed lane discharges the non-vacuity obligation.**",
    ];
    for anchor in ANCHORS {
        assert!(
            RFC_0037.contains(anchor),
            "RFC 0037 no longer carries {anchor:?}; the disposition this lane implements has \
             moved or been withdrawn, and this lane must be revisited rather than left standing"
        );
        let mutated = RFC_0037.replacen(anchor, "", 1);
        assert_ne!(mutated, RFC_0037);
        assert!(!mutated.contains(anchor));
    }

    // F12 is discharged *by this lane*, and the RFC says so. If that bookkeeping is
    // reverted, the flag and the code disagree and this test reports it.
    assert!(
        RFC_0037.contains("**Discharged by the bn-1dsih Forge task-assembly lane, 2026-08-09**"),
        "RFC 0037's F12 must record the lane that discharges it"
    );
}
