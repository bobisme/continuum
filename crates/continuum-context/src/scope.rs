//! The **independent audit** of stage 6's observer-scoped reduction (RFC 0028, "Validation";
//! INV-013).
//!
//! # Scope: bn-imhw2 — the compiler's stage group 3 (stages 5–7)
//!
//! # An audit, deliberately not a licence
//!
//! > | 6 | observer projection | the intent's observers | scoping under INV-013; **never a
//! > guarantee by itself** |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! Every other checker in this crate ends in a [`crate::guarantee::License`]:
//! [`crate::causal::CausalOrder::closure_violation`] licenses `CausallyClosed`,
//! [`crate::monitor::PropertyMonitor`] licenses `PropertyPreserving`. This one ends in a
//! [`ScopeVerdict`] and **nothing else**, because RFC 0028's Licenses column for stage 6 names
//! no member of the closed thirteen. `ScopeVerdict::Justified` is not a guarantee, does not
//! become one, and cannot be traded for one: there is no path from this module to
//! `License::issue`, and `stage_six_licenses_nothing` pins that a compile's guarantee set is
//! equal with and without stage 6.
//!
//! Making the audit exist anyway is the point. INV-013 says reductions "are justified relative
//! to named observers/properties and fairness obligations"; a reduction whose justification
//! nobody checks is a reduction on the compiler's word, which is the thing docs/33's
//! "Architectural separation" and INV-004's discipline exist to refuse. So the reduction is
//! checked, the check is independent, and the result is recorded — it simply does not license a
//! wire-visible claim, because the RFC does not give it one to license.
//!
//! # The independence argument
//!
//! The same three separations [`crate::monitor`] states, at this stage:
//!
//! 1. **It is a different computation.** [`crate::observer::ObserverFilter::retain`] *selects*:
//!    it seeds from the scope and takes a backward closure. [`ScopeAudit::verdict`] *examines
//!    each dropped candidate individually* against the observer set, the protected set, and the
//!    attribution, and then re-checks closure. Neither result is derivable from the other by
//!    reading the other's code; a filter bug shows up here as a named violation rather than as
//!    a shared assumption.
//! 2. **It reads only what a checker may read.** [`ScopeAudit::verdict`] takes a
//!    [`crate::causal::CausalOrder`] and a [`ScopeReduction`]. It is not handed the filter's
//!    seeds, the trail, the accounting, or the stage-6 decision, and every reduction it can be
//!    asked about — including ones the filter cannot produce — gets an answer decided from its
//!    own premise. That is what makes
//!    `the_audit_rejects_hand_built_reductions_the_filter_cannot_produce` falsifiable rather
//!    than a restatement.
//! 3. **It is a different module, and this one names nothing of the filter's.** `scope.rs`
//!    imports [`crate::observer::ObserverProjection`] — the shared *premise*, exactly as
//!    `monitor.rs` imports `PropertyAutomaton` — and imports neither `ObserverFilter` nor
//!    [`crate::compile`]. `the_scope_audit_names_nothing_of_the_filters` reads this file's own
//!    bytes and pins it.
//!
//! And, as there, what the separation does **not** buy: the audit and the filter are given the
//! same *observers*. [`crate::compile::CausalCompile::with_observer_projection`] takes the two
//! as separate arguments so a caller may audit against a wider observer set than the filter was
//! given; where they disagree, the audit wins and the reduction is recorded unjustified.
//!
//! # What the audit checks, and why in this order
//!
//! 1. The reduction **added** something → [`ScopeViolation::CandidateAdded`]. A projection is a
//!    narrowing; a stage that grew the set was not scoping.
//! 2. The projection does not apply here ([`crate::observer::ObserverProjection::applicability`]):
//!    - and the reduction dropped nothing → [`ScopeVerdict::Inapplicable`]. No justification was
//!      needed because no reduction was made.
//!    - and it dropped something → [`ScopeViolation::VacuousReduction`]. A reduction justified
//!      by an observer set that observes nothing here is justified by nothing, and this is the
//!      arm that stops "no observer publishes it" from becoming a licence to drop everything.
//! 3. Each dropped candidate, in canonical order:
//!    - it is **protected** → [`ScopeViolation::ProtectedCandidateDropped`]. Reported before the
//!      observer check because it is the specific rule RFC 0028 states — "a compiler MUST NOT
//!      exclude an abstraction-relevant hidden event on the grounds that no observer publishes
//!      it" — and reporting it as a mere scope error would hide the rule behind a symptom.
//!    - a **named observer publishes** it → [`ScopeViolation::ObservedCandidateDropped`], naming
//!      the observer, because INV-013's justification is *relative to* a named observer and a
//!      violation should name the one it contradicts.
//!    - its observability was **never declared** →
//!      [`ScopeViolation::UnattributedCandidateDropped`]. No justification, no reduction.
//! 4. The result is not downward closed → [`ScopeViolation::NotCausallyClosed`]. Stage 6 sits
//!    after stage 2 and must not break the guarantee stage 2 licensed.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the audit is independent of the filter | RFC 0028, "Validation" | `the_audit_rejects_hand_built_reductions_the_filter_cannot_produce` |
//! | a protected candidate may not be scoped away | RFC 0028, "Selection and the causal core" | `dropping_a_protected_candidate_is_the_named_violation` |
//! | a reduction names the observer it contradicts | INV-013 | `dropping_an_observed_candidate_names_the_observer` |
//! | an undeclared candidate cannot be reduced away | INV-013 | `dropping_an_unattributed_candidate_is_unjustified` |
//! | a vacuous justification is not a justification | INV-008; the monitor's precedent | `a_reduction_under_an_inapplicable_projection_is_vacuous` |
//! | closure survives the projection | RFC 0028 stage 6 "with stage 2" | `an_unclosed_result_is_unjustified` |
//! | the honest reduction passes | anti-vacuity | `the_filters_own_output_is_justified` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_intent::observers::ObserverId;
use continuum_value::value::Name;

