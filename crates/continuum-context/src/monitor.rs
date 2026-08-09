//! The **independent property monitor** that licenses `PropertyPreserving` (RFC 0028,
//! "Validation").
//!
//! # Scope: bn-1kj2n — the compiler's stage group 2 (stages 3–4)
//!
//! > - **An independent property monitor** on `PropertyPreserving` — an implementation that
//! >   reuses the compiler's own automaton has checked nothing.
//! >
//! > — RFC 0028, "Validation"
//!
//! # The independence argument
//!
//! Three separations, in decreasing order of how much they buy:
//!
//! 1. **It is a different computation, not the same one run twice.**
//!    [`crate::property::PropertyFilter`] *selects*: it seeds from the relevance set and takes
//!    a backward closure. [`PropertyMonitor`] *runs the automaton* — twice, once over the whole
//!    causal order and once over the proposed selection — and compares the two runs. Neither
//!    result is derivable from the other by reading the other's code; a filter bug shows up
//!    here as a divergence rather than as a shared assumption. This is the closure checker's
//!    precedent exactly ([`crate::causal::CausalOrder::closure_violation`]: "reads the order's
//!    edges and the selection it is handed, knowing nothing about how that selection was
//!    made").
//! 2. **It reads only what a checker may read.** [`PropertyMonitor::verdict`] takes a
//!    [`crate::causal::CausalOrder`] and a selection. It is not handed the filter's
//!    intermediate, the trail, the accounting, or the stage-3 decision. Every selection it can
//!    be asked about — including ones the filter cannot produce — gets an answer decided from
//!    the automaton and the order alone, which is what makes
//!    `the_monitor_rejects_hand_built_selections_the_filter_cannot_produce` a falsifiable claim
//!    rather than a restatement.
//! 3. **It is a different module, and this one names nothing of the filter's.** `monitor.rs`
//!    imports [`crate::property::PropertyAutomaton`] — the shared *premise*, which both sides
//!    must of course agree about, exactly as stage 2's slicer and checker share
//!    [`crate::causal::CausalOrder`] — and imports neither `PropertyFilter` nor
//!    [`crate::compile`]. That is mechanically pinned from outside the crate by
//!    `the_monitor_module_names_nothing_of_the_filters`, which reads this file's own bytes.
//!
//! What the separation does **not** buy, stated so nothing downstream over-reads it: the
//! monitor and the filter are given the same *property*. RFC 0028's sentence is about reusing
//! the compiler's automaton *implementation* — a "check" that replays the filter's own
//! decision — not about the two disagreeing over what the property is. A caller may
//! deliberately supply a stricter monitor than the filter was given, and
//! [`crate::compile::CausalCompile::with_property`] takes the two as separate arguments so
//! that is expressible; where they disagree, the monitor wins and the guarantee is refused.
//! And C6 is unchanged: the artifact records no checker identity, so "a promotion-relevant
//! consumer MUST re-run the checker rather than read the field".
//!
//! # What the monitor checks, and why in this order
//!
//! RFC 0028's "Formal model" states the obligation this monitor is the finite instance of:
//!
//! > **Preservation** — for a finite causal graph and a property automaton, a causally closed
//! > slice containing the automaton's observed events yields the same verdict as the full
//! > graph.
//! >
//! > — RFC 0028, "Formal model"
//!
//! The check is the theorem's own shape: the two hypotheses, then the conclusion.
//!
//! 1. The automaton declares no observed events at all → [`Inapplicable::EmptyAlphabet`].
//! 2. Nothing in this execution is observed → [`Inapplicable::NoObservedEvent`]. This is the
//!    boundary the guarantee's anti-vacuity turns on: the runs *would* agree, trivially, and a
//!    monitor that answered "preserving" there would license a claim no check established.
//!    Both arms decline the licence and name why (INV-008's discipline, at a checker).
//! 3. The observed sequences differ → [`MonitorDisagreement::ObservedEventDropped`], naming
//!    the first observed event, in the order's own topological order, that the selection lacks.
//! 4. An abstraction-relevant hidden event present in the order is missing from the selection →
//!    [`MonitorDisagreement::AbstractionDependencyDropped`]. It is reported *before* the
//!    closure violation it usually also causes, because "a compiler MUST NOT exclude an
//!    abstraction-relevant hidden event on the grounds that no observer publishes it" is the
//!    specific rule RFC 0028 states, and a report of "not causally closed" would hide it behind
//!    a symptom.
//! 5. The selection is not downward closed → [`MonitorDisagreement::NotCausallyClosed`]. The
//!    theorem's other hypothesis, and the reason the pipeline table reads "`PropertyPreserving`,
//!    **with stage 2**".
//! 6. The verdicts differ → [`MonitorDisagreement::VerdictDiverged`]. With 3–5 passed this arm
//!    is unreachable, because the two observed sequences are then equal and the automaton is
//!    deterministic; it is kept because the *check* is the run comparison and 3 is its
//!    diagnosis, so a future automaton whose verdict is not a function of its observed sequence
//!    alone fails here rather than passing silently. (The crate's precedent for a deliberately
//!    unreachable typed arm is [`crate::compile::CompileError::Unreachable`].)
//!
//! # What is declined here
//!
//! - **Deciding what the pack does about a disagreement.** The monitor returns a verdict; it
//!   does not drop items, rewrite a slice, or fail a compile. [`crate::compile`] claims the
//!   guarantee exactly when the verdict is [`MonitorVerdict::Preserving`] and publishes the
//!   selection either way, which is stage 2's own precedent — "a slicer bug costs the guarantee
//!   instead of producing a false one".
//! - **A per-item property attribution.** `selected[]` items carry no guarantee field at
//!   `schema_epoch` 1 (RFC 0028 F2); the monitor's verdict is about the selection as a whole.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the licence needs an independent monitor | RFC 0028, "Validation" | `the_monitor_rejects_hand_built_selections_the_filter_cannot_produce` |
//! | a dropped observed event is caught | RFC 0028, "Formal model" | `a_dropped_observed_event_is_caught_even_when_the_slice_is_closed` |
//! | a dropped hidden dependency is caught | RFC 0028, "Selection and the causal core" | `a_dropped_abstraction_dependency_is_caught` |
//! | closure is a hypothesis of preservation | RFC 0028, "Formal model" | `an_unclosed_selection_is_not_preserving` |
//! | an unobserved execution is typed, not preserving | anti-vacuity; INV-008 | `an_execution_the_automaton_does_not_observe_is_inapplicable` |
//! | the honest slice passes | anti-vacuity | `the_whole_order_and_its_property_core_both_preserve` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::value::Name;

