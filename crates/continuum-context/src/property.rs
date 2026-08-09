//! The property automaton stage 3 filters against, and stage 3's own computation
//! (RFC 0028, "Compiler pipeline" stage 3; "Selection and the causal core").
//!
//! # Scope: bn-1kj2n — the compiler's stage group 2 (stages 3–4)
//!
//! > | 3 | property-automaton relevance filtering | the property automaton |
//! > `PropertyPreserving`, with stage 2 |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! This module holds the **input** and the **filter**. It deliberately does *not* hold the
//! checker: RFC 0028's Validation section requires "an independent property monitor on
//! `PropertyPreserving` — an implementation that reuses the compiler's own automaton has
//! checked nothing", so the checker is [`crate::monitor`], a separate module that names
//! nothing in this one except [`PropertyAutomaton`] itself. See that module's "The
//! independence argument" for what the separation buys and what it does not.
//!
//! # Why the automaton is declared here and not imported
//!
//! The same reason [`crate::causal::CausalOrder`] is: `continuum-cir` is a documented stub in
//! this workspace and `continuum-context` may not import an engine
//! (`tools/check_crate_boundaries.py`). So the automaton is a **value the compiler is
//! handed** — a finite, deterministic monitor over candidate identities — and any producer (a
//! CML property elaboration, an LTL-to-Büchi translation, a hand-written safety monitor)
//! projects into it. Stating it as a premise rather than computing it is also what makes the
//! checker's independence expressible at all: a compiler that derived its own automaton and
//! then checked preservation against that derivation would be checking its own arithmetic.
//!
//! # What "relevant" means, and the one exclusion RFC 0028 forbids
//!
//! > The **causal core** is the sub-slice required by the claimed preserved guarantees. […]
//! > A compiler MUST NOT exclude an abstraction-relevant hidden event on the grounds that no
//! > observer publishes it: INV-013 scopes reduction to named observers and properties, and
//! > an event the abstraction depends on is in scope for the property.
//! >
//! > — RFC 0028, "Selection and the causal core"
//!
//! An automaton therefore declares **two** sets, not one: its [`PropertyAutomaton::alphabet`]
//! — the events it steps on, which is what an observer publishes — and its
//! [`PropertyAutomaton::hidden_dependencies`] — the events the abstraction depends on that no
//! observer publishes. [`PropertyAutomaton::relevant`] is their union and is what
//! [`PropertyFilter`] keeps. A filter written against the alphabet alone is precisely the
//! compiler that sentence prohibits, and [`crate::monitor::PropertyMonitor`] rejects its
//! output at [`crate::monitor::MonitorDisagreement::AbstractionDependencyDropped`].
//!
//! The two sets are disjoint by construction ([`AutomatonError::ObservedIsNotHidden`]): an
//! event the automaton steps on is published to it, so calling it hidden as well would make
//! the distinction the RFC's sentence turns on unrecoverable from the artifact.
//!
//! # Coverage is declared, because a drop is only a proof in a complete declaration
//!
//! > `slice-irrelevant` is reserved for items *provably* outside the property-directed slice;
//! > an item dropped because the compiler could not decide is `heuristic-cutoff`, never
//! > `slice-irrelevant`.
//! >
//! > — RFC 0028, "Omission manifest"
//!
//! Stage 3 is the stage that computes the *property-directed* slice, so this is where that
//! sentence is exact rather than analogous. Whether a stage-3 drop is a proof turns on
//! whether the automaton's declaration is exhaustive: under [`Coverage::Total`] the relevance
//! set names everything the property can depend on, non-membership is a decision by set
//! comparison, and the drop is `slice-irrelevant`; under [`Coverage::Partial`] an
//! unenumerated hidden dependency could have made the candidate relevant, so the same
//! computation proves nothing and the drop is `heuristic-cutoff`.
//!
//! [`Coverage::omission_reason`] makes the choice a consequence of the input's own
//! declaration rather than a judgement a compiler author makes per call — the discipline
//! [`crate::causal::Completeness`] set for stage 2, applied to the second input that can be
//! partial.
//!
//! # Determinism (INV-005)
//!
//! The automaton is deterministic by construction: transitions are a `BTreeMap` keyed by
//! state and then by symbol, so `(state, symbol)` has at most one target and declaring one
//! twice is [`AutomatonError::TransitionRepeated`] rather than a last-writer-wins race. A run
//! is a fold over a sequence the caller supplies; this module holds no clock, no entropy and
//! no hash-map iteration.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the alphabet is what the automaton steps on | this module's premise | `the_alphabet_is_exactly_the_transition_symbols` |
//! | an observed event is not also hidden | RFC 0028, "Selection and the causal core" | `an_observed_event_cannot_also_be_hidden` |
//! | transitions are deterministic | INV-005 | `a_repeated_transition_is_refused` |
//! | a violating state must be a state | this module's premise | `a_violating_state_from_nowhere_is_refused` |
//! | a run steps only on alphabet symbols | this module's premise | `a_run_ignores_symbols_outside_the_alphabet` |
//! | the verdict machinery is not vacuous | anti-vacuity | `two_traces_reach_two_verdicts` |
//! | the filter keeps hidden dependencies | RFC 0028, "Selection and the causal core" | `the_filter_keeps_an_abstraction_relevant_hidden_event` |
//! | the filter's output stays downward closed | RFC 0028 stage 3 "with stage 2" | `the_filter_re_closes_what_it_keeps` |
//! | total vs partial coverage decides the reason | RFC 0028, "Omission manifest" | `coverage_decides_the_omission_reason` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Name;

