//! Dedicated evidence map for `INV-012` — non-vacuous synthesis (`notes/plan/plan.md:351`,
//! the invariant bone `bn-2z0b`).
//!
//! > Forge objectives include required progress/availability behaviors and mutation
//! > challenges. Safety by disabling the system is rejected.
//! >
//! > — `notes/plan/plan.md`, INV-012's contract sentence
//!
//! bn-2z0b was groomed against bn-37d (PR 4, Intent Contract) because non-vacuity is
//! *first* enforced there, and this file's scope stays the Phase A enforcement point:
//! `src/optimization.rs`'s `optimization`/`non_vacuity` field group and
//! `IntentContract`'s use of it. What has changed since is *who calls it*:
//! `continuum-forge`'s task-assembly lane landed at bn-1dsih and is now the production
//! caller RFC 0037 AO2 names, cited live by
//! [`boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed`].
//! The mutation-challenge half of plan §14.5 is still unlanded, and that test remains
//! the tripwire for it.
//!
//! # What already closes most of this sentence, and where
//!
//! PR-4/IMPL-08 already landed the guard and a wide adversarial suite for it, and none
//! of that is reimplemented here:
//!
//! - [`Optimization::require_non_vacuity`](continuum_intent::optimization::Optimization::require_non_vacuity)
//!   is the obligation itself, with its own module-doc rationale for why it is a
//!   *caller-invoked* check rather than a decode-time rejection (INV-003: the schema
//!   admits an empty `non_vacuity` set, so a decoder that rejected one would reject
//!   artifacts the shape authority accepts).
//! - `src/optimization.rs`'s own unit tests
//!   (`a_removed_non_vacuity_behavior_is_never_reported_under_optimization`,
//!   `the_empty_group_is_a_declaration_and_not_an_absence`) and
//!   `tests/optimization_contract.rs`'s
//!   (`an_empty_non_vacuity_set_is_schema_legal_and_fails_the_inv_012_obligation`,
//!   `a_removed_non_vacuity_behavior_is_reported_under_non_vacuity_and_nothing_else`,
//!   `the_two_policy_keys_govern_the_two_halves_of_one_group`) already prove: the split
//!   between `optimization` and `non_vacuity` as policy keys, the plan-§14.5 behaviors
//!   discharging the obligation, and the empty set failing it with a typed error naming
//!   INV-012.
//! - `tests/optimization_adversarial.rs` already proves the group's *key* vocabulary
//!   fails closed — a fourth key, a near-miss spelling of `non_vacuity`
//!   (`nonvacuity`, `non-vacuity`), a duplicate key, and every truncation are typed
//!   errors and never panics or silent drops.
//!
//! Nothing above is re-tested here. What follows is genuinely new ground: the guard
//! exercised at the *whole-contract* level against real corpus fixtures (not the bare
//! `Optimization` group in isolation), the specific "safety by disabling" mutant forms
//! plan §14.5 and `docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md` name, what the
//! cross-field checker does and does not catch, and an anti-drift, mutation-tested
//! cross-check between the code and its two normative prose sources (plan.md and
//! `PLAN_REQUIREMENTS.json`) and the schema.
//!
//! # Clause → test map
//!
//! - **"required progress/availability behaviors"** —
//!   [`positive_the_die_hard_contract_declares_a_progress_behavior_and_the_guard_accepts_it`],
//!   [`positive_the_replicated_register_contract_declares_hard_soft_and_non_vacuity_together`].
//!   Both decode a real corpus fixture whose `optimization.non_vacuity` is a plan-§14.5
//!   positive behavior, discharge the obligation, and round-trip to stable canonical
//!   bytes and identity.
//! - **"Safety by disabling the system is rejected"** —
//!   [`negative_disabling_the_die_hards_only_progress_behavior_is_schema_legal_but_inv_012_rejects_it`]
//!   (the mutant: emptying the one behavior a real fixture declares is still
//!   schema-valid and still decodes, but the obligation now fails with a typed error
//!   naming INV-012 — never a panic, never silently accepted);
//!   [`negative_a_repair_that_removes_the_progress_behavior_is_exactly_the_relation_the_fixtures_own_policy_blocks`]
//!   (the same mutant, read as a *revision*: the removal classifies under
//!   `non_vacuity` and nothing else, and the fixture's own `no-removal` verb denies
//!   exactly that relation — tying `require_non_vacuity`, `Optimization::changes`, and
//!   `PolicyVerb::denies` into one attack-and-block scenario, none of which was
//!   previously composed together at the whole-contract level).
//! - **the boundary the guard must not over-reject** —
//!   [`boundary_zero_hard_and_zero_soft_objectives_do_not_block_acceptance`] (a
//!   contract need declare no hard constraint and no soft objective at all — one
//!   non-vacuity behavior suffices, which is exactly the die-hard fixture's own shape).
//! - **a boundary this bone's brief asks to be reported rather than patched** —
//!   [`boundary_the_guard_counts_behaviors_it_does_not_judge_their_content`] (the three
//!   sets are opaque strings classified by membership only — RFC 0031 — so a
//!   syntactically-present but semantically trivial behavior discharges the obligation
//!   exactly as a genuine one does; the module doc calls this open, and real triviality
//!   detection is plan §14.5's "property mutation and hidden semantic variants",
//!   i.e. Forge's territory, not this crate's).
//! - **the two-call contract, ratified** — bn-2z0b reported as a concern that
//!   `IntentContract::check`'s `ContractVerdict` carries no non-vacuity rule, so a
//!   caller trusting `check().is_well_formed()` alone would accept a "safety by
//!   disabling" contract with no signal. `bn-32kpe` decided it: RFC 0037 gains
//!   "Acceptance obligations that are not well-formedness" (AO1–AO4) and correction 17,
//!   ratifying the two-call contract rather than adding a sixth rule. The three tests
//!   below now pin the *decision* rather than reporting an open concern —
//!   [`ratified_the_cross_field_checker_decides_five_rules_and_non_vacuity_is_the_second_call`]
//!   (the fixture and its emptied mutant produce the identical verdict; the obligation
//!   still catches it),
//!   [`ratified_a_vacuous_contract_can_be_wholly_well_formed_and_the_verdict_names_the_rules_it_decided`]
//!   (the sharper form: a contract that is clean on all five rules stays clean with its
//!   non-vacuity set emptied, and the verdict's own rendering enumerates the rules it
//!   decided — so a sixth rule joining the surface breaks this test rather than
//!   arriving unnoticed), and
//!   [`ratified_a_directional_verb_over_an_empty_set_is_dormant_not_ill_formed`] (the
//!   corpus evidence for AO4, which is what decided against the narrower candidate rule
//!   that would have flagged only the `no-removal`-over-empty-`non_vacuity` pair).
//! - **the second call's production caller, landed** —
//!   [`boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed`].
//!   This file used to record that nothing outside tests called the obligation, which
//!   RFC 0037 carried as flag F12. bn-1dsih landed `continuum-forge`'s task-assembly
//!   lane — `crates/continuum-forge/src/task.rs`'s `assemble`, AO2's caller with
//!   standing — and that test now cites it by content: the lane's second call, its
//!   public entry point, its skip-the-second-call mutant test, and the RFC's own
//!   discharge annotation. It stays a tripwire in both directions, because the *other*
//!   half of plan §14.5 — mutation challenges and hidden semantic variants — is still
//!   unlanded, and the same test pins that absence against the lane's real source.
//! - **the schema does not itself enforce non-emptiness** —
//!   [`schema_does_not_encode_inv_012s_non_emptiness_constraint`]. Confirmed by exact
//!   text match against the live schema, so this reports rather than edits it (schema
//!   changes are epoch-governed and out of this bone's fence).
//! - **anti-drift, proven non-vacuous** — [`the_check_is_not_vacuous`]. Three
//!   independent sources — `plan.md`'s prose, `PLAN_REQUIREMENTS.json`'s machine-read
//!   dossier copy of the same sentence, and the schema's `non_vacuity` shape — are each
//!   read by a small anchor-based extractor and compared against a fixed expectation;
//!   each comparison is then shown capable of *failing* by mutating a local copy of its
//!   one source and re-running the same extractor, per docs/03 §8 and the
//!   `crates/continuum-task/tests/budget_dimensions.rs` precedent this borrows its name
//!   from.
//! - **the citation itself, held to its source** —
//!   [`the_disposition_this_file_pins_is_the_one_rfc_0037_records`]. The three
//!   `ratified_*` tests quote RFC 0037's AO section; this one reads the live RFC and
//!   proves each anchor is really there and really capable of going missing, so a
//!   withdrawn or reworded disposition breaks the tests that rest on it rather than
//!   leaving them citing prose that no longer exists.
//!
//! # House rules, inherited from the PR-4/INV-006/INV-014 evidence precedent
//!
//! - `src/` carries no behavior change from this file's bones. bn-2z0b touched nothing;
//!   bn-32kpe changed documentation only (`src/optimization.rs`'s flag became a
//!   citation of the RFC 0037 disposition it asked for, and `src/contract.rs` states
//!   what its verdict does not decide). No rule was added to or removed from the
//!   cross-field surface, which is the decision itself. No test outside this file is
//!   touched by either bone.
//! - Every fixture here is a real, already-reviewed corpus file
//!   (`tests/fixtures/die-hard-contract.json`,
//!   `tests/fixtures/replicated-register-contract.json`, both already used by
//!   `tests/pr4_exit_evidence.rs`) or a real repository document read at compile time
//!   via `include_str!`; nothing here shells out, reads a clock, or touches a network.
//! - Every mutant is exactly one bounded, linear substring replacement on a fixed,
//!   small input (the largest fixture used is under 7 KiB; every mutation is a single
//!   `.replace`/`.replacen`, never a loop of doublings and never nested growth), per
//!   this bone's hard OOM rule. [`fixture_sizes_stay_well_under_the_len_guard`] pins
//!   the sizes mechanically.

