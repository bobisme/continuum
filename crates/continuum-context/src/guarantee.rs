//! The pack's closed thirteen-member `guarantees` vocabulary, and the rule that keeps a
//! guarantee a *checked claim* rather than a requested one (RFC 0028, "Guarantee classes").
//!
//! # Scope: bn-21vno — the compiler's stage group 1 (stages 1–2)
//!
//! > **A guarantee is a checked claim, never a score.** Every member of the `guarantees`
//! > set names a specific checker and a specific failure mode. A pack MUST NOT publish a
//! > guarantee whose checker did not run and pass.
//! >
//! > — RFC 0028, "Summary"
//!
//! [`Guarantee`] is declared with all thirteen members even though this stage group can
//! license exactly one of them ([`Guarantee::CausallyClosed`], from stage 2). That is the
//! precedent [`crate::selection::SelectionKind`] and [`crate::expansion::ExpansionRelation`]
//! already set in this crate: "an enum missing members of a closed schema enum is a
//! vocabulary that cannot round-trip a conforming instance". What is genuinely this stage
//! group's own is the *licensing* path, and it is deliberately narrow.
//!
//! # Rule C1 is a type here, not a review note
//!
//! > **C1 — Requested is not achieved.** `context.compile`'s `guarantees` list is a
//! > *request*. A daemon MUST NOT echo a requested guarantee it did not achieve.
//! >
//! > — RFC 0028, "Guarantee classes"
//!
//! The wire spelling invites the collapse — one word, `guarantees`, for the request and for
//! the claim (RFC 0028 correction 10, F6) — so the two are different types here.
//! [`RequestedGuarantees`] is what a caller asked for and has no path into a pack.
//! [`GuaranteeSet`] is what was achieved, and it can only be extended by a [`License`],
//! whose constructor is `pub(crate)`: no code outside this crate can mint one, and inside
//! the crate only a stage's own checker does. A daemon holding a request list and a
//! [`GuaranteeSet`] cannot accidentally publish the first as the second, because the first
//! does not convert.
//!
//! [`RequestedGuarantees::unachieved`] then names what C1's second sentence owes the
//! manifest: every requested-but-unachieved guarantee "MUST appear in the omission manifest
//! with reason `unsupported` […] or `budget`".
//!
//! # Rule C2 is checked where its precondition is licensed
//!
//! > **C2 — `ReplayPreserving` implies `CausallyClosed`.** A core that replays contains
//! > every causal predecessor its replay consumes, so a pack claiming `ReplayPreserving`
//! > MUST also claim and check `CausallyClosed`, and MUST carry a non-null `replay`.
//! >
//! > — RFC 0028, "Guarantee classes"
//!
//! [`GuaranteeSet::seal`] refuses a set holding `ReplayPreserving` without `CausallyClosed`.
//! Nothing in this stage group can license `ReplayPreserving` — the replay checker is not
//! this crate's and not yet built — so the rule is stated at the point its *precondition*
//! is produced (stage 2), where a later stage group adding the replay checker will find it
//! already enforced rather than have to remember it.
//!
//! # What is declined here
//!
//! - **Per-item guarantee attribution.** `selected[]` items carry no guarantee field at
//!   `schema_epoch` 1 (RFC 0028 F2), and rule C4's structural segregation is the interim
//!   answer. A set is per *pack*, and this module says nothing about which item carries
//!   which claim.
//! - **Checker attribution.** RFC 0028 F9: "the artifact records the guarantee but neither
//!   the checker identity nor the check result". A [`License`] carries the guarantee and
//!   nothing else, because there is no field on the wire for what produced it. The
//!   consequence C6 draws — "a promotion-relevant consumer MUST re-run the checker rather
//!   than read the field" — is unchanged by anything here.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | thirteen members, the schema's tokens, in the schema's order | `context-pack.schema.json` `guarantees` | `the_guarantee_set_is_exactly_the_schema_enum` |
//! | fail closed on an unrecognized token | RFC 0028, "Fail closed on unrecognized tokens" | `an_unknown_guarantee_token_does_not_parse` |
//! | requested is not achieved | RFC 0028 C1 | `a_request_does_not_become_a_claim` |
//! | unachieved requests are owed to the manifest | RFC 0028 C1 | `an_unachieved_request_is_named` |
//! | `ReplayPreserving` implies `CausallyClosed` | RFC 0028 C2 | `replay_preserving_without_causal_closure_is_refused` |
//! | the set is deterministically ordered | `rule ordering.deterministic` | `a_sealed_set_is_in_schema_order` |
//! | a license is unforgeable outside this crate | RFC 0028, "Summary" | the `compile_fail` doctest below |
//!
//! ```compile_fail
//! // `License::issue` is `pub(crate)`: no consumer of this crate can mint permission to
//! // claim a guarantee whose checker never ran.
//! use continuum_context::guarantee::{Guarantee, GuaranteeSet, License};
//!
//! let mut set = GuaranteeSet::empty();
//! set.claim(License::issue(Guarantee::ReplayPreserving));
//! ```