use crate::causal::{CausalError, CausalOrder};
use crate::omission::OmissionReason;

/// One state of a [`PropertyAutomaton`].
///
/// A newtype over [`Name`] rather than a bare `Name`, so a state and a symbol cannot be
/// swapped at a call site: both are canonical identifiers and the transition relation reads
/// `(state, symbol) -> state`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AutomatonState(Name);

impl AutomatonState {
    /// Name a state.
    #[must_use]
    pub const fn new(name: Name) -> Self {
        Self(name)
    }

    /// The state's identity.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.0
    }
}

impl fmt::Display for AutomatonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// What a run of the automaton concluded.
///
/// Two members, because a monitor over a finite trace concludes exactly one of them. This is
/// *not* the pack's `verdict` field (`crate::verdict::Verdict`, four members including the
/// typed `inconclusive` arm): a pack's verdict is the evaluation's, and an automaton run is
/// one input to it. Conflating them would put an engine's inconclusiveness inside a monitor
/// that cannot express it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AutomatonVerdict {
    /// The run ended outside every violating state.
    Satisfied,
    /// The run ended in a violating state.
    Violated,
}

impl AutomatonVerdict {
    /// Both members, in declaration order.
    pub const ALL: [Self; 2] = [Self::Satisfied, Self::Violated];

    /// A stable token for an intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Violated => "violated",
        }
    }
}

impl fmt::Display for AutomatonVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether the automaton's relevance declaration names everything the property can depend on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Coverage {
    /// The producer declares the relevance set exhaustive: a candidate outside
    /// [`PropertyAutomaton::relevant`] is a candidate the property cannot depend on. A
    /// stage-3 drop is then a proof.
    Total,
    /// The producer declares the relevance set partial, and names why. A stage-3 drop then
    /// proves nothing, so it is recorded as `heuristic-cutoff`.
    Partial {
        /// Why the declaration is incomplete.
        reason: CoverageGap,
    },
}

impl Coverage {
    /// The manifest reason a stage-3 drop is recorded under, given this declaration.
    #[must_use]
    pub const fn omission_reason(&self) -> OmissionReason {
        match self {
            Self::Total => OmissionReason::SliceIrrelevant,
            Self::Partial { .. } => OmissionReason::HeuristicCutoff,
        }
    }

    /// Whether non-relevance under this declaration is a proof.
    #[must_use]
    pub const fn is_total(&self) -> bool {
        matches!(self, Self::Total)
    }
}

/// Why an automaton's relevance declaration is only partial.
///
/// A closed set: an untyped free-text reason here would be the place an undecided drop could
/// be dressed up as a decided one (INV-008's discipline, applied to the input — the same
/// reading [`crate::causal::PartialReason`] states for stage 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoverageGap {
    /// The abstraction's hidden dependencies are not fully enumerated — exactly the risk
    /// RFC 0028's "abstraction-relevant hidden event" sentence names.
    UnenumeratedAbstraction,
    /// The property has structure this automaton does not model (INV-008's `Unsupported`, at
    /// the input).
    UnsupportedPropertyFragment,
    /// The alphabet could not be fully resolved against the candidate identities (INV-008's
    /// `InsufficientTelemetry`, at the input).
    InsufficientTelemetry,
}

