//! The third leg: the types against sources the IDL does not own.
//!
//! `rule conformance.registry_agreement` names two authorities and refuses to prefer
//! either:
//!
//! > The operation set declared here MUST equal the plan §10.2 registry exactly, and each
//! > operation's `authority` clause MUST equal the RFC 0027 registry-table entry for it.
//! > A generator or validator MUST fail closed on any disagreement rather than preferring
//! > either source.
//!
//! `tests/idl_conformance.rs` checks the types against the IDL. This file checks them
//! against RFC 0027's authority table and RFC 0026's verdict distribution — documents
//! maintained by hand, beside the IDL, by different edits. Agreement with one source
//! cannot be manufactured by editing the other, which is the point of having three legs
//! rather than two.
//!
//! It also holds the vocabularies RFC 0026 says must agree with `continuum-value` to
//! their counterparts there:
//!
//! > The six members and their spellings are `crates/continuum-value/src/assurance.rs`'s
//! > `InconclusiveReason`; the crate and this RFC MUST agree token for token.
//! >
//! > — RFC 0026, "Verdicts"

use std::collections::BTreeMap;

use continuumd::protocol::registry::OPERATIONS;
use continuumd::protocol::spec::{Annotation, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, Compatibility, EvidenceKind, InconclusiveReason,
};

const RFC_0026: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/rfcs/0026-continuumd-native-protocol.md"
);
const RFC_0027: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/rfcs/0027-agent-tool-protocol.md"
);

/// The rows of the first table under `heading` whose header row is `header`.
///
/// Returns each row's cells, trimmed, with the separator row dropped. Panics rather than
/// returning nothing: a heading or header that stopped matching means the RFC was
/// restructured, and silently checking zero rows is the failure mode this whole file
/// exists to prevent.
fn table(path: &str, heading: &str, header: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path).expect("the RFC is readable");
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("{path} has the heading {heading:?}"));
    let section = &text[start..];
    let section = section
        .split_once("\n## ")
        .map_or(section, |(before, _)| before);
    let table_start = section
        .find(header)
        .unwrap_or_else(|| panic!("{path} has the table header {header:?} under {heading:?}"));
    let mut rows = Vec::new();
    for line in section[table_start..].lines().skip(1) {
        if !line.starts_with('|') {
            break;
        }
        if line.starts_with("|---") || line.starts_with("|:-") {
            continue;
        }
        // A cell may contain `\|` — RFC 0026's verdict table spells its vocabularies
        // that way. Hide the escaped pipes before splitting on the real ones.
        const ESCAPED_PIPE: char = '\u{1}';
        rows.push(
            line.replace("\\|", &ESCAPED_PIPE.to_string())
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().replace(ESCAPED_PIPE, "|"))
                .collect(),
        );
    }
    assert!(!rows.is_empty(), "{path}: {header:?} has no rows");
    rows
}

fn unbacktick(cell: &str) -> String {
    cell.replace(['`', '*'], "")
}

