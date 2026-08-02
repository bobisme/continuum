//! The nine dimensions, held to their normative sources, and the charge/exhaust behaviour
//! of each one in both the metered and the unmetered arm.
//!
//! # Why three sources
//!
//! SD-12 is "one budget/cost dimension list shared by RFC 0026, `verification-task`, and
//! the §8.6 cost ledger" (`notes/plan/plan.md` §25). A Rust enum that merely *claims* to
//! spell that list is the drift SD-12 exists to prevent, so this file compares
//! [`CostDimension::ALL`] against three independently maintained artifacts:
//!
//! | source | form | what it pins |
//! |---|---|---|
//! | `schemas/continuumd-native-protocol.idl` | `struct Budget` declarations | the members **and their order** |
//! | `rfcs/0026-continuumd-native-protocol.md` | the prose enumeration in "Cost and omissions" | the members and their order, written by hand |
//! | `schemas/verification-task.schema.json` | the `budget` object's `properties` | the members as a *set*, plus `additionalProperties: false` |
//!
//! The three disagree in shape on purpose — a declaration list, a sentence, and an
//! alphabetically-sorted JSON object — so a copy-paste error that survives one is very
//! unlikely to survive all three. This is docs/03 §8's independent-paths rule applied to a
//! vocabulary rather than to a checker, and it is the same device
//! `crates/continuumd/tests/idl_conformance.rs` uses for the protocol registry.
//!
//! `the_check_is_not_vacuous` mutates each source and requires the comparison to fail, so a
//! parser that quietly found nothing cannot pass.

use continuum_task::budget::dimension::{CostDimension, MeterSet, Metering};
use continuum_task::budget::{Budget, BudgetFault, BudgetLedger, Exhaustion};

const IDL: &str = include_str!("../../../notes/plan/schemas/continuumd-native-protocol.idl");
const RFC_0026: &str = include_str!("../../../notes/plan/rfcs/0026-continuumd-native-protocol.md");
const VERIFICATION_TASK: &str =
    include_str!("../../../notes/plan/schemas/verification-task.schema.json");

// =====================================================================================
// The three readers
// =====================================================================================

/// The field names of the IDL's `struct Budget`, in declaration order.
///
/// Deliberately unforgiving about the *shape* it expects and deliberately blind to
/// everything else in the file: doc comments are dropped, and a line inside the block that
/// is not `name: Type …;` is not a field.
fn idl_budget_fields(idl: &str) -> Vec<String> {
    let start = idl
        .find("struct Budget {")
        .expect("the IDL declares `struct Budget`");
    let body = &idl[start + "struct Budget {".len()..];
    let end = body.find('}').expect("`struct Budget` is closed");
    body[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("///"))
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim().to_owned())
        .collect()
}

/// The backticked tokens of RFC 0026's own enumeration, in the order it writes them.
fn rfc_budget_tokens(rfc: &str) -> Vec<String> {
    const LEAD: &str = "`Cost` reports the same nine dimensions as `Budget` (";
    let start = rfc
        .find(LEAD)
        .expect("RFC 0026 enumerates the dimensions in \"Cost and omissions\"");
    let body = &rfc[start + LEAD.len()..];
    let end = body.find(')').expect("the enumeration is parenthesised");
    body[..end]
        .split(',')
        .map(|token| token.trim().trim_matches('`').to_owned())
        .collect()
}

/// The property names of `verification-task.schema.json`'s `budget` object.
fn schema_budget_properties(schema: &str) -> Vec<String> {
    let budget = object_after(schema, "\"budget\": {");
    let properties = object_after(&budget, "\"properties\": {");
    let mut names = Vec::new();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut current = String::new();
    let mut chars = properties.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if !in_string => {
                in_string = true;
                current.clear();
            }
            '"' if in_string => {
                in_string = false;
                if depth == 0 {
                    names.push(current.clone());
                }
            }
            '\\' if in_string => {
                chars.next();
            }
            _ if in_string => current.push(character),
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
    }
    names
}