use crate::causal::{CausalOrder, ClosureViolation};
use crate::observer::{ObserverProjection, ProjectionApplicability, ProjectionInapplicable};

/// The reduction stage 6 performed, stated as an auditable object.
///
/// Publicly constructible on purpose: the audit is only a checker if a *hand-built* reduction —
/// one [`crate::observer::ObserverFilter`] would never produce — can be handed to it and get an
/// answer decided from the premise alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeReduction {
    before: BTreeSet<Name>,
    after: BTreeSet<Name>,
    protected: BTreeSet<Name>,
}

impl ScopeReduction {
    /// A reduction from `before` to `after`, over a `protected` set the projection may not
    /// touch.
    #[must_use]
    pub fn new(
        before: impl IntoIterator<Item = Name>,
        after: impl IntoIterator<Item = Name>,
        protected: impl IntoIterator<Item = Name>,
    ) -> Self {
        Self {
            before: before.into_iter().collect(),
            after: after.into_iter().collect(),
            protected: protected.into_iter().collect(),
        }
    }

    /// The working set stage 6 was handed.
    #[must_use]
    pub const fn before(&self) -> &BTreeSet<Name> {
        &self.before
    }

    /// The working set it left.
    #[must_use]
    pub const fn after(&self) -> &BTreeSet<Name> {
        &self.after
    }

    /// The set observer scoping may not remove — the property automaton's relevance set where
    /// stage 3 ran, and empty where it did not.
    #[must_use]
    pub const fn protected(&self) -> &BTreeSet<Name> {
        &self.protected
    }

    /// What the reduction removed, in canonical order.
    #[must_use]
    pub fn dropped(&self) -> BTreeSet<Name> {
        self.before.difference(&self.after).cloned().collect()
    }

    /// What the reduction added — empty for every reduction that is one.
    #[must_use]
    pub fn added(&self) -> BTreeSet<Name> {
        self.after.difference(&self.before).cloned().collect()
    }

    /// Whether the reduction removed nothing.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.before == self.after
    }
}

/// The checker that decides whether an observer-scoped reduction was justified.
///
/// Built over a projection the caller supplies *separately* from the one stage 6 was given —
/// see the module documentation's "The independence argument".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeAudit {
    projection: ObserverProjection,
}