impl CoverageGap {
    /// All three members, in declaration order.
    pub const ALL: [Self; 3] = [
        Self::UnenumeratedAbstraction,
        Self::UnsupportedPropertyFragment,
        Self::InsufficientTelemetry,
    ];

    /// A stable token for the auditable intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnenumeratedAbstraction => "unenumerated-abstraction",
            Self::UnsupportedPropertyFragment => "unsupported-property-fragment",
            Self::InsufficientTelemetry => "insufficient-telemetry",
        }
    }
}

impl fmt::Display for CoverageGap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A finite, deterministic property automaton over candidate identities: stage 3's premise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyAutomaton {
    initial: AutomatonState,
    transitions: BTreeMap<AutomatonState, BTreeMap<Name, AutomatonState>>,
    alphabet: BTreeSet<Name>,
    violating: BTreeSet<AutomatonState>,
    hidden: BTreeSet<Name>,
    coverage: Coverage,
}

impl PropertyAutomaton {
    /// Build an automaton.
    ///
    /// `transitions` are `(from, symbol, to)` triples; the alphabet is exactly the symbols
    /// appearing in them. `violating` names the states whose occupancy at the end of a run is
    /// a violation. `hidden` names the abstraction-relevant events no observer publishes.
    ///
    /// # Errors
    ///
    /// [`AutomatonError::TransitionRepeated`] when one `(state, symbol)` is declared twice —
    /// a nondeterministic automaton has no single run and therefore no deterministic verdict
    /// (INV-005); [`AutomatonError::UnknownState`] for a violating state that no transition
    /// and no initial declaration names, since a violation nothing can reach is a declaration
    /// error rather than a property; and [`AutomatonError::ObservedIsNotHidden`] for an event
    /// declared both observed and hidden.
    pub fn new(
        initial: AutomatonState,
        transitions: impl IntoIterator<Item = (AutomatonState, Name, AutomatonState)>,
        violating: impl IntoIterator<Item = AutomatonState>,
        hidden: impl IntoIterator<Item = Name>,
        coverage: Coverage,
    ) -> Result<Self, AutomatonError> {
        let mut table: BTreeMap<AutomatonState, BTreeMap<Name, AutomatonState>> = BTreeMap::new();
        let mut alphabet: BTreeSet<Name> = BTreeSet::new();
        let mut states: BTreeSet<AutomatonState> = BTreeSet::new();
        states.insert(initial.clone());
        for (from, symbol, to) in transitions {
            states.insert(from.clone());
            states.insert(to.clone());
            alphabet.insert(symbol.clone());
            let row = table.entry(from.clone()).or_default();
            if row.insert(symbol.clone(), to).is_some() {
                return Err(AutomatonError::TransitionRepeated {
                    state: from,
                    symbol,
                });
            }
        }
        let violating: BTreeSet<AutomatonState> = violating.into_iter().collect();
        if let Some(stray) = violating.difference(&states).next().cloned() {
            return Err(AutomatonError::UnknownState { state: stray });
        }
        let hidden: BTreeSet<Name> = hidden.into_iter().collect();
        if let Some(both) = hidden.intersection(&alphabet).next().cloned() {
            return Err(AutomatonError::ObservedIsNotHidden { id: both });
        }
        Ok(Self {
            initial,
            transitions: table,
            alphabet,
            violating,
            hidden,
            coverage,
        })
    }

    /// The state a run starts in.
    #[must_use]
    pub const fn initial(&self) -> &AutomatonState {
        &self.initial
    }

    /// The events this automaton steps on — what an observer publishes to it.
    #[must_use]
    pub const fn alphabet(&self) -> &BTreeSet<Name> {
        &self.alphabet
    }

    /// The abstraction-relevant events no observer publishes, which are nevertheless in scope
    /// for the property (RFC 0028, "Selection and the causal core"; INV-013).
    #[must_use]
    pub const fn hidden_dependencies(&self) -> &BTreeSet<Name> {
        &self.hidden
    }

