//! **Stage 6**, observer projection — the one stage of this group whose input this workspace
//! actually has (RFC 0028, "Compiler pipeline"; INV-013; plan §5.2; RFC 0037).
//!
//! # Scope: bn-imhw2 — the compiler's stage group 3 (stages 5–7)
//!
//! > | 6 | observer projection | the intent's observers | scoping under INV-013; never a
//! > guarantee by itself |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! > **INV-013 — Property-scoped reduction.** Independence, symmetry, abstraction, slicing,
//! > and quotienting are justified relative to named observers/properties and fairness
//! > obligations.
//! >
//! > — plan §2
//!
//! # Why this stage is implemented and its neighbours are refused
//!
//! Stages 5 and 7 have no producing subsystem in this workspace ([`crate::proof`],
//! [`crate::correspondence`]). Stage 6 does. `continuum-observer` is a PR-1 / IMPL-01 scaffold
//! — but the *intent's* observers are not there. They are
//! [`continuum_intent::observers::ObserverSet`], a landed field group with a unit key
//! ([`ObserverId`]), the four RFC 0031 projections ([`ProjectionKind`]), decode/encode, a
//! canonical identity, and W1/W2 enforcement — and `continuum-intent` is already one of this
//! crate's three dependencies. RFC 0028 names stage 6's input as "the intent's observers", not
//! as an observer *engine*, so the input is present and the stage runs for real.
//!
//! The evidence is pinned rather than asserted: `tests/pr11_compiler_stages_5_7.rs` builds
//! stage 6's input out of `continuum_intent::observers` types and would stop compiling if that
//! group moved or narrowed.
//!
//! # The one thing the compiler is missing, and how it is supplied
//!
//! An observer's projections are sets of *element names* — event families, state components —
//! while a causal order's candidates are [`Name`] identities with a
//! [`crate::selection::SelectionKind`]. Nothing in this workspace relates the two: that
//! mapping is a property of the running system's instrumentation, which `continuum-cir` (a
//! stub) would carry. So it is a **declared input**, [`Observation`], exactly as the causal
//! order and the property automaton are: the deployment says which projection element each
//! candidate publishes into, and this module reduces relative to that declaration and nothing
//! else.
//!
//! A candidate with **no** declaration is *unattributed*, and stage 6 never drops one. That is
//! INV-013 read literally: a reduction must be *justified* relative to the named observers,
//! and there is no justification for removing something whose observability nobody declared.
//! No justification, no reduction — the same direction stage 4 takes with an uncorroborated
//! source claim, where "undecided" is never allowed to become "disproved".
//!
//! # The exclusion RFC 0028 forbids, made structural
//!
//! > A compiler MUST NOT exclude an abstraction-relevant hidden event on the grounds that no
//! > observer publishes it: INV-013 scopes reduction to named observers and properties, and an
//! > event the abstraction depends on is in scope for the property.
//! >
//! > — RFC 0028, "Selection and the causal core"
//!
//! This sentence is about *this stage*. It is the only stage that could commit the error,
//! because it is the only stage whose criterion is "does an observer publish it". So
//! [`ObserverFilter::retain`] takes a `protected` set — the property automaton's relevance set
//! when stage 3 ran, [`crate::property::PropertyAutomaton::relevant`] — and retains it
//! unconditionally, then re-closes under the causal order so the result is still downward
//! closed.
//!
//! A consequence worth stating because it is a theorem and not a coincidence: **when stage 3
//! has run, stage 6 drops nothing.** Stage 3's output is the backward closure of the relevance
//! set inside the slice, so every member is either relevant (protected) or a causal ancestor of
//! something relevant (restored by the re-closure). Observer scoping therefore *cannot* undo
//! property-directed slicing, which is exactly what RFC 0028's sentence demands, and
//! `stage_six_cannot_undo_stage_three` pins it. Where stage 6 does real narrowing is the
//! configuration RFC 0028 requires a deployment be able to run — the plain causal slice, with
//! stage 3 not configured.
//!
//! # Never a guarantee by itself
//!
//! RFC 0028's Licenses column for this stage is "scoping under INV-013; **never a guarantee by
//! itself**", and this module honours it by having nothing to honour it with: no
//! [`crate::guarantee::License`] is issued anywhere in stage 6's path, and
//! `stage_six_licenses_nothing` pins that the guarantee set of a compile is byte-equal with and
//! without the stage. The checker that decides whether the reduction was justified is
//! [`crate::scope::ScopeAudit`], and its affirmative verdict licenses nothing either — it is
//! an *audit*, deliberately not a licence.
//!
//! # Determinism (INV-005)
//!
//! Every set is a `BTree`, the attribution is a `BTreeMap`, the observer set is already
//! id-ordered, and the re-closure computes a set. No clock, no entropy, no hash-map iteration.
//!
//! # What is declined here
//!
//! - **Deciding an observer's projections.** Those are the intent contract's, and INV-001
//!   makes them protected data; this module reads them and never edits them. Coarsening one is
//!   a privileged intent revision (RFC 0031 `observers` → `coarsened` → `review`), not
//!   something a compiler does.
//! - **Fairness obligations.** INV-013 names three justification sources — observers,
//!   properties, and fairness obligations. This stage consults the first, stage 3 supplies the
//!   second, and the third has no producer here. That is precisely why every stage-6 drop is
//!   recorded `heuristic-cutoff` and never `slice-irrelevant`; see [`crate::compile`]'s
//!   "Which stage's reason a drop is recorded under".
//! - **The `knowledge_projection` and `security_projection` kinds have no candidate kind of
//!   their own** in `selected[]`'s eleven-member vocabulary. They are carried and compared all
//!   the same, because [`Observation`] is keyed by [`ProjectionKind`] rather than by candidate
//!   kind — a deployment that declares a candidate into one of them is answered, and this
//!   module invents no mapping between the two vocabularies.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the scope is what a named observer publishes | RFC 0028 stage 6 | `the_scope_is_what_a_named_observer_publishes` |
//! | an unattributed candidate is never dropped | INV-013 | `an_unattributed_candidate_is_never_dropped` |
//! | a protected candidate is never dropped | RFC 0028, "Selection and the causal core" | `a_protected_candidate_survives_the_projection` |
//! | the projection re-closes what it keeps | RFC 0028 stage 6 "with stage 2" | `the_projection_re_closes_what_it_keeps` |
//! | an empty observer set is inapplicable, not total | INV-008; the monitor's precedent | `an_intent_that_binds_no_observer_is_inapplicable` |
//! | a coarsened observer narrows the scope | RFC 0031, `observers` | `a_coarsened_observer_narrows_the_scope` |
//! | deterministic (INV-005) | INV-005 | `two_projections_of_one_input_agree` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::observers::{ObserverId, ObserverSet, ProjectionKind};
use continuum_value::value::Name;

