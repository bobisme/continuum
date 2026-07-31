//! PR-1 exit evidence.
//!
//! > **Exit:** an unsupported empty task returns a valid machine result naming every
//! > epoch and no misleading success flag.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 1
//!
//! Each test below is one clause of that sentence, and the test names are the clauses.
//! The file is an *integration* test on purpose: it links `continuum-task`,
//! `continuum-value` (epochs — PR-1 / IMPL-02; assurance — PR-1 / IMPL-03) and
//! `continuum-evidence` (the claim-status lattice — PR-1 / IMPL-06) exactly as a caller
//! would, so what it demonstrates is the six PR-1 deliverables working together and not
//! one module's private view of itself.

use continuum_evidence::claim_status::ClaimStatus;
use continuum_task::result::{TaskOutcome, VerificationTaskResult};
use continuum_value::assurance::{
    AssuranceDimension, AssuranceEnvelope, InconclusiveReason, UnsupportedReason,
};
use continuum_value::epoch::{EpochBinding, EpochKind, EpochSet, ProtocolEpoch};

/// The subject of the exit condition: the result of an unsupported empty task that
/// pins nothing.
fn unsupported_empty_task() -> VerificationTaskResult {
    VerificationTaskResult::unsupported_empty_task(EpochSet::unpinned())
}

// --- "returns a valid machine result …" ------------------------------------------------

#[test]
fn an_unsupported_empty_task_returns_a_valid_machine_result() {
    // The whole exit sentence in one place; the tests that follow take it clause by
    // clause.
    let result = unsupported_empty_task();

    // … naming every epoch …
    assert_eq!(result.epochs().entries().len(), EpochKind::ALL.len());
    // … the outcome is typed inconclusive-unsupported …
    assert_eq!(
        result.outcome(),
        TaskOutcome::Inconclusive(InconclusiveReason::Unsupported)
    );
    // … and every assurance dimension is named with a typed reason …
    assert_eq!(
        result.assurance().unsupported_dimensions(),
        AssuranceDimension::ALL.to_vec()
    );
    // … with nothing anywhere in it that reads as success.
    assert!(
        !rendered_words(&result)
            .iter()
            .any(|word| SUCCESS_WORDS.iter().any(|forbidden| word == forbidden))
    );
}

#[test]
fn the_result_is_returned_by_value_never_as_an_ok_variant() {
    // "No misleading success flag" starts at the signature: the constructor is
    // infallible and hands back the result itself. A `Result<VerificationTaskResult, _>`
    // would hand every caller an `Ok` to mistake for "the task succeeded" — this line
    // compiles only because there is no such wrapper.
    let result: VerificationTaskResult = unsupported_empty_task();
    assert_eq!(result.outcome().verdict_token(), "inconclusive");
}

// --- "… naming every epoch …" ----------------------------------------------------------

#[test]
fn the_result_names_every_one_of_the_six_epochs() {
    // START_HERE, PR 1: "protocol, semantic, intent, evidence, proof, and corpus
    // epochs"; RFC 0026 requires all six be named and never conflated.
    let result = unsupported_empty_task();
    let names: Vec<&str> = result
        .epochs()
        .entries()
        .iter()
        .map(|entry| entry.kind.as_str())
        .collect();

    assert_eq!(
        names,
        [
            "protocol", "semantic", "intent", "evidence", "proof", "corpus"
        ]
    );
}

#[test]
fn an_epoch_the_result_cannot_pin_is_a_named_absence_never_an_omitted_field() {
    // IDL rule `envelope.epochs_named`: "An epoch the result cannot pin reads null; it
    // is never an absent field."
    let result = unsupported_empty_task();

    for entry in &result.epochs().entries() {
        assert_eq!(
            entry.binding,
            EpochBinding::Unpinned,
            "{} should read as a named absence",
            entry.kind
        );
        assert_eq!(entry.binding.as_str(), None);
        assert!(!entry.binding.is_pinned());
    }
    // The named absences are reported as a list of what is missing (INV-007), which is
    // the only thing a result that pinned nothing may say about itself.
    assert_eq!(result.epochs().unpinned_kinds(), EpochKind::ALL.to_vec());
}