    /// The relevance set: the alphabet together with the hidden dependencies.
    ///
    /// This is what [`PropertyFilter`] seeds its slice from. A filter written against
    /// [`PropertyAutomaton::alphabet`] alone is the compiler RFC 0028 prohibits.
    #[must_use]
    pub fn relevant(&self) -> BTreeSet<Name> {
        self.alphabet.union(&self.hidden).cloned().collect()
    }

    /// Whether this candidate is in the relevance set.
    #[must_use]
    pub fn is_relevant(&self, id: &Name) -> bool {
        self.alphabet.contains(id) || self.hidden.contains(id)
    }

    /// Whether the automaton steps on this candidate.
    #[must_use]
    pub fn observes(&self, id: &Name) -> bool {
        self.alphabet.contains(id)
    }

    /// The coverage the producer declared.
    #[must_use]
    pub const fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    /// Whether a state is violating.
    #[must_use]
    pub fn is_violating(&self, state: &AutomatonState) -> bool {
        self.violating.contains(state)
    }

    /// Run the automaton over `trace`, a sequence of candidate identities in a causal order.
    ///
    /// Symbols outside the alphabet are not observed and do not step the automaton — that is
    /// what makes a *slice* comparable to the whole graph at all. A symbol in the alphabet
    /// with no transition out of the current state leaves the state unchanged; the automaton
    /// is a partial deterministic monitor, not a total one, and a missing transition is
    /// "nothing about this state changes", never a trap.
    #[must_use]
    pub fn run(&self, trace: &[Name]) -> AutomatonRun {
        let mut state = self.initial.clone();
        let mut visited = vec![state.clone()];
        let mut observed: Vec<Name> = Vec::new();
        for symbol in trace {
            if !self.alphabet.contains(symbol) {
                continue;
            }
            observed.push(symbol.clone());
            if let Some(next) = self
                .transitions
                .get(&state)
                .and_then(|row| row.get(symbol))
                .cloned()
            {
                state = next;
            }
            visited.push(state.clone());
        }
        let verdict = if self.violating.contains(&state) {
            AutomatonVerdict::Violated
        } else {
            AutomatonVerdict::Satisfied
        };
        AutomatonRun {
            observed,
            visited,
            verdict,
        }
    }
}

/// One run of a [`PropertyAutomaton`]: what it observed, where it went, what it concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomatonRun {
    observed: Vec<Name>,
    visited: Vec<AutomatonState>,
    verdict: AutomatonVerdict,
}

impl AutomatonRun {
    /// The alphabet symbols the run stepped on, in the order they were supplied.
    #[must_use]
    pub fn observed(&self) -> &[Name] {
        &self.observed
    }

    /// The states the run visited, starting with the initial state.
    #[must_use]
    pub fn visited(&self) -> &[AutomatonState] {
        &self.visited
    }

    /// The state the run ended in.
    #[must_use]
    pub fn final_state(&self) -> &AutomatonState {
        self.visited
            .last()
            .expect("a run always records its initial state")
    }

    /// What the run concluded.
    #[must_use]
    pub const fn verdict(&self) -> AutomatonVerdict {
        self.verdict
    }
}

/// **Stage 3's computation**: narrow a slice to what the property is directed at.
///
/// This is the *producer*, not the checker. Nothing here licenses anything — the licence
/// comes from [`crate::monitor::PropertyMonitor`], which reads this function's output and
/// knows nothing about how it was produced (RFC 0028, "Validation").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyFilter;