use crate::causal::{CausalOrder, ClosureViolation};
use crate::property::{AutomatonVerdict, PropertyAutomaton};

/// The checker that licenses `PropertyPreserving`.
///
/// Built over an automaton the caller supplies *separately* from the one the filter was given
/// — see the module documentation's "The independence argument".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyMonitor {
    automaton: PropertyAutomaton,
}

impl PropertyMonitor {
    /// Monitor `automaton`.
    #[must_use]
    pub const fn over(automaton: PropertyAutomaton) -> Self {
        Self { automaton }
    }

    /// The property this monitor checks against.
    #[must_use]
    pub const fn automaton(&self) -> &PropertyAutomaton {
        &self.automaton
    }

    /// Decide whether `selection` preserves the property automaton's verdict-relevant
    /// structure, relative to the whole of `order`.
    ///
    /// The check order is the preservation obligation's own shape; see the module
    /// documentation.
    #[must_use]
    pub fn verdict(&self, order: &CausalOrder, selection: &BTreeSet<Name>) -> MonitorVerdict {
        if self.automaton.alphabet().is_empty() {
            return MonitorVerdict::Inapplicable(Inapplicable::EmptyAlphabet);
        }

        let topological = order.topological_order();
        if !topological.iter().any(|id| self.automaton.observes(id)) {
            return MonitorVerdict::Inapplicable(Inapplicable::NoObservedEvent);
        }

        let sliced: Vec<Name> = topological
            .iter()
            .filter(|id| selection.contains(*id))
            .cloned()
            .collect();
        let whole_run = self.automaton.run(&topological);
        let slice_run = self.automaton.run(&sliced);

        if whole_run.observed() != slice_run.observed()
            && let Some(dropped) = whole_run
                .observed()
                .iter()
                .find(|id| !selection.contains(*id))
                .cloned()
        {
            return MonitorVerdict::Divergent(MonitorDisagreement::ObservedEventDropped {
                id: dropped,
            });
        }

        if let Some(dropped) = self
            .automaton
            .hidden_dependencies()
            .iter()
            .find(|id| order.contains(id) && !selection.contains(*id))
            .cloned()
        {
            return MonitorVerdict::Divergent(MonitorDisagreement::AbstractionDependencyDropped {
                id: dropped,
            });
        }

        if let Some(violation) = order.closure_violation(selection) {
            return MonitorVerdict::Divergent(MonitorDisagreement::NotCausallyClosed(violation));
        }

        if whole_run.verdict() != slice_run.verdict() {
            return MonitorVerdict::Divergent(MonitorDisagreement::VerdictDiverged {
                whole: whole_run.verdict(),
                slice: slice_run.verdict(),
            });
        }

        MonitorVerdict::Preserving
    }
}