use crate::causal::{CausalError, CausalOrder};

/// What one candidate publishes: the projection kind it appears in, and the element name it
/// appears as.
///
/// The element is a `String` because that is what an observer's four sets hold
/// (`intent-contract.schema.json`; `continuum_intent::observers`), and because that module is
/// explicit that "elements are *not* required to be non-empty strings: the schema states no
/// `minLength` for them". Narrowing it here would refuse contracts the schema admits, which is
/// the opposite of the narrowing [`crate::source::SourceRef`] is entitled to make.
///
/// INV-016 is unaffected: the element is compared for **equality** against the intent
/// contract's own declared sets and never reaches a `summary`, an omission record, or any
/// other rendered field. It is protected intent data, not untrusted source text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Observation {
    kind: ProjectionKind,
    element: String,
}

impl Observation {
    /// Declare that a candidate publishes into `kind` as `element`.
    #[must_use]
    pub fn new(kind: ProjectionKind, element: impl Into<String>) -> Self {
        Self {
            kind,
            element: element.into(),
        }
    }

    /// The projection this candidate publishes into.
    #[must_use]
    pub const fn kind(&self) -> ProjectionKind {
        self.kind
    }

    /// The element name it publishes as.
    #[must_use]
    pub fn element(&self) -> &str {
        &self.element
    }
}