use continuum_intent::change_policy::{PolicyField, PolicyVerb, Relation};
use continuum_intent::contract::{
    CheckEnvironment, ContractFinding, IntentContract, WellFormednessRule,
};
use continuum_intent::optimization::{NonVacuityObligation, Optimization, OptimizationError};
use continuum_value::identity::Fnv1aPlaceholder;

// --- fixtures and normative sources, read at compile time ---------------------------------

/// The golden Die Hard Intent Contract — `optimization.hard = []`,
/// `optimization.soft = []`, `optimization.non_vacuity` carries one behavior, and
/// `policy.non_vacuity = "no-removal"`. Already the corpus's own instance of the
/// boundary this file names: a contract with *zero* hard constraints and *zero* soft
/// objectives is non-vacuous on the strength of one behavior alone.
const DIE_HARD_FIXTURE: &str = include_str!("fixtures/die-hard-contract.json");

/// The one behavior `DIE_HARD_FIXTURE` declares — `README.md:9`'s "shortest state with
/// `big = 4` at depth 6", transcribed as the witness that makes the safety failure
/// meaningful (`tests/pr4_exit_evidence.rs`'s own provenance table).
const DIE_HARD_BEHAVIOR: &str =
    "a reachable state with big == 4 witnesses the NotSolved refutation";

