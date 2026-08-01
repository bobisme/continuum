//! The LRAT checker: every step re-derived by unit propagation (PR 9).
//!
//! # What is being re-derived
//!
//! > SMT proof terms SHOULD use Alethe where supported. SAT subproofs MAY use LRAT.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Inductive
//! >   invariant"
//!
//! A clausal refutation is a sequence of clause additions, each of which must be
//! *reverse unit propagation* (RUP) derivable from the clauses active at that point.
//! Concretely, for a derived clause `C` with antecedent chain `h₁ … hₙ`:
//!
//! 1. assume every literal of `C` false;
//! 2. walk the chain in order; each `hᵢ` must be *unit* under the running assignment —
//!    exactly one unassigned literal, every other literal false — and its unassigned
//!    literal is then assigned true;
//! 3. the final antecedent `hₙ` must instead be *falsified*, which is the conflict.
//!
//! A conflict from `¬C` means `C` is implied by the antecedents, so adding `C`
//! preserves the formula's models. The refutation is complete when a step derives the
//! empty clause: `⊥` is implied, so the original formula has no model.
//!
//! Nothing in that argument depends on the solver being correct, on the antecedents
//! being the *right* ones, or on the chain being minimal — a wrong hint simply fails to
//! propagate and the step is rejected. That is why docs/03 §7 places a checked LRAT
//! proof in the `certificate checked` row and an unchecked one in `solver-trusted`, and
//! why ADR-0017 says `unsat` is `CHECKED_CERTIFICATE` "only when proof artifacts are
//! independently checked".
//!
//! # Why the chain is walked, not searched
//!
//! A plain DRAT checker performs full unit propagation to closure at every step, which
//! is where DRAT checking spends its time and where a checker acquires a search
//! heuristic of its own. LRAT's contribution is that the producer names the antecedents
//! and their order, so the checker becomes a linear walk with no strategy: the trusted
//! base gains no search, and the certificate gets bigger instead. That trade is exactly
//! plan §20's "no shared optimized evaluator code with any engine".
//!
//! Requiring each antecedent to be *unit* rather than merely useful is what keeps the
//! walk honest. A satisfied antecedent proves nothing, and an antecedent with two
//! unassigned literals is not a propagation; admitting either would let a producer pad
//! a chain until a conflict happened to appear.
//!
//! # Order of obligations
//!
//! Fixed, so a diagnostic is reproducible and a mutation suite can name the rejection a
//! given mutation produces:
//!
//! 1. decode (wire form; see [`crate::wire`]);
//! 2. per step, in derivation order:
//!    - an addition's clause is not a tautology, and its chain propagates to a
//!      conflict exactly at its last antecedent;
//!    - a deletion names only defined, still-active clauses;
//! 3. the empty clause is derived, and nothing follows it.
//!
//! # Determinism
//!
//! The checker reads no clock, draws no randomness, opens no socket, and iterates only
//! over sequences whose order is a decode-time invariant (INV-005; ADR-0003; docs/12
//! §1). Two runs over the same bytes produce byte-identical verdicts on any platform,
//! and [`CheckedClaim::propagations`] reports the work as a function of the bytes —
//! RFC 0005's "deterministic resource accounting".

use crate::verdict::{CertificateKind, CheckedClaim, Rejection, Verdict};
use crate::wire::{Body, Clause, DecodeFailure, Envelope, LratBody, Step, decode};

/// Check one SAT certificate from its wire form.
///
/// The crate's whole public checking surface. It takes bytes — never a structure a
/// solver adapter could hand over — and returns the closed [`Verdict`] vocabulary. It
/// cannot panic on any input: see the module documentation of [`crate::wire`] and the
/// `garbage_bytes_never_panic` test.
#[must_use]
pub fn check_certificate(bytes: &[u8]) -> Verdict {
    let certificate = match decode(bytes) {
        Ok(certificate) => certificate,
        Err(DecodeFailure::Rejected(rejection)) => return Verdict::Rejected(rejection),
        Err(DecodeFailure::Unsupported(feature)) => return Verdict::Unsupported(feature),
    };
    match certificate.body() {
        Body::Lrat(body) => check_lrat(certificate.envelope(), body),
    }
}

// ---------------------------------------------------------------------------
// the clause store
// ---------------------------------------------------------------------------