/// What the monitor concluded about one selection.
///
/// Three arms, not a boolean: "preserved", "demonstrably not preserved", and "the question
/// does not arise here" are three different facts, and only the first licenses anything
/// (INV-008's discipline, at a checker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorVerdict {
    /// The selection preserves the automaton's verdict-relevant structure. This is the only
    /// verdict that licenses `PropertyPreserving`.
    Preserving,
    /// The monitor disagrees with the selection, and names how.
    Divergent(MonitorDisagreement),
    /// The check does not apply to this execution, and names why. No licence, and no
    /// accusation either.
    Inapplicable(Inapplicable),
}

impl MonitorVerdict {
    /// Whether this verdict licenses `PropertyPreserving`.
    #[must_use]
    pub const fn is_preserving(&self) -> bool {
        matches!(self, Self::Preserving)
    }

    /// A stable token for an auditable intermediate's rendering — a closed vocabulary
    /// member's own spelling, never caller-supplied prose (INV-016).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Preserving => "property-preserving",
            Self::Divergent(_) => "monitor-disagreement",
            Self::Inapplicable(_) => "monitor-inapplicable",
        }
    }
}

impl fmt::Display for MonitorVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preserving => f.write_str(
                "the selection preserves the property automaton's verdict-relevant structure",
            ),
            Self::Divergent(disagreement) => write!(f, "{disagreement}"),
            Self::Inapplicable(reason) => write!(f, "{reason}"),
        }
    }
}

/// How a selection fails to preserve the automaton's verdict-relevant structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorDisagreement {
    /// An event the automaton steps on is in the order and not in the selection, so the two
    /// runs observe different sequences.
    ObservedEventDropped {
        /// The first such event, in the order's topological order.
        id: Name,
    },
    /// An abstraction-relevant hidden event is in the order and not in the selection.
    ///
    /// > A compiler MUST NOT exclude an abstraction-relevant hidden event on the grounds that
    /// > no observer publishes it.
    /// >
    /// > — RFC 0028, "Selection and the causal core"
    AbstractionDependencyDropped {
        /// The first such event, in canonical order.
        id: Name,
    },
    /// The selection is not downward closed, so the preservation obligation's other
    /// hypothesis fails.
    NotCausallyClosed(ClosureViolation),
    /// The two runs reached different verdicts. See the module documentation: with the checks
    /// above passed this arm is unreachable for a deterministic automaton, and it is kept so
    /// that a future one whose verdict is not a function of its observed sequence fails here
    /// rather than passing silently.
    VerdictDiverged {
        /// What the whole order concluded.
        whole: AutomatonVerdict,
        /// What the selection concluded.
        slice: AutomatonVerdict,
    },
}

impl fmt::Display for MonitorDisagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservedEventDropped { id } => write!(
                f,
                "`{id}` is observed by the property automaton and is not selected; the slice's \
                 run and the whole order's run observe different sequences"
            ),
            Self::AbstractionDependencyDropped { id } => write!(
                f,
                "`{id}` is an abstraction-relevant hidden event and is not selected; no \
                 observer publishing it is not a ground for excluding it (RFC 0028, INV-013)"
            ),
            Self::NotCausallyClosed(violation) => write!(
                f,
                "{violation}; a slice that is not causally closed cannot be property-preserving \
                 on this argument (RFC 0028, \"Formal model\")"
            ),
            Self::VerdictDiverged { whole, slice } => write!(
                f,
                "the whole order runs to `{whole}` and the selection runs to `{slice}`"
            ),
        }
    }
}