#[test]
fn the_authority_of_every_operation_equals_rfc_0027s_registry_row() {
    let rows = table(
        RFC_0027,
        "## The operation authority registry",
        "| Operation | Authority | Annotations |",
    );
    assert_eq!(rows.len(), 73, "RFC 0027 declares 73 rows");

    let declared: BTreeMap<&str, &continuumd::protocol::spec::OperationSpec> =
        OPERATIONS.iter().map(|spec| (spec.name, spec)).collect();

    let mut disagreements = Vec::new();
    for row in &rows {
        let name = unbacktick(&row[0]);
        let Some(spec) = declared.get(name.as_str()) else {
            disagreements.push(format!(
                "RFC 0027 has a row for {name:?}; the registry does not"
            ));
            continue;
        };
        // RFC 0027: "token-identical with the corresponding IDL `authority` clause,
        // allowing for the `revise-intent`/`revise_intent` spelling of one token" — the
        // wire token is the hyphenated one.
        if spec.authority.as_wire() != row[1] {
            disagreements.push(format!(
                "{name}: authority {:?}, RFC 0027 says {:?}",
                spec.authority.as_wire(),
                row[1]
            ));
        }
        let theirs: Vec<String> = row[2]
            .split_whitespace()
            .map(|token| unbacktick(token).trim_start_matches('@').to_owned())
            .collect();
        let mine: Vec<String> = spec
            .annotations
            .iter()
            .map(|annotation| annotation.as_idl().to_owned())
            .collect();
        if mine != theirs {
            disagreements.push(format!(
                "{name}: annotations {mine:?}, RFC 0027 says {theirs:?}"
            ));
        }
    }
    for spec in OPERATIONS {
        if !rows.iter().any(|row| unbacktick(&row[0]) == spec.name) {
            disagreements.push(format!(
                "the registry declares {:?}; RFC 0027 has no row",
                spec.name
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "the types and RFC 0027 disagree:\n  {}",
        disagreements.join("\n  ")
    );
}

#[test]
fn the_authority_and_annotation_distribution_matches_rfc_0027() {
    // RFC 0027 states the distribution as "a conformance test derives from the table
    // rather than asserting beside it". This derives it from the *types* and compares it
    // to the RFC's stated counts, so a miscount in either is a failure.
    let rows = table(
        RFC_0027,
        "## The operation authority registry",
        "| Authority | Count | Annotation | Count |",
    );

    let mut authorities: BTreeMap<String, usize> = BTreeMap::new();
    let mut annotations: BTreeMap<String, usize> = BTreeMap::new();
    for spec in OPERATIONS {
        *authorities
            .entry(spec.authority.as_wire().to_owned())
            .or_default() += 1;
        for annotation in spec.annotations {
            *annotations
                .entry(annotation.as_idl().to_owned())
                .or_default() += 1;
        }
    }

    let mut checked = 0;
    for row in &rows {
        let authority = unbacktick(&row[0]);
        let annotation = unbacktick(&row[2]);
        if !authority.is_empty() && authority != "total" {
            let stated: usize = unbacktick(&row[1]).parse().expect("a count");
            assert_eq!(
                authorities.get(&authority).copied().unwrap_or_default(),
                stated,
                "authority {authority}"
            );
            checked += 1;
        }
        if authority == "total" {
            assert_eq!(unbacktick(&row[1]).parse::<usize>().expect("a count"), 73);
        }
        if !annotation.is_empty() {
            let stated: usize = unbacktick(&row[3]).parse().expect("a count");
            let key = annotation.trim_start_matches('@').to_owned();
            assert_eq!(
                annotations.get(&key).copied().unwrap_or_default(),
                stated,
                "annotation {key}"
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked,
        AuthorityLevel::ALL.len() + 7,
        "every authority and every annotation is covered by a stated count"
    );
}

#[test]
fn the_verdict_distribution_matches_rfc_0026() {
    let rows = table(
        RFC_0026,
        "### Verdicts",
        "| Variant | Value type | Operations |",
    );

    let mut stated: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows {
        let variant = unbacktick(&row[0]);
        let count: usize = row[2].parse().expect("an operation count");
        // The last row's variant cell is an em dash: operations with no `verdict` clause.
        let key = if variant == "—" {
            "none".to_owned()
        } else {
            variant
        };
        stated.insert(key, count);
    }
    assert_eq!(stated.values().sum::<usize>(), 73);

    let mut derived: BTreeMap<String, usize> = BTreeMap::new();
    for spec in OPERATIONS {
        let key = match spec.verdict {
            None => "none",
            Some("SemanticVerdictValue") => "semantic",
            Some("EvaluationVerdictValue") => "evaluation",
            Some("PolicyVerdictValue") => "policy",
            Some("StructuralVerdictValue") => "structural",
            Some(other) => panic!("{} declares an unknown verdict type {other}", spec.name),
        };
        *derived.entry(key.to_owned()).or_default() += 1;
    }

    assert_eq!(derived, stated);
}

#[test]
fn the_common_error_union_is_annotation_driven() {
    // `rule errors.common`: every operation may return five codes; every `@mutation`
    // adds two more. The rule is stated against annotations, so the annotations have to
    // be right for it to be implementable at all — this asserts the two facts RFC 0026
    // calls out by name.
    //
    // > Two operations — `workspace.seal` and `task.status` — declare an empty `errors`
    // > clause, which means exactly "this operation adds nothing to the common union",
    // > not "this operation cannot fail".
    // >
    // > — RFC 0026, correction 15
    //
    // The IDL declares *three*: `task.subscribe` has `errors [];` too. The count and the
    // enumeration in that correction are both short by one. This asserts the IDL, which
    // is the normative side of the disagreement — "where the prose and this file
    // disagree, this file decides and the RFC is corrected" (IDL header). The correction
    // to RFC 0026 is not this bone's to make; the discrepancy is recorded here so it
    // cannot be lost, and the assertion below is what the wire actually declares.
    let empty: Vec<&str> = OPERATIONS
        .iter()
        .filter(|spec| spec.errors.is_empty())
        .map(|spec| spec.name)
        .collect();
    assert_eq!(empty, ["workspace.seal", "task.status", "task.subscribe"]);

    let seal = continuumd::protocol::registry::operation("workspace.seal").expect("declared");
    assert!(seal.has(Annotation::Mutation), "workspace.seal mutates");
    assert!(!seal.has(Annotation::Readonly));

    // Every operation carries exactly one of `@readonly`/`@mutation`; the idempotency-key
    // obligation of the request envelope is stated against that partition.
    for spec in OPERATIONS {
        let readonly = spec.has(Annotation::Readonly);
        let mutation = spec.has(Annotation::Mutation);
        assert!(
            readonly ^ mutation,
            "{} must be exactly one of @readonly/@mutation",
            spec.name
        );
    }
}

/// The `continuum-value` token for a compatibility verdict.
///
/// Written as an exhaustive match on purpose: a member added there stops this file from
/// compiling, which is the strongest available form of "the crate and this RFC MUST
/// agree token for token".
fn value_compatibility_token(value: continuum_value::epoch::Compatibility) -> &'static str {
    match value {
        continuum_value::epoch::Compatibility::Preserved => "Preserved",
        continuum_value::epoch::Compatibility::Revalidate => "Revalidate",
        continuum_value::epoch::Compatibility::Incompatible => "Incompatible",
    }
}

#[test]
fn the_vocabularies_agree_with_continuum_value_token_for_token() {
    use continuum_value::assurance::{
        AssuranceLevel, EvidenceClass, InconclusiveReason as ValueInconclusiveReason,
    };

    let mine: Vec<&str> = InconclusiveReason::ALL
        .iter()
        .map(|reason| reason.as_wire())
        .collect();
    let theirs: Vec<&str> = ValueInconclusiveReason::ALL
        .iter()
        .map(|reason| reason.as_str())
        .collect();
    assert_eq!(mine, theirs, "InconclusiveReason (RFC 0026, \"Verdicts\")");

    let mine: Vec<&str> = AssuranceClass::ALL
        .iter()
        .map(|class| class.as_wire())
        .collect();
    let theirs: Vec<&str> = AssuranceLevel::ALL
        .iter()
        .map(|level| level.as_str())
        .collect();
    assert_eq!(mine, theirs, "AssuranceClass against AssuranceLevel");

    let mine: Vec<&str> = EvidenceKind::ALL
        .iter()
        .map(|kind| kind.as_wire())
        .collect();
    let theirs: Vec<&str> = EvidenceClass::ALL
        .iter()
        .map(|class| class.as_str())
        .collect();
    assert_eq!(mine, theirs, "EvidenceKind against EvidenceClass");

    let theirs = [
        continuum_value::epoch::Compatibility::Preserved,
        continuum_value::epoch::Compatibility::Revalidate,
        continuum_value::epoch::Compatibility::Incompatible,
    ];
    let mine: Vec<&str> = Compatibility::ALL
        .iter()
        .map(|value| value.as_wire())
        .collect();
    let theirs: Vec<&str> = theirs
        .iter()
        .copied()
        .map(value_compatibility_token)
        .collect();
    assert_eq!(mine, theirs, "Compatibility (plan §4.6)");
}