#[test]
fn a_served_connection_pins_the_protocol_epoch_and_the_result_still_names_all_six() {
    // RFC 0026: `protocol_version` is negotiated at connection open and is "never null
    // on a served connection" (IDL `EpochSet.protocol`). Pinning one epoch must not
    // change how many the result names.
    let epochs = EpochSet::unpinned().with_protocol(ProtocolEpoch::new(3, 0));
    let result = VerificationTaskResult::unsupported_empty_task(epochs);
    let entries = result.epochs().entries();

    assert_eq!(entries.len(), EpochKind::ALL.len());
    assert_eq!(entries[0].kind, EpochKind::Protocol);
    assert_eq!(entries[0].binding.as_str(), Some("3.0"));
    for entry in &entries[1..] {
        assert_eq!(entry.binding, EpochBinding::Unpinned);
    }
    assert_eq!(
        result.epochs().unpinned_kinds(),
        vec![
            EpochKind::Semantic,
            EpochKind::Intent,
            EpochKind::Evidence,
            EpochKind::Proof,
            EpochKind::Corpus,
        ]
    );
    // Pinning an epoch does not decide anything: the outcome is unchanged.
    assert_eq!(
        result.outcome(),
        TaskOutcome::Inconclusive(InconclusiveReason::Unsupported)
    );
}

// --- "… the outcome is typed inconclusive-unsupported …" -------------------------------

#[test]
fn the_outcome_is_typed_inconclusive_with_the_unsupported_reason() {
    let result = unsupported_empty_task();

    assert_eq!(
        result.outcome(),
        TaskOutcome::Inconclusive(InconclusiveReason::Unsupported)
    );
    assert_eq!(result.outcome().verdict_token(), "inconclusive");
    assert_eq!(
        result.inconclusive_reason(),
        Some(InconclusiveReason::Unsupported)
    );
    assert_eq!(
        result
            .inconclusive_reason()
            .map(InconclusiveReason::as_str)
            .expect("an inconclusive outcome names its INV-008 reason"),
        "Unsupported"
    );
}

#[test]
fn unsupported_is_a_reason_and_never_a_verdict() {
    // assurance-result.schema.json: "Unsupported and engine-error outcomes are not
    // verdicts either: they are inconclusive with `inconclusive_reason`
    // Unsupported/EngineError (INV-008)."
    //
    // The verdict token is `inconclusive` for every reason INV-008 distinguishes: the
    // reason never reaches the verdict position, and the closed verdict vocabulary
    // stays three wide.
    let tokens: Vec<&str> = InconclusiveReason::ALL
        .into_iter()
        .map(|reason| TaskOutcome::Inconclusive(reason).verdict_token())
        .collect();
    assert_eq!(tokens, ["inconclusive"; 6]);

    assert_eq!(TaskOutcome::Established.verdict_token(), "established");
    assert_eq!(TaskOutcome::Refuted.verdict_token(), "refuted");
    assert_eq!(TaskOutcome::Established.inconclusive_reason(), None);
    assert_eq!(TaskOutcome::Refuted.inconclusive_reason(), None);
}

#[test]
fn an_inconclusive_outcome_carries_its_reason_verbatim_never_a_default() {
    // INV-008's reasons are "distinct outcomes"; a result may not blur two of them.
    for reason in InconclusiveReason::ALL {
        let result = VerificationTaskResult::new(
            TaskOutcome::Inconclusive(reason),
            AssuranceEnvelope::all_unsupported(
                &UnsupportedReason::new(VerificationTaskResult::EMPTY_TASK).expect("canonical"),
            ),
            EpochSet::unpinned(),
        );
        assert_eq!(result.inconclusive_reason(), Some(reason));
    }
}

#[test]
fn the_inconclusive_claim_status_obligation_is_discharged_by_the_result() {
    // The seam `continuum-evidence`'s claim_status.rs left for this bone: the lattice
    // states the obligation ("`Inconclusive` carries a typed reason per INV-008", plan
    // §11.4) without carrying the payload; the payload type lives in `continuum-value`;
    // the PR-1 exit joins them.
    let result = unsupported_empty_task();
    let status = result
        .outcome()
        .claim_status()
        .expect("an inconclusive verdict determines the `inconclusive` status");
    assert_eq!(status, ClaimStatus::Inconclusive);

    let obligations = status.schema_obligations();
    assert!(obligations.typed_inconclusive_reason);
    assert!(result.inconclusive_reason().is_some(), "obligation unpaid");

    // …and the obligations this result does *not* owe are the ones it must not pretend
    // to meet: it names no independent-checker basis and no promoting service, because
    // it reached neither `validated` nor any service-restricted status.
    assert!(!obligations.validation_basis);
    assert!(!obligations.service_identity);
}

#[test]
fn a_refuted_outcome_names_the_refuted_status_and_an_established_one_names_none() {
    // docs/44 "Status authority": `any → refuted`. An established verdict determines no
    // status by itself — which assurance status it reaches is a property of the evidence
    // (RFC 0031's ladder), not of the word.
    assert_eq!(
        TaskOutcome::Refuted.claim_status(),
        Some(ClaimStatus::Refuted)
    );
    assert_eq!(TaskOutcome::Established.claim_status(), None);
}

