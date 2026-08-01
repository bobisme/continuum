//! Dedicated exit evidence for `PR-4-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-4-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 4's Exit line).
//!
//! > **Exit:** Die Hard and replicated-register intents serialize canonically; ordinary
//! > operations cannot mutate them.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 4
//!
//! This file asserts that sentence end to end, as directly as it reads, the way
//! `crates/continuum-workspace/tests/pr2_exit_evidence.rs` and `pr3_exit_evidence.rs`
//! did for PRs 2 and 3. PR 4 landed across ten bones — nine field groups plus the
//! shared expression codec — and `tests/pr4_property_evidence.rs` already discharged
//! the *properties* half of clause one against the Die Hard corpus fixture. What none
//! of them is is a witness that the whole **contract** serializes canonically: a claim
//! set is not an Intent Contract, and the thing PR 4's exit names is the document.
//!
//! # Evidence map
//!
//! The exit sentence has three clauses. Each is carried by the tests named beside it;
//! the suites named after them reached the same claim first, at the field-group level,
//! and remain the corroborating evidence.
//!
//! - **"Die Hard […] intents serialize canonically"** —
//!   [`die_hards_intent_serializes_to_its_golden_bytes`],
//!   [`the_die_hard_golden_bytes_decode_back_to_the_same_contract`],
//!   [`a_reordered_and_reformatted_die_hard_contract_recanonicalizes`]. The whole
//!   twenty-key document round-trips with one byte spelling, and a document whose keys
//!   are shuffled, whose arrays arrive out of order, and which is pretty-printed with
//!   whitespace re-canonicalizes to the *same bytes and the same identity* — ID5's
//!   determinism claim and ID7's "reformatting it […] MUST NOT change it", made about
//!   a corpus fixture rather than an invented one. Corroborated by
//!   `pr4_property_evidence::die_hards_properties_serialize_to_their_golden_bytes`
//!   (the `claims` group's own golden vector, whose three claims this contract reuses
//!   verbatim) and by every field group's `*_adversarial_decode.rs` round-trip test.
//! - **"replicated-register intent serializes canonically"** —
//!   [`the_replicated_register_intent_serializes_to_its_golden_bytes`],
//!   [`the_replicated_register_golden_bytes_decode_back_to_the_same_contract`],
//!   [`a_reordered_and_reformatted_replicated_register_contract_recanonicalizes`].
//!   Same discipline, on a contract that exercises what Die Hard does not: quorum
//!   quantifiers, three classified assumptions, an observer a claim actually binds,
//!   all six fault classes, a fairness constraint, three nondeterminism sites, two
//!   declared fragments, and a `review` verb with its reviewers named.
//! - **"ordinary operations cannot mutate them"** —
//!   [`every_public_operation_leaves_the_contract_bit_identical`],
//!   [`no_type_in_this_crate_has_a_mut_self_method`], and the `compile_fail` doctest
//!   on `contract`'s module documentation. The first exercises every public operation
//!   the crate offers on a contract and asserts its bytes and identity are unchanged
//!   afterwards; the second is the type-level statement, checked mechanically against
//!   the crate's own sources rather than asserted in prose. Corroborated by
//!   `crates/continuum-workspace/src/publication.rs`'s `compile_fail`/passing doctest
//!   pair, which is the house idiom this borrows, and by `src/lib.rs`'s standing claim
//!   ("No type in this crate has a `&mut self` method").
//! - **boundary: what moves the identity and what does not** —
//!   [`id7s_checkable_list_holds_for_both_corpus_intents`]. ID7 is a list, so it is
//!   tested as one: renaming, re-`source`-ing, and re-handling the contract leave the
//!   identity fixed; changing a claim, an assumption, a bound, a fault, a fairness
//!   constraint, the completion policy, a nondeterminism class, the assurance
//!   requirement, an optimization or non-vacuity string, a security-policy element, a
//!   scope element, an abstraction-map binding, a policy verb, or a reviewer list moves
//!   it.
//! - **the schema decides shape** — [`the_closed_top_level_vocabulary_is_the_schemas_own`],
//!   [`the_schemas_own_validated_example_decodes_as_a_whole_contract`], and
//!   [`the_schemas_example_is_schema_valid_and_violates_w7`]. The first reads
//!   `intent-contract.schema.json` with this crate's own reader and compares the twenty
//!   keys it declares against the twenty a full contract encodes, so the closed
//!   top-level vocabulary is the schema's rather than a transcription of it (INV-003) —
//!   the transcription-distrust discipline `assurance_policy_contract.rs` applies to the
//!   assurance ladder. The other two decode the dossier's validated example as a whole
//!   contract and show it is schema-valid *and* W7-violating, which is the whole reason
//!   `decode` and `check` are different surfaces.
//! - **adversarial** — [`every_truncation_of_a_contract_is_a_typed_error`],
//!   [`an_unknown_top_level_key_is_refused`],
//!   [`a_duplicated_field_group_is_refused_rather_than_resolved_by_arrival_order`],
//!   [`each_cross_group_rule_produces_its_own_typed_verdict`], and
//!   [`the_preimage_contains_no_id2_excluded_key_at_any_depth`]. Every adversarial
//!   input here is **linear** in the fixture: the truncation sweep walks one document's
//!   bytes, and no test builds an input by textual doubling. `len() < 8192` is asserted
//!   before each sweep, so an input that grew past kilobyte scale fails the guard
//!   rather than the machine.
//!
//! # Fixture provenance
//!
//! Neither intent is invented. Every field is transcribed from the corpus, and where
//! the corpus states a fact in prose rather than in a field, the transcription is named
//! as such beside its citation.
//!
//! ## Die Hard — `notes/plan/corpus/tla-examples/ports/TV-009/`
//!
//! | Contract field | Source |
//! |---|---|
//! | `claims[TypeOK]` | `DieHard.ctm:30` — `invariant TypeOK { big in 0..5 && small in 0..3 }` |
//! | `claims[NotSolved]` | `DieHard.ctm:31` — `invariant NotSolved { big != 4 }` |
//! | `claims[SolutionReachesFourGallons]` | `port.json` `lean.theorems` — `Continuum.Examples.DieHard.solution_ends_with_four_gallons` |
//! | `assumptions[JugCapacities]` | `DieHard.ctm:4-5` — the `where` clauses *declare* the state space; `DieHard.ctm:30` *claims* it holds. Transcribed as the declaration, classified `environment` |
//! | `bounds.values = 6` | `DieHard.ctm:4` — `big: Nat where big <= 5`, the largest declared value domain, `{0…5}` |
//! | `bounds.nodes = 1` | `DieHard.ctm:2-6` — one modeled system; unlike `replicated_register.ctm:3`, no `type Node` is declared |
//! | `bounds.faults = 0` | `DieHard.ctm:10-25` — six jug actions, no fault action |
//! | `bounds.depth = null` | `default.model.toml:5` — `search = "breadth-first-exact"` is exhaustive, not depth-bounded; `expected_shortest_failure_depth` is a expected *result* |
//! | `fault_model.enabled = []` | as `bounds.faults`; `port.json` `features` names no fault feature |
//! | `fairness = []` | `DieHard.ctm` declares no `fairness` line (contrast `replicated_register.ctm:63`) |
//! | `completion_policy` | `DieHard.ctm:29` — `always(step(Next) \|\| stutter(state))`; `default.model.toml:4` — `deadlock = "allow"` |
//! | `nondeterminism[Next]` | `DieHard.ctm:27` — `action Next = FillSmall \| … \| BigToSmall`, an adversarial choice among six actions |
//! | `scope.components` | `DieHard.ctm:2` — `model DieHard {` |
//! | `scope.fragments = [Finite]` | `port.json` `features: ["finite", …]` and `models[0].mode: "exhaustive"` — the same reading `pr4_property_evidence.rs` already takes |
//! | `assurance.minimum = validated` | `README.md:10` — "closure/type certificate accepted by the independent Python checker", which is `continuum_value::assurance::AssuranceLevel::Validated`'s definition verbatim ("An independent certificate or translation checker passed") |
//! | `assurance.independent_checker` | `README.md:10` — same line |
//! | `assurance.clean_recompute` | `default.model.toml:7-9` — the run's state and edge counts are *pinned exact numbers*; a search that did not recompute cleanly could not pin them |
//! | `assurance.accepted_evidence_classes` | `default.model.toml:5` (`finite-exact`) and `README.md:10` (`certificate`) |
//! | `optimization.non_vacuity` | `README.md:9` — "shortest state with `big = 4` at depth 6": the safety failure is only meaningful because the witness is reachable |
//! | `security_policy.data_classification = public` | `port.json` `corpus.repository` — a public upstream repository |
//! | `intent_id` | **authored.** The corpus assigns no `in_*` handle; see [`the_declared_handle_can_be_the_one_the_contract_mints`] for the W9-clean variant |
//!
//! ## Replicated register — `notes/plan/examples/`
//!
//! | Contract field | Source |
//! |---|---|
//! | `claims[Agreement]` | `replicated_register.ctm:48-53` |
//! | `claims[StableWitness]` | `replicated_register.ctm:55-61` |
//! | `claims[RuntimeToAbstract]` | `replicated_register.scenario.toml:35` names the property; `replicated_register.md:129` states it — "Every stable quorum transition maps to zero or one `Choose`; ordinary polls, reservation, delivery, retries, and cancellation cleanup stutter". Transcribed as that sentence |
//! | `assumptions[QuorumIntersection]` | `replicated_register.ctm:7` — `const Quorum: Set[Set[Node]]`; the quorum property `ctm:55-61` relies on. Transcribed, classified `environment` |
//! | `assumptions[NetworkNoForgery]` | `replicated_register.scenario.toml:14-16` — loss, duplication, and reordering are all permitted, so the residual network guarantee is that nothing is delivered that was not sent. Transcribed; `fidelity_profile = adversarial-envelope` from `scenario.toml:12` (`network/adversarial-v0`) |
//! | `assumptions[DurableLogIsAPrefix]` | `replicated_register.scenario.toml:20-21` — `allow_volatile_suffix_loss`, `allow_torn_last_record`. Transcribed, classified `storage`; `fidelity_profile` **undeclared**, because the corpus states no fidelity level for `storage/append-log-v0` and absence is not the same as a declared value (RFC 0037) |
//! | `observers[client]` | `replicated_register.scenario.toml:30` — `crash_after = "client.reply:Committed"`; `replicated_register.ctm:10` — `chosen` is the abstract state the client's acknowledgement is about |
//! | `claims[RuntimeToAbstract].observer = client` | **transcription judgement.** `replicated_register.md:129`'s refinement is stated at the acknowledgement boundary (`md:91-106`, the causal core), which is the client's |
//! | `bounds.{values,nodes} = {2,3}` | `replicated_register.scenario.toml:7-8` — `[domains] nodes = 3`, `values = 2`; restated at `md:113-118` |
//! | `bounds.faults = 2` | `replicated_register.scenario.toml:24` — `max_crashes = 2`; restated at `md:118` as "crashes ≤ 2" |
//! | `bounds.depth = null` | no depth bound is stated anywhere in the example |
//! | `fault_model.enabled` | `crash`/`recovery` from `ctm:36,42`; `loss`/`duplication` from `scenario.toml:14-15`; `partition` from `scenario.toml:26`; `delay` from `scenario.toml:16` (`allow_reordering` — reordering is not a member of the plan §5.2 fault vocabulary and `delay` is the class it is a consequence of) |
//! | `fault_model.profiles` | `replicated_register.scenario.toml:12,19` |
//! | `fairness[weak:Recover]` | `replicated_register.ctm:63` — `fairness weak Recover`, unconditional |
//! | `completion_policy` | `replicated_register.scenario.toml:36` — `asupersync::Quiescence` is checked, so a quiet terminal state is an expected outcome rather than a deadlock violation |
//! | `nondeterminism` | `delivery_order` from `scenario.toml:16`; `quorum_selection` from `ctm:28` (`Choose(epoch, value, q: Set[Node])`); `crash_schedule` from `ctm:36` with `scenario.toml:24` |
//! | `scope.components` | `replicated_register.scenario.toml:34,36` — `abstract_register::…` and `asupersync::…` |
//! | `scope.fragments = [Finite, Temporal]` | `md:112` — "Exact finite exploration"; `ctm:63` and `scenario.toml:36` — a fairness declaration and a quiescence property are temporal |
//! | `trust_boundaries` | trusted: the two declared pack profiles (`scenario.toml:12,19`); opaque: the runtime scheduler, whose "ordinary polls, reservation, delivery, retries, and cancellation cleanup" are outside the abstraction (`md:129`) |
//! | `assurance` | `md:142` — "certificate checked by `continuum-kernel`" gives `Validated` and `independent_checker`; `md:133-138` — replay "across clean process invocation" gives `clean_recompute`; the accepted classes are `md:112` (`finite-exact`), `md:125` (`dpor-complete`), `md:129` (`refinement-proof`), `md:142` (`certificate`) |
//! | `optimization.soft` | `replicated_register.md:106` — "The minimal counterexample should not contain unrelated task polls, timer checks, or independent messages" |
//! | `optimization.non_vacuity` | `replicated_register.md:37-42` — `Agreement` holds vacuously if nothing is ever chosen, so a behavior that chooses is the non-vacuity obligation (INV-012) |
//! | `security_policy.redaction_classes` | `replicated_register.md:133` — counterexamples reproduce "from its crashpack" |
//! | `policy.assumptions = review` + `policy_reviewers` | **authored.** The example states no change policy; a `review` verb with its reviewers named is included deliberately, because the dossier's own validated example is the *unnamed* case (W7) and this fixture is the named one |
//!
//! No test here shells out, reads a clock, or touches a network. `include_str!` is
//! compile-time, so every coupling to the dossier is a build-time fact.