impl fmt::Display for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.element)
    }
}

/// **Stage 6's premise**: the intent's observers, and the deployment's declaration of what each
/// candidate publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverProjection {
    observers: ObserverSet,
    attribution: BTreeMap<Name, Observation>,
}

impl ObserverProjection {
    /// Build the projection input.
    ///
    /// # Errors
    ///
    /// [`ProjectionError::RepeatedCandidate`] when one candidate is attributed twice. A
    /// candidate with two declared observabilities has no single reading, and silently keeping
    /// the last one would make the reduction a function of iteration order (INV-005) — the
    /// refusal [`crate::causal::CausalOrder::new`] makes for a repeated node, at this input.
    pub fn new(
        observers: ObserverSet,
        attribution: impl IntoIterator<Item = (Name, Observation)>,
    ) -> Result<Self, ProjectionError> {
        let mut declared: BTreeMap<Name, Observation> = BTreeMap::new();
        for (id, observation) in attribution {
            if declared.insert(id.clone(), observation).is_some() {
                return Err(ProjectionError::RepeatedCandidate { id });
            }
        }
        Ok(Self {
            observers,
            attribution: declared,
        })
    }

    /// The intent's observer set.
    #[must_use]
    pub const fn observers(&self) -> &ObserverSet {
        &self.observers
    }

    /// What this candidate publishes, where the deployment declared it.
    #[must_use]
    pub fn observation(&self, id: &Name) -> Option<&Observation> {
        self.attribution.get(id)
    }

    /// Whether this candidate's observability was declared at all.
    ///
    /// The distinction the reduction turns on: an *undeclared* candidate is never dropped.
    #[must_use]
    pub fn is_attributed(&self, id: &Name) -> bool {
        self.attribution.contains_key(id)
    }

    /// Every declared attribution, in canonical candidate order.
    pub fn attributed(&self) -> impl Iterator<Item = (&Name, &Observation)> {
        self.attribution.iter()
    }

    /// The named observers that publish this candidate, in id order.
    ///
    /// Empty for an unattributed candidate — "nobody declared it" is not "nobody sees it", and
    /// [`ObserverProjection::is_attributed`] is the predicate that keeps the two apart.
    #[must_use]
    pub fn observers_of(&self, id: &Name) -> BTreeSet<ObserverId> {
        let Some(observation) = self.attribution.get(id) else {
            return BTreeSet::new();
        };
        self.observers
            .iter()
            .filter(|observer| {
                observer
                    .projection(observation.kind())
                    .contains(observation.element())
            })
            .map(|observer| observer.id().clone())
            .collect()
    }

    /// Whether some named observer publishes this candidate.
    #[must_use]
    pub fn publishes(&self, id: &Name) -> bool {
        !self.observers_of(id).is_empty()
    }

    /// The candidates of `order` that some named observer publishes: the observer scope.
    #[must_use]
    pub fn scope(&self, order: &CausalOrder) -> BTreeSet<Name> {
        order
            .nodes()
            .map(|(id, _)| id)
            .filter(|id| self.publishes(id))
            .cloned()
            .collect()
    }

    /// The named observers this order gives anything to observe, in id order.
    ///
    /// This is the set a justified reduction is justified *relative to* (INV-013), and it is
    /// what [`crate::scope::ScopeVerdict::Justified`] reports.
    #[must_use]
    pub fn engaged_observers(&self, order: &CausalOrder) -> BTreeSet<ObserverId> {
        order
            .nodes()
            .flat_map(|(id, _)| self.observers_of(id))
            .collect()
    }