impl ScopeAudit {
    /// Audit against `projection`.
    #[must_use]
    pub const fn over(projection: ObserverProjection) -> Self {
        Self { projection }
    }

    /// The projection this audit checks against.
    #[must_use]
    pub const fn projection(&self) -> &ObserverProjection {
        &self.projection
    }

    /// Decide whether `reduction` is justified relative to the named observers.
    ///
    /// The check order is the module documentation's, and each step is stated there with the
    /// reason it comes where it does.
    #[must_use]
    pub fn verdict(&self, order: &CausalOrder, reduction: &ScopeReduction) -> ScopeVerdict {
        if let Some(added) = reduction.added().into_iter().next() {
            return ScopeVerdict::Unjustified(ScopeViolation::CandidateAdded { id: added });
        }

        let dropped = reduction.dropped();
        if let ProjectionApplicability::Inapplicable(reason) = self.projection.applicability(order)
        {
            return match dropped.into_iter().next() {
                None => ScopeVerdict::Inapplicable(reason),
                Some(id) => {
                    ScopeVerdict::Unjustified(ScopeViolation::VacuousReduction { id, reason })
                }
            };
        }

        for id in &dropped {
            if reduction.protected().contains(id) {
                return ScopeVerdict::Unjustified(ScopeViolation::ProtectedCandidateDropped {
                    id: id.clone(),
                });
            }
            if let Some(observer) = self.projection.observers_of(id).into_iter().next() {
                return ScopeVerdict::Unjustified(ScopeViolation::ObservedCandidateDropped {
                    id: id.clone(),
                    observer,
                });
            }
            if !self.projection.is_attributed(id) {
                return ScopeVerdict::Unjustified(ScopeViolation::UnattributedCandidateDropped {
                    id: id.clone(),
                });
            }
        }

        if let Some(violation) = order.closure_violation(reduction.after()) {
            return ScopeVerdict::Unjustified(ScopeViolation::NotCausallyClosed(violation));
        }

        ScopeVerdict::Justified {
            observers: self.projection.engaged_observers(order),
        }
    }
}

/// What the audit concluded about one reduction.
///
/// Three arms, not a boolean, and none of them a licence: "justified", "demonstrably not
/// justified", and "the question does not arise here" are three different facts (INV-008's
/// discipline, at a checker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeVerdict {
    /// The reduction is justified relative to these named observers (INV-013).
    ///
    /// **This licenses nothing.** RFC 0028: stage 6 is "never a guarantee by itself".
    Justified {
        /// The named observers this order gives something to observe, in id order.
        observers: BTreeSet<ObserverId>,
    },
    /// The reduction is not justified, and the audit names how.
    Unjustified(
        /// The first violation, in the check order the module documentation states.
        ScopeViolation,
    ),
    /// Observer scoping does not apply to this order, and no reduction was made.
    Inapplicable(
        /// Why it does not apply.
        ProjectionInapplicable,
    ),
}

impl ScopeVerdict {
    /// Whether the reduction was justified.
    ///
    /// Note what this is *not*: a predicate any guarantee is gated on. Nothing in this crate
    /// reads it to issue a [`crate::guarantee::License`].
    #[must_use]
    pub const fn is_justified(&self) -> bool {
        matches!(self, Self::Justified { .. })
    }

    /// A stable token for an auditable intermediate's rendering — a closed vocabulary member's
    /// own spelling, never caller-supplied prose (INV-016).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Justified { .. } => "scope-justified",
            Self::Unjustified(_) => "scope-unjustified",
            Self::Inapplicable(_) => "scope-inapplicable",
        }
    }
}

impl fmt::Display for ScopeVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Justified { observers } => write!(
                f,
                "the reduction is justified relative to {} named observer(s) (INV-013)",
                observers.len()
            ),
            Self::Unjustified(violation) => write!(f, "{violation}"),
            Self::Inapplicable(reason) => write!(f, "{reason}"),
        }
    }
}

