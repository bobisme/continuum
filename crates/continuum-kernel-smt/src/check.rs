//! The SMT checker: a fully replayed propositional skeleton, honestly labelled (PR 9).
//!
//! # What is being re-derived, and what is being believed
//!
//! > Backend proofs can be Alethe/LRAT/solver-specific plus checked theory lemmas.
//! > Until theory-proof support is mature, the result is `TRUSTED_SOLVER`, not
//! > `CHECKED_CERTIFICATE`.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §6.2
//!
//! The refutation is checked exactly as a clausal proof is: for a derived clause `C`
//! with antecedent chain `h₁ … hₙ`, assume every literal of `C` false, require each
//! `hᵢ` to be *unit* under the running assignment and propagate it, and require `hₙ` to
//! be *falsified*. The proof is complete when a step derives the empty clause.
//!
//! What the antecedents are is where SMT differs from SAT. An antecedent identifier
//! falls in one of three ranges:
//!
//! - an **assertion** — a clause of the propositional skeleton the caller asserted;
//! - a **theory lemma** — a clause the producer says is valid in a named theory;
//! - a **derived clause** — something this checker already accepted.
//!
//! The first and third are free: they are part of the problem, or they were re-derived
//! here. The second is not. The kernel has no arithmetic, no congruence closure, and no
//! decision procedure; it cannot tell a valid lemma from an invalid one, and it does not
//! pretend to. So it does the one honest thing available: it records *which theories the
//! refutation actually reached for*, and reports them.
//!
//! Hence the two outcomes of a verified certificate:
//!
//! - no lemma was used → [`crate::AssuranceClass::CheckedCertificate`]. A propositionally
//!   unsatisfiable set of assertions is unsatisfiable under every theory, so there is
//!   nothing left to trust beyond the encoding.
//! - some lemma was used → [`crate::AssuranceClass::TrustedSolver`], with every used theory in
//!   [`CheckedClaim::trusted_components`]. This is ADR-0017's rule verbatim, and
//!   RFC 0005's "Proof-producing solver policy" preference order sitting at level 3
//!   ("one pinned solver with explicit solver-trusted claim") rather than level 1.
//!
//! # Why "used", and why the over-approximation
//!
//! A lemma is counted as used when an accepted step names it as an antecedent. That is
//! deliberately coarser than tracing the dependency cone of the empty clause: a step
//! whose conclusion never feeds `⊥` still contributes its theories to the trusted set.
//! The error is in the safe direction — the report can name a theory the proof did not
//! strictly need, never omit one it did — and it keeps the accounting a single forward
//! pass with no reachability analysis inside the trusted base.
//!
//! A lemma the proof never mentions at all contributes nothing, which is why a producer
//! cannot inflate a certificate's trusted set by attaching unused lemmas, and cannot
//! shrink it by hiding used ones.
//!
//! # Order of obligations
//!
//! 1. decode (wire form; see [`crate::wire`]);
//! 2. per step, in derivation order: a resolution's clause is not a tautology and its
//!    chain propagates to a conflict exactly at its last antecedent; a deletion names
//!    only defined, still-active clauses;
//! 3. the empty clause is derived, and nothing follows it;
//! 4. the used-theory set determines the assurance class.
//!
//! # Determinism
//!
//! No clock, no randomness, no socket, and iteration only over sequences whose order is
//! a decode-time invariant (INV-005; ADR-0003; docs/12 §1). The theory set is a
//! `BTreeSet`, so it is reported in one fixed order on every platform.

use std::collections::BTreeSet;

use crate::verdict::{CertificateKind, CheckedClaim, Rejection, Verdict};
use crate::wire::{Body, Clause, DecodeFailure, Envelope, SmtProofBody, Step, Token, decode};

/// Check one SMT certificate from its wire form.
///
/// The crate's whole public checking surface. It takes bytes — never a structure a
/// solver adapter could hand over — and returns the closed [`Verdict`] vocabulary. A
/// `Verified` verdict is not the end of the story: read
/// [`CheckedClaim::assurance_class`] and [`CheckedClaim::trusted_components`] before
/// rendering it.
#[must_use]
pub fn check_certificate(bytes: &[u8]) -> Verdict {
    let certificate = match decode(bytes) {
        Ok(certificate) => certificate,
        Err(DecodeFailure::Rejected(rejection)) => return Verdict::Rejected(rejection),
        Err(DecodeFailure::Unsupported(feature)) => return Verdict::Unsupported(feature),
    };
    match certificate.body() {
        Body::SmtProof(body) => check_smt_proof(certificate.envelope(), body),
    }
}