    /// Whether an observer-directed reduction is a *check* at all over this order.
    ///
    /// The discipline [`crate::monitor::Inapplicable`] states for the property monitor, applied
    /// to the second reduction justification INV-013 names: a reduction "justified" by an
    /// observer set that observes nothing here is justified by nothing, and a stage that
    /// narrowed on that basis would be reducing without a justification.
    #[must_use]
    pub fn applicability(&self, order: &CausalOrder) -> ProjectionApplicability {
        if self.observers.is_empty() {
            return ProjectionApplicability::Inapplicable(ProjectionInapplicable::NoNamedObserver);
        }
        if self.scope(order).is_empty() {
            return ProjectionApplicability::Inapplicable(
                ProjectionInapplicable::NoObservedCandidate,
            );
        }
        ProjectionApplicability::Applicable
    }
}

/// Whether stage 6 can reduce over an order at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionApplicability {
    /// A named observer publishes something here, so scoping is a real reduction.
    Applicable,
    /// It is not, and this is why. No reduction, and no accusation either.
    Inapplicable(
        /// Why the projection does not apply.
        ProjectionInapplicable,
    ),
}

impl ProjectionApplicability {
    /// Whether stage 6 may narrow.
    #[must_use]
    pub const fn is_applicable(self) -> bool {
        matches!(self, Self::Applicable)
    }

    /// A stable token for the auditable intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applicable => "observer-projected",
            Self::Inapplicable(_) => "observer-inapplicable",
        }
    }
}

/// Why observer projection does not apply to an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectionInapplicable {
    /// The intent binds no observer.
    ///
    /// RFC 0037 is explicit that an empty `observers` array "is a *declaration that the set is
    /// empty*, and is classified as such; it is not 'unspecified'" — so this is a real, read
    /// declaration and not a missing input. What it cannot be is a *justification*: every
    /// reduction it would license is vacuous.
    NoNamedObserver,
    /// The intent binds observers, and none of them publishes any candidate of this order.
    ///
    /// Distinct from [`ProjectionInapplicable::NoNamedObserver`] for the reason
    /// [`crate::monitor::Inapplicable::EmptyAlphabet`] and
    /// [`crate::monitor::Inapplicable::NoObservedEvent`] are distinct: one is a property of the
    /// intent, the other of this execution.
    NoObservedCandidate,
}

impl ProjectionInapplicable {
    /// Both members, in declaration order.
    pub const ALL: [Self; 2] = [Self::NoNamedObserver, Self::NoObservedCandidate];

    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoNamedObserver => "no-named-observer",
            Self::NoObservedCandidate => "no-observed-candidate",
        }
    }
}

impl fmt::Display for ProjectionInapplicable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoNamedObserver => f.write_str(
                "the intent binds no observer, so every observer-scoped reduction it would \
                 justify is vacuous (INV-013)",
            ),
            Self::NoObservedCandidate => f.write_str(
                "no named observer publishes any candidate of this causal order, so scoping \
                 reduces nothing that was justified",
            ),
        }
    }
}

/// **Stage 6's computation**: narrow a slice to what the named observers scope, keeping what
/// the property protects.
///
/// This is the *producer*, not the checker. Nothing here licenses anything — and, unlike
/// stages 2 and 3, there is nothing for it to license: RFC 0028's Licenses column for stage 6
/// reads "never a guarantee by itself". [`crate::scope::ScopeAudit`] decides whether the
/// reduction was justified, and its verdict is an audit rather than a licence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserverFilter;