/// The replicated-register Intent Contract — `optimization.soft` carries one objective
/// *and* `optimization.non_vacuity` carries one behavior, so it exercises the shape
/// `DIE_HARD_FIXTURE` does not: all three sets non-empty at once is not required, but
/// admissible.
const REPLICATED_REGISTER_FIXTURE: &str =
    include_str!("fixtures/replicated-register-contract.json");

/// `intent-contract.schema.json`, the shape authority (INV-003).
const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

/// The dossier's own validated example contract — the one document in the tree that
/// `tests/pr4_exit_evidence.rs` shows produces a *clean* verdict (all five rules hold),
/// which is what makes it the right subject for the sharp form of the two-call pin: an
/// otherwise-clean contract stays clean when its non-vacuity set is emptied.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The dossier's plan document — the primary source of INV-012's contract sentence.
const PLAN_MD: &str = include_str!("../../../notes/plan/plan.md");

/// The machine-read dossier extraction of every plan requirement, including INV-012's
/// own copy of the same sentence — a second, independently-generated source (docs/03
/// §8's "independent paths" applied to documents rather than to a checker).
const PLAN_REQUIREMENTS_JSON: &str =
    include_str!("../../../notes/plan/notes/PLAN_REQUIREMENTS.json");

/// RFC 0037 — the normative home of the disposition the three `ratified_*` tests pin.
/// Read at compile time so the citation is checked rather than asserted: a deleted or
/// reworded AO section breaks the tests that quote it instead of leaving them citing
/// prose that no longer exists.
const RFC_0037: &str = include_str!("../../../notes/plan/rfcs/0037-intent-contract.md");

/// `continuum-forge`'s own crate documentation, cited so the split between what has
/// landed there and what has not is checked against real text rather than asserted in
/// prose. `include_str!` reads bytes and declares no dependency edge, so this citation
/// leaves `tools/check_crate_boundaries.py`'s `forge-not-imported-by-verifier` rule
/// untouched — the same device this file already uses for `plan.md` and the schema.
const CONTINUUM_FORGE_LIB: &str = include_str!("../../continuum-forge/src/lib.rs");

/// `continuum-forge`'s task-assembly lane — the production caller of
/// [`Optimization::require_non_vacuity`](continuum_intent::optimization::Optimization::require_non_vacuity),
/// landed at bn-1dsih. Cited by content so that a lane which stopped making the second
/// call breaks this file rather than leaving it claiming a caller that no longer calls.
const CONTINUUM_FORGE_TASK: &str = include_str!("../../continuum-forge/src/task.rs");

/// That lane's own evidence suite, cited by test name rather than re-run here: a
/// `tests/*.rs` file is its own crate and this one takes no dependency on
/// `continuum-forge`.
const CONTINUUM_FORGE_TASK_TESTS: &str =
    include_str!("../../continuum-forge/tests/non_vacuity_second_call.rs");

/// INV-012's contract sentence, transcribed once. Every extractor below is checked
/// against this fixed constant, and [`the_check_is_not_vacuous`] proves each extraction
/// is a real reading of its source rather than a hardcoded match.
const EXPECTED_CONTRACT_SENTENCE: &str = "Forge objectives include required progress/availability \
     behaviors and mutation challenges. Safety by disabling the system is rejected.";

/// The exact, whitespace-precise `optimization.non_vacuity` property block the schema
/// declares: three keys (`items`, `type`, `uniqueItems`) and nothing else — no
/// `minItems`, no `required`. An exact-text match rather than a JSON-path read, because
/// the point being pinned is the *absence* of a sibling key, and a JSON-path reader
/// would not notice one appearing beside the keys it already looks for.
const NON_VACUITY_SCHEMA_BLOCK: &str = "        \"non_vacuity\": {\n          \"items\": {\n            \"type\": \"string\"\n          },\n          \"type\": \"array\",\n          \"uniqueItems\": true\n        },";

// --- helpers -------------------------------------------------------------------------------

/// `DIE_HARD_FIXTURE` with its one non-vacuity behavior emptied — the "safety by
/// disabling the system" mutant. Schema-legal (the schema declares no `required` or
/// `minItems` on `non_vacuity`; see [`schema_does_not_encode_inv_012s_non_emptiness_constraint`])
/// and therefore still decodable; INV-012-illegal once a caller invokes the obligation.
fn vacuous_die_hard() -> String {
    let mutated = DIE_HARD_FIXTURE.replace(
        &format!("\"non_vacuity\":[\"{DIE_HARD_BEHAVIOR}\"]"),
        "\"non_vacuity\":[]",
    );
    assert_ne!(
        mutated, DIE_HARD_FIXTURE,
        "the mutation must actually remove the behavior, not merely copy the fixture"
    );
    mutated
}

/// `SCHEMA_EXAMPLE` with its one non-vacuity behavior emptied — the same "safety by
/// disabling the system" mutation as [`vacuous_die_hard`], applied to the document that
/// is otherwise clean on all five cross-field rules. The example is pretty-printed, so
/// the substring is the three-line block rather than the canonical one-liner; it is
/// still one bounded, linear `replacen` on a 4.5 KiB input.
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

/// The `CheckEnvironment` a contract's own canonical encoding mints — the same
/// construction `tests/pr4_exit_evidence.rs`'s `die_hard_environment` uses, rebuilt
/// here because a `tests/*.rs` file is its own crate and cannot `use` a sibling one.
fn minted_environment(contract: &IntentContract) -> CheckEnvironment {
    CheckEnvironment::new().with_minted_intent_id(contract.mint_intent_id::<Fnv1aPlaceholder>())
}