use std::collections::BTreeSet;

use continuum_value::assurance::{AssuranceLevel, EvidenceClass};
use continuum_value::identity::Fnv1aPlaceholder;

use continuum_intent::assumptions::{
    Assumption, AssumptionClassification, AssumptionSet, FidelityProfile,
    UnitKey as AssumptionUnitKey,
};
use continuum_intent::assurance_policy::AssurancePolicy;
use continuum_intent::ast::{
    ActionModality, Binder, ComparisonOperator, Formula, Fragment, Identifier, Literal, Term,
};
use continuum_intent::bounds::{Bounds, DeclaredBound, ExplorationBound};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyReviewers, PolicyTable, PolicyVerb};
use continuum_intent::contract::{
    AbstractionMaps, CheckEnvironment, ChoiceClass, ChoiceSite, CompletionPolicy,
    ContractDecodeError, ContractFinding, ContractParts, DataClassification, IntentContract,
    IntentId, MapId, MapRole, NondeterminismSet, Scope, SecurityPolicy, TrustBoundaries,
    WellFormednessRule,
};
use continuum_intent::fairness::{
    ActionName, FairnessConstraint, FairnessKey, FairnessKind, FairnessSet,
};
use continuum_intent::faults::{FaultClass, FaultModel, ProfileName};
use continuum_intent::observers::{Observer, ObserverId, ObserverSet, ProjectionKind};
use continuum_intent::optimization::{NonVacuityObligation, Objective, Optimization};
use continuum_intent::property::{Claim, ClaimKind, ClaimSet, PropertyExpression, UnitKey};

/// The dossier's validated Intent Contract example, included at compile time.
///
/// `notes/plan/tools/validate_dossier.py` validates this exact file against
/// `schemas/intent-contract.schema.json` on every `just check`.
const SCHEMA_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/intent-contract.example.json");

/// The golden artifact form of the Die Hard intent.
const DIE_HARD_FIXTURE: &str = include_str!("fixtures/die-hard-contract.json");

/// The golden artifact form of the replicated-register intent.
const REPLICATED_REGISTER_FIXTURE: &str =
    include_str!("fixtures/replicated-register-contract.json");

/// The crate's sources, for the type-level immutability statement.
///
/// Every module in `src/`. `include_str!` needs literal paths, so the list is written
/// out; a new module that is not added here is a module this check does not see, which
/// is why [`no_type_in_this_crate_has_a_mut_self_method`] also asserts the count.
const CRATE_SOURCES: [(&str, &str); 16] = [
    ("assumptions.rs", include_str!("../src/assumptions.rs")),
    (
        "assurance_policy.rs",
        include_str!("../src/assurance_policy.rs"),
    ),
    ("ast.rs", include_str!("../src/ast.rs")),
    ("bounds.rs", include_str!("../src/bounds.rs")),
    (
        "canonical_json.rs",
        include_str!("../src/canonical_json.rs"),
    ),
    ("change_policy.rs", include_str!("../src/change_policy.rs")),
    ("codec.rs", include_str!("../src/codec.rs")),
    ("contract.rs", include_str!("../src/contract.rs")),
    ("cpnf.rs", include_str!("../src/cpnf.rs")),
    ("fairness.rs", include_str!("../src/fairness.rs")),
    ("faults.rs", include_str!("../src/faults.rs")),
    ("identity.rs", include_str!("../src/identity.rs")),
    ("lib.rs", include_str!("../src/lib.rs")),
    ("observers.rs", include_str!("../src/observers.rs")),
    ("optimization.rs", include_str!("../src/optimization.rs")),
    ("property.rs", include_str!("../src/property.rs")),
];