/// How a reduction fails to be justified relative to the named observers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeViolation {
    /// The "reduction" put something in that was not there. A projection is a narrowing.
    CandidateAdded {
        /// The first added identity, in canonical order.
        id: Name,
    },
    /// Something was dropped under a projection that justifies no reduction here.
    VacuousReduction {
        /// The first dropped identity, in canonical order.
        id: Name,
        /// Why the projection does not apply.
        reason: ProjectionInapplicable,
    },
    /// A candidate the property protects was dropped.
    ///
    /// > A compiler MUST NOT exclude an abstraction-relevant hidden event on the grounds that
    /// > no observer publishes it.
    /// >
    /// > — RFC 0028, "Selection and the causal core"
    ProtectedCandidateDropped {
        /// The first such identity, in canonical order.
        id: Name,
    },
    /// A candidate a named observer publishes was dropped.
    ObservedCandidateDropped {
        /// The dropped identity.
        id: Name,
        /// The first named observer that publishes it, in id order.
        observer: ObserverId,
    },
    /// A candidate whose observability nobody declared was dropped: no justification, no
    /// reduction.
    UnattributedCandidateDropped {
        /// The dropped identity.
        id: Name,
    },
    /// The reduction's result is not downward closed under the causal order.
    NotCausallyClosed(
        /// The first way closure fails.
        ClosureViolation,
    ),
}