// ---------------------------------------------------------------------------
// the clause store
// ---------------------------------------------------------------------------

/// The clauses currently available to a propagation chain, by identifier.
///
/// Flat storage with a strictly ascending identifier column, so lookup is a binary
/// search over the identifiers themselves. Deletion clears a flag; nothing is moved, so
/// an identifier's meaning never changes.
#[derive(Debug, Default)]
struct ClauseStore {
    ids: Vec<u32>,
    starts: Vec<u32>,
    lengths: Vec<u32>,
    active: Vec<bool>,
    literals: Vec<i32>,
}

impl ClauseStore {
    fn push(&mut self, id: u32, literals: &[i32]) {
        self.ids.push(id);
        self.starts.push(narrow_u32(self.literals.len()));
        self.lengths.push(narrow_u32(literals.len()));
        self.active.push(true);
        self.literals.extend_from_slice(literals);
    }

    fn find(&self, id: u32) -> Option<usize> {
        self.ids.binary_search(&id).ok()
    }

    fn is_active(&self, slot: usize) -> bool {
        self.active.get(slot).copied().unwrap_or(false)
    }

    fn deactivate(&mut self, slot: usize) {
        if let Some(flag) = self.active.get_mut(slot) {
            *flag = false;
        }
    }

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

/// An atom is not assigned.
const UNASSIGNED: u8 = 0;
/// An atom is assigned true.
const TRUE: u8 = 1;
/// An atom is assigned false.
const FALSE: u8 = 2;

/// The partial assignment one propagation chain runs under.
///
/// Sized from the largest atom index the certificate's clauses actually mention, never
/// from the declared `atom_count`: a declared count is not allowed to reserve memory
/// before the bytes that justify it have been read (RFC 0005 "Resource bounds";
/// docs/16 PO-KER-002).
#[derive(Debug)]
struct Assignment {
    value: Vec<u8>,
    trail: Vec<u32>,
}

impl Assignment {
    fn new(atoms: u32) -> Self {
        let slots = (atoms as usize).saturating_add(1);
        Self {
            value: vec![UNASSIGNED; slots],
            trail: Vec::new(),
        }
    }

    fn reset(&mut self) {
        for atom in self.trail.drain(..) {
            if let Some(slot) = self.value.get_mut(atom as usize) {
                *slot = UNASSIGNED;
            }
        }
    }

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