/// Why the preservation check does not apply to an execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Inapplicable {
    /// The automaton declares no transitions, so it observes nothing anywhere and can
    /// distinguish no two slices. A property problem, not an execution one.
    EmptyAlphabet,
    /// The automaton has an alphabet, and no candidate of this causal order is in it. The two
    /// runs would agree vacuously, which is not a check.
    NoObservedEvent,
}

impl Inapplicable {
    /// Both members, in declaration order.
    pub const ALL: [Self; 2] = [Self::EmptyAlphabet, Self::NoObservedEvent];

    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyAlphabet => "empty-alphabet",
            Self::NoObservedEvent => "no-observed-event",
        }
    }
}

impl fmt::Display for Inapplicable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAlphabet => f.write_str(
                "the property automaton declares no transitions, so it observes nothing and \
                 preserving it is not a checkable claim",
            ),
            Self::NoObservedEvent => f.write_str(
                "no candidate of this causal order is observed by the property automaton, so \
                 the two runs agree vacuously and nothing was checked",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::{AutomatonState, Coverage, PropertyAutomaton, PropertyFilter};
    use crate::selection::SelectionKind;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn state(text: &str) -> AutomatonState {
        AutomatonState::new(name(text))
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    /// `e_begin -> e_submit -> e_ack -> e_loss`, plus an isolated hidden `e_flush` and an
    /// isolated, property-irrelevant `e_probe`.
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

    fn automaton() -> PropertyAutomaton {
        PropertyAutomaton::new(
            state("q_0"),
            [
                (state("q_0"), name("e_submit"), state("q_1")),
                (state("q_1"), name("e_ack"), state("q_2")),
                (state("q_2"), name("e_loss"), state("q_bad")),
            ],
            [state("q_bad")],
            [name("e_flush")],
            Coverage::Total,
        )
        .expect("a well-formed automaton")
    }

    fn monitor() -> PropertyMonitor {
        PropertyMonitor::over(automaton())
    }

    /// The honest core: the property-directed sub-slice of the whole order.
    fn property_core() -> BTreeSet<Name> {
        PropertyFilter::retain(
            &automaton(),
            &order(),
            &ids(&[
                "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe",
            ]),
        )
        .expect("inside the order")
    }

    #[test]
    fn the_whole_order_and_its_property_core_both_preserve() {
        let order = order();
        let universe: BTreeSet<Name> = order.nodes().map(|(id, _)| id.clone()).collect();
        assert_eq!(
            monitor().verdict(&order, &universe),
            MonitorVerdict::Preserving
        );

        let core = property_core();
        assert_eq!(
            core,
            ids(&["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"])
        );
        assert_eq!(monitor().verdict(&order, &core), MonitorVerdict::Preserving);
        assert!(monitor().verdict(&order, &core).is_preserving());
        assert_eq!(MonitorVerdict::Preserving.as_str(), "property-preserving");
    }

    /// The independence claim, made falsifiable — the precedent
    /// `the_checker_rejects_hand_built_selections_the_slicer_cannot_produce` set for stage 2.
    /// Each selection below is hand-built and [`PropertyFilter`] cannot produce it; the
    /// monitor decides each from the automaton and the order alone.
    #[test]
    fn the_monitor_rejects_hand_built_selections_the_filter_cannot_produce() {
        let order = order();

        // The whole causal core minus one observed event, still closed.
        assert_eq!(
            monitor().verdict(&order, &ids(&["e_begin", "e_submit", "e_flush"])),
            MonitorVerdict::Divergent(MonitorDisagreement::ObservedEventDropped {
                id: name("e_ack"),
            })
        );
        // Everything observed, and the hidden dependency excluded.
        assert_eq!(
            monitor().verdict(&order, &ids(&["e_begin", "e_submit", "e_ack", "e_loss"])),
            MonitorVerdict::Divergent(MonitorDisagreement::AbstractionDependencyDropped {
                id: name("e_flush"),
            })
        );
        // Everything relevant, and a causal predecessor missing.
        assert_eq!(
            monitor().verdict(&order, &ids(&["e_submit", "e_ack", "e_loss", "e_flush"])),
            MonitorVerdict::Divergent(MonitorDisagreement::NotCausallyClosed(
                ClosureViolation::MissingPredecessor {
                    selected: name("e_submit"),
                    predecessor: name("e_begin"),
                }
            ))
        );
        // And the honest one still passes, so the monitor is not merely a rejector.
        assert!(monitor().verdict(&order, &property_core()).is_preserving());
    }

    /// The load-bearing negative: a selection the *closure* checker licenses and the property
    /// monitor refuses. Two guarantees, two checkers, and neither implies the other.
    #[test]
    fn a_dropped_observed_event_is_caught_even_when_the_slice_is_closed() {
        let order = order();
        let truncated = ids(&["e_begin", "e_submit", "e_flush"]);
        assert!(order.is_downward_closed(&truncated));
        assert_eq!(order.closure_violation(&truncated), None);
        assert_eq!(
            monitor().verdict(&order, &truncated),
            MonitorVerdict::Divergent(MonitorDisagreement::ObservedEventDropped {
                id: name("e_ack"),
            })
        );
    }

    #[test]
    fn a_dropped_abstraction_dependency_is_caught() {
        let order = order();
        // A filter written against the alphabet alone produces exactly this: the closure of
        // the observed events, with the hidden dependency excluded because no observer
        // publishes it. It is causally closed, and the monitor still refuses it.
        let observed_only = ids(&["e_begin", "e_submit", "e_ack", "e_loss"]);
        assert!(order.is_downward_closed(&observed_only));
        assert_eq!(
            monitor().verdict(&order, &observed_only),
            MonitorVerdict::Divergent(MonitorDisagreement::AbstractionDependencyDropped {
                id: name("e_flush"),
            })
        );
    }

    #[test]
    fn an_unclosed_selection_is_not_preserving() {
        let order = order();
        assert!(matches!(
            monitor().verdict(&order, &ids(&["e_submit", "e_ack", "e_loss", "e_flush"])),
            MonitorVerdict::Divergent(MonitorDisagreement::NotCausallyClosed(_))
        ));
    }

    #[test]
    fn a_selection_from_outside_the_order_is_not_preserving() {
        let order = order();
        let mut stray = property_core();
        stray.insert(name("z_9"));
        assert_eq!(
            monitor().verdict(&order, &stray),
            MonitorVerdict::Divergent(MonitorDisagreement::NotCausallyClosed(
                ClosureViolation::NotInOrder {
                    selected: name("z_9"),
                }
            ))
        );
    }

    #[test]
    fn an_execution_the_automaton_does_not_observe_is_inapplicable() {
        let elsewhere = PropertyAutomaton::new(
            state("q_0"),
            [(state("q_0"), name("e_elsewhere"), state("q_1"))],
            [state("q_1")],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton");
        assert_eq!(
            PropertyMonitor::over(elsewhere).verdict(&order(), &property_core()),
            MonitorVerdict::Inapplicable(Inapplicable::NoObservedEvent)
        );
    }

    #[test]
    fn an_automaton_that_observes_nothing_at_all_is_inapplicable() {
        let silent = PropertyAutomaton::new(state("q_0"), [], [], [], Coverage::Total)
            .expect("a well-formed automaton");
        let verdict = PropertyMonitor::over(silent).verdict(&order(), &property_core());
        assert_eq!(
            verdict,
            MonitorVerdict::Inapplicable(Inapplicable::EmptyAlphabet)
        );
        assert!(!verdict.is_preserving());
        assert_eq!(verdict.as_str(), "monitor-inapplicable");
    }

    #[test]
    fn an_empty_selection_of_an_observed_execution_is_a_disagreement_not_an_inapplicability() {
        // The distinction the two arms exist to keep: "the property observes nothing here" is
        // not the same fact as "the slice kept none of what the property observes".
        assert_eq!(
            monitor().verdict(&order(), &BTreeSet::new()),
            MonitorVerdict::Divergent(MonitorDisagreement::ObservedEventDropped {
                id: name("e_submit"),
            })
        );
    }

    #[test]
    fn the_inapplicable_reasons_have_distinct_tokens() {
        let tokens: BTreeSet<&str> = Inapplicable::ALL
            .into_iter()
            .map(Inapplicable::as_str)
            .collect();
        assert_eq!(tokens.len(), Inapplicable::ALL.len());
    }

    #[test]
    fn two_runs_of_one_monitor_agree() {
        let order = order();
        let core = property_core();
        assert_eq!(
            monitor().verdict(&order, &core),
            monitor().verdict(&order, &core)
        );
    }
}
