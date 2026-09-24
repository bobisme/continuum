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
//! # What each wire epoch lets the kernel re-derive
//!
//! docs/03 §6.1 lists a checker step a wire-epoch-1 certificate does not let the
//! kernel perform: "recompute every enabled transition". That certificate carries its
//! own successor relation and names the model only by digest, so the kernel
//! re-derives every obligation *over* the carried relation, and the correspondence
//! between that relation and the model stays trusted. A wire-epoch-1 claim reports
//! it: [`CheckedClaim::trusted_components`] lists `certificate-model-correspondence`.
//!
//! A wire-epoch-2 certificate carries the model's canonical encoding (bn-35y4f; RFC
//! 0005 correction 1). The kernel decodes it with its own decoder (`crate::model`),
//! evaluates every action at every table state with its own evaluator, and requires
//! each carried row to equal the re-derived row. That closes the residual bn-2npu
//! measured: a producer whose evaluator or emitter writes a closed, in-domain relation
//! that is not the model's is rejected. The claim no longer lists
//! `certificate-model-correspondence`. What stays trusted is
//! `envelope-digest-binding`, now including that the carried model is the caller's;
//! the claim exposes the carried encoding for that comparison
//! ([`CheckedClaim::model_identity`]).
//!
//! Neither epoch weakens the closure obligation. A producer that stops exploring
//! early cannot hide it: at epoch 1 an unexplored successor is a carried vector absent
//! from the table, and at epoch 2 it is a re-derived successor absent from the table.
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
//! At wire epoch 2 the order is the one `check_model_closure` documents: precharge,
//! `S ⊆ Dom(M)`, `Init(M) ⊆ S`, the carried relation equal to the model's, and the
//! invariant.
//!
//! # Determinism
//!
//! The checker reads no clock, draws no randomness, opens no socket, and iterates
//! only over sequences whose order is a decode-time invariant (INV-005; ADR-0003;
//! docs/12 §1). Two runs over the same bytes produce byte-identical verdicts on any
//! platform.

use crate::model::Fault;
use crate::verdict::Resource;
use crate::verdict::{CertificateKind, CheckedClaim, Feature, PropertyClass, Rejection, Verdict};
use crate::wire::{
    Body, DecodeFailure, Envelope, FiniteClosureBody, MAX_EVALUATION_WORK, MAX_EXPRESSION_DEPTH,
    ModelClosureBody, StateTable, StateTypeBody, Variable, decode,
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
        Body::ModelClosure(body) => check_model_closure(certificate.envelope(), body),
        Body::StateType(body) => check_state_type(certificate.envelope(), body),
    }
}