/// The environment under which `SCHEMA_EXAMPLE` is clean — the same three facts
/// `tests/pr4_exit_evidence.rs`'s `the_schemas_example_is_schema_valid_and_well_formed`
/// supplies, rebuilt here because a `tests/*.rs` file is its own crate.
///
/// The minted handle is the example's own declared `in_ack_v1` *as a literal*, exactly
/// as that test supplies it. That is what holds W9 fixed across the mutation, so the
/// only variable between the two verdicts below is the non-vacuity set: a mint
/// recomputed from the mutated bytes would move W9 and confound the comparison this
/// test exists to make.
fn schema_example_environment() -> CheckEnvironment {
    CheckEnvironment::new()
        .with_domain_pack_profiles(["storage-posix-v1".to_owned()])
        .with_correspondence_maps(["map_register_abs_v1".to_owned()])
        .with_minted_intent_id("in_ack_v1")
}

/// Whether `bytes` decodes to a contract whose non-vacuity obligation holds. `false`
/// covers both "does not decode" and "decodes but the obligation fails" — the guard a
/// synthesis-task caller would actually run.
fn is_accepted_as_non_vacuous(bytes: &[u8]) -> bool {
    IntentContract::decode(bytes)
        .map(|contract| contract.optimization().require_non_vacuity().is_ok())
        .unwrap_or(false)
}

/// INV-012's contract sentence, read from `plan.md`'s own heading.
///
/// The anchor matches only the heading's fixed prefix, not the whole line: the
/// traceability extractor's own `(delivered: …)` annotation (`generate_traceability.py`
/// / `docs/12`'s pattern) is appended to this exact heading once this bone's evidence
/// lands, and a brittle whole-line anchor would break on that annotation rather than
/// on a genuine drift in the contract sentence below it.
fn plan_md_contract_sentence(plan_md: &str) -> &str {
    let anchor = "### INV-012 \u{2014} Non-vacuous synthesis";
    let start = plan_md
        .find(anchor)
        .expect("plan.md declares the INV-012 heading verbatim");
    let after_heading = &plan_md[start + anchor.len()..];
    let heading_end = after_heading
        .find('\n')
        .expect("the heading line is terminated");
    let body = after_heading[heading_end + 1..].trim_start_matches('\n');
    let end = body
        .find('\n')
        .expect("the contract sentence ends its own line");
    &body[..end]
}

/// The same sentence, read from `PLAN_REQUIREMENTS.json`'s machine-generated
/// `metadata.contract` field for `INV-012` — an independently produced copy.
fn plan_requirements_contract_sentence(json_text: &str) -> &str {
    let anchor = "\"id\": \"INV-012\",\n      \"metadata\": {\n        \"contract\": \"";
    let start = json_text
        .find(anchor)
        .expect("PLAN_REQUIREMENTS.json declares INV-012 with a metadata.contract field");
    let rest = &json_text[start + anchor.len()..];
    let end = rest
        .find("\"\n")
        .expect("the contract string is closed on its own line");
    &rest[..end]
}

// --- sizes: the hard OOM rule, pinned -------------------------------------------------------

#[test]
fn fixture_sizes_stay_well_under_the_len_guard() {
    assert!(DIE_HARD_FIXTURE.len() < 8192, "{}", DIE_HARD_FIXTURE.len());
    assert!(
        REPLICATED_REGISTER_FIXTURE.len() < 8192,
        "{}",
        REPLICATED_REGISTER_FIXTURE.len()
    );
    assert!(vacuous_die_hard().len() < 8192);
    assert!(SCHEMA_EXAMPLE.len() < 8192, "{}", SCHEMA_EXAMPLE.len());
    assert!(vacuous_schema_example().len() < 8192);
}

// --- positive: required progress/availability behaviors -------------------------------------

#[test]
fn positive_the_die_hard_contract_declares_a_progress_behavior_and_the_guard_accepts_it() {
    let contract = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes())
        .expect("the fixture decodes");
    assert_eq!(contract.optimization().require_non_vacuity(), Ok(()));
    assert_eq!(contract.optimization().non_vacuity_len(), 1);
    assert_eq!(
        contract
            .optimization()
            .non_vacuity()
            .map(NonVacuityObligation::as_str)
            .collect::<Vec<_>>(),
        vec![DIE_HARD_BEHAVIOR]
    );
    // Zero hard constraints, zero soft objectives — the obligation does not lean on
    // either. See boundary_zero_hard_and_zero_soft_objectives_do_not_block_acceptance.
    assert_eq!(contract.optimization().hard().count(), 0);
    assert_eq!(contract.optimization().soft().count(), 0);

    // Canonical bytes and identity are stable across a decode/re-encode/decode round
    // trip — ID5/ID7, exercised end to end rather than assumed from the bare group.
    let bytes = contract.to_artifact_bytes();
    assert_eq!(bytes, DIE_HARD_FIXTURE.trim_end().as_bytes());
    let redecoded = IntentContract::decode(&bytes).expect("the encoder's own output decodes");
    assert_eq!(redecoded, contract);
    assert_eq!(redecoded.identity(), contract.identity());
}

#[test]
fn positive_the_replicated_register_contract_declares_hard_soft_and_non_vacuity_together() {
    let contract = IntentContract::decode(REPLICATED_REGISTER_FIXTURE.trim_end().as_bytes())
        .expect("the fixture decodes");
    assert_eq!(contract.optimization().require_non_vacuity(), Ok(()));
    assert_eq!(contract.optimization().non_vacuity_len(), 1);
    assert_eq!(contract.optimization().soft().count(), 1);
    assert_eq!(contract.optimization().hard().count(), 0);
    let bytes = contract.to_artifact_bytes();
    assert_eq!(bytes, REPLICATED_REGISTER_FIXTURE.trim_end().as_bytes());
    let redecoded = IntentContract::decode(&bytes).expect("the encoder's own output decodes");
    assert_eq!(redecoded.identity(), contract.identity());
}

// --- boundary: the guard must not over-reject ------------------------------------------------