use core::fmt;
use std::collections::BTreeSet;

use continuum_intent::canonical_json::Json;

/// One member of `context-pack.schema.json`'s closed thirteen-member `guarantees` enum.
///
/// Declared in the schema's own enum order, which is also this type's [`Ord`], so a sorted
/// `guarantees` array is stable across processes and platforms (`rule
/// ordering.deterministic`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Guarantee {
    /// The selected core replays to the same verdict under the pinned `semantic_epoch`.
    /// Checked by replay execution against `replay` (INV-006); stage 2 supplies only its
    /// precondition.
    ReplayPreserving,
    /// The slice preserves the property automaton's verdict-relevant structure. Checked by
    /// an *independent* property monitor — stage 3's licence, not this stage group's.
    PropertyPreserving,
    /// Every selected item lies on the proof dependency slice of the named obligations.
    /// Stage 5's licence.
    ProofRelevant,
    /// Ranked-relevant only; no semantic claim. Stage 9's licence, and mixing it into a
    /// preserved core is the INV-007 violation rule C4 exists to prevent.
    HeuristicRelevant,
    /// Removing any one selected item destroys the witness. Stage 8's licence, on a
    /// single-item removal transcript.
    OneMinimal,
    /// No smaller selected set witnesses the failure, within declared bounds. Stage 8.
    CardinalityMinimal,
    /// Minimal configuration under causal closure. Stage 8.
    CausallyMinimal,
    /// Domains and names reduced. Stage 8.
    ValueMinimal,
    /// Tasks and nodes reduced. Stage 8.
    OwnerMinimal,
    /// No unnecessary injected fault. Stage 8.
    FaultMinimal,
    /// A human/agent study objective was met. RFC 0028 correction 5: this one is *not*
    /// minimizer-checkable and "MUST NOT be emitted from a minimizer transcript".
    ExplanationMinimal,
    /// The selection is downward-closed under the causal order. Checked by
    /// [`crate::causal::CausalOrder::closure_violation`] — **stage 2's licence, and the only
    /// one this stage group issues.**
    CausallyClosed,
    /// Counterfactual items are valid under a *named* causality model.
    CounterfactualUnderNamedModel,
}

impl Guarantee {
    /// All thirteen members, in the schema's enum order.
    ///
    /// The schema's order and RFC 0028's guarantee *table* order differ (the table groups
    /// the minimality classes together; the schema does not). INV-003 decides: "schemas
    /// decide artifact shape, prose does not", so the schema's order is this type's [`Ord`]
    /// and therefore the published `guarantees` sort key.
    pub const ALL: [Self; 13] = [
        Self::ReplayPreserving,
        Self::PropertyPreserving,
        Self::ProofRelevant,
        Self::HeuristicRelevant,
        Self::OneMinimal,
        Self::CardinalityMinimal,
        Self::CausallyMinimal,
        Self::ValueMinimal,
        Self::OwnerMinimal,
        Self::FaultMinimal,
        Self::ExplanationMinimal,
        Self::CausallyClosed,
        Self::CounterfactualUnderNamedModel,
    ];

