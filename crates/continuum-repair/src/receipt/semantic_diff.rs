//! PR-22 / IMPL-03: the receipt's `semantic_diff` and `intent_diff` — the RFC 0031
//! diff the promotion was decided against, derived by the system and never taken from
//! the claim.
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | `semantic_diff` names the RFC 0031 classification the promotion was decided against | RFC 0032 receipt table; `promotion-receipt.schema.json` | the skeleton's [`ReceiptDiff`] is [`crate::diff`]'s computation for the transaction version the receipt names, from the stores the version names (the evidence scope as of that version), never a caller's value |
//! | both fields name the same `diff_*` identity; a receipt naming two diff artifacts is malformed | RFC 0032 correction 10 | the skeleton renders one handle into both fields; a claim whose two fields differ is refused `ClaimRefusal::DiffFieldsDisagree` before any store is read |
//! | a client-supplied diff is never trusted | RFC 0031 "Wire surface"; RFC 0032 "checkable by reference" | a claimed handle is compared with the recomputed one: `VerifyRefusal::SemanticDiff` with `DiffClaimRefusal::HandleMismatch`, or `LayerMismatch` when the claim is the diff of these inputs under the other program layer; presented artifact bytes are compared byte for byte ([`super::verify_referenced_diff`]) |
//! | a receipt never presents a diff under another program layer than the recompute | cr-3psesg; RFC 0031 "an unrequested layer is not evidence of absence" | the layer is bound into the `diff_` handle (PR-20 / IMPL-04), so a handle minted under the other layer is refused `LayerMismatch` |
//! | a diff that cannot be computed is carried, never omitted | RFC 0031 "Fail-closed rule"; RFC 0032 correction 11; INV-007, INV-008 | [`ReceiptDiff::Undiffable`] with a closed [`UndiffableCause`], rendered as the `unknowns` entry `semantic_diff_undiffable:<cause>`; the skeleton then renders no handle, and every claim is refused `VerifyRefusal::SemanticDiffUndiffable` |
//! | an unclassified program side is carried, and no semantic preservation is claimed on it | RFC 0031 "Wire surface"; RFC 0032 gate 3, correction 8 | the `unknowns` entry `semantic_diff_unclassified:program_layer`; a claim listing gate 3 `passed` when the recomputed diff does not pass it is refused `VerifyRefusal::IntentIntegrityNotDerived` |
//!
//! # Which undiffable inputs reach a receipt
//!
//! The skeleton resolves the base intent, its standing and both snapshot bindings
//! before it computes the diff, and refuses them typed. A diff that is then undiffable
//! for one of those reasons read the stores and got another answer: that is a store
//! defect, refused as the matching [`super::ComposeRefusal`], not an unknown. A store that
//! did not answer is `ComposeRefusal::StoreUnavailable`, because an unknown must
//! re-derive identically at verification and a silent store is not a fact about the
//! repair. What remains is a fact about the stored inputs: the classifier refused them,
//! or the artifact did not render. Those are the [`UndiffableCause`]s. A malformed
//! evidence scope (a duplicated or bad evidence identity) is among them, unlike the
//! `CandidateEvidence` defects the skeleton refuses: it is what the evidence graph holds
//! as of the version, so it re-derives identically at verification, and it gates
//! nothing either way, because an undiffable receipt verifies no claim.
//!
//! An undiffable skeleton renders no `semantic_diff` or `intent_diff`, which the schema
//! requires, so no schema-valid receipt can be emitted from it: the unknown is carried in
//! the skeleton and its fragments, and promotion stops there.
//!
//! [`crate::diff::ImpactScope`] has no `Unknown` answer: the diff is a published artifact
//! and must not vary by caller, so the scope is not scoped to the caller's standing,
//! unlike the other receipt stores (ADR-0037).
//!
//! # What is not here
//!
//! The anchor version of the evidence scope. The skeleton reads the scope as of the
//! version the receipt names; the version gate 3 was decided on, the `ready` version
//! and the `promoted` version differ in identity, and evidence recorded between them
//! would move the handle. Which one anchors the scope is `ReceiptSeam::PromotionRecord`'s
//! (PR-20 promote, which also writes `semantic_diff` into a version); the tests' stores
//! answer one scope per version.
//!
//! A typed wire marker for "program layer not classified" in the diff artifact itself:
//! `semantic-diff.schema.json` cannot express it (a pending user decision; bn-1b69). The
//! receipt carries the fact in `unknowns`, whose entries the schema types as bare
//! strings, so the token is this crate's grammar ([`super::unknowns`]).