impl fmt::Display for ScopeViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CandidateAdded { id } => write!(
                f,
                "`{id}` is in the projection's output and not in its input; observer projection \
                 is a narrowing"
            ),
            Self::VacuousReduction { id, reason } => write!(
                f,
                "`{id}` was dropped although {reason}; a reduction justified by nothing is not \
                 justified (INV-013)"
            ),
            Self::ProtectedCandidateDropped { id } => write!(
                f,
                "`{id}` is in scope for the property and was dropped by observer projection; no \
                 observer publishing it is not a ground for excluding it (RFC 0028, INV-013)"
            ),
            Self::ObservedCandidateDropped { id, observer } => write!(
                f,
                "`{id}` is published by the named observer `{observer}` and was dropped; the \
                 reduction contradicts the observer it claims to be justified against (INV-013)"
            ),
            Self::UnattributedCandidateDropped { id } => write!(
                f,
                "`{id}` has no declared observability and was dropped; a reduction must be \
                 justified relative to a named observer, and nothing justifies this one \
                 (INV-013)"
            ),
            Self::NotCausallyClosed(violation) => write!(
                f,
                "{violation}; observer projection runs after stage 2 and may not break the \
                 closure stage 2 licensed"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::{Observation, ObserverFilter};
    use crate::selection::SelectionKind;
    use continuum_intent::observers::{Observer, ObserverSet, ProjectionKind};

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    fn observer(id: &str, events: &[&str]) -> Observer {
        Observer::new(
            ObserverId::new(id).expect("a non-empty id"),
            [(
                ProjectionKind::Events,
                events.iter().map(|event| (*event).to_owned()).collect(),
            )],
        )
        .expect("a well-formed observer")
    }

    fn observers(members: impl IntoIterator<Item = Observer>) -> ObserverSet {
        ObserverSet::from_observers(members).expect("a well-formed set")
    }

    fn event(element: &str) -> Observation {
        Observation::new(ProjectionKind::Events, element)
    }

    fn order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_submit"), SelectionKind::Event),
                (name("e_ack"), SelectionKind::Event),
                (name("e_loss"), SelectionKind::Event),
                (name("e_flush"), SelectionKind::Event),
                (name("e_probe"), SelectionKind::Event),
            ],
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
        )
        .expect("a well-formed order")
    }

    fn universe() -> BTreeSet<Name> {
        order().nodes().map(|(id, _)| id.clone()).collect()
    }

    /// Everything is attributed, so every drop is decidable; `client` publishes the chain's
    /// tail only.
    fn projection() -> ObserverProjection {
        ObserverProjection::new(
            observers([observer("client", &["Acked"])]),
            [
                (name("e_begin"), event("Internal")),
                (name("e_submit"), event("Internal")),
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Internal")),
                (name("e_flush"), event("Internal")),
                (name("e_probe"), event("Internal")),
            ],
        )
        .expect("a well-formed projection")
    }

    fn audit() -> ScopeAudit {
        ScopeAudit::over(projection())
    }

    fn honest_reduction(protected: &BTreeSet<Name>) -> ScopeReduction {
        let after = ObserverFilter::retain(&projection(), &order(), &universe(), protected)
            .expect("inside the order");
        ScopeReduction::new(universe(), after, protected.iter().cloned())
    }

    #[test]
    fn the_filters_own_output_is_justified() {
        let reduction = honest_reduction(&BTreeSet::new());
        assert_eq!(reduction.after(), &ids(&["e_begin", "e_submit", "e_ack"]));
        let verdict = audit().verdict(&order(), &reduction);
        assert!(verdict.is_justified());
        assert_eq!(verdict.as_str(), "scope-justified");
        let ScopeVerdict::Justified { observers } = verdict else {
            panic!("justified");
        };
        assert_eq!(
            observers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["client"]
        );
    }

    /// The independence claim, made falsifiable — the precedent
    /// `the_monitor_rejects_hand_built_selections_the_filter_cannot_produce` set for stage 3.
    #[test]
    fn the_audit_rejects_hand_built_reductions_the_filter_cannot_produce() {
        let order = order();
        // Drops the one candidate the named observer publishes.
        assert!(matches!(
            audit().verdict(
                &order,
                &ScopeReduction::new(universe(), ids(&["e_begin", "e_submit"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::ObservedCandidateDropped { .. })
        ));
        // Grows the set instead of narrowing it.
        assert_eq!(
            audit().verdict(
                &order,
                &ScopeReduction::new(ids(&["e_begin"]), ids(&["e_begin", "e_ack"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::CandidateAdded { id: name("e_ack") })
        );
        // Keeps the observed candidate and drops its causal predecessor.
        assert!(matches!(
            audit().verdict(
                &order,
                &ScopeReduction::new(universe(), ids(&["e_submit", "e_ack"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::NotCausallyClosed(
                ClosureViolation::MissingPredecessor { .. }
            ))
        ));
        // And the honest one still passes, so the audit is not merely a rejector.
        assert!(
            audit()
                .verdict(&order, &honest_reduction(&BTreeSet::new()))
                .is_justified()
        );
    }

    #[test]
    fn dropping_a_protected_candidate_is_the_named_violation() {
        // `e_probe` is attributed and unobserved, so the projection would drop it — and the
        // audit refuses the drop the moment the property protects it. Reported as the
        // protection violation rather than as a generic scope error, which is the whole point
        // of the check order.
        assert_eq!(
            audit().verdict(
                &order(),
                &ScopeReduction::new(
                    universe(),
                    ids(&["e_begin", "e_submit", "e_ack"]),
                    ids(&["e_probe"])
                )
            ),
            ScopeVerdict::Unjustified(ScopeViolation::ProtectedCandidateDropped {
                id: name("e_probe")
            })
        );
        // And the filter, given the same protection, does not produce that reduction at all.
        let honest = honest_reduction(&ids(&["e_probe"]));
        assert!(honest.after().contains(&name("e_probe")));
        assert!(audit().verdict(&order(), &honest).is_justified());
    }

    #[test]
    fn dropping_an_observed_candidate_names_the_observer() {
        assert_eq!(
            audit().verdict(
                &order(),
                &ScopeReduction::new(universe(), ids(&["e_begin", "e_submit"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::ObservedCandidateDropped {
                id: name("e_ack"),
                observer: ObserverId::new("client").expect("a non-empty id"),
            })
        );
    }

    #[test]
    fn dropping_an_unattributed_candidate_is_unjustified() {
        let projection = ObserverProjection::new(
            observers([observer("client", &["Acked"])]),
            [(name("e_ack"), event("Acked"))],
        )
        .expect("a well-formed projection");
        // `e_probe` was never declared, so nothing justifies removing it.
        assert_eq!(
            ScopeAudit::over(projection).verdict(
                &order(),
                &ScopeReduction::new(
                    universe(),
                    ids(&["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"]),
                    []
                )
            ),
            ScopeVerdict::Unjustified(ScopeViolation::UnattributedCandidateDropped {
                id: name("e_probe")
            })
        );
    }

    #[test]
    fn a_reduction_under_an_inapplicable_projection_is_vacuous() {
        let unbound = ScopeAudit::over(
            ObserverProjection::new(observers([]), [(name("e_ack"), event("Acked"))])
                .expect("a well-formed projection"),
        );
        // Nothing dropped: no justification was needed, so no accusation is made.
        assert_eq!(
            unbound.verdict(&order(), &ScopeReduction::new(universe(), universe(), [])),
            ScopeVerdict::Inapplicable(ProjectionInapplicable::NoNamedObserver)
        );
        // Something dropped: an empty observer set justifies no reduction at all.
        assert_eq!(
            unbound.verdict(
                &order(),
                &ScopeReduction::new(universe(), ids(&["e_begin"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::VacuousReduction {
                id: name("e_ack"),
                reason: ProjectionInapplicable::NoNamedObserver,
            })
        );
    }

    #[test]
    fn an_order_no_named_observer_publishes_is_inapplicable_and_not_justified() {
        let elsewhere = ScopeAudit::over(
            ObserverProjection::new(
                observers([observer("client", &["Elsewhere"])]),
                [(name("e_ack"), event("Acked"))],
            )
            .expect("a well-formed projection"),
        );
        let verdict = elsewhere.verdict(&order(), &ScopeReduction::new(universe(), universe(), []));
        assert_eq!(
            verdict,
            ScopeVerdict::Inapplicable(ProjectionInapplicable::NoObservedCandidate)
        );
        assert!(!verdict.is_justified());
        assert_eq!(verdict.as_str(), "scope-inapplicable");
    }

    #[test]
    fn an_unclosed_result_is_unjustified() {
        // Every drop is individually justified — `e_begin` is attributed and unobserved — and
        // the result is still refused, because stage 6 may not break stage 2's closure.
        assert!(matches!(
            audit().verdict(
                &order(),
                &ScopeReduction::new(ids(&["e_begin", "e_submit"]), ids(&["e_submit"]), [])
            ),
            ScopeVerdict::Unjustified(ScopeViolation::NotCausallyClosed(_))
        ));
    }

    #[test]
    fn a_stricter_audit_than_the_filter_refuses_the_reduction() {
        // The two premises are supplied separately, so a caller may audit against a wider
        // observer set than the filter was given. The audit wins.
        let stricter = ScopeAudit::over(
            ObserverProjection::new(
                observers([observer("auditor", &["Acked", "Internal"])]),
                [
                    (name("e_begin"), event("Internal")),
                    (name("e_submit"), event("Internal")),
                    (name("e_ack"), event("Acked")),
                    (name("e_loss"), event("Internal")),
                    (name("e_flush"), event("Internal")),
                    (name("e_probe"), event("Internal")),
                ],
            )
            .expect("a well-formed projection"),
        );
        let reduction = honest_reduction(&BTreeSet::new());
        assert!(audit().verdict(&order(), &reduction).is_justified());
        assert!(matches!(
            stricter.verdict(&order(), &reduction),
            ScopeVerdict::Unjustified(ScopeViolation::ObservedCandidateDropped { .. })
        ));
    }

    #[test]
    fn the_reduction_reports_its_own_shape() {
        let reduction = ScopeReduction::new(universe(), ids(&["e_begin"]), ids(&["e_flush"]));
        assert_eq!(reduction.before(), &universe());
        assert_eq!(reduction.after(), &ids(&["e_begin"]));
        assert_eq!(reduction.protected(), &ids(&["e_flush"]));
        assert_eq!(reduction.dropped().len(), 5);
        assert!(reduction.added().is_empty());
        assert!(!reduction.is_identity());
        assert!(ScopeReduction::new(universe(), universe(), []).is_identity());
    }

    #[test]
    fn two_audits_of_one_reduction_agree() {
        let reduction = honest_reduction(&BTreeSet::new());
        assert_eq!(
            audit().verdict(&order(), &reduction),
            audit().verdict(&order(), &reduction)
        );
    }
}