#[test]
fn boundary_zero_hard_and_zero_soft_objectives_do_not_block_acceptance() {
    // The literal smallest legal non-vacuity declaration: one single-character
    // behavior, nothing else.
    let smallest = Optimization::new(
        [],
        [],
        [NonVacuityObligation::new("x").expect("a one-character behavior is non-empty")],
    )
    .expect("zero hard and zero soft objectives are not a defect");
    assert_eq!(smallest.require_non_vacuity(), Ok(()));
    assert!(!smallest.is_empty());
    assert_eq!(smallest.non_vacuity_len(), 1);

    // And the real corpus already contains this exact shape.
    let die_hard = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes()).expect("decodes");
    assert_eq!(die_hard.optimization().hard().count(), 0);
    assert_eq!(die_hard.optimization().soft().count(), 0);
    assert_eq!(die_hard.optimization().require_non_vacuity(), Ok(()));
}

// --- negative/mutant: safety by disabling the system is rejected ----------------------------

#[test]
fn negative_disabling_the_die_hards_only_progress_behavior_is_schema_legal_but_inv_012_rejects_it()
{
    let mutant = vacuous_die_hard();
    // Still schema-legal, so it must still decode — a decoder that rejected it would
    // reject artifacts the shape authority admits (INV-003; see
    // schema_does_not_encode_inv_012s_non_emptiness_constraint).
    let contract = IntentContract::decode(mutant.trim_end().as_bytes())
        .expect("the vacuous mutant still decodes");
    assert_eq!(contract.optimization().non_vacuity_len(), 0);
    // And the obligation — the only place INV-012 is actually enforced — is a typed
    // rejection, never a panic and never silently accepted.
    let error = contract
        .optimization()
        .require_non_vacuity()
        .expect_err("a contract with no non-vacuity behavior violates INV-012");
    assert_eq!(error, OptimizationError::NoNonVacuityObligation);
    let message = error.to_string();
    assert!(message.contains("INV-012"), "{message}");
    assert!(
        message.contains("satisfies every safety claim"),
        "the message must name the vacuity attack, not just report failure: {message}"
    );
}

#[test]
fn negative_a_repair_that_removes_the_progress_behavior_is_exactly_the_relation_the_fixtures_own_policy_blocks()
 {
    let before = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes())
        .expect("the fixture decodes");
    let mutant = vacuous_die_hard();
    let after = IntentContract::decode(mutant.trim_end().as_bytes()).expect("the mutant decodes");

    // The removal is reported under `non_vacuity` and nothing else — RFC 0037
    // correction 5, exercised here as a real revision between two whole contracts
    // rather than between two bare `Optimization` groups.
    let changes = before.optimization().changes(after.optimization());
    assert_eq!(
        changes,
        vec![(
            PolicyField::NonVacuity,
            DIE_HARD_BEHAVIOR.to_owned(),
            Relation::Removed
        )]
    );

    // The fixture itself declares `non_vacuity = "no-removal"` — this is not an
    // invented policy, it is what the corpus document already carries — and that verb
    // denies exactly the relation this repair produces. This is the mechanism that
    // actually stops "safety by disabling the system": not a rejection of the
    // resulting document (which is schema-legal and decodes, per the previous test),
    // but a block on the *transaction* that would produce it.
    let verb = before.policy().verb(PolicyField::NonVacuity);
    assert_eq!(verb, PolicyVerb::NoRemoval);
    assert!(verb.denies(Relation::Removed));
}

#[test]
fn ratified_the_cross_field_checker_decides_five_rules_and_non_vacuity_is_the_second_call() {
    // bn-2z0b pinned this as an open concern; bn-32kpe decided it, and the behavior
    // below is now the ratified design rather than a reported gap:
    //
    // > **AO1 — An empty `optimization.non_vacuity` is well formed.** […] No W-rule
    // > rejects it, and a checker MUST NOT add one.
    // >
    // > **AO3 — A well-formedness verdict is not an acceptance decision and MUST NOT
    // > be read as one.** The verdict names the rules it decided; a caller that needs
    // > both answers makes both calls.
    // >
    // > — RFC 0037, "Acceptance obligations that are not well-formedness"
    //   (correction 17)
    //
    // `IntentContract::check`'s five rules (W2, W3, W7, W8, W9) carry none for
    // non-vacuity, so the fixture and its emptied mutant are indistinguishable to it —
    // and a caller who treats a verdict as full acceptance accepts a "safety by
    // disabling" contract. The remedy RFC 0037 chose is that the caller with standing
    // makes the second call, not that the surface grows a sixth rule; the alternatives
    // and why each lost are in that RFC's "Rejected alternatives".
    let good = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes())
        .expect("the fixture decodes");
    let mutant = vacuous_die_hard();
    let vacuous = IntentContract::decode(mutant.trim_end().as_bytes()).expect("the mutant decodes");

    let good_verdict = good.check(&minted_environment(&good));
    let vacuous_verdict = vacuous.check(&minted_environment(&vacuous));

    // Both verdicts carry exactly the one finding `tests/pr4_exit_evidence.rs` already
    // established for this fixture (the corpus assigns no `in_*` handle, so W9 is the
    // only rule it fails) — emptying the non-vacuity set adds nothing to either
    // verdict.
    assert_eq!(good_verdict.findings().len(), 1, "{good_verdict}");
    assert_eq!(vacuous_verdict.findings().len(), 1, "{vacuous_verdict}");
    assert_eq!(good_verdict.findings()[0].rule(), WellFormednessRule::W9);
    assert_eq!(vacuous_verdict.findings()[0].rule(), WellFormednessRule::W9);
    assert!(matches!(
        &good_verdict.findings()[0],
        ContractFinding::DeclaredIdentityMismatch { declared, .. } if declared == "in_die_hard_v1"
    ));
    assert!(matches!(
        &vacuous_verdict.findings()[0],
        ContractFinding::DeclaredIdentityMismatch { declared, .. } if declared == "in_die_hard_v1"
    ));
    // The obligation itself still catches it — the point is that `check` alone does
    // not, so the caller with standing (the lane that assembles a synthesis task
    // against this contract — RFC 0037 AO2) must invoke it explicitly. The two calls
    // together are the acceptance decision; neither alone is.
    assert_eq!(
        vacuous.optimization().require_non_vacuity(),
        Err(OptimizationError::NoNonVacuityObligation)
    );
    assert_eq!(good.optimization().require_non_vacuity(), Ok(()));
}