/// The clauses currently available to a propagation chain, by identifier.
///
/// Flat storage with a strictly ascending identifier column: the wire form requires
/// addition identifiers to increase, so lookup is a binary search over the identifiers
/// themselves rather than an index into a producer-supplied map the kernel would have
/// to trust. Deletion clears a flag; nothing is ever moved, so an identifier's meaning
/// never changes.
#[derive(Debug, Default)]
struct ClauseStore {
    ids: Vec<u32>,
    starts: Vec<u32>,
    lengths: Vec<u32>,
    active: Vec<bool>,
    literals: Vec<i32>,
}

impl ClauseStore {
    /// Append a clause under `id`. Callers push in strictly ascending identifier
    /// order, which the decoder has already established.
    fn push(&mut self, id: u32, literals: &[i32]) {
        self.ids.push(id);
        self.starts.push(narrow_u32(self.literals.len()));
        self.lengths.push(narrow_u32(literals.len()));
        self.active.push(true);
        self.literals.extend_from_slice(literals);
    }

    /// The storage slot holding `id`, or `None` when no clause has that identifier.
    fn find(&self, id: u32) -> Option<usize> {
        self.ids.binary_search(&id).ok()
    }

    /// Whether the clause in `slot` has not been deleted.
    fn is_active(&self, slot: usize) -> bool {
        self.active.get(slot).copied().unwrap_or(false)
    }

    /// Mark the clause in `slot` deleted.
    fn deactivate(&mut self, slot: usize) {
        if let Some(flag) = self.active.get_mut(slot) {
            *flag = false;
        }
    }

    /// The literals of the clause in `slot`.
    fn literals(&self, slot: usize) -> Option<&[i32]> {
        let start = usize::try_from(*self.starts.get(slot)?).ok()?;
        let length = usize::try_from(*self.lengths.get(slot)?).ok()?;
        let end = start.checked_add(length)?;
        self.literals.get(start..end)
    }
}

// ---------------------------------------------------------------------------
// the assignment
// ---------------------------------------------------------------------------

/// A variable is not assigned.
const UNASSIGNED: u8 = 0;
/// A variable is assigned true.
const TRUE: u8 = 1;
/// A variable is assigned false.
const FALSE: u8 = 2;

/// The partial assignment one propagation chain runs under.
///
/// Sized from the largest variable the certificate's clauses actually mention, never
/// from the declared `variable_count`: a declared count is not allowed to reserve
/// memory before the bytes that justify it have been read (RFC 0005 "Resource bounds";
/// docs/16 PO-KER-002). The trail makes resetting between steps proportional to what
/// was assigned rather than to the variable count.
#[derive(Debug)]
struct Assignment {
    value: Vec<u8>,
    trail: Vec<u32>,
}

impl Assignment {
    fn new(variables: u32) -> Self {
        let slots = (variables as usize).saturating_add(1);
        Self {
            value: vec![UNASSIGNED; slots],
            trail: Vec::new(),
        }
    }

    /// Undo every assignment, leaving the array clear for the next step.
    fn reset(&mut self) {
        for variable in self.trail.drain(..) {
            if let Some(slot) = self.value.get_mut(variable as usize) {
                *slot = UNASSIGNED;
            }
        }
    }

    /// [`TRUE`], [`FALSE`] or [`UNASSIGNED`] for `literal` under this assignment.
    fn value_of(&self, literal: i32) -> u8 {
        let raw = self
            .value
            .get(literal.unsigned_abs() as usize)
            .copied()
            .unwrap_or(UNASSIGNED);
        if raw == UNASSIGNED {
            UNASSIGNED
        } else if (raw == TRUE) == (literal > 0) {
            TRUE
        } else {
            FALSE
        }
    }

    /// Assign the variable of `literal` so that `literal` is true.
    fn satisfy(&mut self, literal: i32) {
        let variable = literal.unsigned_abs();
        if let Some(slot) = self.value.get_mut(variable as usize) {
            *slot = if literal > 0 { TRUE } else { FALSE };
            self.trail.push(variable);
        }
    }
}

/// What a clause looks like under a partial assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseState {
    /// Some literal is already true; the clause cannot propagate.
    Satisfied,
    /// Every literal is false; this is the conflict a chain must end in.
    Falsified,
    /// Exactly one literal is unassigned and the rest are false.
    Unit(i32),
    /// Two or more literals are unassigned; not a propagation.
    Ambiguous(u32),
}