// --- AST helpers -------------------------------------------------------------------

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("a corpus identifier is well formed")
}

fn unit(id: &str) -> UnitKey {
    UnitKey::new(id).expect("a corpus unit key is non-empty")
}

/// `assumptions[].id`, which is a *different* type from `claims[].id`.
///
/// `property::UnitKey` and `assumptions::UnitKey` are distinct newtypes with the same
/// name and the same discipline. That is deliberate — an assumption's key cannot be
/// passed where a claim's is expected — and it is why this file needs two helpers.
fn assumption_unit(id: &str) -> AssumptionUnitKey {
    AssumptionUnitKey::new(id).expect("a corpus unit key is non-empty")
}

fn state(name: &str) -> Term {
    Term::State {
        name: ident(name),
        indices: Vec::new(),
    }
}

fn indexed(name: &str, indices: Vec<Term>) -> Term {
    Term::State {
        name: ident(name),
        indices,
    }
}

fn var(name: &str) -> Term {
    Term::Var { name: ident(name) }
}

fn constant(name: &str) -> Term {
    Term::Constant { name: ident(name) }
}

fn integer(value: i64) -> Term {
    Term::Literal {
        value: Literal::Integer(value),
    }
}

fn apply(operator: &str, args: Vec<Term>) -> Term {
    Term::apply(ident(operator), args).expect("at least one argument")
}

fn compare(op: ComparisonOperator, left: Term, right: Term) -> Formula {
    Formula::compare(op, left, right)
}

fn predicate(name: &str, args: Vec<Term>) -> Formula {
    Formula::predicate(ident(name), args)
}

fn occurs(name: &str) -> Formula {
    Formula::action(ident(name), ActionModality::Occurs)
}

fn forall(variable: &str, domain: Term, body: Formula) -> Formula {
    Formula::forall(Binder::new(ident(variable), domain), body)
}

fn exists(variable: &str, domain: Term, body: Formula) -> Formula {
    Formula::exists(Binder::new(ident(variable), domain), body)
}

fn expression(formula: &Formula, fragment: Fragment, source: &str) -> PropertyExpression {
    PropertyExpression::normalized(formula, Some(fragment), Some(source.to_owned()))
        .expect("a corpus expression normalizes")
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("canonical output is UTF-8")
}

// --- the Die Hard intent -----------------------------------------------------------

/// `invariant TypeOK { big in 0..5 && small in 0..3 }` (`DieHard.ctm:30`).
fn die_hard_type_ok() -> Formula {
    Formula::always(
        Formula::and(vec![
            compare(ComparisonOperator::Ge, state("big"), integer(0)),
            compare(ComparisonOperator::Le, state("big"), integer(5)),
            compare(ComparisonOperator::Ge, state("small"), integer(0)),
            compare(ComparisonOperator::Le, state("small"), integer(3)),
        ])
        .expect("four conjuncts"),
    )
}

/// `invariant NotSolved { big != 4 }` (`DieHard.ctm:31`).
fn die_hard_not_solved() -> Formula {
    Formula::always(compare(ComparisonOperator::Ne, state("big"), integer(4)))
}

/// `Continuum.Examples.DieHard.solution_ends_with_four_gallons` (`port.json`).
fn die_hard_solution() -> Formula {
    Formula::eventually(compare(ComparisonOperator::Eq, state("big"), integer(4)))
}

/// The `where` clauses of `DieHard.ctm:4-5`, as the assumption they are.
fn die_hard_capacities() -> Formula {
    Formula::always(
        Formula::and(vec![
            compare(ComparisonOperator::Le, state("big"), integer(5)),
            compare(ComparisonOperator::Le, state("small"), integer(3)),
        ])
        .expect("two conjuncts"),
    )
}