#[test]
fn ratified_a_vacuous_contract_can_be_wholly_well_formed_and_the_verdict_names_the_rules_it_decided()
 {
    // The sharp form. The die-hard fixture fails W9 (the corpus assigns it no handle),
    // so the test above compares two *one-finding* verdicts and cannot show that a
    // vacuous contract reaches a fully clean one. The dossier's own validated example
    // can: `tests/pr4_exit_evidence.rs` establishes it is clean on all five rules.
    let good = IntentContract::decode(SCHEMA_EXAMPLE.as_bytes()).expect("the example decodes");
    assert_eq!(good.optimization().non_vacuity_len(), 1);
    let good_verdict = good.check(&schema_example_environment());
    assert!(good_verdict.is_well_formed(), "{good_verdict}");

    let mutant = vacuous_schema_example();
    let vacuous = IntentContract::decode(mutant.as_bytes()).expect("the mutant still decodes");
    assert_eq!(vacuous.optimization().non_vacuity_len(), 0);
    // Nothing else moved: the mutation is the non-vacuity set and only it.
    assert_eq!(vacuous.optimization().hard().count(), 1);
    assert_eq!(vacuous.optimization().soft().count(), 1);
    assert_eq!(vacuous.policy(), good.policy());

    // AO1: an empty non-vacuity set is well formed. A contract that requires no
    // behavior to remain possible passes every rule the surface carries.
    let vacuous_verdict = vacuous.check(&schema_example_environment());
    assert!(vacuous_verdict.is_well_formed(), "{vacuous_verdict}");
    assert_eq!(vacuous_verdict, good_verdict);

    // AO3: the verdict states which rules it decided, in its own words. This is the
    // tripwire for the disposition — a sixth rule joining the surface, or a rule
    // leaving it, changes this rendering and lands a reader back on bn-32kpe's
    // reasoning instead of letting the surface's meaning drift silently.
    assert_eq!(
        vacuous_verdict.to_string(),
        "the contract satisfies W2, W3, W7, W8, and W9"
    );

    // And the second call is where INV-012 is answered.
    assert_eq!(
        vacuous.optimization().require_non_vacuity(),
        Err(OptimizationError::NoNonVacuityObligation)
    );
    assert_eq!(good.optimization().require_non_vacuity(), Ok(()));
}

#[test]
fn ratified_a_directional_verb_over_an_empty_set_is_dormant_not_ill_formed() {
    // The corpus evidence behind RFC 0037 AO4 — the fact that decided against the
    // narrower candidate rule (flag the `no-removal`-over-empty-`non_vacuity` pair as
    // an unenforceable block, the W7 analogy `src/optimization.rs` used to raise).
    //
    // > `no-removal` on `non_vacuity` governs *revisions* […]. It says nothing about
    // > the set's size, and an empty set does not make it ill-formed — the verb
    // > governs whatever the set comes to hold, which is the same dormant shape
    // > `no-removal` has on `faults` over an empty `fault_model.enabled` and on
    // > `optimization` over empty `hard`/`soft` sets, both of which the corpus carries
    // > today.
    // >
    // > — RFC 0037, AO4
    //
    // Both halves of that sentence are checked against the real document rather than
    // asserted, because the whole argument rests on the shape being ordinary.
    let die_hard = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes())
        .expect("the fixture decodes");

    // `faults: no-removal` over a fault model with nothing in it.
    assert_eq!(
        die_hard.policy().verb(PolicyField::Faults),
        PolicyVerb::NoRemoval
    );
    assert!(die_hard.fault_model().enabled().is_empty());
    assert!(die_hard.fault_model().profiles().is_empty());
    assert_eq!(die_hard.fault_model().units().count(), 0);

    // `optimization: no-removal` over empty `hard` and `soft` sets.
    assert_eq!(
        die_hard.policy().verb(PolicyField::Optimization),
        PolicyVerb::NoRemoval
    );
    assert_eq!(die_hard.optimization().hard().count(), 0);
    assert_eq!(die_hard.optimization().soft().count(), 0);

    // Both verbs deny `removed`, and neither can fire today — exactly the shape the
    // candidate rule would have called ill-formed on `non_vacuity`. The fixture is a
    // reviewed corpus document, so the shape is ordinary rather than defective, and
    // the surface reports nothing against either of them.
    assert!(PolicyVerb::NoRemoval.denies(Relation::Removed));
    let verdict = die_hard.check(&minted_environment(&die_hard));
    assert_eq!(verdict.findings().len(), 1, "{verdict}");
    assert_eq!(verdict.findings()[0].rule(), WellFormednessRule::W9);

    // The third `no-removal` verb on the same document — `non_vacuity` — is the one
    // whose set is non-empty, so on this contract it is the only one of the three that
    // is live. That asymmetry is a fact about the contract's content, not about its
    // governance, which is why AO1 puts the content question on the lane with standing.
    assert_eq!(
        die_hard.policy().verb(PolicyField::NonVacuity),
        PolicyVerb::NoRemoval
    );
    assert_eq!(die_hard.optimization().non_vacuity_len(), 1);
}

// --- boundary: the guard counts behaviors, it does not judge their content ------------------