    /// The schema's wire token for this member.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::ReplayPreserving => "ReplayPreserving",
            Self::PropertyPreserving => "PropertyPreserving",
            Self::ProofRelevant => "ProofRelevant",
            Self::CausallyClosed => "CausallyClosed",
            Self::CounterfactualUnderNamedModel => "CounterfactualUnderNamedModel",
            Self::OneMinimal => "OneMinimal",
            Self::CardinalityMinimal => "CardinalityMinimal",
            Self::CausallyMinimal => "CausallyMinimal",
            Self::ValueMinimal => "ValueMinimal",
            Self::OwnerMinimal => "OwnerMinimal",
            Self::FaultMinimal => "FaultMinimal",
            Self::ExplanationMinimal => "ExplanationMinimal",
            Self::HeuristicRelevant => "HeuristicRelevant",
        }
    }

    /// Parse a wire token, failing closed on anything else.
    ///
    /// > A consumer that reads a guarantee […] it does not recognize MUST reject the pack
    /// > with `MalformedRequest` […] and MUST NOT treat the unknown token as absent, as
    /// > `HeuristicRelevant`, or as permission to read the remaining guarantees as complete.
    /// >
    /// > — RFC 0028, "Fail closed on unrecognized tokens"
    #[must_use]
    pub fn from_wire_str(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|member| member.as_wire_str() == text)
    }

    /// Whether this member makes any semantic claim at all.
    ///
    /// `HeuristicRelevant` is the one that does not, and rule C4 turns on the distinction.
    #[must_use]
    pub const fn is_semantic(self) -> bool {
        !matches!(self, Self::HeuristicRelevant)
    }
}

impl fmt::Display for Guarantee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// Permission to claim one guarantee, issued by the checker that established it.
///
/// The constructor is `pub(crate)`, so a license is unforgeable outside this crate and,
/// inside it, is only produced where a check actually ran. This is the whole mechanism by
/// which "a pack MUST NOT publish a guarantee whose checker did not run and pass" is a
/// property of the types rather than of somebody's diligence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct License(Guarantee);

impl License {
    /// Issue a license. Crate-private by design — see the type's documentation.
    #[must_use]
    pub(crate) const fn issue(guarantee: Guarantee) -> Self {
        Self(guarantee)
    }

    /// The guarantee this license permits.
    #[must_use]
    pub const fn guarantee(self) -> Guarantee {
        self.0
    }
}

/// What a caller *asked* a compile to achieve — never what it achieved.
///
/// This type has no conversion into [`GuaranteeSet`], which is the point.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestedGuarantees(BTreeSet<Guarantee>);

impl RequestedGuarantees {
    /// A request list.
    #[must_use]
    pub fn of(requested: impl IntoIterator<Item = Guarantee>) -> Self {
        Self(requested.into_iter().collect())
    }

    /// Nothing requested.
    #[must_use]
    pub fn none() -> Self {
        Self(BTreeSet::new())
    }

    /// The requested members, in schema order.
    #[must_use]
    pub fn members(&self) -> Vec<Guarantee> {
        self.0.iter().copied().collect()
    }

    /// The requested guarantees `achieved` does not contain.
    ///
    /// Each one is owed an omission record — "with reason `unsupported` (outside the
    /// engine's semantics) or `budget` (dropped by packing)" — and, where its absence makes
    /// the answer unusable, a typed `inconclusive` verdict (RFC 0028 C1).
    #[must_use]
    pub fn unachieved(&self, achieved: &GuaranteeSet) -> Vec<Guarantee> {
        self.0
            .iter()
            .filter(|guarantee| !achieved.contains(**guarantee))
            .copied()
            .collect()
    }
}

/// The guarantees a compile actually established, each one behind a [`License`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuaranteeSet(BTreeSet<Guarantee>);

impl GuaranteeSet {
    /// A compile that has established nothing yet.
    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    /// Record an established guarantee. Idempotent: one checker running twice claims once.
    pub fn claim(&mut self, license: License) {
        self.0.insert(license.guarantee());
    }