fn die_hard_parts() -> ContractParts {
    ContractParts {
        intent_id: IntentId::new("in_die_hard_v1").expect("handle"),
        name: Some("Die Hard — TV-009".to_owned()),
        claims: ClaimSet::from_claims([
            Claim::new(
                unit("NotSolved"),
                ClaimKind::Safety,
                expression(&die_hard_not_solved(), Fragment::Finite, "big != 4"),
                None,
            ),
            Claim::new(
                unit("SolutionReachesFourGallons"),
                ClaimKind::Liveness,
                expression(
                    &die_hard_solution(),
                    Fragment::Finite,
                    "eventually (big == 4)",
                ),
                None,
            ),
            Claim::new(
                unit("TypeOK"),
                ClaimKind::Safety,
                expression(
                    &die_hard_type_ok(),
                    Fragment::Finite,
                    "big in 0..5 && small in 0..3",
                ),
                None,
            ),
        ])
        .expect("three distinct unit keys"),
        assumptions: AssumptionSet::from_assumptions([Assumption::new(
            assumption_unit("JugCapacities"),
            expression(
                &die_hard_capacities(),
                Fragment::Finite,
                "big: Nat where big <= 5; small: Nat where small <= 3",
            ),
            Some(AssumptionClassification::Environment),
            None,
        )])
        .expect("one assumption"),
        observers: ObserverSet::from_observers([]).expect("the corpus declares no observer"),
        abstraction_maps: AbstractionMaps::empty(),
        scope: Scope::new(["DieHard".to_owned()], [Fragment::Finite], None).expect("scope"),
        trust_boundaries: TrustBoundaries::new([], []).expect("a closed model"),
        bounds: Bounds::new(
            ExplorationBound::Bounded(6),
            DeclaredBound::Declared(1),
            DeclaredBound::Declared(0),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        fault_model: FaultModel::new([], []).expect("no fault action"),
        fairness: FairnessSet::from_constraints([]).expect("no fairness line"),
        completion_policy: CompletionPolicy::StutterForever,
        nondeterminism: NondeterminismSet::new([(
            ChoiceSite::new("Next").expect("site"),
            ChoiceClass::Demonic,
        )])
        .expect("one site"),
        assurance: AssurancePolicy::new(
            AssuranceLevel::Validated,
            Some(true),
            Some(true),
            [EvidenceClass::FiniteExact, EvidenceClass::Certificate],
        )
        .expect("assurance"),
        optimization: Optimization::new(
            [],
            [],
            [NonVacuityObligation::new(
                "a reachable state with big == 4 witnesses the NotSolved refutation",
            )
            .expect("obligation")],
        )
        .expect("optimization"),
        security_policy: SecurityPolicy::new(
            DataClassification::Public,
            [],
            ["revise-intent".to_owned()],
        )
        .expect("security policy"),
        policy: PolicyTable::new([
            (PolicyField::AbstractionMaps, PolicyVerb::Unlocked),
            (PolicyField::Assumptions, PolicyVerb::Locked),
            (PolicyField::Assurance, PolicyVerb::NoDowngrade),
            (PolicyField::Bounds, PolicyVerb::NoDecrease),
            (PolicyField::CompletionPolicy, PolicyVerb::Locked),
            (PolicyField::Fairness, PolicyVerb::Unlocked),
            (PolicyField::Faults, PolicyVerb::NoRemoval),
            (PolicyField::NonVacuity, PolicyVerb::NoRemoval),
            (PolicyField::Nondeterminism, PolicyVerb::Unlocked),
            (PolicyField::Observers, PolicyVerb::Unlocked),
            (PolicyField::Optimization, PolicyVerb::NoRemoval),
            (PolicyField::Properties, PolicyVerb::Locked),
            (PolicyField::Scope, PolicyVerb::Unlocked),
            (PolicyField::SecurityPolicy, PolicyVerb::NoRemoval),
            (PolicyField::TrustBoundaries, PolicyVerb::NoExpansion),
        ])
        .expect("fifteen admissible verbs"),
        policy_reviewers: PolicyReviewers::empty(),
    }
}

fn die_hard() -> IntentContract {
    IntentContract::new(die_hard_parts())
}

/// The Die Hard intent is checked against exactly what the corpus provides: no domain
/// packs (it declares no profile) and no correspondence maps (it declares none).
fn die_hard_environment(contract: &IntentContract) -> CheckEnvironment {
    CheckEnvironment::new().with_minted_intent_id(contract.mint_intent_id::<Fnv1aPlaceholder>())
}

// --- the replicated-register intent ------------------------------------------------

/// `chosen.get(e)`.
fn chosen_at(epoch: Term) -> Term {
    apply("get", vec![state("chosen"), epoch])
}

/// `stable[n].get(e)`.
fn stable_at(node: Term, epoch: Term) -> Term {
    apply("get", vec![indexed("stable", vec![node]), epoch])
}

/// `Some(v)`.
fn some(value: Term) -> Term {
    apply("Some", vec![value])
}

/// `invariant Agreement` (`replicated_register.ctm:48-53`).
fn register_agreement() -> Formula {
    forall(
        "epoch",
        constant("Epochs"),
        forall(
            "v1",
            constant("Values"),
            forall(
                "v2",
                constant("Values"),
                Formula::implies(
                    Formula::and(vec![
                        compare(
                            ComparisonOperator::Eq,
                            chosen_at(var("epoch")),
                            some(var("v1")),
                        ),
                        compare(
                            ComparisonOperator::Eq,
                            chosen_at(var("epoch")),
                            some(var("v2")),
                        ),
                    ])
                    .expect("two conjuncts"),
                    compare(ComparisonOperator::Eq, var("v1"), var("v2")),
                ),
            ),
        ),
    )
}

/// `invariant StableWitness` (`replicated_register.ctm:55-61`).
fn register_stable_witness() -> Formula {
    forall(
        "epoch",
        constant("Epochs"),
        forall(
            "value",
            constant("Values"),
            Formula::implies(
                compare(
                    ComparisonOperator::Eq,
                    chosen_at(var("epoch")),
                    some(var("value")),
                ),
                exists(
                    "q",
                    constant("Quorum"),
                    forall(
                        "n",
                        var("q"),
                        compare(
                            ComparisonOperator::Eq,
                            stable_at(var("n"), var("epoch")),
                            some(var("value")),
                        ),
                    ),
                ),
            ),
        ),
    )
}

/// `RuntimeToAbstract` (`replicated_register.scenario.toml:35`,
/// `replicated_register.md:129`).
fn register_runtime_to_abstract() -> Formula {
    Formula::always(Formula::implies(
        occurs("StableQuorumTransition"),
        Formula::or(vec![occurs("Choose"), predicate("stutters", Vec::new())])
            .expect("two disjuncts"),
    ))
}

/// Any two quorums intersect (`replicated_register.ctm:7`).
fn register_quorum_intersection() -> Formula {
    forall(
        "q1",
        constant("Quorum"),
        forall(
            "q2",
            constant("Quorum"),
            exists(
                "n",
                constant("Nodes"),
                Formula::and(vec![
                    compare(ComparisonOperator::Member, var("n"), var("q1")),
                    compare(ComparisonOperator::Member, var("n"), var("q2")),
                ])
                .expect("two conjuncts"),
            ),
        ),
    )
}

/// The residual network guarantee (`replicated_register.scenario.toml:14-16`).
fn register_network_no_forgery() -> Formula {
    Formula::always(Formula::implies(
        occurs("Deliver"),
        predicate("was_sent", Vec::new()),
    ))
}

/// The residual storage guarantee (`replicated_register.scenario.toml:20-21`).
fn register_durable_prefix() -> Formula {
    Formula::always(Formula::implies(
        occurs("Recover"),
        predicate("durable_log_is_a_prefix_of_appended", Vec::new()),
    ))
}

fn register_parts() -> ContractParts {
    ContractParts {
        intent_id: IntentId::new("in_replicated_register_v1").expect("handle"),
        name: Some("Cancel-Correct Replicated Register".to_owned()),
        claims: ClaimSet::from_claims([
            Claim::new(
                unit("Agreement"),
                ClaimKind::Safety,
                expression(
                    &register_agreement(),
                    Fragment::Finite,
                    "forall epoch, v1, v2: chosen.get(epoch) == Some(v1) && chosen.get(epoch) == \
                     Some(v2) => v1 == v2",
                ),
                None,
            ),
            Claim::new(
                unit("RuntimeToAbstract"),
                ClaimKind::Refinement,
                expression(
                    &register_runtime_to_abstract(),
                    Fragment::Finite,
                    "every stable quorum transition maps to zero or one Choose; ordinary polls, \
                     reservation, delivery, retries, and cancellation cleanup stutter",
                ),
                Some("client".to_owned()),
            ),
            Claim::new(
                unit("StableWitness"),
                ClaimKind::Safety,
                expression(
                    &register_stable_witness(),
                    Fragment::Finite,
                    "forall epoch, value: chosen.get(epoch) == Some(value) => exists q in Quorum: \
                     forall n in q: stable[n].get(epoch) == Some(value)",
                ),
                None,
            ),
        ])
        .expect("three distinct unit keys"),
        assumptions: AssumptionSet::from_assumptions([
            Assumption::new(
                assumption_unit("DurableLogIsAPrefix"),
                expression(
                    &register_durable_prefix(),
                    Fragment::Finite,
                    "allow_volatile_suffix_loss, allow_torn_last_record",
                ),
                Some(AssumptionClassification::Storage),
                // Undeclared on purpose: the corpus states no fidelity level for
                // `storage/append-log-v0`, and absence is not a declared value.
                None,
            ),
            Assumption::new(
                assumption_unit("NetworkNoForgery"),
                expression(
                    &register_network_no_forgery(),
                    Fragment::Finite,
                    "allow_loss, allow_duplication, allow_reordering",
                ),
                Some(AssumptionClassification::Network),
                Some(FidelityProfile::AdversarialEnvelope),
            ),
            Assumption::new(
                assumption_unit("QuorumIntersection"),
                expression(
                    &register_quorum_intersection(),
                    Fragment::Finite,
                    "forall q1, q2 in Quorum: exists n in Nodes: n in q1 && n in q2",
                ),
                Some(AssumptionClassification::Environment),
                None,
            ),
        ])
        .expect("three distinct unit keys"),
        observers: ObserverSet::from_observers([Observer::new(
            ObserverId::new("client").expect("id"),
            [
                (ProjectionKind::Events, vec!["Committed".to_owned()]),
                (ProjectionKind::State, vec!["chosen".to_owned()]),
            ],
        )
        .expect("observer")])
        .expect("one observer"),
        abstraction_maps: AbstractionMaps::new([(
            MapRole::new("runtime to abstract register").expect("role"),
            MapId::new("map_runtime_to_abstract_v1").expect("map id"),
        )])
        .expect("one binding"),
        scope: Scope::new(
            ["abstract_register".to_owned(), "asupersync".to_owned()],
            [Fragment::Finite, Fragment::Temporal],
            Some("protocol".to_owned()),
        )
        .expect("scope"),
        trust_boundaries: TrustBoundaries::new(
            [
                "network/adversarial-v0".to_owned(),
                "storage/append-log-v0".to_owned(),
            ],
            ["asupersync-scheduler".to_owned()],
        )
        .expect("boundaries"),
        bounds: Bounds::new(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(2),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        fault_model: FaultModel::new(
            [
                FaultClass::Crash,
                FaultClass::Recovery,
                FaultClass::Partition,
                FaultClass::Loss,
                FaultClass::Duplication,
                FaultClass::Delay,
            ],
            [
                ProfileName::new("network/adversarial-v0").expect("profile"),
                ProfileName::new("storage/append-log-v0").expect("profile"),
            ],
        )
        .expect("fault model"),
        fairness: FairnessSet::from_constraints([FairnessConstraint::new(
            FairnessKey::new(
                FairnessKind::Weak,
                ActionName::new("Recover").expect("action"),
            ),
            None,
        )
        .expect("unconditional weak fairness")])
        .expect("one constraint"),
        completion_policy: CompletionPolicy::StutterForever,
        nondeterminism: NondeterminismSet::new([
            (
                ChoiceSite::new("crash_schedule").expect("site"),
                ChoiceClass::Demonic,
            ),
            (
                ChoiceSite::new("delivery_order").expect("site"),
                ChoiceClass::Scheduler,
            ),
            (
                ChoiceSite::new("quorum_selection").expect("site"),
                ChoiceClass::Demonic,
            ),
        ])
        .expect("three sites"),
        assurance: AssurancePolicy::new(
            AssuranceLevel::Validated,
            Some(true),
            Some(true),
            [
                EvidenceClass::FiniteExact,
                EvidenceClass::DporComplete,
                EvidenceClass::RefinementProof,
                EvidenceClass::Certificate,
            ],
        )
        .expect("assurance"),
        optimization: Optimization::new(
            [],
            [Objective::new(
                "the minimal counterexample contains no unrelated task polls, timer checks, or \
                 independent messages",
            )
            .expect("objective")],
            [NonVacuityObligation::new(
                "some behavior chooses a value, so Agreement does not hold vacuously",
            )
            .expect("obligation")],
        )
        .expect("optimization"),
        security_policy: SecurityPolicy::new(
            DataClassification::Internal,
            ["crashpack".to_owned()],
            ["revise-intent".to_owned()],
        )
        .expect("security policy"),
        policy: PolicyTable::new([
            (PolicyField::AbstractionMaps, PolicyVerb::Review),
            (PolicyField::Assumptions, PolicyVerb::Review),
            (PolicyField::Assurance, PolicyVerb::NoDowngrade),
            (PolicyField::Bounds, PolicyVerb::NoDecrease),
            (PolicyField::CompletionPolicy, PolicyVerb::Locked),
            (PolicyField::Fairness, PolicyVerb::Review),
            (PolicyField::Faults, PolicyVerb::NoRemoval),
            (PolicyField::NonVacuity, PolicyVerb::NoRemoval),
            (PolicyField::Nondeterminism, PolicyVerb::Unlocked),
            (PolicyField::Observers, PolicyVerb::Review),
            (PolicyField::Optimization, PolicyVerb::NoRemoval),
            (PolicyField::Properties, PolicyVerb::Locked),
            (PolicyField::Scope, PolicyVerb::Unlocked),
            (PolicyField::SecurityPolicy, PolicyVerb::NoRemoval),
            (PolicyField::TrustBoundaries, PolicyVerb::NoExpansion),
        ])
        .expect("fifteen admissible verbs"),
        policy_reviewers: PolicyReviewers::new([
            (
                PolicyField::AbstractionMaps,
                BTreeSet::from(["verification-lead".to_owned()]),
            ),
            (
                PolicyField::Assumptions,
                BTreeSet::from(["verification-lead".to_owned()]),
            ),
            (
                PolicyField::Fairness,
                BTreeSet::from(["verification-lead".to_owned()]),
            ),
            (
                PolicyField::Observers,
                BTreeSet::from(["verification-lead".to_owned()]),
            ),
        ])
        .expect("four reviewed fields"),
    }
}

fn replicated_register() -> IntentContract {
    IntentContract::new(register_parts())
}

/// The replicated register's environment: the two pack profiles the scenario declares,
/// the one correspondence map the contract binds, and the handle the lane minted.
fn register_environment(contract: &IntentContract) -> CheckEnvironment {
    CheckEnvironment::new()
        .with_domain_pack_profiles([
            "network/adversarial-v0".to_owned(),
            "storage/append-log-v0".to_owned(),
        ])
        .with_correspondence_maps(["map_runtime_to_abstract_v1".to_owned()])
        .with_minted_intent_id(contract.mint_intent_id::<Fnv1aPlaceholder>())
}

/// Re-spell a canonical document as a *non*-canonical one that means the same thing:
/// every object's keys reversed, every set-valued array's members reversed, and two
/// spaces of insignificant whitespace after each separator.
///
/// This is a reformat, and ID7 requires a reformat to leave the identity alone. The
/// output is linear in the input — it re-renders the same values, never duplicating a
/// subtree.
///
/// Arrays *inside* a property AST are left in their authored order. RFC 0037 names the
/// three keys that carry an AST — `claims[].expression`, `assumptions[].expression`,
/// and `fairness[].condition` — and beneath them array order is not a spelling: N4
/// orders a junction's operands, but CPNF-1 "never reorders `args`", "no algebraic law
/// is assumed for an operator whose laws are not declared". Reversing them would be a
/// different formula, not a different spelling of one, so the reformat stops at those
/// keys. Object keys are still reversed all the way down, since ID5 fixes key order
/// everywhere.
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

// --- PR-4-EXIT, clause 1: Die Hard serializes canonically ---------------------------

#[test]
fn die_hards_intent_serializes_to_its_golden_bytes() {
    let encoded = utf8(&die_hard().to_artifact_bytes());
    assert_eq!(
        encoded,
        DIE_HARD_FIXTURE.trim_end(),
        "the Die Hard intent no longer serializes to its checked-in golden form; if the change \
         is intended, RFC 0037 ID5 makes it an encoding change and therefore an identity change \
         for every stored contract"
    );
}

#[test]
fn the_die_hard_golden_bytes_decode_back_to_the_same_contract() {
    let decoded = IntentContract::decode(DIE_HARD_FIXTURE.trim_end().as_bytes()).expect("decodes");
    let built = die_hard();
    assert_eq!(decoded, built);
    assert_eq!(decoded.identity(), built.identity());
    assert_eq!(decoded.to_artifact_bytes(), built.to_artifact_bytes());
    // Every group survived the trip, not just the ones a shallow equality would notice.
    assert_eq!(decoded.claims().len(), 3);
    assert_eq!(decoded.assumptions().len(), 1);
    assert_eq!(decoded.scope().declared_fragments().len(), 1);
    assert_eq!(decoded.nondeterminism().len(), 1);
    assert_eq!(
        decoded.completion_policy(),
        CompletionPolicy::StutterForever
    );
}

#[test]
fn a_reordered_and_reformatted_die_hard_contract_recanonicalizes() {
    let contract = die_hard();
    let spelled_differently = reformat(&contract.artifact_json());
    assert_ne!(
        spelled_differently.as_bytes(),
        contract.to_artifact_bytes().as_slice()
    );
    let decoded = IntentContract::decode(spelled_differently.as_bytes())
        .expect("a reformat is still the same document");
    assert_eq!(decoded.to_artifact_bytes(), contract.to_artifact_bytes());
    assert_eq!(decoded.identity(), contract.identity());
}

#[test]
fn the_die_hard_intent_is_well_formed_except_for_its_authored_handle() {
    let contract = die_hard();
    let verdict = contract.check(&die_hard_environment(&contract));
    // The corpus assigns no `in_*` handle, so the fixture authors one; W9 is the only
    // rule it fails, and it names both sides rather than saying "invalid".
    assert_eq!(verdict.findings().len(), 1, "{verdict}");
    assert!(matches!(
        verdict.findings_for(WellFormednessRule::W9).next(),
        Some(ContractFinding::DeclaredIdentityMismatch { declared, .. })
            if declared == "in_die_hard_v1"
    ));
}

#[test]
fn the_declared_handle_can_be_the_one_the_contract_mints() {
    // ID2 puts `intent_id` outside its own preimage, so a contract's handle can be
    // minted from the contract and then declared on it without moving the identity.
    let draft = die_hard();
    let minted = draft.mint_intent_id::<Fnv1aPlaceholder>();
    let mut parts = die_hard_parts();
    parts.intent_id = IntentId::new(&minted).expect("a minted handle matches the §4.4 pattern");
    let contract = IntentContract::new(parts);
    assert_eq!(contract.identity(), draft.identity());

    let verdict = contract.check(&die_hard_environment(&contract));
    assert!(verdict.is_well_formed(), "{verdict}");
}

// --- PR-4-EXIT, clause 2: the replicated register serializes canonically ------------

#[test]
fn the_replicated_register_intent_serializes_to_its_golden_bytes() {
    let encoded = utf8(&replicated_register().to_artifact_bytes());
    assert_eq!(
        encoded,
        REPLICATED_REGISTER_FIXTURE.trim_end(),
        "the replicated-register intent no longer serializes to its checked-in golden form"
    );
}

#[test]
fn the_replicated_register_golden_bytes_decode_back_to_the_same_contract() {
    let decoded =
        IntentContract::decode(REPLICATED_REGISTER_FIXTURE.trim_end().as_bytes()).expect("decodes");
    let built = replicated_register();
    assert_eq!(decoded, built);
    assert_eq!(decoded.identity(), built.identity());
    assert_eq!(decoded.to_artifact_bytes(), built.to_artifact_bytes());
    assert_eq!(decoded.claims().len(), 3);
    assert_eq!(decoded.assumptions().len(), 3);
    assert_eq!(decoded.observers().len(), 1);
    assert_eq!(decoded.fairness().len(), 1);
    assert_eq!(decoded.nondeterminism().len(), 3);
    assert_eq!(decoded.abstraction_maps().len(), 1);
    assert_eq!(decoded.fault_model().enabled().len(), 6);
}

#[test]
fn a_reordered_and_reformatted_replicated_register_contract_recanonicalizes() {
    let contract = replicated_register();
    let spelled_differently = reformat(&contract.artifact_json());
    assert_ne!(
        spelled_differently.as_bytes(),
        contract.to_artifact_bytes().as_slice()
    );
    let decoded = IntentContract::decode(spelled_differently.as_bytes())
        .expect("a reformat is still the same document");
    assert_eq!(decoded.to_artifact_bytes(), contract.to_artifact_bytes());
    assert_eq!(decoded.identity(), contract.identity());
}

#[test]
fn the_replicated_register_intent_is_well_formed_except_for_its_authored_handle() {
    let contract = replicated_register();
    let verdict = contract.check(&register_environment(&contract));
    assert_eq!(verdict.findings().len(), 1, "{verdict}");
    assert_eq!(verdict.findings()[0].rule(), WellFormednessRule::W9);
    // W2, W3, W7, and W8 all hold: a claim binds a declared observer, both fragments
    // are declared and every expression names one, every `review` verb names its
    // reviewers, and both pack profiles and the one abstraction map resolve.
    assert_eq!(verdict.findings_for(WellFormednessRule::W2).count(), 0);
    assert_eq!(verdict.findings_for(WellFormednessRule::W3).count(), 0);
    assert_eq!(verdict.findings_for(WellFormednessRule::W7).count(), 0);
    assert_eq!(verdict.findings_for(WellFormednessRule::W8).count(), 0);
}

#[test]
fn the_fault_classes_encode_in_wire_order_not_declaration_order() {
    // All six classes are enabled, so this is the whole ordering rule at once. The
    // schema declares them crash, recovery, partition, loss, duplication, delay; the
    // canonical spelling is code-point order on the wire token.
    let encoded = utf8(&replicated_register().to_artifact_bytes());
    assert!(
        encoded
            .contains(r#""enabled":["crash","delay","duplication","loss","partition","recovery"]"#),
        "{encoded}"
    );
}

#[test]
fn the_two_declared_fragments_encode_in_code_point_order() {
    // Declaration order is Finite, Symbolic, Temporal, …; code-point order puts
    // Finite before Temporal too, so this fixture cannot tell them apart — the
    // in-module test `scope_fragments_encode_in_code_point_order_not_declaration_order`
    // uses a triple that can. What this asserts is that the fixture's own spelling is
    // the canonical one.
    let encoded = utf8(&replicated_register().to_artifact_bytes());
    assert!(
        encoded.contains(r#""fragments":["Finite","Temporal"]"#),
        "{encoded}"
    );
}

// --- PR-4-EXIT, clause 3: ordinary operations cannot mutate them --------------------

#[test]
fn every_public_operation_leaves_the_contract_bit_identical() {
    let contract = replicated_register();
    let bytes_before = contract.to_artifact_bytes();
    let identity_before = contract.identity().clone();

    // Every public operation the crate offers on a contract, exercised in one pass.
    // The point is not that each one happens to be harmless; it is that this is the
    // *whole* list, and none of it takes `&mut self`.
    let _ = contract.intent_id();
    let _ = contract.name();
    let _ = contract.claims();
    let _ = contract.assumptions();
    let _ = contract.observers();
    let _ = contract.abstraction_maps();
    let _ = contract.scope();
    let _ = contract.trust_boundaries();
    let _ = contract.bounds();
    let _ = contract.fault_model();
    let _ = contract.fairness();
    let _ = contract.completion_policy();
    let _ = contract.nondeterminism();
    let _ = contract.assurance();
    let _ = contract.optimization();
    let _ = contract.security_policy();
    let _ = contract.policy();
    let _ = contract.policy_reviewers();
    let _ = contract.identity();
    let _ = contract.identity_preimage_json();
    let _ = contract.identity_preimage_bytes();
    let _ = contract.artifact_json();
    let _ = contract.to_artifact_bytes();
    let _ = contract.mint_intent_id::<Fnv1aPlaceholder>();
    let _ = contract.check(&register_environment(&contract));
    let _ = contract.clone();
    let _ = format!("{contract:?}");
    let _ = contract == replicated_register();

    // Derivations reachable through the lent groups: comparisons, joins, verdicts,
    // relations. Every one of them returns a new value.
    let _ = contract.bounds().relation_to(contract.bounds());
    let _ = contract.policy().join(contract.policy());
    let _ = contract.assurance().change_to(contract.assurance());
    let _ = contract
        .assurance()
        .evidence_class_changes(contract.assurance());
    let _ = contract.optimization().require_non_vacuity();
    let _ = contract.fault_model().units().count();
    let _ = contract.claims().observer_references();
    let _ = contract
        .claims()
        .check_observers(&contract.observers().declared_ids());
    let _ = contract
        .assumptions()
        .check_fragments(&contract.scope().declared_fragments());
    let _ = contract
        .policy()
        .check_reviewers(contract.policy_reviewers());

    // Round-tripping is a derivation too, not an edit.
    let _ = IntentContract::decode(&contract.to_artifact_bytes()).expect("decodes");

    assert_eq!(contract.to_artifact_bytes(), bytes_before);
    assert_eq!(contract.identity(), &identity_before);
    assert_eq!(
        contract.identity().canonical_bytes(),
        identity_before.canonical_bytes()
    );
}

#[test]
fn no_type_in_this_crate_has_a_mut_self_method() {
    // The type-level half of "ordinary operations cannot mutate them", checked against
    // the sources rather than asserted in prose. The single exception is the private
    // JSON parser's cursor, which advances over the *input bytes* while scanning them
    // and is not a contract-carrying type; every other `&mut self` in this crate would
    // be a mutator on an artifact and is what INV-001 forbids.
    //
    // Comment lines are skipped, because several module docs quote the rule they are
    // documenting — the check is about receivers, not about prose.
    for (name, source) in CRATE_SOURCES {
        let receivers = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| line.matches("&mut self").count() + line.matches("self: &mut").count())
            .sum::<usize>();
        if name == "canonical_json.rs" {
            assert!(
                receivers > 0,
                "the parser cursor's `&mut self` methods have moved or been renamed; re-derive \
                 this exception before trusting the rest of the check"
            );
        } else {
            assert_eq!(
                receivers, 0,
                "{name} declares a `&mut self` method; INV-001 is that ordinary operations \
                 cannot mutate intent, and a setter is the affordance by which they would"
            );
        }
    }
    // A module added to `src/` and not to this list is a module this check cannot see.
    let declared = include_str!("../src/lib.rs")
        .lines()
        .filter(|line| line.starts_with("pub mod ") || line.starts_with("pub(crate) mod "))
        .count();
    assert_eq!(
        declared + 1,
        CRATE_SOURCES.len(),
        "src/lib.rs declares {declared} modules; CRATE_SOURCES lists {} files (the modules plus \
         lib.rs itself)",
        CRATE_SOURCES.len()
    );
}

// --- boundary: ID7's checkable list -------------------------------------------------

#[test]
fn id7s_checkable_list_holds_for_both_corpus_intents() {
    for (label, parts) in [
        ("die hard", die_hard_parts()),
        ("replicated register", register_parts()),
    ] {
        let base = IntentContract::new(parts.clone());
        let identity = base.identity().clone();

        // --- ID7's "MUST NOT change it" half ---
        let mut renamed = parts.clone();
        renamed.name = Some("a different display name".to_owned());
        assert_eq!(
            IntentContract::new(renamed).identity(),
            &identity,
            "{label}: renaming moved the identity"
        );

        let mut rehandled = parts.clone();
        rehandled.intent_id = IntentId::new("in_some_other_handle").expect("handle");
        assert_eq!(
            IntentContract::new(rehandled).identity(),
            &identity,
            "{label}: re-handling moved the identity"
        );

        // Rewriting `source` — the display-only rendering — is out of the preimage.
        let mut resourced = parts.clone();
        resourced.claims = ClaimSet::from_claims(base.claims().iter().map(|claim| {
            Claim::new(
                claim.unit().clone(),
                claim.class(),
                PropertyExpression::normalized(
                    claim.expression().ast(),
                    claim.expression().fragment(),
                    Some("a completely different rendering".to_owned()),
                )
                .expect("already normal"),
                claim.observer().map(str::to_owned),
            )
        }))
        .expect("same unit keys");
        assert_eq!(
            IntentContract::new(resourced).identity(),
            &identity,
            "{label}: rewriting `source` moved the identity"
        );

        // --- ID7's "MUST change it" half, one field group at a time ---
        let mut moved: Vec<&'static str> = Vec::new();
        let mut assert_moves = |name: &'static str, changed: ContractParts| {
            assert_ne!(
                IntentContract::new(changed).identity(),
                &identity,
                "{label}: changing {name} did not move the identity"
            );
            moved.push(name);
        };

        let first_unit = base
            .claims()
            .iter()
            .next()
            .expect("claims carries at least one")
            .unit()
            .clone();
        let mut claims = parts.clone();
        claims.claims = ClaimSet::from_claims(
            base.claims()
                .iter()
                .filter(|claim| claim.unit() != &first_unit)
                .cloned(),
        )
        .expect("at least one claim remains");
        assert_moves("a claim", claims);

        let mut assumptions = parts.clone();
        assumptions.assumptions =
            AssumptionSet::from_assumptions(base.assumptions().iter().skip(1).cloned())
                .expect("assumptions may be empty");
        assert_moves("an assumption", assumptions);

        let mut observers = parts.clone();
        observers.observers = ObserverSet::from_observers(
            base.observers().iter().cloned().chain([Observer::new(
                ObserverId::new("auditor").expect("id"),
                [(ProjectionKind::Events, vec!["Audited".to_owned()])],
            )
            .expect("observer")]),
        )
        .expect("distinct ids");
        assert_moves("an observer", observers);

        let mut bounds = parts.clone();
        bounds.bounds = Bounds::new(
            ExplorationBound::Bounded(1),
            DeclaredBound::Declared(1),
            DeclaredBound::Declared(0),
            ExplorationBound::Unbounded,
        )
        .expect("bounds");
        assert_moves("a bound", bounds);

        let mut faults = parts.clone();
        faults.fault_model = FaultModel::new(
            base.fault_model()
                .enabled()
                .iter()
                .copied()
                .chain([FaultClass::Crash])
                .collect::<BTreeSet<_>>(),
            base.fault_model().profiles().iter().cloned(),
        )
        .expect("fault model");
        if faults.fault_model != *base.fault_model() {
            assert_moves("a fault class", faults);
        } else {
            // Every class is already enabled; remove one instead.
            let mut faults = parts.clone();
            faults.fault_model = FaultModel::new(
                base.fault_model()
                    .enabled()
                    .iter()
                    .copied()
                    .filter(|class| *class != FaultClass::Crash),
                base.fault_model().profiles().iter().cloned(),
            )
            .expect("fault model");
            assert_moves("a fault class", faults);
        }

        let mut fairness = parts.clone();
        fairness.fairness = FairnessSet::from_constraints(
            base.fairness()
                .iter()
                .cloned()
                .chain([FairnessConstraint::new(
                    FairnessKey::new(
                        FairnessKind::Strong,
                        ActionName::new("Choose").expect("action"),
                    ),
                    None,
                )
                .expect("constraint")]),
        )
        .expect("distinct keys");
        assert_moves("a fairness constraint", fairness);

        let mut completion = parts.clone();
        completion.completion_policy = CompletionPolicy::DeadlockViolation;
        assert_moves("the completion policy", completion);

        let mut nondeterminism = parts.clone();
        nondeterminism.nondeterminism = NondeterminismSet::new(
            base.nondeterminism()
                .iter()
                .map(|(site, _)| (site.clone(), ChoiceClass::Angelic)),
        )
        .expect("same sites");
        assert_moves("a nondeterminism class", nondeterminism);

        let mut assurance = parts.clone();
        assurance.assurance =
            AssurancePolicy::new(AssuranceLevel::Observed, Some(true), Some(true), [])
                .expect("assurance");
        assert_moves("the assurance requirement", assurance);

        let mut optimization = parts.clone();
        optimization.optimization = Optimization::new(
            [Objective::new("an added objective").expect("objective")],
            base.optimization().soft().cloned(),
            base.optimization().non_vacuity().cloned(),
        )
        .expect("optimization");
        assert_moves("an optimization string", optimization);

        let mut non_vacuity = parts.clone();
        non_vacuity.optimization = Optimization::new(
            base.optimization().hard().cloned(),
            base.optimization().soft().cloned(),
            [],
        )
        .expect("optimization");
        assert_moves("a non-vacuity string", non_vacuity);

        let mut security = parts.clone();
        security.security_policy = SecurityPolicy::new(
            DataClassification::Restricted,
            base.security_policy()
                .redaction_classes()
                .map(str::to_owned),
            base.security_policy()
                .capability_requirements()
                .map(str::to_owned),
        )
        .expect("security policy");
        assert_moves("a security-policy element", security);

        let mut scope = parts.clone();
        scope.scope = Scope::new(
            base.scope()
                .components()
                .map(str::to_owned)
                .chain(["an added component".to_owned()]),
            base.scope().fragments(),
            base.scope().abstraction_level().map(str::to_owned),
        )
        .expect("scope");
        assert_moves("a scope element", scope);

        let mut maps = parts.clone();
        maps.abstraction_maps = AbstractionMaps::new(
            base.abstraction_maps()
                .iter()
                .map(|(role, map_id)| (role.clone(), map_id.clone()))
                .chain([(
                    MapRole::new("an added role").expect("role"),
                    MapId::new("map_added_v1").expect("map id"),
                )]),
        )
        .expect("distinct roles");
        assert_moves("an abstraction-map binding", maps);

        let mut policy = parts.clone();
        policy.policy = PolicyTable::all_unlocked();
        assert_moves("a policy verb", policy);

        let mut reviewers = parts.clone();
        reviewers.policy_reviewers = PolicyReviewers::new(
            base.policy_reviewers()
                .iter()
                .map(|(field, principals)| (field, principals.clone()))
                .chain([(
                    PolicyField::Scope,
                    BTreeSet::from(["an-added-principal".to_owned()]),
                )]),
        )
        .expect("distinct fields");
        assert_moves("a reviewer list", reviewers);

        assert_eq!(
            moved.len(),
            16,
            "{label}: ID7 names sixteen things that must move the identity; {moved:?} moved"
        );
    }
}

// --- adversarial --------------------------------------------------------------------

#[test]
fn every_truncation_of_a_contract_is_a_typed_error() {
    let bytes = die_hard().to_artifact_bytes();
    // Linear in the fixture, kilobyte scale, and guarded: an input that grew past this
    // fails the guard rather than the machine.
    assert!(bytes.len() < 8192, "fixture grew to {} bytes", bytes.len());
    for cut in 0..bytes.len() {
        let error = IntentContract::decode(&bytes[..cut]);
        assert!(
            error.is_err(),
            "a prefix of {cut} bytes decoded as a whole contract"
        );
        // Rendering the error is part of the contract: a rejection nobody can read is
        // a rejection nobody can act on.
        let _ = error.unwrap_err().to_string();
    }
}

#[test]
fn an_unknown_top_level_key_is_refused() {
    let encoded = utf8(&die_hard().to_artifact_bytes());
    let injected = encoded.replacen('{', r#"{"aardvark":"extra","#, 1);
    assert!(injected.len() < 8192);
    assert_eq!(
        IntentContract::decode(injected.as_bytes()),
        Err(ContractDecodeError::UnknownField {
            field: "intent-contract",
            key: "aardvark".to_owned(),
        }),
        "additionalProperties: false is the schema's, and a reader that ignored it would \
         silently drop a protected field group"
    );
}

#[test]
fn a_duplicated_field_group_is_refused_rather_than_resolved_by_arrival_order() {
    let encoded = utf8(&die_hard().to_artifact_bytes());
    // One extra `claims` key carrying the empty array. Last-writer-wins would silently
    // discard every claim in the contract.
    let injected = encoded.replacen('{', r#"{"claims":[],"#, 1);
    assert!(injected.len() < 8192);
    let error = IntentContract::decode(injected.as_bytes()).expect_err("two claims keys");
    assert!(
        matches!(
            error,
            ContractDecodeError::Json(continuum_intent::canonical_json::JsonError::DuplicateKey {
                ref key,
                ..
            }) if key == "claims"
        ),
        "{error}"
    );
}

#[test]
fn each_cross_group_rule_produces_its_own_typed_verdict() {
    let base = register_parts();

    // W2 — retarget the observer binding at an id no observer declares.
    let mut w2 = base.clone();
    w2.observers = ObserverSet::from_observers([]).expect("empty");
    let verdict = IntentContract::new(w2).check(&CheckEnvironment::new());
    assert!(
        verdict
            .findings()
            .contains(&ContractFinding::DanglingObserver {
                unit: "RuntimeToAbstract".to_owned(),
                observer: "client".to_owned(),
            }),
        "{verdict}"
    );

    // W3 — withdraw the fragment every expression declares.
    let mut w3 = base.clone();
    w3.scope =
        Scope::new(["abstract_register".to_owned()], [Fragment::Symbolic], None).expect("scope");
    let verdict = IntentContract::new(w3).check(&CheckEnvironment::new());
    assert!(
        verdict
            .findings()
            .contains(&ContractFinding::UndeclaredFragment {
                group: "claims",
                unit: "Agreement".to_owned(),
                fragment: Fragment::Finite,
            }),
        "{verdict}"
    );
    // Both AST-carrying groups are checked, and the finding says which one it is.
    assert!(
        verdict.findings().iter().any(|finding| matches!(
            finding,
            ContractFinding::UndeclaredFragment {
                group: "assumptions",
                ..
            }
        )),
        "{verdict}"
    );

    // W7 — keep the four `review` verbs and drop the reviewers.
    let mut w7 = base.clone();
    w7.policy_reviewers = PolicyReviewers::empty();
    let verdict = IntentContract::new(w7).check(&CheckEnvironment::new());
    assert!(
        matches!(
            verdict.findings_for(WellFormednessRule::W7).next(),
            Some(ContractFinding::UnenforceableReview { .. })
        ),
        "{verdict}"
    );

    // W8 — both halves, against an environment that resolves nothing.
    let verdict = replicated_register().check(&CheckEnvironment::new());
    assert_eq!(verdict.findings_for(WellFormednessRule::W8).count(), 2);
    assert!(
        verdict
            .findings()
            .contains(&ContractFinding::UnresolvedAbstractionMap {
                role: "runtime to abstract register".to_owned(),
                map_id: "map_runtime_to_abstract_v1".to_owned(),
            }),
        "{verdict}"
    );

    // W9 — an unverified identity is not a verified one.
    assert!(
        verdict
            .findings()
            .contains(&ContractFinding::DeclaredIdentityUnverified),
        "{verdict}"
    );

    // Every finding renders.
    for finding in verdict.findings() {
        assert!(finding.to_string().starts_with(finding.rule().wire()));
    }
}

#[test]
fn the_preimage_contains_no_id2_excluded_key_at_any_depth() {
    // ID2's exclusion list is closed, so it is checkable: walk the preimage and fail
    // if any excluded key is reachable from it. This is the claim the module
    // documentation makes about the thirteen groups whose artifact and identity
    // spellings coincide, checked rather than asserted.
    for contract in [die_hard(), replicated_register()] {
        let preimage = contract.identity_preimage_json();

        // `name` and `intent_id` are excluded at the top level, and only there: `name`
        // is also the key an AST predicate, action, or term carries, and excluding
        // *those* would be a different rule about a different thing.
        let top = preimage.as_object().expect("the preimage is an object");
        assert!(!top.contains_key("name"));
        assert!(!top.contains_key("intent_id"));
        assert_eq!(top.len(), 18);

        // `expression.source`, `expression.normal_form`, and every `$comment` are
        // excluded at every depth.
        let mut offenders = Vec::new();
        collect_excluded_keys(&preimage, &mut offenders);
        assert!(
            offenders.is_empty(),
            "the identity preimage reaches ID2-excluded keys: {offenders:?}"
        );
        // And the preimage really is the identity.
        assert_eq!(
            preimage.to_canonical_bytes(),
            contract.identity().canonical_bytes()
        );
        assert_eq!(
            preimage.to_canonical_bytes(),
            contract.identity_preimage_bytes()
        );
        // The artifact form, by contrast, carries all of them but `$comment`.
        let artifact = utf8(&contract.to_artifact_bytes());
        assert!(artifact.contains(r#""normal_form":"cpnf-1""#));
        assert!(artifact.contains(r#""source":"#));
        assert!(artifact.contains(r#""intent_id":"#));
        assert!(artifact.contains(r#""name":"#));
        assert!(!artifact.contains("$comment"));
    }
}

fn collect_excluded_keys(json: &Json, offenders: &mut Vec<String>) {
    match json {
        Json::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "source" | "normal_form" | "$comment") {
                    offenders.push(key.clone());
                }
                collect_excluded_keys(value, offenders);
            }
        }
        Json::Array(items) => {
            for item in items {
                collect_excluded_keys(item, offenders);
            }
        }
        _ => {}
    }
}

// --- the schema cross-check ---------------------------------------------------------

/// `intent-contract.schema.json`, read with this crate's own reader.
const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

#[test]
fn the_closed_top_level_vocabulary_is_the_schemas_own() {
    // INV-003: the schema decides shape. A hand-transcribed key list is a second
    // spelling of the schema and drifts; this reads the schema itself, with the
    // reader this crate ships, and compares it to what a full contract encodes —
    // the same transcription-distrust discipline `assurance_policy_contract.rs`
    // applies to the assurance ladder.
    let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema is JSON");
    let schema = schema.as_object().expect("the schema is an object");

    assert_eq!(
        schema.get("additionalProperties").and_then(Json::as_bool),
        Some(false),
        "the top level is closed, which is what makes an unknown key a rejection"
    );

    let declared: BTreeSet<&str> = schema
        .get("properties")
        .and_then(Json::as_object)
        .expect("the schema declares properties")
        .keys()
        .map(String::as_str)
        .collect();

    // The Die Hard fixture declares a `name`, so its artifact form carries every one
    // of the schema's twenty top-level keys.
    let artifact = die_hard().artifact_json();
    let encoded: BTreeSet<&str> = artifact
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        encoded, declared,
        "the encoder's top-level vocabulary and the schema's have diverged"
    );
    assert_eq!(declared.len(), 20);

    // The eighteen required keys are exactly the twenty minus the two RFC 0037 calls
    // optional, and the encoder always writes them.
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Json::as_array)
        .expect("the schema declares required")
        .iter()
        .filter_map(Json::as_str)
        .collect();
    assert_eq!(required.len(), 18);
    assert!(!required.contains("name"));
    assert!(!required.contains("policy_reviewers"));
    assert!(required.is_subset(&declared));

    // A contract with no `name` still writes all eighteen.
    let mut anonymous = die_hard_parts();
    anonymous.name = None;
    let artifact = IntentContract::new(anonymous).artifact_json();
    let written: BTreeSet<&str> = artifact
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert!(required.is_subset(&written));
    assert_eq!(
        written.len(),
        19,
        "eighteen required keys plus policy_reviewers"
    );
}

#[test]
fn the_schemas_own_validated_example_decodes_as_a_whole_contract() {
    // `notes/plan/tools/validate_dossier.py` has already agreed this document is legal
    // under `intent-contract.schema.json`. This is where the types find out (INV-003).
    let contract = IntentContract::decode(SCHEMA_EXAMPLE.as_bytes()).expect(
        "this crate's types must admit exactly what the schema's validated example carries",
    );
    assert_eq!(contract.intent_id().as_str(), "in_ack_v1");
    assert_eq!(contract.name(), Some("durable acknowledgements"));
    assert_eq!(contract.claims().len(), 1);
    assert_eq!(contract.assumptions().len(), 1);
    assert_eq!(contract.observers().len(), 1);
    assert_eq!(contract.abstraction_maps().len(), 1);

    // Round-trip through this crate's canonical form and back.
    let round_tripped = IntentContract::decode(&contract.to_artifact_bytes()).expect("re-decodes");
    assert_eq!(round_tripped, contract);
    assert_eq!(round_tripped.identity(), contract.identity());
    assert_eq!(
        round_tripped.to_artifact_bytes(),
        contract.to_artifact_bytes()
    );
}

#[test]
fn the_schemas_example_is_schema_valid_and_well_formed() {
    // The adjudication `bn-fp0g` raised and `bn-16pyr` resolved: nine of the
    // example's fifteen fields carry the `review` verb, and — since bn-16pyr fixed
    // the example — `policy_reviewers` names a principal for each of them. The
    // decode/check split this test used to demonstrate through the violation is
    // still demonstrated: decode accepts the artifact on schema-shape grounds
    // alone, and the checker's verdict is a separate surface (here, clean).
    let contract = IntentContract::decode(SCHEMA_EXAMPLE.as_bytes()).expect("decodes");
    assert_eq!(
        contract.policy_reviewers().len(),
        9,
        "the example names a reviewer set for each of its nine review-verb fields \
         (bn-16pyr); this assertion and `change_policy_contract.rs`'s pinning test \
         move together"
    );
    assert_eq!(
        contract
            .policy()
            .iter()
            .filter(|(_, verb)| *verb == PolicyVerb::Review)
            .count(),
        9,
        "nine of the example's fifteen fields carry the review verb"
    );
    let verdict = contract.check(
        &CheckEnvironment::new()
            .with_domain_pack_profiles(["storage-posix-v1".to_owned()])
            .with_correspondence_maps(["map_register_abs_v1".to_owned()])
            .with_minted_intent_id("in_ack_v1"),
    );
    // W2, W3, W7, W8, and W9 all hold on the dossier's own example.
    assert!(verdict.is_well_formed(), "{verdict}");
    assert_eq!(verdict.findings().len(), 0, "{verdict}");
}