// --- "… and no misleading success flag." -----------------------------------------------

/// Words a machine result of an undecided task must not contain. `true`/`false` are
/// included because the strongest form of "no success flag" is no boolean at all.
const SUCCESS_WORDS: [&str; 13] = [
    "true",
    "false",
    "ok",
    "okay",
    "success",
    "successful",
    "verified",
    "valid",
    "passed",
    "pass",
    "done",
    "complete",
    "completed",
];

/// The alphanumeric words of a result's derived `Debug` rendering, lowercased.
///
/// `Debug` is derived transitively over every field of the result, its envelope and its
/// epochs, so a boolean field or a variant named `Ok`/`Success` anywhere inside would
/// appear here. Whole words are compared rather than substrings, so that honest tokens
/// like `Unsupported` or `empty-task` are not mistaken for flags.
fn rendered_words(result: &VerificationTaskResult) -> Vec<String> {
    format!("{result:?}")
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

#[test]
fn no_field_of_the_result_can_be_read_as_success() {
    let result = unsupported_empty_task();
    let words = rendered_words(&result);

    for forbidden in SUCCESS_WORDS {
        assert!(
            !words.iter().any(|word| word == forbidden),
            "the result of an unsupported empty task renders the word {forbidden:?}: {result:?}"
        );
    }
    // The rendering is not vacuous: the words that *are* there are the typed ones.
    assert!(words.iter().any(|word| word == "inconclusive"));
    assert!(words.iter().any(|word| word == "unsupported"));
}

#[test]
fn the_honest_summary_is_a_list_of_what_is_missing_not_a_flag() {
    // plan §3 B11: "Verified" alone is prohibited in machine output; INV-007 asks for
    // the omission manifest instead. Both lists are total — nine dimensions, six epochs
    // — so "nothing was established" is stated by enumerating, never by a bit.
    let result = unsupported_empty_task();

    assert_eq!(
        result.assurance().unsupported_dimensions(),
        AssuranceDimension::ALL.to_vec()
    );
    assert_eq!(result.epochs().unpinned_kinds(), EpochKind::ALL.to_vec());
}

// --- the envelope the result must carry ------------------------------------------------

#[test]
fn the_result_carries_all_nine_assurance_dimensions_each_with_a_typed_reason() {
    // RFC 0026 rule `envelope.assurance_required` and plan §3 B11: "Every envelope
    // dimension names its producing engine or carries a typed `Unsupported` value; a
    // dimension is never silently omitted."
    let result = unsupported_empty_task();
    let entries = result.assurance().entries();

    assert_eq!(entries.len(), AssuranceDimension::ALL.len());
    for (entry, dimension) in entries.iter().zip(AssuranceDimension::ALL) {
        assert_eq!(entry.dimension, dimension);
        assert_eq!(
            entry.evidence.engine(),
            None,
            "{dimension} claims a producing engine the empty task never ran"
        );
        assert!(
            entry.evidence.unsupported_reason().is_some(),
            "{dimension} is unsupported without a typed reason"
        );
    }
}

#[test]
fn the_memory_dimension_reads_unsupported_sequential_consistency_only() {
    // plan §3 B11 / §24.5: until the ADR-0032 weak-memory lane ships, *every* envelope's
    // memory dimension carries this reason — including an empty task's.
    let result = unsupported_empty_task();

    assert_eq!(
        result
            .assurance()
            .dimension(AssuranceDimension::MemoryModel)
            .unsupported_reason()
            .map(UnsupportedReason::as_str),
        Some("sequential-consistency-only")
    );
    // The other eight say why *this* task covered nothing, and say it in one spelling.
    for dimension in AssuranceDimension::ALL {
        if dimension == AssuranceDimension::MemoryModel {
            continue;
        }
        assert_eq!(
            result
                .assurance()
                .dimension(dimension)
                .unsupported_reason()
                .map(UnsupportedReason::as_str),
            Some(VerificationTaskResult::EMPTY_TASK),
            "{dimension}"
        );
    }
}

#[test]
fn the_result_is_deterministic_and_compares_by_value() {
    // docs/19 §7's determinism matrix and RFC 0026's `ordering.deterministic`: two
    // identical requests return identical results. Nothing in the seed reads a clock, a
    // counter, or an environment, so equal inputs give equal results.
    assert_eq!(unsupported_empty_task(), unsupported_empty_task());
    assert_eq!(
        format!("{:?}", unsupported_empty_task()),
        format!("{:?}", unsupported_empty_task())
    );
    assert_ne!(
        unsupported_empty_task(),
        VerificationTaskResult::unsupported_empty_task(
            EpochSet::unpinned().with_protocol(ProtocolEpoch::new(3, 0))
        )
    );
}