impl PropertyFilter {
    /// The property-directed sub-slice of `slice`.
    ///
    /// Seeds are the slice members in [`PropertyAutomaton::relevant`]; the result is their
    /// backward closure, intersected back with `slice`. Two properties follow, and both are
    /// load-bearing:
    ///
    /// - **It keeps hidden dependencies.** Seeding from the relevance set rather than the
    ///   alphabet is the whole content of "a compiler MUST NOT exclude an abstraction-relevant
    ///   hidden event on the grounds that no observer publishes it".
    /// - **It stays downward closed when its input was.** `PropertyPreserving` is licensed
    ///   "with stage 2" (RFC 0028's pipeline table) and the preservation obligation in RFC
    ///   0028's "Formal model" has causal closure as a hypothesis, so a filter that broke
    ///   closure would have destroyed the theorem it is a step toward. Re-closing the seeds
    ///   rather than intersecting bluntly is how.
    ///
    /// The intersection with `slice` matters only where the input was *not* closed — a slice
    /// with a redaction hole in it. There the closure of the seeds can reach a withheld
    /// ancestor, and stage 3 must not re-admit what the pre-pass removed.
    ///
    /// # Errors
    ///
    /// [`CausalError::UnknownEndpoint`] when `slice` names something outside `order`; the
    /// closure of a seed from nowhere is not a statement about this order.
    pub fn retain(
        automaton: &PropertyAutomaton,
        order: &CausalOrder,
        slice: &BTreeSet<Name>,
    ) -> Result<BTreeSet<Name>, CausalError> {
        let seeds: BTreeSet<Name> = slice
            .iter()
            .filter(|id| automaton.is_relevant(id))
            .cloned()
            .collect();
        let closed = order.backward_closure(&seeds)?;
        Ok(closed.intersection(slice).cloned().collect())
    }
}

/// A way a [`PropertyAutomaton`] fails to be one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomatonError {
    /// One `(state, symbol)` pair is declared twice.
    TransitionRepeated {
        /// The state the transitions leave.
        state: AutomatonState,
        /// The symbol declared twice out of it.
        symbol: Name,
    },
    /// A violating state that no transition and no initial declaration names.
    UnknownState {
        /// The state that is not a state of this automaton.
        state: AutomatonState,
    },
    /// An event declared both observed and hidden.
    ObservedIsNotHidden {
        /// The event on both sides.
        id: Name,
    },
}

impl fmt::Display for AutomatonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransitionRepeated { state, symbol } => write!(
                f,
                "`{state}` declares two transitions on `{symbol}`; a nondeterministic \
                 automaton has no single run and therefore no deterministic verdict (INV-005)"
            ),
            Self::UnknownState { state } => write!(
                f,
                "`{state}` is declared violating but is not a state of this automaton"
            ),
            Self::ObservedIsNotHidden { id } => write!(
                f,
                "`{id}` is declared both observed and hidden; an event the automaton steps on \
                 is published to it, and the abstraction-relevant hidden events are the ones \
                 no observer publishes (RFC 0028, \"Selection and the causal core\")"
            ),
        }
    }
}

