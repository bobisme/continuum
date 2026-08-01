//! The certificate checkers: finite closure and state typing (PR 9, IMPL-01).
//!
//! # What is being re-derived
//!
//! > For finite safety: `Init ⊆ S`, `Post(S) ⊆ S`, `S ⊆ P`.
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Closed
//! >   reachable set"
//!
//! The formal statement of the same three obligations, and the proof that they are
//! sufficient, is `lean/Continuum/Certificate.lean`:
//!
//! ```text
//! structure ClosureCertificate (sys) (member) (safe) : Prop where
//!   containsInit : ∀ s, sys.init s → member s
//!   closed       : ∀ s t, member s → sys.step s t → member t
//!   safeOnMember : ∀ s, member s → safe s
//! ```
//!
//! with `ClosureCertificate.provesSafety` deriving `∀ s, sys.Reachable s → safe s`
//! (RFC 0012 rung T0, "finite closure certificate soundness"). [`check_certificate`]
//! is the executable presentation of the same three conjuncts against the
//! certificate's own carried data, in the same order Lean's
//! `FiniteSystem.checkClosure` evaluates them.
//!
//! # What "self-contained" costs, stated honestly
//!
//! docs/03 §6.1 lists a checker step this crate cannot perform on a self-contained
//! artifact: "recompute every enabled transition". Recomputing transitions requires
//! the *model*, and a kernel that read the model would need a transition-relation
//! evaluator — which is the one thing plan §20 forbids it from sharing with an
//! engine, and which would put the model's elaborator inside the trusted base.
//!
//! So the split is drawn where the dossier draws it. The certificate carries its own
//! successor relation; the kernel re-derives every obligation *over* that relation
//! and never trusts the producer's own summary of it. What remains trusted is the
//! correspondence between the carried relation and the model named by
//! `Envelope::model_digest`, and that is reported, not hidden, by
//! [`CheckedClaim::trusted_components`]. RFC 0005 puts the envelope hashes in the
//! certificate for exactly this reason, and RFC 0024's receipt is where the binding
//! is recorded.
//!
//! Nothing about that weakens the closure obligation itself. A producer that stops
//! exploring early cannot hide it: successors are carried as state *vectors*, so an
//! unexplored successor is a vector absent from the table and
//! [`Rejection::ClosureFailure`] follows.
//!
//! # Order of obligations
//!
//! Fixed, so a diagnostic is reproducible and a mutation suite can name the
//! rejection a given mutation produces:
//!
//! 1. decode (wire form; see [`crate::wire`]);
//! 2. property class supported, else [`Feature::PropertyClass`];
//! 3. `S ⊆ P` — every table state satisfies the declared state domain;
//! 4. `Init ⊆ S` — every declared initial state is in the table;
//! 5. `Post(S) ⊆ S` — every transition names a declared action and lands in the
//!    table.
//!
//! # Determinism
//!
//! The checker reads no clock, draws no randomness, opens no socket, and iterates
//! only over sequences whose order is a decode-time invariant (INV-005; ADR-0003;
//! docs/12 §1). Two runs over the same bytes produce byte-identical verdicts on any
//! platform.

use crate::verdict::{CertificateKind, CheckedClaim, Feature, PropertyClass, Rejection, Verdict};
use crate::wire::{
    Body, DecodeFailure, Envelope, FiniteClosureBody, StateDomain, StateTable, StateTypeBody,
    decode,
};

/// Check one certificate from its wire form.
///
/// The crate's whole public checking surface. It takes bytes — never a structure an
/// engine could hand over — and returns the closed [`Verdict`] vocabulary. It cannot
/// panic on any input: see the module documentation of [`crate::wire`] and the
/// `garbage_bytes_never_panic` test.
#[must_use]
pub fn check_certificate(bytes: &[u8]) -> Verdict {
    let certificate = match decode(bytes) {
        Ok(certificate) => certificate,
        Err(DecodeFailure::Rejected(rejection)) => return Verdict::Rejected(rejection),
        Err(DecodeFailure::Unsupported(feature)) => return Verdict::Unsupported(feature),
    };
    match certificate.body() {
        Body::FiniteClosure(body) => check_finite_closure(certificate.envelope(), body),
        Body::StateType(body) => check_state_type(certificate.envelope(), body),
    }
}