/// `Init ⊆ S`, `Post(S) ⊆ S`, `S ⊆ P` over the certificate's own carried relation.
fn check_finite_closure(envelope: &Envelope, body: &FiniteClosureBody) -> Verdict {
    let class_code = body.property_class_code();
    // Wire epoch 1 carries no model, so the one class it can evaluate is the
    // declared state domain; an invariant needs the model (wire epoch 2).
    let property = match PropertyClass::from_code(class_code) {
        Some(PropertyClass::StateDomain) => PropertyClass::StateDomain,
        Some(PropertyClass::Invariant) | None => {
            return Verdict::Unsupported(Feature::PropertyClass { found: class_code });
        }
    };

    // S ⊆ P.
    if let Some(rejection) = violated_state(body.domain().variables(), body.table()) {
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

/// The wire-epoch-2 check: every obligation re-derived from the carried model.
///
/// In order, after the precharge:
///
/// 1. `S ⊆ Dom(M)` — every table state lies in the model's declared domain, so every
///    table state is a state of the model;
/// 2. `Init(M) ⊆ S` — every initial state *of the model* is in the table;
/// 3. `Post(S) ⊆ S`, exactly — for every table state the kernel evaluates every
///    action of the model, locates each successor in the table, and requires the
///    sorted, deduplicated result to equal the carried row entry for entry;
/// 4. `S ⊆ P` — for [`PropertyClass::Invariant`], the named predicate holds at every
///    table state. For [`PropertyClass::StateDomain`], `P` is `Dom(M)` and step 1
///    discharged it.
///
/// Step 3 is what closes bn-2npu's residual: a row the producer wrote from a faulty
/// evaluator, or wrote wrongly from a correct one, is not the kernel's row.
fn check_model_closure(envelope: &Envelope, body: &ModelClosureBody) -> Verdict {
    let model = body.model();
    let table = body.table();

    // Precharge (RFC 0005 "Resource bounds"): the whole evaluation is bounded from
    // decoded sizes before the first expression is evaluated.
    let work = evaluation_work(body);
    let needed = work.unwrap_or(u64::MAX);
    if work.is_none() || needed > MAX_EVALUATION_WORK {
        return Verdict::Unsupported(Feature::ResourceBound {
            resource: Resource::EvaluationWork,
            needed,
            limit: MAX_EVALUATION_WORK,
        });
    }

    // 1. S ⊆ Dom(M).
    if let Some(rejection) = violated_state(model.variables(), table) {
        return Verdict::Rejected(rejection);
    }

    // 2. Init(M) ⊆ S.
    for (position, state) in model.initial_states().iter().enumerate() {
        if table.position(state).is_none() {
            return Verdict::Rejected(Rejection::InitialStateNotInTable {
                position: narrow_u32(position),
            });
        }
    }

    // 3. The carried relation is the model's relation.
    let mut scratch: Vec<i64> = Vec::new();
    let mut derived: Vec<(u16, u32)> = Vec::new();
    for index in 0..table.len() {
        let Some(state) = table.state(index) else {
            return Verdict::Rejected(Rejection::RelationMismatch {
                state: index,
                entry: 0,
            });
        };
        derived.clear();
        let walk = model.successors(
            state,
            &mut scratch,
            |action, target| match table.position(target) {
                Some(position) => {
                    derived.push((action, position));
                    Ok(())
                }
                None => Err(WalkFault::Rejection(Rejection::SuccessorNotInTable {
                    state: index,
                    action,
                })),
            },
            |action, fault| walk_fault(index, action, fault),
        );
        match walk {
            Ok(()) => {}
            Err(WalkFault::DepthExhausted) => {
                return Verdict::Unsupported(expression_depth_unsupported());
            }
            Err(WalkFault::Rejection(rejection)) => return Verdict::Rejected(rejection),
        }
        // Actions are visited in index order, so sorting orders targets within an
        // action; two outcomes with one target are one transition.
        derived.sort_unstable();
        derived.dedup();
        // Fail closed: a missing row is a mismatch, never an empty row. Unreachable,
        // because the decoder reads exactly one row per table state.
        let Some(carried) = body.row(index) else {
            return Verdict::Rejected(Rejection::RelationMismatch {
                state: index,
                entry: 0,
            });
        };
        if carried != derived.as_slice() {
            let entry = carried
                .iter()
                .zip(derived.iter())
                .position(|(c, d)| c != d)
                .unwrap_or_else(|| carried.len().min(derived.len()));
            return Verdict::Rejected(Rejection::RelationMismatch {
                state: index,
                entry: narrow_u32(entry),
            });
        }
    }

    // 4. S ⊆ P for the invariant class.
    let invariant = match body.predicate() {
        Some(predicate) => {
            for index in 0..table.len() {
                let holds = table
                    .state(index)
                    .map(|state| model.holds(predicate, state));
                match holds {
                    Some(Ok(true)) => {}
                    Some(Ok(false)) | None => {
                        return Verdict::Rejected(Rejection::InvariantViolated { state: index });
                    }
                    // The evaluator's own budget, not a fact about the certificate
                    // (bn-ympz5): typed-inconclusive, never a rejection.
                    Some(Err(Fault::DepthExhausted)) => {
                        return Verdict::Unsupported(expression_depth_unsupported());
                    }
                    // Listed by name, not `_`, so a future `Fault` variant fails to
                    // compile here instead of silently falling into `Rejected`
                    // (bn-ympz5, adversarial review of this bone).
                    Some(Err(Fault::Overflow | Fault::OutsideDomain { .. })) => {
                        return Verdict::Rejected(Rejection::EvaluationOverflow { state: index });
                    }
                }
            }
            model.predicate_name(predicate).cloned()
        }
        None => None,
    };

    Verdict::Verified(CheckedClaim::model_bound(
        body.property(),
        envelope.clone(),
        (
            table.len(),
            narrow_u32(model.initial_states().len()),
            body.transition_count(),
        ),
        model.identity(),
        invariant,
    ))
}

/// What `Model::successors` can fail with, inside step 3 of [`check_model_closure`].
///
/// A [`Fault::DepthExhausted`] is kept apart from every other fault on purpose: it is
/// the evaluator's own resource limit, never a defect the certificate carries, so it
/// must become [`Verdict::Unsupported`], not a member of [`Rejection`] (INV-008;
/// bn-ympz5, cr-18l29w). Every other fault, and every rejection `visit` reports
/// directly (a derived successor missing from the table), is a genuine finding about
/// the certificate and stays a [`Rejection`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum WalkFault {
    /// A genuine defect: the certificate is wrong about something re-derivable.
    Rejection(Rejection),
    /// The evaluator ran out of its own nesting budget. See [`Fault::DepthExhausted`].
    DepthExhausted,
}

/// The routing decision itself, pulled out of the `successors` fault closure so it is
/// directly unit-testable (bn-ympz5, adversarial review of this bone): a test that
/// only exercises [`expression_depth_unsupported`]'s shape does not pin that
/// `Fault::DepthExhausted` actually reaches `WalkFault::DepthExhausted` rather than
/// `WalkFault::Rejection(Rejection::EvaluationOverflow { .. })` — the exact
/// regression this bone fixes, and the one a future edit could silently reintroduce.
const fn walk_fault(state: u32, action: u16, fault: Fault) -> WalkFault {
    match fault {
        Fault::Overflow => WalkFault::Rejection(Rejection::EvaluationOverflow { state }),
        Fault::OutsideDomain { variable, value } => {
            WalkFault::Rejection(Rejection::UpdateOutsideDomain {
                state,
                action,
                variable,
                value,
            })
        }
        // The evaluator's own budget, not a fact about the certificate (bn-ympz5):
        // typed-inconclusive, never a rejection.
        Fault::DepthExhausted => WalkFault::DepthExhausted,
    }
}

/// The typed-inconclusive outcome for an evaluator that exhausted its own nesting
/// budget ([`Fault::DepthExhausted`]).
///
/// Spelled the same way the decoder spells the identical bound in `depth_bound`
/// (`crate::model::decode`): same [`Resource::ExpressionDepth`], same
/// `limit`. The decoder guarantees no certificate it accepts can drive the
/// evaluator's budget past this limit, so `needed` is never observed exactly; it is
/// reported as one past the limit, the same lower-bound convention the decoder's own
/// `push` (`Resource::ExpressionNodes`) uses when a bound is merely exceeded, not
/// measured.
fn expression_depth_unsupported() -> Feature {
    let limit = u64::try_from(MAX_EXPRESSION_DEPTH).unwrap_or(u64::MAX);
    Feature::ResourceBound {
        resource: Resource::ExpressionDepth,
        needed: limit.saturating_add(1),
        limit,
    }
}

/// The row-sort share charged per derived successor: one comparison per level of a
/// sort over at most `2^24` entries (the model section cannot hold more outcomes).
const SORT_UNITS_PER_OUTCOME: u64 = 24;

/// The precharge of [`check_model_closure`], or `None` when it does not fit in a
/// `u64`.
///
/// Units: one per expression node evaluated, one per state component copied or
/// compared. A table lookup is a binary search, so it costs `arity` per probe and
/// `bits(states) + 1` probes. Per table state: the domain check (`arity`), every
/// guard, every outcome of every action as if enabled (its assignments, a state copy
/// and a lookup), a sort of the derived row (one lookup's worth per outcome, already
/// counted), the row comparison (included in the copy), and the invariant. Plus one
/// lookup per initial state.
fn evaluation_work(body: &ModelClosureBody) -> Option<u64> {
    let model = body.model();
    let states = u64::from(body.table().len());
    let arity = u64::try_from(model.variables().len()).ok()?;
    let probes =
        u64::from(u32::BITS.checked_sub(body.table().len().leading_zeros())?).checked_add(1)?;
    // Two probes' worth per lookup, plus a flat charge per outcome for its share of
    // the row sort: `k log k` comparisons over a row of `k` outcomes, and `k` is
    // bounded by the model section's bytes (`log2 k < 24`).
    let lookup = arity
        .checked_mul(probes)?
        .checked_mul(2)?
        .checked_add(SORT_UNITS_PER_OUTCOME)?;
    let invariant = body
        .predicate()
        .map_or(0, |index| model.predicate_size(index));
    let per_state = model
        .successor_work(lookup)?
        .checked_add(arity)?
        .checked_add(invariant)?;
    let initial = u64::try_from(model.initial_states().len())
        .ok()?
        .checked_mul(lookup)?;
    states.checked_mul(per_state)?.checked_add(initial)
}

/// Every state in the table lies in the declared state domain (docs/16 PO-MOD-003).
fn check_state_type(envelope: &Envelope, body: &StateTypeBody) -> Verdict {
    if let Some(rejection) = violated_state(body.domain().variables(), body.table()) {
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
fn violated_state(domain: &[Variable], table: &StateTable) -> Option<Rejection> {
    for index in 0..table.len() {
        // Fail closed: a table index the table cannot answer is a violation, never a
        // pass. Unreachable, because the decoder reads exactly `len` states.
        let Some(state) = table.state(index) else {
            return Some(Rejection::PropertyViolated {
                state: index,
                variable: 0,
                value: 0,
            });
        };
        for (position, variable) in domain.iter().enumerate() {
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
    use crate::verdict::{Field, Resource, TokenFault};
    use crate::wire::{LEGACY_WIRE_EPOCH, MAGIC, MAX_TOKEN_BYTES, WIRE_EPOCH};

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
                header: LEGACY_WIRE_EPOCH,
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

    // --- wire epoch 2: the model-bound finite closure (bn-35y4f) -------------

    use crate::fixture::{Expr, ModelPlan, PlanV2};

    fn verdict_v2(plan: &PlanV2) -> Verdict {
        check_certificate(&plan.encode())
    }

    fn rejection_v2(plan: &PlanV2) -> Rejection {
        match verdict_v2(plan) {
            Verdict::Rejected(rejection) => rejection,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    /// Keep only the states `keep` admits, drop transitions into the others, and
    /// renumber the rows.
    fn restrict(plan: &PlanV2, keep: impl Fn(&[i64]) -> bool) -> PlanV2 {
        let kept: Vec<usize> = (0..plan.states.len())
            .filter(|i| keep(&plan.states[*i]))
            .collect();
        let renumber = |old: u32| kept.iter().position(|k| *k == old as usize);
        let mut out = plan.clone();
        out.states = kept.iter().map(|i| plan.states[*i].clone()).collect();
        out.rows = kept
            .iter()
            .map(|i| {
                plan.rows[*i]
                    .iter()
                    .filter_map(|(a, t)| renumber(*t).map(|n| (*a, u32::try_from(n).unwrap())))
                    .collect()
            })
            .collect();
        out
    }

    #[test]
    fn the_model_bound_die_hard_certificate_is_verified_without_the_correspondence_trust() {
        let Verdict::Verified(claim) = verdict_v2(&PlanV2::diehard()) else {
            panic!("the wire-epoch-2 Die Hard certificate must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::FiniteClosure);
        assert_eq!(claim.property(), PropertyClass::StateDomain);
        assert_eq!(claim.wire_epoch(), 2);
        assert_eq!(claim.states(), 16);
        assert_eq!(claim.transitions(), 96);
        assert_eq!(claim.initial_states(), 1);
        assert_eq!(claim.trusted_components(), ["envelope-digest-binding"]);
        assert_eq!(
            claim.model_identity(),
            Some(ModelPlan::diehard().encode().as_slice())
        );
        assert_eq!(claim.invariant(), None);
    }

    #[test]
    fn a_legacy_claim_still_names_the_correspondence_it_trusts() {
        let Verdict::Verified(claim) = verdict(&Plan::diehard_closure()) else {
            panic!("the wire-epoch-1 certificate must still check green");
        };
        assert_eq!(claim.wire_epoch(), 1);
        assert_eq!(claim.model_identity(), None);
        assert!(
            claim
                .trusted_components()
                .contains(&"certificate-model-correspondence")
        );
    }

    #[test]
    fn an_invariant_of_the_model_is_established_or_refuted_at_every_state() {
        let Verdict::Verified(claim) = verdict_v2(&PlanV2::diehard_invariant("TypeOK")) else {
            panic!("TypeOK holds at every reachable Die Hard state");
        };
        assert_eq!(claim.property(), PropertyClass::Invariant);
        assert_eq!(claim.invariant().map(|t| t.as_str()), Some("TypeOK"));

        // The puzzle's point: big = 4 is reachable, so "big is never 4" is false.
        assert!(matches!(
            rejection_v2(&PlanV2::diehard_invariant("BigNotFour")),
            Rejection::InvariantViolated { .. }
        ));
        assert_eq!(
            rejection_v2(&PlanV2::diehard_invariant("NoSuchPredicate")),
            Rejection::UnresolvedName {
                field: Field::PropertyPredicate
            }
        );
    }

    #[test]
    fn an_invariant_needs_the_model_so_epoch_one_cannot_carry_it() {
        let mut plan = Plan::diehard_closure();
        plan.property_class = 2;
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::PropertyClass { found: 2 })
        );
    }

    #[test]
    fn epoch_two_defines_only_the_finite_closure_family() {
        let mut plan = PlanV2::diehard();
        plan.kind = 2;
        assert_eq!(
            verdict_v2(&plan),
            Verdict::Unsupported(Feature::CertificateKind { found: 2 })
        );
        let mut plan = PlanV2::diehard();
        plan.property_class = 9;
        assert_eq!(
            verdict_v2(&plan),
            Verdict::Unsupported(Feature::PropertyClass { found: 9 })
        );
    }

    /// Metamorphic relation: equivalent guard normalization (docs/19 §3). Rewriting
    /// every guard of the carried model into a logically equivalent form — `true` as
    /// `not(not(true))`, and as `true and true` — keeps the verdict, the counts and the
    /// property; only the carried identity changes.
    #[test]
    fn metamorphic_equivalent_guard_normalization_keeps_the_verdict() {
        let green = verdict_v2(&PlanV2::diehard());
        let Verdict::Verified(green) = green else {
            panic!("the green certificate verifies");
        };
        let normalizations: [fn() -> Expr; 2] = [
            || Expr::Not(Box::new(Expr::Not(Box::new(Expr::Bool(true))))),
            || Expr::And(Box::new(Expr::Bool(true)), Box::new(Expr::Bool(true))),
        ];
        for normalize in normalizations {
            let mut model = ModelPlan::diehard();
            for action in &mut model.actions {
                action.guard = normalize();
            }
            let mut plan = PlanV2::diehard();
            plan.model = model.encode();
            let Verdict::Verified(claim) = verdict_v2(&plan) else {
                panic!("an equivalent guard normalization changed the verdict");
            };
            assert_eq!(claim.states(), green.states());
            assert_eq!(claim.transitions(), green.transitions());
            assert_eq!(claim.initial_states(), green.initial_states());
            assert_eq!(claim.property(), green.property());
            assert_ne!(claim.model_identity(), green.model_identity());
            // bn-ympz5: a certificate's `wire_epoch()` now feeds `Receipt::wire_epoch_id`
            // per receipt (kernel review of bn-35y4f, cr-18l29w); it must be as
            // invariant under this relation as every other re-derived fact above.
            assert_eq!(claim.wire_epoch(), green.wire_epoch());
        }
    }

    // The bn-2npu producer faults, as certificates.

    #[test]
    fn a_dropped_transition_is_not_the_models_relation() {
        // emit-drops-first-transition-of-each-row: a closed sub-relation.
        let mut plan = PlanV2::diehard();
        plan.rows[3].remove(0);
        assert_eq!(
            rejection_v2(&plan),
            Rejection::RelationMismatch { state: 3, entry: 0 }
        );
    }

    #[test]
    fn a_retargeted_transition_is_not_the_models_relation() {
        // emit-retargets-to-the-first-state: in the table, so closed.
        let green = PlanV2::diehard();
        for state in 0..green.rows.len() {
            for entry in 0..green.rows[state].len() {
                let mut plan = green.clone();
                let (action, target) = plan.rows[state][entry];
                let retarget = if target == 0 { 1 } else { 0 };
                plan.rows[state][entry] = (action, retarget);
                plan.rows[state].sort_unstable();
                plan.rows[state].dedup();
                assert!(
                    matches!(rejection_v2(&plan), Rejection::RelationMismatch { state: s, .. }
                        if s == u32::try_from(state).unwrap()),
                    "row {state} entry {entry} retargeted was not caught"
                );
            }
        }
    }

    #[test]
    fn an_extra_or_relabelled_transition_is_not_the_models_relation() {
        let mut plan = PlanV2::diehard();
        let last = plan.rows[0].last().copied().unwrap();
        plan.rows[0].push((last.0, last.1.saturating_add(1).min(15)));
        plan.rows[0].sort_unstable();
        plan.rows[0].dedup();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::RelationMismatch { state: 0, .. }
        ));
    }

    #[test]
    fn a_model_whose_evaluation_differs_from_the_rows_is_caught() {
        // eval-update-off-by-one / eval-frame-rule-broken / eval-skips-the-last-action:
        // the rows are the correct relation, the carried model is not the one that
        // wrote them. Either way round, the two disagree.
        let mut model = ModelPlan::diehard();
        model.actions[4].outcomes[0][0].1 = Expr::Int(2); // fill-small := 2
        let mut plan = PlanV2::diehard();
        plan.model = model.encode();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::RelationMismatch { .. } | Rejection::SuccessorNotInTable { .. }
        ));

        let mut model = ModelPlan::diehard();
        model.actions[5].guard = Expr::Bool(false);
        let mut plan = PlanV2::diehard();
        plan.model = model.encode();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::RelationMismatch { .. }
        ));
    }

    #[test]
    fn a_missing_initial_state_of_the_model_is_caught() {
        // emit-drops-first-initial-state: the initial states are the model's.
        let mut model = ModelPlan::diehard();
        model.initial = vec![vec![0, 0], vec![1, 1]];
        let mut plan = PlanV2::diehard();
        plan.model = model.encode();
        assert_eq!(
            rejection_v2(&plan),
            Rejection::InitialStateNotInTable { position: 1 }
        );
    }

    #[test]
    fn a_table_missing_a_reachable_state_is_a_closure_failure_against_the_model() {
        let plan = restrict(&PlanV2::diehard(), |s| s != [5, 3]);
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::SuccessorNotInTable { .. }
        ));
    }

    #[test]
    fn a_model_that_leaves_its_domain_or_overflows_has_no_relation_to_certify() {
        let mut model = ModelPlan::diehard();
        model.actions[3].outcomes[0][0].1 = Expr::Int(6); // fill-big := 6
        let mut plan = PlanV2::diehard();
        plan.model = model.encode();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::UpdateOutsideDomain {
                action: 3,
                variable: 0,
                value: 6,
                ..
            }
        ));

        let mut model = ModelPlan::diehard();
        model.overflowing_guard();
        let mut plan = PlanV2::diehard();
        plan.model = model.encode();
        assert_eq!(
            rejection_v2(&plan),
            Rejection::EvaluationOverflow { state: 0 }
        );
    }

    #[test]
    fn a_table_state_outside_the_models_domain_is_rejected() {
        let mut plan = PlanV2::diehard();
        plan.states.push(vec![6, 0]);
        plan.rows.push(Vec::new());
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::PropertyViolated {
                variable: 0,
                value: 6,
                ..
            }
        ));
    }

    #[test]
    fn every_carried_index_is_range_checked_at_decode() {
        let mut plan = PlanV2::diehard();
        plan.rows[0].push((0, 16));
        plan.rows[0].sort_unstable();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::TargetOutOfRange {
                state: 0,
                target: 16,
                ..
            }
        ));
        let mut plan = PlanV2::diehard();
        plan.rows[0].push((6, 0));
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::UnknownAction {
                state: 0,
                action: 6,
                ..
            }
        ));
        let mut plan = PlanV2::diehard();
        plan.rows[0].reverse();
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::TransitionAction,
                ..
            }
        ));
    }

    #[test]
    fn the_model_section_is_bounded_before_it_is_decoded() {
        let mut plan = PlanV2::diehard();
        plan.declared_model_len = Some(crate::wire::MAX_MODEL_BYTES + 1);
        assert!(matches!(
            rejection_v2(&plan),
            Rejection::Truncated {
                field: Field::ModelLength,
                ..
            }
        ));
        let mut plan = PlanV2::diehard();
        let mut model = ModelPlan::diehard().encode();
        model.resize(crate::wire::MAX_MODEL_BYTES as usize + 1, 0);
        plan.model = model;
        assert!(matches!(
            verdict_v2(&plan),
            Verdict::Unsupported(Feature::ResourceBound {
                resource: Resource::ModelBytes,
                ..
            })
        ));
    }

    #[test]
    fn evaluation_work_is_charged_before_any_evaluation() {
        // A guard of 2^20 - 1 nodes (a balanced conjunction 19 deep) over 20,000
        // states: about 2 * 10^10 units, over the 2^33 budget. The rows are all empty
        // and wrong, so a check that evaluated first would reject instead.
        fn balanced(depth: u32) -> Expr {
            if depth == 0 {
                Expr::Bool(true)
            } else {
                Expr::And(Box::new(balanced(depth - 1)), Box::new(balanced(depth - 1)))
            }
        }
        let model = ModelPlan {
            variables: vec![("x".to_owned(), 0, 1_000_000)],
            actions: vec![crate::fixture::PlannedAction {
                name: "a".to_owned(),
                guard: balanced(19),
                outcomes: vec![vec![]],
            }],
            initial: vec![vec![0]],
            predicates: Vec::new(),
            ..ModelPlan::two_outcomes()
        };
        let plan = PlanV2 {
            model: model.encode(),
            states: (0..20_000).map(|x| vec![x]).collect(),
            rows: vec![Vec::new(); 20_000],
            ..PlanV2::diehard()
        };
        assert!(matches!(
            verdict_v2(&plan),
            Verdict::Unsupported(Feature::ResourceBound {
                resource: Resource::EvaluationWork,
                ..
            })
        ));
    }

    #[test]
    fn evaluator_depth_exhaustion_is_unsupported_not_a_rejection() {
        // bn-ympz5 (kernel review of bn-35y4f, cr-18l29w): `check_model_closure`
        // must turn `Fault::DepthExhausted` into the same `Feature::ResourceBound`
        // shape the decoder's own `depth_bound` uses for
        // `Resource::ExpressionDepth` (same `Resource`, same `limit`), never
        // `Rejection::EvaluationOverflow`. No certificate a live decoder accepts can
        // drive the evaluator there — the decoder's bound and the evaluator's
        // starting budget match exactly (see
        // `model::tests::an_exhausted_evaluator_budget_is_a_distinct_fault_from_overflow`
        // and `model::tests::expression_depth_beyond_the_bound_is_unsupported_not_rejected`)
        // — so this pins the translation `check_model_closure` performs directly,
        // the same way that test pins the evaluator side.
        let limit = u64::try_from(MAX_EXPRESSION_DEPTH).unwrap();
        assert_eq!(
            expression_depth_unsupported(),
            Feature::ResourceBound {
                resource: Resource::ExpressionDepth,
                needed: limit + 1,
                limit,
            }
        );
    }

    #[test]
    fn walk_fault_routes_depth_exhaustion_to_unsupported_not_rejected() {
        // bn-ympz5 (adversarial review of this bone): the test above only pins the
        // *shape* `expression_depth_unsupported()` builds; it never calls
        // `walk_fault`, so it would not catch `Fault::DepthExhausted` being folded
        // back into `WalkFault::Rejection(EvaluationOverflow)` (the exact regression
        // this bone fixes) or the `Some(Err(Fault::DepthExhausted))` arm in the
        // invariant loop being deleted. This pins the routing function itself.
        assert_eq!(
            walk_fault(7, 3, Fault::DepthExhausted),
            WalkFault::DepthExhausted
        );
        // Every other fault stays a genuine rejection, named for the state and
        // action `walk_fault` was given — never silently swallowed into the same
        // typed-inconclusive outcome.
        assert_eq!(
            walk_fault(7, 3, Fault::Overflow),
            WalkFault::Rejection(Rejection::EvaluationOverflow { state: 7 })
        );
        assert_eq!(
            walk_fault(
                7,
                3,
                Fault::OutsideDomain {
                    variable: 2,
                    value: 99,
                },
            ),
            WalkFault::Rejection(Rejection::UpdateOutsideDomain {
                state: 7,
                action: 3,
                variable: 2,
                value: 99,
            })
        );
    }

    #[test]
    fn every_prefix_of_a_model_bound_certificate_is_refused() {
        let bytes = PlanV2::diehard().encode();
        for cut in 0..bytes.len() {
            assert!(
                !check_certificate(&bytes[..cut]).is_verified(),
                "a {cut}-byte prefix verified"
            );
        }
    }

    #[test]
    fn single_byte_corruptions_of_a_model_bound_certificate_never_verify_a_different_relation() {
        // A corruption may still verify only if it leaves the relation, the model
        // and the property unchanged — i.e. it touched an envelope label, which is
        // the trusted envelope-digest binding.
        let green_plan = PlanV2::diehard();
        let green = green_plan.encode();
        let envelope_end = green.len()
            - green_plan.model.len()
            - 4
            - 2
            - 4
            - 16 * 16
            - green_plan
                .rows
                .iter()
                .map(|r| 4 + 6 * r.len())
                .sum::<usize>();
        for position in envelope_end..green.len() {
            for delta in [0x01_u8, 0x80] {
                let mut mutated = green.clone();
                mutated[position] ^= delta;
                if let Verdict::Verified(claim) = check_certificate(&mutated) {
                    // Only a model change that is semantically inert could verify,
                    // and then the claim reports the changed identity.
                    assert_ne!(
                        claim.model_identity(),
                        Some(green_plan.model.as_slice()),
                        "byte {position} xor {delta:#04x} verified with the green identity"
                    );
                }
            }
        }
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