impl core::error::Error for AutomatonError {}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn trace(names: &[&str]) -> Vec<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    /// `q_0 -e_submit-> q_1 -e_ack-> q_2 -e_loss-> q_bad`, with `e_flush` as an
    /// abstraction-relevant hidden event.
    fn automaton(coverage: Coverage) -> PropertyAutomaton {
        PropertyAutomaton::new(
            state("q_0"),
            [
                (state("q_0"), name("e_submit"), state("q_1")),
                (state("q_1"), name("e_ack"), state("q_2")),
                (state("q_2"), name("e_loss"), state("q_bad")),
            ],
            [state("q_bad")],
            [name("e_flush")],
            coverage,
        )
        .expect("a well-formed automaton")
    }

    /// `e_begin -> e_submit -> e_ack -> e_loss`, with an isolated `e_flush` and an isolated
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

    #[test]
    fn the_alphabet_is_exactly_the_transition_symbols() {
        let built = automaton(Coverage::Total);
        assert_eq!(built.alphabet(), &ids(&["e_submit", "e_ack", "e_loss"]));
        assert!(built.observes(&name("e_ack")));
        assert!(!built.observes(&name("e_flush")));
        assert_eq!(built.hidden_dependencies(), &ids(&["e_flush"]));
        assert_eq!(
            built.relevant(),
            ids(&["e_submit", "e_ack", "e_loss", "e_flush"])
        );
        // The relevance set is strictly wider than the alphabet, which is the whole point.
        assert!(built.is_relevant(&name("e_flush")));
        assert!(!built.is_relevant(&name("e_begin")));
    }

    #[test]
    fn an_observed_event_cannot_also_be_hidden() {
        assert_eq!(
            PropertyAutomaton::new(
                state("q_0"),
                [(state("q_0"), name("e_ack"), state("q_1"))],
                [],
                [name("e_ack")],
                Coverage::Total,
            ),
            Err(AutomatonError::ObservedIsNotHidden { id: name("e_ack") })
        );
    }

    #[test]
    fn a_repeated_transition_is_refused() {
        assert_eq!(
            PropertyAutomaton::new(
                state("q_0"),
                [
                    (state("q_0"), name("e_ack"), state("q_1")),
                    (state("q_0"), name("e_ack"), state("q_2")),
                ],
                [],
                [],
                Coverage::Total,
            ),
            Err(AutomatonError::TransitionRepeated {
                state: state("q_0"),
                symbol: name("e_ack"),
            })
        );
        // Even a byte-identical redeclaration: one `(state, symbol)`, one declaration.
        assert!(
            PropertyAutomaton::new(
                state("q_0"),
                [
                    (state("q_0"), name("e_ack"), state("q_1")),
                    (state("q_0"), name("e_ack"), state("q_1")),
                ],
                [],
                [],
                Coverage::Total,
            )
            .is_err()
        );
    }

    #[test]
    fn a_violating_state_from_nowhere_is_refused() {
        assert_eq!(
            PropertyAutomaton::new(
                state("q_0"),
                [(state("q_0"), name("e_ack"), state("q_1"))],
                [state("q_9")],
                [],
                Coverage::Total,
            ),
            Err(AutomatonError::UnknownState {
                state: state("q_9")
            })
        );
        // Anti-vacuity: the initial state and every transition endpoint are states.
        for accepted in [state("q_0"), state("q_1")] {
            assert!(
                PropertyAutomaton::new(
                    state("q_0"),
                    [(state("q_0"), name("e_ack"), state("q_1"))],
                    [accepted],
                    [],
                    Coverage::Total,
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn an_automaton_with_no_transitions_observes_nothing() {
        let empty =
            PropertyAutomaton::new(state("q_0"), [], [], [], Coverage::Total).expect("well formed");
        assert!(empty.alphabet().is_empty());
        let run = empty.run(&trace(&["e_ack", "e_loss"]));
        assert!(run.observed().is_empty());
        assert_eq!(run.verdict(), AutomatonVerdict::Satisfied);
        assert_eq!(run.final_state(), &state("q_0"));
    }

    #[test]
    fn a_run_ignores_symbols_outside_the_alphabet() {
        let built = automaton(Coverage::Total);
        let run = built.run(&trace(&[
            "e_begin", "e_submit", "e_flush", "e_ack", "e_probe", "e_loss",
        ]));
        assert_eq!(run.observed(), trace(&["e_submit", "e_ack", "e_loss"]));
        assert_eq!(
            run.visited(),
            [state("q_0"), state("q_1"), state("q_2"), state("q_bad")]
        );
        assert_eq!(run.verdict(), AutomatonVerdict::Violated);
    }

    #[test]
    fn a_missing_transition_holds_the_state_rather_than_trapping() {
        let built = automaton(Coverage::Total);
        // `e_loss` out of `q_0` has no transition: the state is unchanged, and the run is
        // still satisfied because `q_0` is not violating.
        let run = built.run(&trace(&["e_loss"]));
        assert_eq!(run.observed(), trace(&["e_loss"]));
        assert_eq!(run.final_state(), &state("q_0"));
        assert_eq!(run.verdict(), AutomatonVerdict::Satisfied);
    }

    /// Anti-vacuity for the verdict machinery: an automaton that concluded the same thing
    /// about every trace would make the monitor's comparison meaningless.
    #[test]
    fn two_traces_reach_two_verdicts() {
        let built = automaton(Coverage::Total);
        assert_eq!(
            built
                .run(&trace(&["e_submit", "e_ack", "e_loss"]))
                .verdict(),
            AutomatonVerdict::Violated
        );
        assert_eq!(
            built.run(&trace(&["e_submit", "e_ack"])).verdict(),
            AutomatonVerdict::Satisfied
        );
        assert_eq!(
            AutomatonVerdict::ALL.map(AutomatonVerdict::as_str),
            ["satisfied", "violated"]
        );
    }

    #[test]
    fn two_runs_of_one_automaton_agree() {
        let built = automaton(Coverage::Total);
        let sequence = trace(&["e_submit", "e_ack", "e_loss"]);
        assert_eq!(built.run(&sequence), built.run(&sequence));
        assert_eq!(automaton(Coverage::Total), automaton(Coverage::Total));
    }

    #[test]
    fn the_filter_keeps_an_abstraction_relevant_hidden_event() {
        let order = order();
        let slice = ids(&[
            "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe",
        ]);
        let kept = PropertyFilter::retain(&automaton(Coverage::Total), &order, &slice)
            .expect("the slice is inside the order");
        // `e_flush` survives although no observer publishes it; `e_probe` does not, because
        // nothing the property is directed at depends on it.
        assert!(kept.contains(&name("e_flush")));
        assert!(!kept.contains(&name("e_probe")));
        assert_eq!(
            kept,
            ids(&["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"])
        );
    }

    #[test]
    fn the_filter_re_closes_what_it_keeps() {
        let order = order();
        let slice = ids(&[
            "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe",
        ]);
        let kept = PropertyFilter::retain(&automaton(Coverage::Total), &order, &slice)
            .expect("inside the order");
        // `e_begin` is neither observed nor hidden, and is kept anyway: an observed event
        // brings its causal predecessors in, so the output is still downward closed.
        assert!(kept.contains(&name("e_begin")));
        assert!(order.is_downward_closed(&kept));
    }

    #[test]
    fn the_filter_cannot_re_admit_what_the_slice_lacks() {
        let order = order();
        // A slice with a hole where `e_submit` was — the shape a redaction pre-pass leaves.
        let holed = ids(&["e_begin", "e_ack", "e_loss", "e_flush"]);
        let kept = PropertyFilter::retain(&automaton(Coverage::Total), &order, &holed)
            .expect("inside the order");
        assert!(!kept.contains(&name("e_submit")));
        assert_eq!(kept, holed);
    }

    #[test]
    fn a_filter_over_an_irrelevant_slice_keeps_nothing() {
        let order = order();
        let kept = PropertyFilter::retain(&automaton(Coverage::Total), &order, &ids(&["e_probe"]))
            .expect("inside the order");
        assert!(kept.is_empty());
    }

    #[test]
    fn a_relevant_seed_from_outside_the_order_is_refused() {
        // The refusal is about *seeds*: a slice member the automaton is directed at, that the
        // order does not hold, would make the closure a statement about another graph.
        let elsewhere = PropertyAutomaton::new(
            state("q_0"),
            [(state("q_0"), name("z_9"), state("q_1"))],
            [],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton");
        assert_eq!(
            PropertyFilter::retain(&elsewhere, &order(), &ids(&["e_ack", "z_9"])),
            Err(CausalError::UnknownEndpoint { id: name("z_9") })
        );
    }

    #[test]
    fn an_irrelevant_stray_is_dropped_rather_than_refused() {
        // A slice member the automaton is *not* directed at never becomes a seed, so it is
        // simply not kept. Stage 3 narrows; it does not audit its input's membership, which
        // is `CausalOrder`'s own refusal and stage 2's.
        assert_eq!(
            PropertyFilter::retain(
                &automaton(Coverage::Total),
                &order(),
                &ids(&["e_ack", "z_9"])
            )
            .expect("no relevant seed is missing"),
            ids(&["e_ack"])
        );
    }

    #[test]
    fn coverage_decides_the_omission_reason() {
        assert!(Coverage::Total.is_total());
        assert_eq!(
            Coverage::Total.omission_reason(),
            OmissionReason::SliceIrrelevant
        );
        for reason in CoverageGap::ALL {
            let partial = Coverage::Partial { reason };
            assert!(!partial.is_total());
            assert_eq!(
                partial.omission_reason(),
                OmissionReason::HeuristicCutoff,
                "a drop decided against a {reason} declaration is not a proof of irrelevance"
            );
        }
    }

    #[test]
    fn the_coverage_gaps_have_distinct_tokens() {
        let tokens: BTreeSet<&str> = CoverageGap::ALL
            .into_iter()
            .map(CoverageGap::as_str)
            .collect();
        assert_eq!(tokens.len(), CoverageGap::ALL.len());
    }
}