    fn satisfy(&mut self, literal: i32) {
        let atom = literal.unsigned_abs();
        if let Some(slot) = self.value.get_mut(atom as usize) {
            *slot = if literal > 0 { TRUE } else { FALSE };
            self.trail.push(atom);
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

/// What the checker learns while replaying one resolution step.
#[derive(Debug, Default)]
struct StepEffect {
    propagations: u64,
    lemmas: Vec<u32>,
}

/// Re-derive one resolution step.
fn check_resolution(
    store: &ClauseStore,
    assignment: &mut Assignment,
    lemma_range: (u32, u32),
    step: u32,
    clause: &Clause,
    hints: &[u32],
) -> Result<StepEffect, Rejection> {
    if clause.is_tautology() {
        return Err(Rejection::VacuousDerivation { step });
    }
    assignment.reset();
    for literal in clause.literals() {
        // Assume `C` false. The clause is not a tautology and its literals are
        // strictly ascending, so no two assumptions touch the same atom.
        assignment.satisfy(literal.wrapping_neg());
    }

    let (first_lemma, last_lemma) = lemma_range;
    let last = hints.len().saturating_sub(1);
    let mut effect = StepEffect::default();
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
        if *id >= first_lemma && *id <= last_lemma {
            effect.lemmas.push(*id);
        }
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
                effect.propagations = effect.propagations.saturating_add(1);
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
        Ok(effect)
    } else {
        Err(Rejection::NoConflict { step })
    }
}

/// Replay a whole refutation and classify what it leaned on.
fn check_smt_proof(envelope: &Envelope, body: &SmtProofBody) -> Verdict {
    let atoms = narrow_u32(body.atoms().len());
    let assertion_count = narrow_u32(body.assertions().len());
    let lemma_count = narrow_u32(body.lemmas().len());

    let mut store = ClauseStore::default();
    for (index, clause) in body.assertions().iter().enumerate() {
        store.push(narrow_u32(index).saturating_add(1), clause.literals());
    }
    for (index, lemma) in body.lemmas().iter().enumerate() {
        let id = assertion_count
            .saturating_add(narrow_u32(index))
            .saturating_add(1);
        store.push(id, lemma.clause().literals());
    }
    // Half-open in spirit, inclusive in form: the empty-lemma case gives a range that
    // no identifier can fall inside, which is exactly the intent.
    let lemma_range = (
        assertion_count.saturating_add(1),
        assertion_count.saturating_add(lemma_count),
    );

    let mut assignment = Assignment::new(observed_atoms(body));
    let mut derived: u32 = 0;
    let mut deleted: u32 = 0;
    let mut propagations: u64 = 0;
    let mut used_lemmas: BTreeSet<u32> = BTreeSet::new();
    let mut refuted_at: Option<u32> = None;

    for (index, step) in body.steps().iter().enumerate() {
        if let Some(step) = refuted_at {
            return Verdict::Rejected(Rejection::ProofContinuesAfterEmptyClause { step });
        }
        let step_index = narrow_u32(index);
        match step {
            Step::Resolve(resolution) => {
                match check_resolution(
                    &store,
                    &mut assignment,
                    lemma_range,
                    step_index,
                    resolution.clause(),
                    resolution.hints(),
                ) {
                    Ok(effect) => {
                        propagations = propagations.saturating_add(effect.propagations);
                        used_lemmas.extend(effect.lemmas);
                    }
                    Err(rejection) => return Verdict::Rejected(rejection),
                }
                store.push(resolution.id(), resolution.clause().literals());
                derived = derived.saturating_add(1);
                if resolution.clause().is_empty() {
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

    let mut theories: BTreeSet<Token> = BTreeSet::new();
    for id in &used_lemmas {
        if let Some(index) = id
            .checked_sub(assertion_count)
            .and_then(|offset| offset.checked_sub(1))
            .and_then(|index| usize::try_from(index).ok())
            && let Some(lemma) = body.lemmas().get(index)
        {
            theories.insert(lemma.theory().clone());
        }
    }

    Verdict::Verified(CheckedClaim::new(
        CertificateKind::SmtProof,
        envelope.clone(),
        atoms,
        assertion_count,
        lemma_count,
        narrow_u32(used_lemmas.len()),
        derived,
        deleted,
        propagations,
        theories.into_iter().collect(),
    ))
}

/// The largest atom index any carried clause mentions.
fn observed_atoms(body: &SmtProofBody) -> u32 {
    let mut largest: u32 = 0;
    for clause in body.assertions() {
        largest = largest.max(largest_atom(clause));
    }
    for lemma in body.lemmas() {
        largest = largest.max(largest_atom(lemma.clause()));
    }
    for step in body.steps() {
        if let Step::Resolve(resolution) = step {
            largest = largest.max(largest_atom(resolution.clause()));
        }
    }
    largest
}

/// The largest atom index one clause mentions.
fn largest_atom(clause: &Clause) -> u32 {
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
    use crate::verdict::{AssuranceClass, Feature, Field, TokenFault};
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

    fn theories(claim: &CheckedClaim) -> Vec<&str> {
        claim.theories().iter().map(Token::as_str).collect()
    }

    // --- green path: the two assurance classes -----------------------------

    #[test]
    fn a_theory_backed_refutation_is_verified_and_solver_trusted() {
        let plan = Plan::ordering_and_congruence();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the hand-checked refutation must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::SmtProof);
        assert_eq!(claim.atoms(), 4);
        assert_eq!(claim.assertions(), 3);
        assert_eq!(claim.lemmas_carried(), 2);
        assert_eq!(claim.lemmas_used(), 2);
        assert_eq!(claim.derived_clauses(), 3);
        assert_eq!(claim.propagations(), 4);
        assert_eq!(theories(&claim), ["LIA", "UF"]);
        assert_eq!(claim.assurance_class(), AssuranceClass::TrustedSolver);
        assert_eq!(
            claim.trusted_components(),
            [
                "envelope-digest-binding",
                "skeleton-model-correspondence",
                "theory-lemma:LIA",
                "theory-lemma:UF",
            ],
            "a theory-backed proof must name every theory it leaned on"
        );
    }

    #[test]
    fn a_purely_propositional_refutation_is_a_checked_certificate() {
        // No lemma is used, so nothing about any theory needs to be believed: an
        // unsatisfiable skeleton is unsatisfiable under every theory.
        let plan = Plan::propositional_conflict();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the propositional conflict must check green");
        };
        assert_eq!(claim.lemmas_carried(), 0);
        assert_eq!(claim.lemmas_used(), 0);
        assert!(claim.theories().is_empty());
        assert_eq!(claim.assurance_class(), AssuranceClass::CheckedCertificate);
        assert_eq!(
            claim.trusted_components(),
            ["envelope-digest-binding", "skeleton-model-correspondence"]
        );
    }

    #[test]
    fn a_single_lemma_proof_is_verified_and_maximally_trusted() {
        // The degenerate honest case: the solver hands over its answer as one theory
        // lemma. That is a *valid* artifact of this format and it verifies — but it
        // verifies as TRUSTED_SOLVER with the theory named, which is exactly the
        // distinction ADR-0017 exists to keep visible.
        let plan = Plan::solver_says_so();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("a one-lemma certificate is well formed");
        };
        assert_eq!(claim.lemmas_used(), 1);
        assert_eq!(theories(&claim), ["QF_UFLIA"]);
        assert_eq!(claim.assurance_class(), AssuranceClass::TrustedSolver);
    }

    #[test]
    fn an_unused_lemma_does_not_enter_the_trusted_set() {
        // A producer cannot inflate the trusted set by attaching lemmas the proof
        // never mentions.
        let mut plan = Plan::propositional_conflict();
        plan.lemmas.push(("NRA".to_owned(), vec![-1, 2]));
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("an unused lemma is dead weight, not a malformed certificate");
        };
        assert_eq!(claim.lemmas_carried(), 1);
        assert_eq!(claim.lemmas_used(), 0);
        assert_eq!(claim.assurance_class(), AssuranceClass::CheckedCertificate);
    }

    #[test]
    fn checking_is_deterministic_over_the_same_bytes() {
        let bytes = Plan::ordering_and_congruence().encode();
        assert_eq!(check_certificate(&bytes), check_certificate(&bytes));
        assert!(check_certificate(&bytes).is_verified());
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
        let bytes = Plan::ordering_and_congruence().encode();
        for cut in 0..bytes.len() {
            assert!(
                !check_certificate(&bytes[..cut]).is_verified(),
                "a {cut}-byte prefix was accepted as a certificate"
            );
        }
    }

    #[test]
    fn a_sibling_kernels_magic_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.magic = *b"CONTSATC";
        assert_eq!(rejection(&plan), Rejection::BadMagic);
    }

    #[test]
    fn a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected() {
        let mut plan = Plan::ordering_and_congruence();
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
        let mut plan = Plan::ordering_and_congruence();
        plan.kind = 7;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::CertificateKind { found: 7 })
        );
    }