/// `Init ⊆ S`, `Post(S) ⊆ S`, `S ⊆ P` over the certificate's own carried relation.
fn check_finite_closure(envelope: &Envelope, body: &FiniteClosureBody) -> Verdict {
    let class_code = body.property_class_code();
    let Some(property) = PropertyClass::from_code(class_code) else {
        return Verdict::Unsupported(Feature::PropertyClass { found: class_code });
    };

    // S ⊆ P.
    if let Some(rejection) = violated_state(body.domain(), body.table()) {
        return Verdict::Rejected(rejection);
    }

    // Init ⊆ S — `ClosureCertificate.containsInit`.
    let mut initial_states: u32 = 0;
    for (position, state) in body.initial_states().iter().enumerate() {
        if body.table().position(state).is_none() {
            return Verdict::Rejected(Rejection::InitialStateNotInTable {
                position: narrow_u32(position),
            });
        }
        initial_states = initial_states.saturating_add(1);
    }

    // Post(S) ⊆ S — `ClosureCertificate.closed`. Row `index` is the successor row of
    // table state `index`; the decoder reads exactly one row per state, so every
    // member of the certificate's own state set carries its complete successor set
    // and an empty row is a declared deadlock, not an omission.
    let action_count = narrow_u32(body.actions().len());
    let mut transitions: u64 = 0;
    for (state_index, row) in body.rows().iter().enumerate() {
        let state = narrow_u32(state_index);
        for (entry_index, transition) in row.iter().enumerate() {
            let entry = narrow_u32(entry_index);
            if u32::from(transition.action()) >= action_count {
                return Verdict::Rejected(Rejection::UnknownAction {
                    state,
                    entry,
                    action: transition.action(),
                });
            }
            if body.table().position(transition.target()).is_none() {
                return Verdict::Rejected(Rejection::ClosureFailure { state, entry });
            }
            transitions = transitions.saturating_add(1);
        }
    }

    Verdict::Verified(CheckedClaim::new(
        CertificateKind::FiniteClosure,
        property,
        envelope.clone(),
        body.table().len(),
        initial_states,
        transitions,
    ))
}

/// Every state in the table lies in the declared state domain (docs/16 PO-MOD-003).
fn check_state_type(envelope: &Envelope, body: &StateTypeBody) -> Verdict {
    if let Some(rejection) = violated_state(body.domain(), body.table()) {
        return Verdict::Rejected(rejection);
    }
    Verdict::Verified(CheckedClaim::new(
        CertificateKind::StateType,
        PropertyClass::StateDomain,
        envelope.clone(),
        body.table().len(),
        0,
        0,
    ))
}

/// The first state that leaves the declared domain, as a rejection, or `None`.
///
/// A state vector shorter than the declared arity is unrepresentable: the decoder
/// reads exactly `arity` components per state. The `else` arm below is therefore
/// unreachable in practice and exists so that no future change to the decoder can
/// turn a shape mismatch into an index panic.
fn violated_state(domain: &StateDomain, table: &StateTable) -> Option<Rejection> {
    for index in 0..table.len() {
        let state = table.state(index)?;
        for (position, variable) in domain.variables().iter().enumerate() {
            let Some(value) = state.get(position) else {
                return Some(Rejection::PropertyViolated {
                    state: index,
                    variable: narrow_u16(position),
                    value: 0,
                });
            };
            if !variable.admits(*value) {
                return Some(Rejection::PropertyViolated {
                    state: index,
                    variable: narrow_u16(position),
                    value: *value,
                });
            }
        }
    }
    None
}