/// The brace-matched object body that follows `opener`.
fn object_after(text: &str, opener: &str) -> String {
    let start = text
        .find(opener)
        .unwrap_or_else(|| panic!("the schema declares {opener}"));
    let body = &text[start + opener.len()..];
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut chars = body.char_indices();
    while let Some((index, character)) = chars.next() {
        match character {
            '"' => in_string = !in_string,
            '\\' if in_string => {
                chars.next();
            }
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                if depth == 0 {
                    return body[..index].to_owned();
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    panic!("{opener} is never closed");
}

fn ours() -> Vec<String> {
    CostDimension::ALL
        .into_iter()
        .map(|dimension| dimension.token().to_owned())
        .collect()
}

// =====================================================================================
// SD-12: one list
// =====================================================================================

#[test]
fn the_nine_dimensions_are_the_idls_struct_budget_in_declaration_order() {
    let idl = idl_budget_fields(IDL);
    assert_eq!(idl.len(), 9, "the IDL declares nine budget dimensions");
    assert_eq!(
        ours(),
        idl,
        "CostDimension::ALL must spell the IDL's `struct Budget`, member for member and in \
         order (SD-12)"
    );
}

#[test]
fn the_nine_dimensions_are_rfc_0026s_own_enumeration() {
    let rfc = rfc_budget_tokens(RFC_0026);
    assert_eq!(rfc.len(), 9, "the RFC enumerates nine dimensions");
    assert_eq!(
        ours(),
        rfc,
        "the RFC's prose and the IDL are independently maintained; both must agree with us"
    );
}

#[test]
fn the_nine_dimensions_are_the_verification_task_schemas_budget_properties() {
    let mut schema = schema_budget_properties(VERIFICATION_TASK);
    schema.sort();
    schema.dedup();
    let mut mine = ours();
    mine.sort();
    assert_eq!(
        mine, schema,
        "the schema is the third source SD-12 holds identical; it is sorted, so this \
         comparison is on the set rather than on the order"
    );
    let budget = object_after(VERIFICATION_TASK, "\"budget\": {");
    assert!(
        budget.contains("\"additionalProperties\": false"),
        "the schema closes the list; a tenth dimension is a schema change, not an addition"
    );
}

#[test]
fn the_check_is_not_vacuous() {
    let renamed = IDL.replacen(
        "  wall_ms: DurationMs optional;",
        "  wall: DurationMs optional;",
        1,
    );
    assert_ne!(
        idl_budget_fields(&renamed),
        ours(),
        "renaming an IDL field must break the comparison"
    );

    let dropped = RFC_0026.replacen("`wall_ms`, ", "", 1);
    assert_ne!(
        rfc_budget_tokens(&dropped),
        ours(),
        "dropping a dimension from the RFC's enumeration must break the comparison"
    );

    let added = VERIFICATION_TASK.replacen(
        "      \"candidates\": {",
        "      \"transitions\": {\n        \"minimum\": 0,\n        \"type\": \"integer\"\n      },\n      \"candidates\": {",
        1,
    );
    let mut widened = schema_budget_properties(&added);
    widened.sort();
    widened.dedup();
    let mut mine = ours();
    mine.sort();
    assert_ne!(
        mine, widened,
        "RFC 0026's F16 tenth dimension must break the comparison, not slip in"
    );
}

// =====================================================================================
// Charging: the metered arm and the unmetered arm, per dimension
// =====================================================================================

#[test]
fn every_dimension_exhausts_at_its_own_ceiling_when_it_is_metered() {
    for dimension in CostDimension::ALL {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(dimension, 10),
            MeterSet::none().with(dimension),
        );
        assert!(
            ledger.charge(dimension, 10).expect("metered").is_admitted(),
            "{dimension}: a charge that reaches the ceiling exactly is admitted"
        );
        let outcome = ledger.charge(dimension, 1).expect("metered");
        assert_eq!(
            outcome.exhaustion(),
            Some(&Exhaustion::new(dimension, 10, 10, 1)),
            "{dimension}: the eleventh unit must exhaust, naming this dimension"
        );
        assert_eq!(
            outcome.exhaustion().map(Exhaustion::error_code_token),
            Some("BudgetExhausted"),
            "{dimension}: exhaustion is an ErrorCode, never a verdict"
        );
        assert_eq!(
            ledger.spend().measured(dimension),
            Some(10),
            "{dimension}: an exhausted charge records nothing"
        );
        assert!(
            ledger.omissions().is_empty(),
            "{dimension}: a ceiling with a meter behind it owes no omission"
        );
        assert_eq!(ledger.enforced(), vec![dimension]);
    }
}

#[test]
fn every_dimension_is_refused_and_named_when_it_is_declared_but_unmetered() {
    for dimension in CostDimension::ALL {
        let mut ledger =
            BudgetLedger::new(Budget::unbounded().with(dimension, 10), MeterSet::none());
        assert_eq!(
            ledger.charge(dimension, 1),
            Err(BudgetFault::UnmeteredDimension { dimension }),
            "{dimension}: spend nothing measured must not be invented"
        );
        assert_eq!(
            ledger.spend().measured(dimension),
            None,
            "{dimension}: a dimension the engine does not measure is absent, never zero"
        );
        let omissions = ledger.omissions();
        assert_eq!(
            omissions.iter().map(|o| o.subject()).collect::<Vec<_>>(),
            vec![dimension.subject()],
            "{dimension}: a declared ceiling with no meter is a typed omission"
        );
        assert_eq!(omissions[0].reason_token(), "unsupported");
        assert!(
            ledger.enforced().is_empty(),
            "{dimension}: nothing is enforced without a meter"
        );
    }
}

#[test]
fn an_undeclared_unmetered_dimension_owes_nothing_and_admits_nothing() {
    let mut ledger = BudgetLedger::new(Budget::unbounded(), MeterSet::none());
    for dimension in CostDimension::ALL {
        assert_eq!(
            ledger.charge(dimension, 1),
            Err(BudgetFault::UnmeteredDimension { dimension })
        );
    }
    assert!(
        ledger.omissions().is_empty(),
        "an omission names something the answer left out; a ceiling nobody declared was \
         not left out"
    );
}

#[test]
fn a_declared_ceiling_of_zero_admits_nothing_and_is_not_an_absent_ceiling() {
    let mut ledger = BudgetLedger::new(
        Budget::unbounded().with(CostDimension::States, 0),
        MeterSet::STATES_ONLY,
    );
    assert_eq!(ledger.headroom(CostDimension::States), Some(0));
    assert!(
        ledger
            .charge(CostDimension::States, 1)
            .expect("metered")
            .exhaustion()
            .is_some()
    );
    assert!(
        ledger
            .charge(CostDimension::States, 0)
            .expect("metered")
            .is_admitted(),
        "spending nothing is always admissible, even at a zero ceiling"
    );
    assert_eq!(ledger.enforced(), vec![CostDimension::States]);
}

#[test]
fn the_states_only_meter_set_reproduces_the_daemons_eight_omissions() {
    let mut budget = Budget::unbounded();
    for dimension in CostDimension::ALL {
        budget = budget.with(dimension, 1_000);
    }
    let ledger = BudgetLedger::new(budget, MeterSet::STATES_ONLY);
    let subjects: Vec<String> = ledger.omissions().iter().map(|o| o.subject()).collect();
    assert_eq!(
        subjects,
        vec![
            "budget.wall_ms".to_owned(),
            "budget.cpu_ms".to_owned(),
            "budget.memory_bytes".to_owned(),
            "budget.solver_ms".to_owned(),
            "budget.proof_ms".to_owned(),
            "budget.tokens".to_owned(),
            "budget.candidates".to_owned(),
            "budget.bytes".to_owned(),
        ],
        "the same eight subjects `continuumd`'s `verification::unenforced` writes by hand, \
         derived from the meter set instead"
    );
    assert_eq!(
        MeterSet::STATES_ONLY.metering(CostDimension::States),
        Metering::Metered
    );
}