    #[test]
    fn an_alethe_theory_step_is_unsupported_never_rejected() {
        // The honesty boundary: a proof carrying real Alethe theory rules is a valid
        // proof this build cannot replay (INV-008).
        let mut plan = Plan::ordering_and_congruence();
        plan.steps.insert(0, PlannedStep::RawKind(5));
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::ProofStep { found: 5 })
        );
    }

    #[test]
    fn trailing_bytes_and_oversized_inputs_are_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.trailing = vec![0x00, 0x01, 0x02];
        assert_eq!(rejection(&plan), Rejection::TrailingBytes { extra: 3 });

        let bytes = vec![0_u8; crate::wire::MAX_CERTIFICATE_BYTES + 1];
        assert_eq!(
            check_certificate(&bytes),
            Verdict::Rejected(Rejection::Oversized {
                found: crate::wire::MAX_CERTIFICATE_BYTES + 1
            })
        );
    }

    // --- adversarial: envelope and atoms -----------------------------------

    #[test]
    fn an_envelope_that_names_a_different_schema_epoch_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.schema_epoch = WIRE_EPOCH.saturating_add(2);
        assert_eq!(
            rejection(&plan),
            Rejection::SchemaEpochMismatch {
                declared: WIRE_EPOCH + 2,
                header: WIRE_EPOCH,
            }
        );
    }

    #[test]
    fn malformed_tokens_are_rejected_by_field() {
        let mut digest = Plan::ordering_and_congruence();
        digest.model_digest = String::new();
        assert_eq!(
            rejection(&digest),
            Rejection::MalformedToken {
                field: Field::ModelDigest,
                fault: TokenFault::Empty,
            }
        );

        let mut atom = Plan::ordering_and_congruence();
        atom.atoms[0] = "f x = f y".to_owned();
        assert!(matches!(
            rejection(&atom),
            Rejection::MalformedToken {
                field: Field::Atom,
                ..
            }
        ));

        let mut theory = Plan::ordering_and_congruence();
        theory.lemmas[0].0 = "L I A".to_owned();
        assert!(matches!(
            rejection(&theory),
            Rejection::MalformedToken {
                field: Field::LemmaTheory,
                ..
            }
        ));

        let mut long = Plan::ordering_and_congruence();
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
    }

    #[test]
    fn an_unordered_or_duplicated_atom_table_is_rejected() {
        let mut unordered = Plan::ordering_and_congruence();
        unordered.atoms.swap(0, 1);
        assert_eq!(
            rejection(&unordered),
            Rejection::NotStrictlyAscending {
                field: Field::Atom,
                index: 1,
            }
        );

        // Two spellings of one atom would be two atoms; one spelling twice would be
        // one atom with two indices. Both are unrepresentable (ADR-0013).
        let mut duplicated = Plan::ordering_and_congruence();
        duplicated.atoms[1] = duplicated.atoms[0].clone();
        assert_eq!(
            rejection(&duplicated),
            Rejection::NotStrictlyAscending {
                field: Field::Atom,
                index: 1,
            }
        );
    }

    #[test]
    fn a_literal_outside_the_atom_table_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.assertions[0] = vec![9];
        assert_eq!(
            rejection(&plan),
            Rejection::AtomOutOfRange {
                clause: 1,
                position: 0,
                literal: 9,
                atoms: 4,
            }
        );
    }

    #[test]
    fn counts_outside_the_declared_range_are_rejected() {
        let mut no_atoms = Plan::ordering_and_congruence();
        no_atoms.atoms.clear();
        assert_eq!(
            rejection(&no_atoms),
            Rejection::CountOutOfRange {
                field: Field::AtomCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_ATOMS),
            }
        );

        let mut no_assertions = Plan::ordering_and_congruence();
        no_assertions.assertions.clear();
        assert_eq!(
            rejection(&no_assertions),
            Rejection::CountOutOfRange {
                field: Field::AssertionCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_ASSERTIONS),
            }
        );

        let mut huge = Plan::ordering_and_congruence();
        huge.declared_atom_count = Some(u32::MAX);
        assert_eq!(
            rejection(&huge),
            Rejection::CountOutOfRange {
                field: Field::AtomCount,
                found: u64::from(u32::MAX),
                min: 1,
                max: u64::from(crate::wire::MAX_ATOMS),
            }
        );
    }

    // --- adversarial: the lemma boundary -----------------------------------

    #[test]
    fn a_tautological_theory_lemma_is_rejected() {
        // A lemma that is a propositional tautology needs no theory, so labelling it
        // with one would enlarge the trusted set for nothing.
        let mut plan = Plan::ordering_and_congruence();
        plan.lemmas[0].1 = vec![-3, 3];
        assert_eq!(rejection(&plan), Rejection::VacuousLemma { lemma: 0 });
    }

    #[test]
    fn relabelling_a_lemma_changes_the_trusted_set_and_nothing_else() {
        // Stated as a test so the boundary cannot drift silently: the theory name is
        // a *label*. The kernel cannot tell `LIA` from `NRA`, and it does not pretend
        // to — it reports what the artifact said.
        let mut plan = Plan::ordering_and_congruence();
        plan.lemmas[0].0 = "NRA".to_owned();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("a theory relabel is not a malformed certificate");
        };
        assert_eq!(theories(&claim), ["NRA", "UF"]);
        assert_eq!(claim.assurance_class(), AssuranceClass::TrustedSolver);
    }

    #[test]
    fn dropping_a_lemma_the_proof_uses_breaks_the_proof() {
        let green = Plan::ordering_and_congruence();
        for index in 0..green.lemmas.len() {
            let mut plan = green.clone();
            plan.lemmas.remove(index);
            assert!(
                !verdict(&plan).is_verified(),
                "the refutation survived losing lemma {index}"
            );
        }
    }

    // --- adversarial: the propagation obligation ---------------------------

    #[test]
    fn a_chain_that_never_conflicts_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        if let PlannedStep::Resolve(resolution) = &mut plan.steps[0] {
            resolution.hints.pop();
        }
        assert_eq!(rejection(&plan), Rejection::NoConflict { step: 0 });
    }

    #[test]
    fn dropping_any_antecedent_breaks_its_step() {
        let green = Plan::ordering_and_congruence();
        for step in 0..green.steps.len() {
            let PlannedStep::Resolve(resolution) = &green.steps[step] else {
                continue;
            };
            for position in 0..resolution.hints.len() {
                let mut plan = green.clone();
                if let PlannedStep::Resolve(resolution) = &mut plan.steps[step] {
                    resolution.hints.remove(position);
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
        let green = Plan::ordering_and_congruence();
        for step in 0..green.steps.len() {
            let PlannedStep::Resolve(resolution) = &green.steps[step] else {
                continue;
            };
            let original = resolution.hints.clone();
            for (position, hint) in original.iter().enumerate() {
                for replacement in 1_u32..=8 {
                    if replacement == *hint {
                        continue;
                    }
                    let mut plan = green.clone();
                    if let PlannedStep::Resolve(resolution) = &mut plan.steps[step] {
                        resolution.hints[position] = replacement;
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
        let mut plan = Plan::ordering_and_congruence();
        if let PlannedStep::Resolve(resolution) = &mut plan.steps[0] {
            resolution.hints[0] = 42;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::HintOutOfRange {
                step: 0,
                position: 0,
                id: 42,
            }
        );
    }

    #[test]
    fn a_reused_clause_identifier_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        if let PlannedStep::Resolve(resolution) = &mut plan.steps[1] {
            resolution.id = 6;
        }
        assert_eq!(
            rejection(&plan),
            Rejection::IdNotIncreasing {
                step: 1,
                found: 6,
                previous: 6,
            }
        );
    }

    #[test]
    fn a_tautological_derivation_carries_no_evidence_and_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        if let PlannedStep::Resolve(resolution) = &mut plan.steps[0] {
            resolution.clause = vec![-1, 1];
            resolution.hints = Vec::new();
        }
        assert_eq!(rejection(&plan), Rejection::VacuousDerivation { step: 0 });
    }

    #[test]
    fn a_proof_that_never_reaches_the_empty_clause_refutes_nothing() {
        let mut plan = Plan::ordering_and_congruence();
        plan.steps.pop();
        assert_eq!(rejection(&plan), Rejection::EmptyClauseNotDerived);
    }

    #[test]
    fn a_proof_that_continues_past_the_empty_clause_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.steps.push(PlannedStep::Delete(vec![1]));
        assert_eq!(
            rejection(&plan),
            Rejection::ProofContinuesAfterEmptyClause { step: 2 }
        );
    }

    #[test]
    fn deletion_is_checked_against_the_active_set() {
        let mut undefined = Plan::ordering_and_congruence();
        undefined.steps.insert(0, PlannedStep::Delete(vec![99]));
        assert_eq!(
            rejection(&undefined),
            Rejection::DeletionOutOfRange {
                step: 0,
                position: 0,
                id: 99,
            }
        );

        let mut twice = Plan::ordering_and_congruence();
        twice.steps.insert(0, PlannedStep::Delete(vec![3]));
        twice.steps.insert(1, PlannedStep::Delete(vec![3]));
        assert_eq!(
            rejection(&twice),
            Rejection::DeletionRepeated {
                step: 1,
                position: 0,
                id: 3,
            }
        );

        let mut unordered = Plan::ordering_and_congruence();
        unordered.steps.insert(0, PlannedStep::Delete(vec![3, 1]));
        assert_eq!(
            rejection(&unordered),
            Rejection::NotStrictlyAscending {
                field: Field::Deletion,
                index: 1,
            }
        );
    }

    #[test]
    fn deleting_a_clause_a_later_step_needs_is_rejected() {
        let mut plan = Plan::ordering_and_congruence();
        plan.steps.insert(0, PlannedStep::Delete(vec![1]));
        assert_eq!(
            rejection(&plan),
            Rejection::HintDeleted {
                step: 1,
                position: 0,
                id: 1,
            }
        );
    }

    // --- adversarial: canonicity and garbage -------------------------------

    #[test]
    fn no_two_byte_strings_decode_to_the_same_certificate() {
        let green = Plan::ordering_and_congruence().encode();
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
        let mut source = Xorshift::new(0x1337_beef_0bad_c0de);
        for length in 0..192_usize {
            for _ in 0..8 {
                let bytes = source.bytes(length);
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        for length in 0..192_usize {
            for _ in 0..8 {
                let mut bytes = MAGIC.to_vec();
                bytes.extend_from_slice(&WIRE_EPOCH.to_be_bytes());
                bytes.extend_from_slice(&1_u16.to_be_bytes());
                bytes.extend_from_slice(&source.bytes(length));
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        let green = Plan::ordering_and_congruence().encode();
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