#[test]
fn boundary_the_guard_counts_behaviors_it_does_not_judge_their_content() {
    // RFC 0031 classifies `optimization.non_vacuity` by set membership only — the
    // three sets are opaque strings today (`src/optimization.rs`'s own module doc:
    // "Whether these strings grow typed forms is an open question"). So a
    // syntactically-present but semantically trivial or placeholder behavior
    // discharges the obligation exactly as a genuine one does: the guard is a
    // non-emptiness count, not a satisfiability judgement.
    for trivial in ["true", "noop", "x", "placeholder"] {
        let group = Optimization::new(
            [],
            [],
            [NonVacuityObligation::new(trivial).expect("non-empty")],
        )
        .expect("one behavior");
        assert_eq!(
            group.require_non_vacuity(),
            Ok(()),
            "a trivial behavior {trivial:?} must not be rejected by this guard — it is not \
             the guard's job to judge content, only its presence"
        );
    }
    // What *would* catch a trivially-satisfiable behavior is plan §14.5's other named
    // defense — "property mutation and hidden semantic variants detect overfitting" —
    // which is Forge's territory (continuum-forge, PR 29), not this crate's. See
    // boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed.
    assert!(
        PLAN_MD.contains("Property mutation and hidden semantic variants detect overfitting."),
        "the defense against a trivially-satisfiable challenge is named in plan §14.5, distinct \
         from the non-vacuity behaviors this guard actually enforces"
    );
}

#[test]
fn boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed() {
    // plan §14.5 states the non-vacuity obligation and the mutation-challenge defense
    // as two separate sentences in one section — the same split RFC 0037 makes
    // structural between `optimization.non_vacuity` and everything Forge does with
    // hidden variants at synthesis time.
    let section_start = PLAN_MD
        .find("### 14.5 Non-vacuity and anti-gaming")
        .expect("plan.md declares §14.5");
    let progress_sentence = PLAN_MD
        .find("Every synthesis task includes positive behaviors or progress scenarios")
        .expect("the non-vacuity half of §14.5");
    let mutation_sentence = PLAN_MD
        .find("Property mutation and hidden semantic variants detect overfitting.")
        .expect("the mutation-challenge half of §14.5");
    assert!(section_start < progress_sentence);
    assert!(
        progress_sentence < mutation_sentence,
        "the two clauses are distinct and ordered"
    );

    // continuum-forge names INV-012 as its own responsibility, and now discharges the
    // half of it that is a contract-admission question: bn-1dsih landed the
    // task-assembly lane RFC 0037 AO2 names, so the obligation implemented in
    // `src/optimization.rs` has a production caller. This is the flipped half of what
    // this test used to pin — it read "continuum-forge is a PR-1/IMPL-01 scaffold, so
    // nothing outside tests makes the second call" — and it stays a tripwire in the
    // same direction: the citations below are matched against the lane's real source,
    // so a lane that stops calling the obligation, or that grows the search this
    // boundary says has not landed, breaks this test rather than leaving it asserting a
    // caller that no longer calls.
    assert!(
        CONTINUUM_FORGE_LIB.contains("INV-012"),
        "continuum-forge's crate doc must still name INV-012 as its own responsibility"
    );
    assert!(
        CONTINUUM_FORGE_TASK.contains("require_non_vacuity"),
        "continuum-forge's task-assembly lane must still make the second call — if this \
         is gone, RFC 0037's F12 is undischarged and its bookkeeping is wrong"
    );
    assert!(
        CONTINUUM_FORGE_TASK.contains("pub fn assemble("),
        "the second call must be reachable from a public lane, not buried in a private \
         helper no caller can invoke"
    );
    assert!(
        CONTINUUM_FORGE_TASK_TESTS.contains(
            "fn mutant_a_lane_that_made_only_the_first_call_would_accept_the_vacuous_contract"
        ),
        "the lane's own suite must keep the skip-the-second-call mutant"
    );

    // The mutation-challenge half of §14.5 is still *not* landed, and this is the
    // tripwire for it: the lane decides whether a contract may become a task and runs no
    // search, so nothing yet detects a trivially-satisfiable declared behavior. The
    // moment continuum-forge grows the enumeration or mutation machinery PR 29 names,
    // these lines change and the boundary above must be re-derived rather than assumed.
    assert!(
        CONTINUUM_FORGE_LIB.contains("no enumeration, no counterexample loop"),
        "continuum-forge must still declare the search half unlanded; if it does not, \
         the 'mutation challenges are deferred' boundary needs revisiting, not assuming"
    );
    for absent in [
        "fn enumerate",
        "fn mutate",
        "struct Candidate",
        "struct Archive",
    ] {
        assert!(
            !CONTINUUM_FORGE_TASK.contains(absent),
            "continuum-forge's task lane grew {absent:?}; the search machinery may be \
             arriving and this boundary must be re-derived"
        );
    }
}

// --- schema: what it does and does not enforce -----------------------------------------------

#[test]
fn schema_does_not_encode_inv_012s_non_emptiness_constraint() {
    // Exactly three keys on `non_vacuity` — `items`, `type`, `uniqueItems` — and
    // nothing else: no `minItems`, no `required`. This is why
    // `Optimization::require_non_vacuity` exists as a caller-invoked check rather than
    // a decode-time rejection (INV-003), and it is a fact about the schema this file
    // holds the code to rather than assumes.
    assert!(
        SCHEMA.contains(NON_VACUITY_SCHEMA_BLOCK),
        "the schema's optimization.non_vacuity property must be exactly {{items, type, \
         uniqueItems}}; if this no longer matches, either the schema changed (out of this \
         bone's fence — schema edits are epoch-governed) or the non-emptiness constraint has \
         moved into the schema, and Optimization::require_non_vacuity's caller-invoked design \
         should be revisited"
    );
}