/// Evaluate `literals` under `assignment`.
fn evaluate(assignment: &Assignment, literals: &[i32]) -> ClauseState {
    let mut unassigned: u32 = 0;
    let mut candidate: i32 = 0;
    for literal in literals {
        match assignment.value_of(*literal) {
            TRUE => return ClauseState::Satisfied,
            UNASSIGNED => {
                unassigned = unassigned.saturating_add(1);
                candidate = *literal;
            }
            _ => {}
        }
    }
    match unassigned {
        0 => ClauseState::Falsified,
        1 => ClauseState::Unit(candidate),
        count => ClauseState::Ambiguous(count),
    }
}

// ---------------------------------------------------------------------------
// the checker
// ---------------------------------------------------------------------------

/// Re-derive one addition step, returning how many propagations it took.
fn check_addition(
    store: &ClauseStore,
    assignment: &mut Assignment,
    step: u32,
    clause: &Clause,
    hints: &[u32],
) -> Result<u64, Rejection> {
    if clause.is_tautology() {
        return Err(Rejection::VacuousDerivation { step });
    }
    assignment.reset();
    for literal in clause.literals() {
        // Assume `C` false: every literal of `C` is falsified. The clause is not a
        // tautology and its literals are strictly ascending, so no two assumptions
        // touch the same variable and the assumption set is consistent by
        // construction.
        assignment.satisfy(literal.wrapping_neg());
    }

    let last = hints.len().saturating_sub(1);
    let mut propagations: u64 = 0;
    let mut conflict = false;
    for (index, id) in hints.iter().enumerate() {
        let position = narrow_u32(index);
        let Some(slot) = store.find(*id) else {
            return Err(Rejection::HintOutOfRange {
                step,
                position,
                id: *id,
            });
        };
        if !store.is_active(slot) {
            return Err(Rejection::HintDeleted {
                step,
                position,
                id: *id,
            });
        }
        let Some(literals) = store.literals(slot) else {
            return Err(Rejection::HintOutOfRange {
                step,
                position,
                id: *id,
            });
        };
        match evaluate(assignment, literals) {
            ClauseState::Satisfied => {
                return Err(Rejection::HintSatisfied {
                    step,
                    position,
                    id: *id,
                });
            }
            ClauseState::Ambiguous(unassigned) => {
                return Err(Rejection::HintNotUnit {
                    step,
                    position,
                    id: *id,
                    unassigned,
                });
            }
            ClauseState::Unit(literal) => {
                assignment.satisfy(literal);
                propagations = propagations.saturating_add(1);
            }
            ClauseState::Falsified => {
                if index != last {
                    return Err(Rejection::ConflictBeforeEnd { step, position });
                }
                conflict = true;
            }
        }
    }
    if conflict {
        Ok(propagations)
    } else {
        Err(Rejection::NoConflict { step })
    }
}

/// Replay a whole refutation.
fn check_lrat(envelope: &Envelope, body: &LratBody) -> Verdict {
    let variables = observed_variables(body);
    let mut store = ClauseStore::default();
    for (index, clause) in body.formula().clauses().iter().enumerate() {
        store.push(narrow_u32(index).saturating_add(1), clause.literals());
    }
    let input_clauses = narrow_u32(body.formula().clauses().len());

    let mut assignment = Assignment::new(variables);
    let mut derived: u32 = 0;
    let mut deleted: u32 = 0;
    let mut propagations: u64 = 0;
    let mut refuted_at: Option<u32> = None;

    for (index, step) in body.steps().iter().enumerate() {
        if let Some(step) = refuted_at {
            return Verdict::Rejected(Rejection::ProofContinuesAfterEmptyClause { step });
        }
        let step_index = narrow_u32(index);
        match step {
            Step::Add(addition) => {
                match check_addition(
                    &store,
                    &mut assignment,
                    step_index,
                    addition.clause(),
                    addition.hints(),
                ) {
                    Ok(count) => propagations = propagations.saturating_add(count),
                    Err(rejection) => return Verdict::Rejected(rejection),
                }
                store.push(addition.id(), addition.clause().literals());
                derived = derived.saturating_add(1);
                if addition.clause().is_empty() {
                    refuted_at = Some(step_index);
                }
            }
            Step::Delete(deletion) => {
                for (position, id) in deletion.ids().iter().enumerate() {
                    let position = narrow_u32(position);
                    let Some(slot) = store.find(*id) else {
                        return Verdict::Rejected(Rejection::DeletionOutOfRange {
                            step: step_index,
                            position,
                            id: *id,
                        });
                    };
                    if !store.is_active(slot) {
                        return Verdict::Rejected(Rejection::DeletionRepeated {
                            step: step_index,
                            position,
                            id: *id,
                        });
                    }
                    store.deactivate(slot);
                    deleted = deleted.saturating_add(1);
                }
            }
        }
    }

    if refuted_at.is_none() {
        return Verdict::Rejected(Rejection::EmptyClauseNotDerived);
    }

    Verdict::Verified(CheckedClaim::new(
        CertificateKind::Lrat,
        envelope.clone(),
        variables,
        input_clauses,
        derived,
        deleted,
        propagations,
    ))
}