    /// Whether this compile established `guarantee`.
    #[must_use]
    pub fn contains(&self, guarantee: Guarantee) -> bool {
        self.0.contains(&guarantee)
    }

    /// Whether nothing was established.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The established members, in schema order.
    #[must_use]
    pub fn members(&self) -> Vec<Guarantee> {
        self.0.iter().copied().collect()
    }

    /// Seal the set for publication, enforcing rule C2.
    ///
    /// # Errors
    ///
    /// [`GuaranteeError::ReplayWithoutClosure`] when `ReplayPreserving` is claimed without
    /// `CausallyClosed`. RFC 0028 C2: "A set assembled from textual relevance is not
    /// accepted as replay-preserving."
    pub fn seal(self) -> Result<SealedGuarantees, GuaranteeError> {
        if self.contains(Guarantee::ReplayPreserving) && !self.contains(Guarantee::CausallyClosed) {
            return Err(GuaranteeError::ReplayWithoutClosure);
        }
        Ok(SealedGuarantees(self.0.into_iter().collect()))
    }
}

/// A published `guarantees` array: sealed under rule C2 and in schema order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedGuarantees(Vec<Guarantee>);

impl SealedGuarantees {
    /// The members, in schema order.
    #[must_use]
    pub fn members(&self) -> &[Guarantee] {
        &self.0
    }

    /// The `guarantees` array as canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::Array(
            self.0
                .iter()
                .map(|guarantee| Json::String(guarantee.as_wire_str().to_owned()))
                .collect(),
        )
    }
}

/// A way a guarantee set fails to be publishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuaranteeError {
    /// `ReplayPreserving` without `CausallyClosed` (rule C2).
    ReplayWithoutClosure,
}

impl fmt::Display for GuaranteeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReplayWithoutClosure => f.write_str(
                "`ReplayPreserving` is claimed without `CausallyClosed`; a core that replays \
                 contains every causal predecessor its replay consumes (RFC 0028 C2)",
            ),
        }
    }
}