impl ObserverFilter {
    /// The observer-scoped sub-slice of `slice`, re-closed under `order`.
    ///
    /// A member of `slice` is a seed when **any** of three things holds:
    ///
    /// 1. it is in `protected` — the property's relevance set, which observer scoping may not
    ///    remove (RFC 0028, "Selection and the causal core"; INV-013);
    /// 2. its observability was never declared — no justification, no reduction;
    /// 3. some named observer publishes it.
    ///
    /// The result is the backward closure of the seeds, intersected back with `slice`. The
    /// closure is what keeps the output downward closed (stage 6 sits after stage 2 and must
    /// not break the guarantee it licensed); the intersection matters only where the input was
    /// not closed — a slice with a redaction hole in it — and stops stage 6 re-admitting what
    /// the pre-pass removed, exactly as [`crate::property::PropertyFilter::retain`] does.
    ///
    /// # Errors
    ///
    /// [`CausalError::UnknownEndpoint`] when `slice` names something outside `order`.
    pub fn retain(
        projection: &ObserverProjection,
        order: &CausalOrder,
        slice: &BTreeSet<Name>,
        protected: &BTreeSet<Name>,
    ) -> Result<BTreeSet<Name>, CausalError> {
        let seeds: BTreeSet<Name> = slice
            .iter()
            .filter(|id| {
                protected.contains(*id) || !projection.is_attributed(id) || projection.publishes(id)
            })
            .cloned()
            .collect();
        let closed = order.backward_closure(&seeds)?;
        Ok(closed.intersection(slice).cloned().collect())
    }
}

/// A way stage 6's premise fails to be one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// One candidate is attributed twice.
    RepeatedCandidate {
        /// The repeated identity.
        id: Name,
    },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedCandidate { id } => write!(
                f,
                "`{id}` is attributed to two observations; a candidate with two declared \
                 observabilities has no single reading"
            ),
        }
    }
}