/// The largest variable index any carried clause mentions.
fn observed_variables(body: &LratBody) -> u32 {
    let mut largest: u32 = 0;
    for clause in body.formula().clauses() {
        largest = largest.max(largest_variable(clause));
    }
    for step in body.steps() {
        if let Step::Add(addition) = step {
            largest = largest.max(largest_variable(addition.clause()));
        }
    }
    largest
}

/// The largest variable index one clause mentions.
fn largest_variable(clause: &Clause) -> u32 {
    let mut largest: u32 = 0;
    for literal in clause.literals() {
        largest = largest.max(literal.unsigned_abs());
    }
    largest
}

/// Saturating `usize -> u32`, for identifiers and diagnostic indices only.
///
/// Every count this is applied to is bounded far below `u32::MAX` by the wire format's
/// declared limits, so saturation is unreachable; it exists so that the conversion has
/// no failure mode at all.
const fn narrow_u32(value: usize) -> u32 {
    if value > u32::MAX as usize {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures; the no-panic covenant \
                  is a property of the shipped checker, not of its test harness"
    )]

    use super::*;
    use crate::fixture::{Plan, PlannedStep, Xorshift};
    use crate::verdict::{Feature, Field, TokenFault};
    use crate::wire::{MAGIC, MAX_TOKEN_BYTES, WIRE_EPOCH};

    fn verdict(plan: &Plan) -> Verdict {
        check_certificate(&plan.encode())
    }

    fn rejection(plan: &Plan) -> Rejection {
        match verdict(plan) {
            Verdict::Rejected(rejection) => rejection,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    // --- green path --------------------------------------------------------

    #[test]
    fn the_three_variable_refutation_is_verified() {
        let plan = Plan::three_variable_refutation();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the hand-checked refutation must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::Lrat);
        assert_eq!(claim.variables(), 3);
        assert_eq!(claim.input_clauses(), 8);
        assert_eq!(claim.derived_clauses(), 7);
        assert_eq!(claim.deleted_clauses(), 6);
        // Every chain of this refutation propagates exactly once before conflicting.
        assert_eq!(claim.propagations(), 7);
        assert_eq!(
            claim.envelope().model_digest().as_str(),
            "blake3:three-variable-cnf"
        );
        assert_eq!(
            claim.trusted_components(),
            ["envelope-digest-binding", "formula-model-correspondence"]
        );
    }

    #[test]
    fn checking_is_deterministic_over_the_same_bytes() {
        let bytes = Plan::three_variable_refutation().encode();
        let first = check_certificate(&bytes);
        let second = check_certificate(&bytes);
        assert_eq!(first, second);
        assert!(first.is_verified());
    }

    #[test]
    fn a_refutation_without_deletions_is_verified_too() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps
            .retain(|step| !matches!(step, PlannedStep::Delete(_)));
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("deletions are an optimisation, not an obligation");
        };
        assert_eq!(claim.deleted_clauses(), 0);
        assert_eq!(claim.derived_clauses(), 7);
    }

    // --- adversarial: framing ---------------------------------------------

    #[test]
    fn empty_input_is_a_truncation_not_a_verdict() {
        assert_eq!(
            check_certificate(&[]),
            Verdict::Rejected(Rejection::Truncated {
                field: Field::Magic,
                offset: 0,
                needed: 8,
                available: 0,
            })
        );
    }

    #[test]
    fn truncated_bytes_are_rejected_at_every_length() {
        let bytes = Plan::three_variable_refutation().encode();
        for cut in 0..bytes.len() {
            let verdict = check_certificate(&bytes[..cut]);
            assert!(
                !verdict.is_verified(),
                "a {cut}-byte prefix was accepted as a certificate"
            );
        }
    }

    #[test]
    fn a_foreign_magic_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.magic = *b"CONTCERT"; // continuum-kernel-core's magic
        assert_eq!(rejection(&plan), Rejection::BadMagic);
    }

    #[test]
    fn a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.wire_epoch = WIRE_EPOCH.saturating_add(1);
        plan.schema_epoch = plan.wire_epoch;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::WireEpoch {
                found: WIRE_EPOCH + 1
            })
        );
    }

    #[test]
    fn an_unimplemented_certificate_family_is_unsupported() {
        let mut plan = Plan::three_variable_refutation();
        plan.kind = 2; // `veripb` in the receipt enum: a real family, not this crate's
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::CertificateKind { found: 2 })
        );
    }

    #[test]
    fn a_rat_step_is_unsupported_never_rejected() {
        // The honesty boundary of this crate, stated as a test: a RAT proof may be
        // perfectly valid, and this build simply cannot replay it (INV-008).
        let mut plan = Plan::three_variable_refutation();
        plan.steps.insert(0, PlannedStep::RawKind(3));
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::ProofStep { found: 3 })
        );
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.trailing = vec![0x00, 0x01];
        assert_eq!(rejection(&plan), Rejection::TrailingBytes { extra: 2 });
    }

    #[test]
    fn an_oversized_input_is_rejected_before_it_is_parsed() {
        let bytes = vec![0_u8; crate::wire::MAX_CERTIFICATE_BYTES + 1];
        assert_eq!(
            check_certificate(&bytes),
            Verdict::Rejected(Rejection::Oversized {
                found: crate::wire::MAX_CERTIFICATE_BYTES + 1
            })
        );
    }

    // --- adversarial: envelope --------------------------------------------

    #[test]
    fn an_envelope_that_names_a_different_schema_epoch_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.schema_epoch = WIRE_EPOCH.saturating_add(3);
        assert_eq!(
            rejection(&plan),
            Rejection::SchemaEpochMismatch {
                declared: WIRE_EPOCH + 3,
                header: WIRE_EPOCH,
            }
        );
    }

    #[test]
    fn malformed_envelope_tokens_are_rejected_by_field() {
        let mut empty = Plan::three_variable_refutation();
        empty.model_digest = String::new();
        assert_eq!(
            rejection(&empty),
            Rejection::MalformedToken {
                field: Field::ModelDigest,
                fault: TokenFault::Empty,
            }
        );

        let mut long = Plan::three_variable_refutation();
        long.producer = "p".repeat(MAX_TOKEN_BYTES + 1);
        assert_eq!(
            rejection(&long),
            Rejection::MalformedToken {
                field: Field::Producer,
                fault: TokenFault::TooLong {
                    found: MAX_TOKEN_BYTES + 1
                },
            }
        );

        let mut space = Plan::three_variable_refutation();
        space.semantic_epoch = "continuum semantics 1".to_owned();
        assert_eq!(
            rejection(&space),
            Rejection::MalformedToken {
                field: Field::SemanticEpoch,
                fault: TokenFault::NonPrintable {
                    offset: 9,
                    byte: b' ',
                },
            }
        );
    }

    #[test]
    fn unordered_domain_packs_are_rejected_as_ambiguous_encodings() {
        let mut plan = Plan::three_variable_refutation();
        plan.domain_packs = vec!["blake3:b".to_owned(), "blake3:a".to_owned()];
        assert_eq!(
            rejection(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::DomainPack,
                index: 1,
            }
        );
    }

    #[test]
    fn envelope_digests_are_carried_labels_not_facts_the_kernel_can_check() {
        // Stated as a test so the boundary cannot drift silently. The envelope is
        // opaque to a self-contained checker: changing a digest byte produces a
        // *different verified claim*, never a rejection, because nothing in the
        // certificate lets the kernel recompute it. Binding those digests to real
        // artifacts is the receipt's obligation (RFC 0024, ADR-0035), which is why
        // `CheckedClaim::trusted_components` names it.
        let mut plan = Plan::three_variable_refutation();
        plan.model_digest = "blake3:some-other-formula".to_owned();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("an envelope relabel is not a malformed certificate");
        };
        assert_eq!(
            claim.envelope().model_digest().as_str(),
            "blake3:some-other-formula"
        );
    }

    // --- adversarial: malformed clauses and counts -------------------------

    #[test]
    fn counts_outside_the_declared_range_are_rejected() {
        let mut no_clauses = Plan::three_variable_refutation();
        no_clauses.clauses.clear();
        assert_eq!(
            rejection(&no_clauses),
            Rejection::CountOutOfRange {
                field: Field::ClauseCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_CLAUSES),
            }
        );

        let mut no_variables = Plan::three_variable_refutation();
        no_variables.variables = 0;
        assert_eq!(
            rejection(&no_variables),
            Rejection::CountOutOfRange {
                field: Field::VariableCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_VARIABLES),
            }
        );

        // An in-range count that overstates the data is the classic "read past the
        // buffer" invitation. Here it swallows the proof section, which is not a valid
        // continuation of the clause list. It is a rejection, and it is not a read
        // past the end — no count is ever allowed to reserve for itself.
        let mut huge = Plan::three_variable_refutation();
        huge.declared_clause_count = Some(crate::wire::MAX_CLAUSES);
        assert!(matches!(
            rejection(&huge),
            Rejection::LiteralOutOfRange { clause: 9, .. }
        ));

        let mut beyond = Plan::three_variable_refutation();
        beyond.declared_clause_count = Some(u32::MAX);
        assert_eq!(
            rejection(&beyond),
            Rejection::CountOutOfRange {
                field: Field::ClauseCount,
                found: u64::from(u32::MAX),
                min: 1,
                max: u64::from(crate::wire::MAX_CLAUSES),
            }
        );

        let mut no_steps = Plan::three_variable_refutation();
        no_steps.steps.clear();
        assert_eq!(
            rejection(&no_steps),
            Rejection::CountOutOfRange {
                field: Field::StepCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_STEPS),
            }
        );
    }

    #[test]
    fn a_zero_literal_names_no_variable_and_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.clauses[0] = vec![0];
        assert_eq!(
            rejection(&plan),
            Rejection::ZeroLiteral {
                clause: 1,
                position: 0
            }
        );
    }

    #[test]
    fn a_literal_beyond_the_declared_variable_count_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.clauses[0] = vec![1, 2, 9];
        assert_eq!(
            rejection(&plan),
            Rejection::LiteralOutOfRange {
                clause: 1,
                position: 2,
                literal: 9,
                variables: 3,
            }
        );
    }

    #[test]
    fn unordered_or_duplicated_literals_are_rejected() {
        let mut unordered = Plan::three_variable_refutation();
        unordered.clauses[0] = vec![2, 1, 3];
        assert_eq!(
            rejection(&unordered),
            Rejection::LiteralsNotAscending {
                clause: 1,
                position: 1
            }
        );

        let mut duplicated = Plan::three_variable_refutation();
        duplicated.clauses[0] = vec![1, 1, 3];
        assert_eq!(
            rejection(&duplicated),
            Rejection::LiteralsNotAscending {
                clause: 1,
                position: 1
            }
        );

        // Raw two's-complement order is *not* the canonical order.
        let mut raw_order = Plan::three_variable_refutation();
        raw_order.clauses[1] = vec![-3, 1, 2];
        assert_eq!(
            rejection(&raw_order),
            Rejection::LiteralsNotAscending {
                clause: 2,
                position: 1
            }
        );
    }

    #[test]
    fn a_reused_or_descending_clause_identifier_is_rejected() {
        let mut reused = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut reused.steps[1] {
            addition.id = 9; // already taken by step 0
        }
        assert_eq!(
            rejection(&reused),
            Rejection::IdNotIncreasing {
                step: 1,
                found: 9,
                previous: 9,
            }
        );

        let mut colliding = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut colliding.steps[0] {
            addition.id = 8; // an input clause's identifier
        }
        assert_eq!(
            rejection(&colliding),
            Rejection::IdNotIncreasing {
                step: 0,
                found: 8,
                previous: 8,
            }
        );
    }

    // --- adversarial: the propagation obligation ---------------------------

    #[test]
    fn a_chain_that_never_conflicts_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.hints.pop(); // drop the conflicting antecedent
        }
        assert_eq!(rejection(&plan), Rejection::NoConflict { step: 0 });
    }

    #[test]
    fn dropping_any_antecedent_breaks_its_step() {
        // Every antecedent of this refutation is load-bearing: the first propagates
        // and the second conflicts, so neither can be removed.
        let green = Plan::three_variable_refutation();
        for step in 0..green.steps.len() {
            let PlannedStep::Add(addition) = &green.steps[step] else {
                continue;
            };
            for position in 0..addition.hints.len() {
                let mut plan = green.clone();
                if let PlannedStep::Add(addition) = &mut plan.steps[step] {
                    addition.hints.remove(position);
                }
                assert!(
                    !verdict(&plan).is_verified(),
                    "step {step} survived losing antecedent {position}"
                );
            }
        }
    }

    #[test]
    fn every_antecedent_retarget_is_rejected() {
        // `Post`-style sweep for a refutation: repointing any single antecedent at any
        // other defined clause must break the chain, whichever step it sits in.
        let green = Plan::three_variable_refutation();
        for step in 0..green.steps.len() {
            let PlannedStep::Add(addition) = &green.steps[step] else {
                continue;
            };
            let original = addition.hints.clone();
            for (position, hint) in original.iter().enumerate() {
                for replacement in 1_u32..=15 {
                    if replacement == *hint {
                        continue;
                    }
                    let mut plan = green.clone();
                    if let PlannedStep::Add(addition) = &mut plan.steps[step] {
                        addition.hints[position] = replacement;
                    }
                    assert!(
                        !verdict(&plan).is_verified(),
                        "step {step} antecedent {position} accepted clause {replacement}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_antecedent_naming_an_undefined_clause_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.hints[0] = 99;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::HintOutOfRange {
                step: 0,
                position: 0,
                id: 99,
            }
        );
    }

    #[test]
    fn an_antecedent_naming_a_deleted_clause_is_rejected() {
        // Step 3 deletes clauses 1..4 and 9..10; step 4 must not reach back for one.
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[4] {
            addition.hints[0] = 1;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::HintDeleted {
                step: 4,
                position: 0,
                id: 1,
            }
        );
    }

    #[test]
    fn a_satisfied_antecedent_is_not_a_propagation() {
        // Clause 5 is `(-1 ∨ 2 ∨ 3)`; under the assumption `¬1, ¬2` of step 0 its
        // first literal is already true, so it proves nothing.
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.hints[0] = 5;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::HintSatisfied {
                step: 0,
                position: 0,
                id: 5,
            }
        );
    }

    #[test]
    fn an_antecedent_with_two_unassigned_literals_is_not_unit() {
        // Step 6 derives `(-1)`, assuming only `1`; clause 7 `(-1 ∨ -2 ∨ 3)` then has
        // two unassigned literals.
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[6] {
            addition.hints[0] = 7;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::HintNotUnit {
                step: 6,
                position: 0,
                id: 7,
                unassigned: 2,
            }
        );
    }

    #[test]
    fn a_chain_that_conflicts_early_is_rejected_as_a_second_encoding() {
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.hints = vec![1, 2, 2];
        }
        assert_eq!(
            rejection(&plan),
            Rejection::ConflictBeforeEnd {
                step: 0,
                position: 1
            }
        );
    }

    #[test]
    fn a_tautological_derivation_carries_no_evidence_and_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.clause = vec![-1, 1];
            addition.hints = Vec::new();
        }
        assert_eq!(rejection(&plan), Rejection::VacuousDerivation { step: 0 });
    }

    #[test]
    fn weakening_a_derived_clause_breaks_the_chain_that_used_it() {
        // Step 0 derives `(1 ∨ 2)`. Adding a literal weakens it, and step 2's chain —
        // which needs it to be unit under `¬1` — stops propagating.
        let mut plan = Plan::three_variable_refutation();
        if let PlannedStep::Add(addition) = &mut plan.steps[0] {
            addition.clause = vec![1, 2, 3];
        }
        assert!(!verdict(&plan).is_verified());
    }

    // --- adversarial: deletion --------------------------------------------

    #[test]
    fn deleting_an_undefined_clause_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps[3] = PlannedStep::Delete(vec![77]);
        assert_eq!(
            rejection(&plan),
            Rejection::DeletionOutOfRange {
                step: 3,
                position: 0,
                id: 77,
            }
        );
    }

    #[test]
    fn deleting_the_same_clause_twice_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps.insert(4, PlannedStep::Delete(vec![1]));
        assert_eq!(
            rejection(&plan),
            Rejection::DeletionRepeated {
                step: 4,
                position: 0,
                id: 1,
            }
        );
    }

    #[test]
    fn an_unordered_deletion_list_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps[3] = PlannedStep::Delete(vec![2, 1]);
        assert_eq!(
            rejection(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::Deletion,
                index: 1,
            }
        );
    }

    // --- adversarial: the refutation as a whole ----------------------------

    #[test]
    fn a_proof_that_never_reaches_the_empty_clause_refutes_nothing() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps.pop();
        assert_eq!(rejection(&plan), Rejection::EmptyClauseNotDerived);
    }

    #[test]
    fn a_proof_that_continues_past_the_empty_clause_is_rejected() {
        let mut plan = Plan::three_variable_refutation();
        plan.steps.push(PlannedStep::Delete(vec![5]));
        assert_eq!(
            rejection(&plan),
            Rejection::ProofContinuesAfterEmptyClause { step: 7 }
        );
    }

    #[test]
    fn dropping_any_input_clause_breaks_the_refutation() {
        // The eight clauses over three variables are jointly unsatisfiable and
        // individually necessary: remove one and the formula has a model, so no
        // refutation of it can survive.
        let green = Plan::three_variable_refutation();
        for index in 0..green.clauses.len() {
            let mut plan = green.clone();
            plan.clauses.remove(index);
            assert!(
                !verdict(&plan).is_verified(),
                "the refutation survived losing input clause {}",
                index + 1
            );
        }
    }

    #[test]
    fn flipping_any_input_literal_breaks_the_refutation() {
        // Every literal of the complete 3-variable CNF is load-bearing: flipping one
        // makes the formula satisfiable.
        let green = Plan::three_variable_refutation();
        for clause in 0..green.clauses.len() {
            for position in 0..green.clauses[clause].len() {
                let mut plan = green.clone();
                plan.clauses[clause][position] = -plan.clauses[clause][position];
                plan.clauses[clause].sort_by_key(|l| (l.unsigned_abs(), *l > 0));
                assert!(
                    !verdict(&plan).is_verified(),
                    "flipping literal {position} of clause {} was accepted",
                    clause + 1
                );
            }
        }
    }

    // --- adversarial: canonicity and garbage -------------------------------

    #[test]
    fn no_two_byte_strings_decode_to_the_same_certificate() {
        // Canonicity, swept exhaustively over single-byte corruptions: RFC 0005's
        // "rejection of duplicate/ambiguous encodings" means the decoder is injective,
        // so a mutation either fails to decode or decodes to a *different*
        // certificate. Nothing in the format is padding.
        let green = Plan::three_variable_refutation().encode();
        let decoded = crate::wire::decode(&green).expect("the green certificate decodes");
        for position in 0..green.len() {
            for delta in [0x01_u8, 0x40, 0xff] {
                let mut mutated = green.clone();
                mutated[position] ^= delta;
                if let Ok(other) = crate::wire::decode(&mutated) {
                    assert_ne!(
                        other, decoded,
                        "byte {position} xor {delta:#04x} decoded to the same certificate"
                    );
                }
            }
        }
    }

    #[test]
    fn garbage_bytes_never_panic() {
        // docs/12 §11 classes "malformed artifact panic in kernel" as a release
        // blocker and docs/16 PO-KER-001 states decoder totality as an obligation. A
        // deterministic xorshift stands in for a fuzzer so the corpus is reproducible
        // (INV-005).
        let mut source = Xorshift::new(0x5a71_2222_0f0f_1001);
        for length in 0..192_usize {
            for _ in 0..8 {
                let bytes = source.bytes(length);
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        // The same, but behind a valid header, so the garbage reaches the body
        // decoder rather than stopping at the magic.
        for length in 0..192_usize {
            for _ in 0..8 {
                let mut bytes = MAGIC.to_vec();
                bytes.extend_from_slice(&WIRE_EPOCH.to_be_bytes());
                bytes.extend_from_slice(&1_u16.to_be_bytes());
                bytes.extend_from_slice(&source.bytes(length));
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        // And spliced garbage inside an otherwise valid certificate.
        let green = Plan::three_variable_refutation().encode();
        assert!(
            green.len() < 8192,
            "the fixture stays small enough to sweep"
        );
        for _ in 0..512 {
            let mut bytes = green.clone();
            let at = source.next_u64() as usize % bytes.len();
            let len = source.next_u64() as usize % 24;
            let patch = source.bytes(len);
            for (offset, byte) in patch.iter().enumerate() {
                if let Some(slot) = bytes.get_mut(at.saturating_add(offset)) {
                    *slot = *byte;
                }
            }
            let _ = check_certificate(&bytes);
        }
    }
}