impl core::error::Error for GuaranteeError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema's `guarantees` enum, transcribed from
    /// `notes/plan/schemas/context-pack.schema.json`, in its order.
    const SCHEMA_TOKENS: [&str; 13] = [
        "ReplayPreserving",
        "PropertyPreserving",
        "ProofRelevant",
        "HeuristicRelevant",
        "OneMinimal",
        "CardinalityMinimal",
        "CausallyMinimal",
        "ValueMinimal",
        "OwnerMinimal",
        "FaultMinimal",
        "ExplanationMinimal",
        "CausallyClosed",
        "CounterfactualUnderNamedModel",
    ];

    #[test]
    fn the_guarantee_set_is_exactly_the_schema_enum() {
        let ours: Vec<&str> = Guarantee::ALL
            .into_iter()
            .map(Guarantee::as_wire_str)
            .collect();
        assert_eq!(ours, SCHEMA_TOKENS);
    }

    #[test]
    fn every_guarantee_round_trips_through_its_wire_token() {
        for guarantee in Guarantee::ALL {
            assert_eq!(
                Guarantee::from_wire_str(guarantee.as_wire_str()),
                Some(guarantee)
            );
        }
    }

    #[test]
    fn an_unknown_guarantee_token_does_not_parse() {
        // Not absent, not `HeuristicRelevant`, not tolerated: `None`, which every caller
        // must turn into a rejection.
        assert_eq!(Guarantee::from_wire_str("Relevant"), None);
        assert_eq!(Guarantee::from_wire_str("replaypreserving"), None);
        assert_eq!(Guarantee::from_wire_str(""), None);
        assert_eq!(Guarantee::from_wire_str("1-minimal"), None);
    }

    #[test]
    fn only_heuristic_relevant_makes_no_semantic_claim() {
        let non_semantic: Vec<Guarantee> = Guarantee::ALL
            .into_iter()
            .filter(|guarantee| !guarantee.is_semantic())
            .collect();
        assert_eq!(non_semantic, [Guarantee::HeuristicRelevant]);
    }

    #[test]
    fn a_request_does_not_become_a_claim() {
        let requested =
            RequestedGuarantees::of([Guarantee::ReplayPreserving, Guarantee::CausallyClosed]);
        let achieved = GuaranteeSet::empty();
        // The request is a list of two; the pack claims nothing. There is no method that
        // turns the first into the second — this test pins the *shape* of C1, and the
        // module's `compile_fail` doctest pins that no external path exists at all.
        assert_eq!(requested.members().len(), 2);
        assert!(achieved.is_empty());
        assert_eq!(requested.unachieved(&achieved).len(), 2);
    }

    #[test]
    fn an_unachieved_request_is_named() {
        let requested = RequestedGuarantees::of([
            Guarantee::ReplayPreserving,
            Guarantee::CausallyClosed,
            Guarantee::OneMinimal,
        ]);
        let mut achieved = GuaranteeSet::empty();
        achieved.claim(License::issue(Guarantee::CausallyClosed));
        assert_eq!(
            requested.unachieved(&achieved),
            [Guarantee::ReplayPreserving, Guarantee::OneMinimal]
        );
    }

    #[test]
    fn an_unrequested_achievement_is_not_an_unachieved_request() {
        // The asymmetry matters: C1 is about echoing a request nobody achieved, not about
        // a compile that established more than it was asked for.
        let requested = RequestedGuarantees::none();
        let mut achieved = GuaranteeSet::empty();
        achieved.claim(License::issue(Guarantee::CausallyClosed));
        assert!(requested.unachieved(&achieved).is_empty());
    }

    #[test]
    fn claiming_twice_claims_once() {
        let mut set = GuaranteeSet::empty();
        set.claim(License::issue(Guarantee::CausallyClosed));
        set.claim(License::issue(Guarantee::CausallyClosed));
        assert_eq!(set.members(), [Guarantee::CausallyClosed]);
    }

    #[test]
    fn replay_preserving_without_causal_closure_is_refused() {
        let mut set = GuaranteeSet::empty();
        set.claim(License::issue(Guarantee::ReplayPreserving));
        assert_eq!(set.seal(), Err(GuaranteeError::ReplayWithoutClosure));
    }

    #[test]
    fn replay_preserving_with_causal_closure_seals() {
        let mut set = GuaranteeSet::empty();
        set.claim(License::issue(Guarantee::ReplayPreserving));
        set.claim(License::issue(Guarantee::CausallyClosed));
        let sealed = set.seal().expect("C2 is satisfied");
        assert_eq!(
            sealed.members(),
            [Guarantee::ReplayPreserving, Guarantee::CausallyClosed]
        );
    }

    #[test]
    fn causal_closure_alone_seals() {
        // C2 runs in one direction only: closure without replay is an ordinary claim.
        let mut set = GuaranteeSet::empty();
        set.claim(License::issue(Guarantee::CausallyClosed));
        assert!(set.seal().is_ok());
    }

    #[test]
    fn an_empty_set_seals_to_an_empty_array() {
        let sealed = GuaranteeSet::empty().seal().expect("nothing to check");
        assert!(sealed.members().is_empty());
        assert_eq!(sealed.to_json().to_canonical_bytes(), b"[]");
    }

    #[test]
    fn a_sealed_set_is_in_schema_order() {
        let mut set = GuaranteeSet::empty();
        // Claimed in reverse schema order; published in schema order.
        for guarantee in [
            Guarantee::HeuristicRelevant,
            Guarantee::OneMinimal,
            Guarantee::CausallyClosed,
            Guarantee::ReplayPreserving,
        ] {
            set.claim(License::issue(guarantee));
        }
        let sealed = set.seal().expect("C2 is satisfied");
        assert_eq!(
            sealed.to_json().to_canonical_bytes(),
            br#"["ReplayPreserving","HeuristicRelevant","OneMinimal","CausallyClosed"]"#
        );
    }
}