/// Saturating `usize -> u32`, for diagnostic indices only.
///
/// Every count this is applied to is bounded far below `u32::MAX` by the wire
/// format's declared limits, so saturation is unreachable; it exists so that the
/// conversion has no failure mode at all.
const fn narrow_u32(value: usize) -> u32 {
    if value > u32::MAX as usize {
        u32::MAX
    } else {
        value as u32
    }
}

/// Saturating `usize -> u16`, for diagnostic indices only. See [`narrow_u32`].
const fn narrow_u16(value: usize) -> u16 {
    if value > u16::MAX as usize {
        u16::MAX
    } else {
        value as u16
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
    use crate::fixture::{Plan, Xorshift};
    use crate::verdict::{Field, TokenFault};
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
    fn diehard_finite_closure_certificate_is_verified() {
        let plan = Plan::diehard_closure();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the Die Hard closure certificate must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::FiniteClosure);
        assert_eq!(claim.property(), PropertyClass::StateDomain);
        // The frozen spike facts of notes/plan/corpus/tla-examples/ports/TV-009:
        // 16 reachable states and 96 labelled successor edges.
        assert_eq!(claim.states(), 16);
        assert_eq!(claim.transitions(), 96);
        assert_eq!(claim.initial_states(), 1);
        assert_eq!(
            claim.envelope().model_digest().as_str(),
            "blake3:diehard-model"
        );
        assert_eq!(
            claim.trusted_components(),
            [
                "certificate-model-correspondence",
                "envelope-digest-binding"
            ]
        );
    }

    #[test]
    fn diehard_state_type_certificate_is_verified() {
        let plan = Plan::diehard_type();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the Die Hard TypeOK certificate must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::StateType);
        assert_eq!(claim.states(), 16);
        assert_eq!(claim.transitions(), 0);
        assert_eq!(claim.initial_states(), 0);
    }

    #[test]
    fn checking_is_deterministic_over_the_same_bytes() {
        let bytes = Plan::diehard_closure().encode();
        let first = check_certificate(&bytes);
        let second = check_certificate(&bytes);
        assert_eq!(first, second);
        assert!(first.is_verified());
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
        let bytes = Plan::diehard_closure().encode();
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
        let mut plan = Plan::diehard_closure();
        plan.magic = *b"NOTACERT";
        assert_eq!(rejection(&plan), Rejection::BadMagic);
    }

    #[test]
    fn a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected() {
        // "Wrong version" is the case INV-008 exists for: the artifact may be
        // perfectly valid under a contract this build has never seen.
        let mut plan = Plan::diehard_closure();
        plan.wire_epoch = WIRE_EPOCH.saturating_add(1);
        plan.schema_epoch = plan.wire_epoch;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::WireEpoch {
                found: WIRE_EPOCH + 1
            })
        );

        let mut older = Plan::diehard_closure();
        older.wire_epoch = 0;
        older.schema_epoch = 0;
        assert_eq!(
            verdict(&older),
            Verdict::Unsupported(Feature::WireEpoch { found: 0 })
        );
    }

    #[test]
    fn an_unimplemented_certificate_family_is_unsupported() {
        let mut plan = Plan::diehard_closure();
        plan.kind = 9;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::CertificateKind { found: 9 })
        );
    }

    #[test]
    fn an_unimplemented_property_class_is_unsupported() {
        let mut plan = Plan::diehard_closure();
        plan.property_class = 7;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::PropertyClass { found: 7 })
        );
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let mut plan = Plan::diehard_closure();
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

    // --- adversarial: internal inconsistency ------------------------------

    #[test]
    fn an_envelope_that_names_a_different_schema_epoch_is_rejected() {
        let mut plan = Plan::diehard_closure();
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
        let mut empty = Plan::diehard_closure();
        empty.model_digest = String::new();
        assert_eq!(
            rejection(&empty),
            Rejection::MalformedToken {
                field: Field::ModelDigest,
                fault: TokenFault::Empty,
            }
        );

        let mut long = Plan::diehard_closure();
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

        let mut space = Plan::diehard_closure();
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
    fn unordered_sequences_are_rejected_as_ambiguous_encodings() {
        let mut packs = Plan::diehard_closure();
        packs.domain_packs = vec!["blake3:b".to_owned(), "blake3:a".to_owned()];
        assert_eq!(
            rejection(&packs),
            Rejection::NotStrictlyAscending {
                field: Field::DomainPack,
                index: 1,
            }
        );

        let mut duplicate_pack = Plan::diehard_closure();
        duplicate_pack.domain_packs = vec!["blake3:a".to_owned(), "blake3:a".to_owned()];
        assert_eq!(
            rejection(&duplicate_pack),
            Rejection::NotStrictlyAscending {
                field: Field::DomainPack,
                index: 1,
            }
        );

        let mut variables = Plan::diehard_closure();
        variables.variables.reverse();
        assert_eq!(
            rejection(&variables),
            Rejection::NotStrictlyAscending {
                field: Field::VariableName,
                index: 1,
            }
        );

        let mut states = Plan::diehard_closure();
        states.states.swap(0, 1);
        assert_eq!(
            rejection(&states),
            Rejection::NotStrictlyAscending {
                field: Field::StateVector,
                index: 1,
            }
        );

        let mut actions = Plan::diehard_closure();
        actions.actions.swap(0, 1);
        assert_eq!(
            rejection(&actions),
            Rejection::NotStrictlyAscending {
                field: Field::ActionName,
                index: 1,
            }
        );

        let mut duplicate_state = Plan::diehard_closure();
        duplicate_state.states[1] = duplicate_state.states[0].clone();
        assert_eq!(
            rejection(&duplicate_state),
            Rejection::NotStrictlyAscending {
                field: Field::StateVector,
                index: 1,
            }
        );

        let mut row = Plan::diehard_closure();
        row.rows[0].reverse();
        assert_eq!(
            rejection(&row),
            Rejection::NotStrictlyAscending {
                field: Field::TransitionAction,
                index: 1,
            }
        );
    }

    #[test]
    fn an_inverted_variable_range_is_rejected() {
        let mut plan = Plan::diehard_closure();
        plan.variables[0].1 = 5;
        plan.variables[0].2 = 0;
        assert_eq!(
            rejection(&plan),
            Rejection::InvertedVariableRange { variable: 0 }
        );
    }

    // --- adversarial: oversized and malformed counts -----------------------

    #[test]
    fn counts_outside_the_declared_range_are_rejected() {
        let mut no_variables = Plan::diehard_closure();
        no_variables.variables.clear();
        assert_eq!(
            rejection(&no_variables),
            Rejection::CountOutOfRange {
                field: Field::VariableCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_VARIABLES),
            }
        );

        // A declared count far beyond the format's bound must be refused on the
        // count itself, never by attempting to reserve for it.
        let mut huge_states = Plan::diehard_closure();
        huge_states.declared_state_count = Some(u32::MAX);
        assert_eq!(
            rejection(&huge_states),
            Rejection::CountOutOfRange {
                field: Field::StateCount,
                found: u64::from(u32::MAX),
                min: 1,
                max: u64::from(crate::wire::MAX_STATES),
            }
        );

        let mut huge_packs = Plan::diehard_closure();
        huge_packs.declared_domain_pack_count = Some(u16::MAX);
        assert_eq!(
            rejection(&huge_packs),
            Rejection::CountOutOfRange {
                field: Field::DomainPackCount,
                found: u64::from(u16::MAX),
                min: 0,
                max: u64::from(crate::wire::MAX_DOMAIN_PACKS),
            }
        );

        let mut no_actions = Plan::diehard_closure();
        no_actions.actions.clear();
        no_actions.rows.iter_mut().for_each(Vec::clear);
        assert_eq!(
            rejection(&no_actions),
            Rejection::CountOutOfRange {
                field: Field::ActionCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_ACTIONS),
            }
        );
    }

    #[test]
    fn a_declared_count_larger_than_the_body_cannot_be_verified() {
        // An in-range count that overstates the data is the classic "read past the
        // buffer" invitation. With nothing after the table it runs out of bytes; in
        // a closure certificate it swallows the following fields, which are not an
        // ascending continuation of the state table. Both are rejections, and
        // neither is a read past the end.
        let mut typing = Plan::diehard_type();
        typing.declared_state_count = Some(crate::wire::MAX_STATES);
        assert!(matches!(
            rejection(&typing),
            Rejection::Truncated {
                field: Field::StateVector,
                ..
            }
        ));

        let mut closure = Plan::diehard_closure();
        closure.declared_state_count = Some(crate::wire::MAX_STATES);
        assert!(matches!(
            rejection(&closure),
            Rejection::NotStrictlyAscending {
                field: Field::StateVector,
                ..
            }
        ));
    }

    // --- adversarial: claims not supported by the carried data -------------

    #[test]
    fn a_certificate_with_no_initial_states_is_rejected() {
        let mut plan = Plan::diehard_closure();
        plan.initial.clear();
        assert_eq!(rejection(&plan), Rejection::NoInitialStates);
    }

    #[test]
    fn an_initial_state_outside_the_table_is_rejected() {
        // `Init ⊆ S` — Lean `ClosureCertificate.containsInit`.
        let mut plan = Plan::diehard_closure();
        plan.initial = vec![vec![2, 2]];
        assert_eq!(
            rejection(&plan),
            Rejection::InitialStateNotInTable { position: 0 }
        );
    }

    #[test]
    fn dropping_a_reachable_state_breaks_closure() {
        // `Post(S) ⊆ S` — Lean `ClosureCertificate.closed`. This is the mutation a
        // truncated exploration produces, and the one a certificate exists to catch.
        let mut plan = Plan::diehard_closure();
        let dropped = plan.states.pop().expect("the table has states");
        plan.rows.pop();
        plan.initial.retain(|state| *state != dropped);
        match rejection(&plan) {
            Rejection::ClosureFailure { .. } => {}
            other => panic!("dropping a reachable state must break closure, got {other:?}"),
        }
    }

    #[test]
    fn a_transition_to_an_unlisted_state_breaks_closure() {
        let mut plan = Plan::diehard_closure();
        plan.rows[0][0].1 = vec![99, 99];
        // Re-sorting keeps the row canonical so the closure check, not the ordering
        // check, is the one under test.
        plan.rows[0].sort();
        let position = plan.rows[0]
            .iter()
            .position(|entry| entry.1 == vec![99, 99])
            .expect("the mutated transition is still present");
        assert_eq!(
            rejection(&plan),
            Rejection::ClosureFailure {
                state: 0,
                entry: u32::try_from(position).expect("small index"),
            }
        );
    }

    #[test]
    fn a_transition_naming_an_undeclared_action_is_rejected() {
        let mut plan = Plan::diehard_closure();
        plan.rows[0][0].0 = 250;
        plan.rows[0].sort();
        let position = plan.rows[0]
            .iter()
            .position(|entry| entry.0 == 250)
            .expect("the mutated transition is still present");
        assert_eq!(
            rejection(&plan),
            Rejection::UnknownAction {
                state: 0,
                entry: u32::try_from(position).expect("small index"),
                action: 250,
            }
        );
    }

    #[test]
    fn a_state_outside_the_declared_domain_is_rejected() {
        // `S ⊆ P` — Lean `ClosureCertificate.safeOnMember`, and docs/16 PO-MOD-003
        // for the state-type family.
        let mut closure = Plan::diehard_closure();
        closure.variables[0].2 = 4; // big <= 4, but the table reaches big = 5
        let Rejection::PropertyViolated {
            variable, value, ..
        } = rejection(&closure)
        else {
            panic!("shrinking the declared domain must violate the safety obligation");
        };
        assert_eq!(variable, 0);
        assert_eq!(value, 5);

        let mut typing = Plan::diehard_type();
        typing.variables[1].2 = 2; // small <= 2, but the table reaches small = 3
        let Rejection::PropertyViolated {
            variable, value, ..
        } = rejection(&typing)
        else {
            panic!("a mistyped state must be rejected");
        };
        assert_eq!(variable, 1);
        assert_eq!(value, 3);
    }

    #[test]
    fn a_missing_successor_row_is_a_truncation() {
        let mut plan = Plan::diehard_closure();
        plan.rows.pop();
        assert!(matches!(rejection(&plan), Rejection::Truncated { .. }));
    }

    // --- adversarial: mutation sweeps --------------------------------------

    #[test]
    fn no_two_byte_strings_decode_to_the_same_certificate() {
        // Canonicity, swept exhaustively over single-byte corruptions: RFC 0005's
        // "rejection of duplicate/ambiguous encodings" means the decoder is
        // injective, so a mutation either fails to decode or decodes to a
        // *different* certificate. Nothing in the format is padding.
        let green = Plan::diehard_closure().encode();
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
    fn every_state_table_mutation_is_rejected() {
        // Perturbing any state of the table breaks at least one carried obligation:
        // the canonical ordering, the declared domain, or the closure of some row
        // that pointed at the original vector.
        let green = Plan::diehard_closure();
        for index in 0..green.states.len() {
            for delta in [-1_i64, 1, 100] {
                let mut plan = green.clone();
                plan.states[index][0] += delta;
                assert!(
                    !verdict(&plan).is_verified(),
                    "state {index} shifted by {delta} was accepted"
                );
            }
        }
    }

    #[test]
    fn every_transition_retarget_is_rejected() {
        // `Post(S) ⊆ S`, swept: redirecting any single transition off the table is
        // a closure failure, whichever row and entry it sits in.
        let green = Plan::diehard_closure();
        for row_index in 0..green.rows.len() {
            for entry_index in 0..green.rows[row_index].len() {
                let mut plan = green.clone();
                plan.rows[row_index][entry_index].1 = vec![-1, -1];
                plan.rows[row_index].sort();
                assert!(
                    matches!(rejection(&plan), Rejection::ClosureFailure { state, .. }
                        if state == u32::try_from(row_index).expect("small index")),
                    "row {row_index} entry {entry_index} was not caught as a closure failure"
                );
            }
        }
    }

    #[test]
    fn envelope_digests_are_carried_labels_not_facts_the_kernel_can_check() {
        // Stated as a test so the boundary cannot drift silently. The envelope is
        // opaque to a self-contained checker: changing a digest byte produces a
        // *different verified claim*, never a rejection, because nothing in the
        // certificate lets the kernel recompute it. Binding those digests to real
        // artifacts is the receipt's obligation (RFC 0024, ADR-0035), which is why
        // `CheckedClaim::trusted_components` names it.
        let mut plan = Plan::diehard_closure();
        plan.model_digest = "blake3:some-other-model".to_owned();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("an envelope relabel is not a malformed certificate");
        };
        assert_eq!(
            claim.envelope().model_digest().as_str(),
            "blake3:some-other-model",
            "the verdict must report the envelope it actually checked against"
        );
    }

    // --- adversarial: garbage cannot panic ---------------------------------

    #[test]
    fn garbage_bytes_never_panic() {
        // docs/12 §11 classes "malformed artifact panic in kernel" as a release
        // blocker. A deterministic xorshift stands in for a fuzzer here so the
        // corpus is reproducible (INV-005); grammar fuzzing of the same decoder is
        // RFC 0005's "Kernel tests" obligation and belongs to the mutation slice.
        let mut source = Xorshift::new(0x5eed_1234_abcd_ef01);
        for length in 0..192_usize {
            for _ in 0..8 {
                let bytes = source.bytes(length);
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        // The same, but with a valid header, so the garbage reaches the body decoder
        // rather than stopping at the magic.
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
        let green = Plan::diehard_closure().encode();
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