use continuum_semantic_diff::artifact::AssembleError;
use continuum_semantic_diff::impact::ProgramLayer;
use continuum_value::assurance::InconclusiveReason;

use super::{ComposeRefusal, SnapshotRole, Store};
use crate::diff::{DiffStore, TransactionDiff, Undiffable};
use crate::transaction::GateStatus;

/// Why no diff could be computed for a receipt, from the stored inputs themselves. A
/// closed set of tokens, so the `unknowns` entry never echoes classifier text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UndiffableCause {
    /// The assembled artifact did not render as an object carrying `diff_id`.
    Unrenderable,
    /// A handle the assembler reads is off its pattern.
    MalformedHandle,
    /// A relation inadmissible on its field reached record construction.
    Classification,
    /// The policy verdict was refused: an incomplete classification (P1), an
    /// unenforceable `review` (W7), or an inadmissible relation.
    Enforcement,
    /// The evidence scope was malformed: a bad or duplicated evidence identity.
    ImpactScope,
}

impl UndiffableCause {
    /// Every cause.
    pub const ALL: [Self; 5] = [
        Self::Unrenderable,
        Self::MalformedHandle,
        Self::Classification,
        Self::Enforcement,
        Self::ImpactScope,
    ];

    /// The token the `unknowns` entry carries.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Unrenderable => "unrenderable",
            Self::MalformedHandle => "malformed_handle",
            Self::Classification => "classification_refused",
            Self::Enforcement => "verdict_refused",
            Self::ImpactScope => "impact_scope_malformed",
        }
    }

    /// The inverse of [`Self::token`].
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cause| cause.token() == token)
    }

    /// The INV-008 reason gate 3 records for it: a defect of the verifier's inputs.
    #[must_use]
    pub const fn inconclusive_reason(self) -> InconclusiveReason {
        InconclusiveReason::EngineError
    }

    /// The cause of `undiffable`, or the compose refusal it is when it is not a fact about
    /// the stored inputs (module docs).
    pub(crate) fn of(undiffable: Undiffable) -> Result<Self, ComposeRefusal> {
        match undiffable {
            Undiffable::Unrenderable => Ok(Self::Unrenderable),
            Undiffable::Unclassifiable(error) => Ok(match error {
                AssembleError::MalformedDiffId { .. }
                | AssembleError::MalformedSnapshotId { .. } => Self::MalformedHandle,
                AssembleError::Classification(_) => Self::Classification,
                AssembleError::Enforcement(_) => Self::Enforcement,
                AssembleError::Impact(_) => Self::ImpactScope,
            }),
            Undiffable::NoCandidate => Err(ComposeRefusal::NoCandidate),
            Undiffable::SnapshotUnresolved(role) => Err(ComposeRefusal::SnapshotUnresolved(role)),
            Undiffable::BaseBoundElsewhere => {
                Err(ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::Before))
            }
            Undiffable::IntentUnregistered(_) => Err(ComposeRefusal::IntentUnregistered),
            Undiffable::BaseNotProtected => Err(ComposeRefusal::IntentNotProtected),
            Undiffable::BaseSuperseded => Err(ComposeRefusal::IntentSuperseded),
            Undiffable::RegistryInconsistent(_) => Err(ComposeRefusal::RegistryInconsistent),
            Undiffable::StoreUnavailable(store) => {
                Err(ComposeRefusal::StoreUnavailable(match store {
                    DiffStore::Snapshots => Store::Snapshots,
                    DiffStore::IntentRegistry => Store::IntentRegistry,
                    DiffStore::Evidence => Store::ImpactScope,
                }))
            }
        }
    }
}

/// What the diff contributes to `unknowns`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffGap {
    /// The diff was computed, and its program-side layer is not classified.
    ProgramLayerUnclassified,
    /// No diff could be computed.
    Undiffable(UndiffableCause),
}

/// The receipt's semantic diff, derived from the stores for the exact transaction
/// version the receipt names. A skeleton holds only one its derivation built; the
/// [`TransactionDiff`] inside cannot be built outside [`crate::diff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptDiff {
    /// The recomputed diff. Its handle is the value of both `semantic_diff` and
    /// `intent_diff`.
    Derived(Box<TransactionDiff>),
    /// No diff could be computed. No handle is rendered, and the fact is an unknown.
    Undiffable(UndiffableCause),
}