impl core::error::Error for ProjectionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::SelectionKind;
    use continuum_intent::observers::Observer;

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

    /// `e_begin -> e_submit -> e_ack -> e_loss`, plus an isolated `e_flush` and an isolated
    /// `e_probe`.
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

    fn event(element: &str) -> Observation {
        Observation::new(ProjectionKind::Events, element)
    }

    /// The client publishes the durability chain's tail and nothing else; `e_probe` is
    /// declared into a family no observer names; `e_flush` and `e_begin` are unattributed.
    fn projection() -> ObserverProjection {
        ObserverProjection::new(
            observers([observer("client", &["Submitted", "Acked", "Lost"])]),
            [
                (name("e_submit"), event("Submitted")),
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Lost")),
                (name("e_probe"), event("Diagnostic")),
            ],
        )
        .expect("a well-formed projection")
    }

    fn universe() -> BTreeSet<Name> {
        order().nodes().map(|(id, _)| id.clone()).collect()
    }

    #[test]
    fn the_scope_is_what_a_named_observer_publishes() {
        let projection = projection();
        assert_eq!(
            projection.scope(&order()),
            ids(&["e_submit", "e_ack", "e_loss"])
        );
        assert!(projection.publishes(&name("e_ack")));
        // Declared into a family nobody names: attributed, and out of scope.
        assert!(projection.is_attributed(&name("e_probe")));
        assert!(!projection.publishes(&name("e_probe")));
        // Never declared: not in scope, and not droppable either.
        assert!(!projection.is_attributed(&name("e_flush")));
        assert!(projection.observers_of(&name("e_flush")).is_empty());
    }

    #[test]
    fn the_named_observers_are_reported_by_id() {
        let projection = ObserverProjection::new(
            observers([
                observer("client", &["Acked"]),
                observer("auditor", &["Acked", "Lost"]),
            ]),
            [
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Lost")),
            ],
        )
        .expect("a well-formed projection");
        let ack: Vec<String> = projection
            .observers_of(&name("e_ack"))
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(ack, ["auditor", "client"]);
        assert_eq!(
            projection
                .observers_of(&name("e_loss"))
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>(),
            ["auditor"]
        );
        assert_eq!(
            projection
                .engaged_observers(&order())
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>(),
            ["auditor", "client"]
        );
    }

    #[test]
    fn an_unattributed_candidate_is_never_dropped() {
        let kept = ObserverFilter::retain(&projection(), &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        assert!(kept.contains(&name("e_flush")), "unattributed, so kept");
        // And the attributed-but-unobserved one is the only drop.
        assert_eq!(
            kept,
            ids(&["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"])
        );
    }

    #[test]
    fn a_protected_candidate_survives_the_projection() {
        // `e_probe` is attributed and no observer publishes it, so the projection drops it —
        // unless the property protects it, which is the one exclusion RFC 0028 forbids.
        let unprotected =
            ObserverFilter::retain(&projection(), &order(), &universe(), &BTreeSet::new())
                .expect("inside the order");
        assert!(!unprotected.contains(&name("e_probe")));

        let protected =
            ObserverFilter::retain(&projection(), &order(), &universe(), &ids(&["e_probe"]))
                .expect("inside the order");
        assert!(protected.contains(&name("e_probe")));
    }

    #[test]
    fn the_projection_re_closes_what_it_keeps() {
        // `e_begin` is unattributed here, so keep the anti-vacuity honest by attributing it
        // into a family no observer names: it is then droppable on its own criterion, and is
        // kept only because `e_submit` needs it.
        let projection = ObserverProjection::new(
            observers([observer("client", &["Acked"])]),
            [
                (name("e_begin"), event("Internal")),
                (name("e_submit"), event("Internal")),
                (name("e_ack"), event("Acked")),
            ],
        )
        .expect("a well-formed projection");
        let kept = ObserverFilter::retain(&projection, &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        assert!(kept.contains(&name("e_begin")));
        assert!(kept.contains(&name("e_submit")));
        assert!(order().is_downward_closed(&kept));
        // `e_loss` is unattributed and kept; `e_probe` is unattributed and kept. What went is
        // nothing — every drop candidate was an ancestor of the observed `e_ack`.
        assert_eq!(kept, universe());
    }

    #[test]
    fn a_slice_the_projection_narrows_stays_downward_closed() {
        let projection = ObserverProjection::new(
            observers([observer("client", &["Acked"])]),
            [
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Internal")),
                (name("e_probe"), event("Internal")),
                (name("e_flush"), event("Internal")),
                (name("e_begin"), event("Internal")),
                (name("e_submit"), event("Internal")),
            ],
        )
        .expect("a well-formed projection");
        let kept = ObserverFilter::retain(&projection, &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        assert_eq!(kept, ids(&["e_begin", "e_submit", "e_ack"]));
        assert!(order().is_downward_closed(&kept));
    }

    #[test]
    fn a_coarsened_observer_narrows_the_scope() {
        // RFC 0031's `observers` → `coarsened` move, seen from the compiler: dropping one
        // element from one set shrinks what the pack is scoped over. The anti-vacuity for the
        // whole module — a projection that reduced the same way whatever the observers said
        // would make every assertion above meaningless.
        let wide = ObserverProjection::new(
            observers([observer("client", &["Submitted", "Acked", "Lost"])]),
            [
                (name("e_submit"), event("Submitted")),
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Lost")),
            ],
        )
        .expect("a well-formed projection");
        let coarse = ObserverProjection::new(
            observers([observer("client", &["Submitted"])]),
            [
                (name("e_submit"), event("Submitted")),
                (name("e_ack"), event("Acked")),
                (name("e_loss"), event("Lost")),
            ],
        )
        .expect("a well-formed projection");
        assert_eq!(wide.scope(&order()), ids(&["e_submit", "e_ack", "e_loss"]));
        assert_eq!(coarse.scope(&order()), ids(&["e_submit"]));

        let wide_kept = ObserverFilter::retain(&wide, &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        let coarse_kept = ObserverFilter::retain(&coarse, &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        assert!(coarse_kept.len() < wide_kept.len());
        assert!(!coarse_kept.contains(&name("e_ack")));
    }

    #[test]
    fn an_intent_that_binds_no_observer_is_inapplicable() {
        let projection = ObserverProjection::new(observers([]), [(name("e_ack"), event("Acked"))])
            .expect("a well-formed projection");
        assert_eq!(
            projection.applicability(&order()),
            ProjectionApplicability::Inapplicable(ProjectionInapplicable::NoNamedObserver)
        );
        assert!(!projection.applicability(&order()).is_applicable());
    }

    #[test]
    fn an_order_no_named_observer_publishes_is_inapplicable() {
        let projection = ObserverProjection::new(
            observers([observer("client", &["Elsewhere"])]),
            [(name("e_ack"), event("Acked"))],
        )
        .expect("a well-formed projection");
        assert_eq!(
            projection.applicability(&order()),
            ProjectionApplicability::Inapplicable(ProjectionInapplicable::NoObservedCandidate)
        );
        // The two arms are distinct facts: one about the intent, one about this execution.
        assert_ne!(
            ProjectionInapplicable::NoNamedObserver.as_str(),
            ProjectionInapplicable::NoObservedCandidate.as_str()
        );
        assert_eq!(ProjectionInapplicable::ALL.len(), 2);
    }

    #[test]
    fn an_applicable_projection_says_so() {
        assert_eq!(
            projection().applicability(&order()),
            ProjectionApplicability::Applicable
        );
        assert_eq!(
            ProjectionApplicability::Applicable.as_str(),
            "observer-projected"
        );
        assert_eq!(
            ProjectionApplicability::Inapplicable(ProjectionInapplicable::NoNamedObserver).as_str(),
            "observer-inapplicable"
        );
    }

    #[test]
    fn a_candidate_attributed_twice_is_refused() {
        assert_eq!(
            ObserverProjection::new(
                observers([observer("client", &["Acked"])]),
                [
                    (name("e_ack"), event("Acked")),
                    (name("e_ack"), event("Something")),
                ],
            ),
            Err(ProjectionError::RepeatedCandidate { id: name("e_ack") })
        );
    }

    #[test]
    fn the_four_projection_kinds_are_all_usable_and_do_not_cross() {
        // A candidate declared into `state_projection` is not published by an observer whose
        // *events* set happens to hold the same string. The kinds are compared, not just the
        // element names.
        let projection = ObserverProjection::new(
            observers([observer("client", &["pending"])]),
            [(
                name("e_ack"),
                Observation::new(ProjectionKind::State, "pending"),
            )],
        )
        .expect("a well-formed projection");
        assert!(!projection.publishes(&name("e_ack")));

        let state_observer = Observer::new(
            ObserverId::new("client").expect("a non-empty id"),
            [
                (ProjectionKind::Events, Vec::new()),
                (ProjectionKind::State, vec!["pending".to_owned()]),
            ],
        )
        .expect("a well-formed observer");
        let matching = ObserverProjection::new(
            observers([state_observer]),
            [(
                name("e_ack"),
                Observation::new(ProjectionKind::State, "pending"),
            )],
        )
        .expect("a well-formed projection");
        assert!(matching.publishes(&name("e_ack")));
        assert_eq!(
            Observation::new(ProjectionKind::State, "pending").to_string(),
            "state_projection:pending"
        );
    }

    #[test]
    fn a_slice_from_outside_the_order_is_refused() {
        let mut stray = universe();
        stray.insert(name("z_9"));
        assert_eq!(
            ObserverFilter::retain(&projection(), &order(), &stray, &BTreeSet::new()),
            Err(CausalError::UnknownEndpoint { id: name("z_9") })
        );
    }

    #[test]
    fn two_projections_of_one_input_agree() {
        let one = ObserverFilter::retain(&projection(), &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        let other = ObserverFilter::retain(&projection(), &order(), &universe(), &BTreeSet::new())
            .expect("inside the order");
        assert_eq!(one, other);
        assert_eq!(projection(), projection());
    }
}