// --- the disposition itself, held to its normative source -------------------------------------

#[test]
fn the_disposition_this_file_pins_is_the_one_rfc_0037_records() {
    // The three `ratified_*` tests above quote RFC 0037's AO section. A citation that
    // is only prose rots silently, so the section's own anchors are matched against the
    // live document — and, per this file's anti-drift discipline, each match is then
    // shown capable of failing.
    const ANCHORS: [&str; 4] = [
        "### Acceptance obligations that are not well-formedness",
        "**AO1 — An empty `optimization.non_vacuity` is well formed.**",
        "**AO3 — A well-formedness verdict is not an acceptance decision and MUST NOT be read as \
         one.**",
        "17. **Non-vacuity is absent from the cross-field verdict surface, and that absence is \
         the design.**",
    ];
    for anchor in ANCHORS {
        assert!(
            RFC_0037.contains(anchor),
            "RFC 0037 no longer carries {anchor:?}; the disposition this file pins has moved or \
             been withdrawn, and the tests citing it must be revisited rather than left standing"
        );
        let mutated = RFC_0037.replacen(anchor, "", 1);
        assert_ne!(mutated, RFC_0037);
        assert!(!mutated.contains(anchor));
    }

    // The decision is a *decline*, not a deferral: the RFC records the alternative it
    // rejected, so a future reader finds the reasoning rather than an open question.
    assert!(
        RFC_0037.contains(
            "**A W-rule making a non-empty `optimization.non_vacuity` a well-formedness \
             condition**"
        ),
        "the rejected W-rule must stay recorded under RFC 0037's \"Rejected alternatives\""
    );
    // And the obligation's caller is named. The flag's own text is kept — flags are not
    // deleted when paid, they are annotated (`F3`, `F4`, `F5`, `F10`, `F11` all carry a
    // "Paid by …" tail) — so both halves are pinned: the flag as raised, and the
    // discharge that answers it.
    assert!(
        RFC_0037.contains("**F12 — No landed lane discharges the non-vacuity obligation.**"),
        "the flag as raised must stay recorded, so a reader finds the reasoning"
    );
    assert!(
        RFC_0037.contains("**Discharged by the bn-1dsih Forge task-assembly lane, 2026-08-09**"),
        "AO2's caller with standing has shipped, and the flag must say so — if this \
         annotation is gone, either the lane was reverted or the bookkeeping was lost, \
         and boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed \
         must be re-derived rather than trusted"
    );
}

// --- anti-drift, proven non-vacuous ----------------------------------------------------------

#[test]
fn plan_md_and_plan_requirements_json_agree_on_inv_012s_contract_sentence() {
    assert_eq!(
        plan_md_contract_sentence(PLAN_MD),
        EXPECTED_CONTRACT_SENTENCE
    );
    assert_eq!(
        plan_requirements_contract_sentence(PLAN_REQUIREMENTS_JSON),
        EXPECTED_CONTRACT_SENTENCE
    );
}

#[test]
fn the_check_is_not_vacuous() {
    // Source 1: plan.md's prose. Mutate the one occurrence of the contract sentence
    // and require the extraction to notice.
    let mutated_plan_md = PLAN_MD.replacen(
        EXPECTED_CONTRACT_SENTENCE,
        "Forge objectives include required progress/availability behaviors. Safety by disabling \
         the system is rejected.",
        1,
    );
    assert_ne!(
        mutated_plan_md, PLAN_MD,
        "the mutation must actually change plan.md's text"
    );
    assert_ne!(
        plan_md_contract_sentence(&mutated_plan_md),
        EXPECTED_CONTRACT_SENTENCE,
        "dropping 'and mutation challenges' from plan.md must break the comparison"
    );

    // Source 2: PLAN_REQUIREMENTS.json's independently generated copy.
    let mutated_requirements = PLAN_REQUIREMENTS_JSON.replacen(
        EXPECTED_CONTRACT_SENTENCE,
        "Forge objectives include required progress/availability behaviors and mutation \
         challenges.",
        1,
    );
    assert_ne!(mutated_requirements, PLAN_REQUIREMENTS_JSON);
    assert_ne!(
        plan_requirements_contract_sentence(&mutated_requirements),
        EXPECTED_CONTRACT_SENTENCE,
        "dropping the 'safety by disabling' clause from the dossier's own copy must break the \
         comparison"
    );

    // Source 3: the schema's non_vacuity shape. Add a minItems sibling — the exact
    // change that would make the non-emptiness constraint schema-enforced — and
    // require the exact-block match to notice.
    let widened_block = "        \"non_vacuity\": {\n          \"items\": {\n            \"type\": \
                          \"string\"\n          },\n          \"minItems\": 1,\n          \"type\": \
                          \"array\",\n          \"uniqueItems\": true\n        },";
    let mutated_schema = SCHEMA.replacen(NON_VACUITY_SCHEMA_BLOCK, widened_block, 1);
    assert_ne!(mutated_schema, SCHEMA);
    assert!(
        !mutated_schema.contains(NON_VACUITY_SCHEMA_BLOCK),
        "adding minItems to non_vacuity must break schema_does_not_encode_inv_012s_non_emptiness_constraint's \
         exact-block match"
    );

    // Source 4: the guard's own behavior. The baseline and the vacuity mutant must
    // disagree, so the comparison this whole file relies on is capable of failing —
    // not merely capable of passing.
    assert_ne!(
        is_accepted_as_non_vacuous(DIE_HARD_FIXTURE.trim_end().as_bytes()),
        is_accepted_as_non_vacuous(vacuous_die_hard().trim_end().as_bytes()),
        "the baseline and its vacuity mutant must be told apart"
    );
}