impl ReceiptDiff {
    /// The `diff_*` handle both fields carry, or `None` when no diff was computed.
    #[must_use]
    pub fn handle(&self) -> Option<&str> {
        match self {
            Self::Derived(diff) => Some(diff.handle()),
            Self::Undiffable(_) => None,
        }
    }

    /// The recomputed diff, if any.
    #[must_use]
    pub fn diff(&self) -> Option<&TransactionDiff> {
        match self {
            Self::Derived(diff) => Some(diff),
            Self::Undiffable(_) => None,
        }
    }

    /// The program-side layer the diff was computed under, if any.
    #[must_use]
    pub fn program_layer(&self) -> Option<ProgramLayer> {
        self.diff().map(|diff| diff.artifact().program_layer())
    }

    /// Gate 3 as the recomputed diff decides it: `passed` only for a repair whose
    /// program-side layer is classified, and `inconclusive` when no diff was computed.
    /// This, not a claim, is what a receipt may say about semantic preservation.
    #[must_use]
    pub fn gate_status(&self) -> GateStatus {
        match self {
            Self::Derived(diff) => diff.gate_status(),
            Self::Undiffable(_) => GateStatus::Inconclusive,
        }
    }

    /// The INV-008 reason when [`Self::gate_status`] is `inconclusive`.
    #[must_use]
    pub fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::Derived(diff) => diff.inconclusive_reason(),
            Self::Undiffable(cause) => Some(cause.inconclusive_reason()),
        }
    }

    /// What the diff contributes to `unknowns`.
    #[must_use]
    pub fn gap(&self) -> Option<DiffGap> {
        match self {
            Self::Derived(diff) => match diff.artifact().program_layer() {
                ProgramLayer::Classified => None,
                ProgramLayer::Unclassified => Some(DiffGap::ProgramLayerUnclassified),
            },
            Self::Undiffable(cause) => Some(DiffGap::Undiffable(*cause)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metamorphic, relation "serialization round trip": every cause token reads back
    /// as its cause, and no two causes render alike.
    #[test]
    fn pr22_impl03_cause_tokens_round_trip_and_are_distinct() {
        let mut seen = std::collections::BTreeSet::new();
        for cause in UndiffableCause::ALL {
            assert_eq!(UndiffableCause::from_token(cause.token()), Some(cause));
            assert!(!cause.token().contains(':'));
            assert!(seen.insert(cause.token()));
        }
        assert_eq!(UndiffableCause::from_token("impact"), None);
    }

    /// Every store-read undiffable is a compose refusal, never an unknown; only the
    /// inputs' own facts are causes.
    #[test]
    fn pr22_impl03_only_facts_about_the_inputs_become_unknowns() {
        assert_eq!(
            UndiffableCause::of(Undiffable::Unrenderable),
            Ok(UndiffableCause::Unrenderable)
        );
        for (undiffable, refusal) in [
            (Undiffable::NoCandidate, ComposeRefusal::NoCandidate),
            (
                Undiffable::StoreUnavailable(DiffStore::Evidence),
                ComposeRefusal::StoreUnavailable(Store::ImpactScope),
            ),
            (
                Undiffable::StoreUnavailable(DiffStore::Snapshots),
                ComposeRefusal::StoreUnavailable(Store::Snapshots),
            ),
            (
                Undiffable::StoreUnavailable(DiffStore::IntentRegistry),
                ComposeRefusal::StoreUnavailable(Store::IntentRegistry),
            ),
            (Undiffable::BaseSuperseded, ComposeRefusal::IntentSuperseded),
            (
                Undiffable::BaseNotProtected,
                ComposeRefusal::IntentNotProtected,
            ),
            (
                Undiffable::BaseBoundElsewhere,
                ComposeRefusal::SnapshotBoundElsewhere(SnapshotRole::Before),
            ),
            (
                Undiffable::SnapshotUnresolved(SnapshotRole::After),
                ComposeRefusal::SnapshotUnresolved(SnapshotRole::After),
            ),
            (
                Undiffable::IntentUnregistered(SnapshotRole::After),
                ComposeRefusal::IntentUnregistered,
            ),
            (
                Undiffable::RegistryInconsistent(SnapshotRole::Before),
                ComposeRefusal::RegistryInconsistent,
            ),
        ] {
            assert_eq!(UndiffableCause::of(undiffable), Err(refusal));
        }
    }
}
