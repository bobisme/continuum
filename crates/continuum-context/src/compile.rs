//! The compiler's pipeline, run: the redaction pre-pass, stage 1 (root selection), stage 2
//! (backward causal slicing), stage 3 (property-automaton relevance filtering) and stage 4
//! (the static/dynamic dependence join), ending in a closed accounting, an auditable trail,
//! and the guarantees those stages actually established.
//!
//! # Scope: bn-21vno (stages 1–2) and bn-1kj2n (stages 3–4) of RFC 0028's ten
//!
//! | Phase | RFC 0028 | Licenses |
//! |---|---|---|
//! | pre-pass | "Redaction runs before stage 1" (correction 12) | nothing |
//! | 1 | root selection | "nothing on its own; a bad root is a wrong pack, not an unsound one" |
//! | 2 | backward causal slicing | `CausallyClosed`, and the precondition of `ReplayPreserving` |
//! | 3 | property-automaton relevance filtering | `PropertyPreserving`, with stage 2 |
//! | 4 | static/dynamic dependence join | admissibility of `source` and `model` items |
//!
//! Stages 3 and 4 are **opt-in**: [`CausalCompile::new`] runs stages 1–2 alone, and
//! [`CausalCompile::with_property`] and [`CausalCompile::with_dependence`] each add one stage.
//! That is not a convenience — RFC 0028 requires a deployment be able to run the pipeline in
//! reduced configurations, a stage that did not run has no auditable intermediate (which is a
//! different fact from an empty one), and a stage whose input the deployment does not have
//! must not be simulated with a default. Stages 5–8 are their own bones and append to
//! [`crate::stage::StageTrail`] the same way; stage 9 is the ranker RFC 0028 requires a
//! deployment be able to run *without* ("the register row's named fallback for the whole
//! capability is 'plain causal slice + expansion' — that is, stages 1–8 plus the expansion
//! protocol, with stage 9 disabled"); stage 10 is [`crate::budget::BudgetPacker`] in
//! substance.
//!
//! # The pipeline is not monotone, and the trail says so
//!
//! Stage 1 selects the roots; stage 2 **grows** that into their causal past; stage 3
//! **narrows** it to what the property is directed at; stage 4 **grows** it again with the
//! `source` and `model` items whose dependence the join corroborates. The one structural
//! invariant across all four is the candidate universe the pre-pass left, which is what
//! [`crate::stage::StageTrail`] enforces.
//!
//! # The three refusals this module exists to make
//!
//! **A redacted root is not a root.** RFC 0028's own reason: "slicing from one leaks the
//! shape of what it dropped". [`CausalCompile::run`] refuses before stage 1 does any work,
//! and the pre-pass's intermediate records the visible universe so an auditor can see what
//! policy left rather than infer it.
//!
//! **A slice that redaction punched a hole in does not claim closure.** Slicing runs over the
//! *whole* order, including withheld nodes, because a sub-order with the withheld nodes
//! deleted would report a "closed" selection that is not closed — the silent answer plan
//! §18.4 forbids. When the backward closure of the roots reaches a withheld node, the node is
//! recorded as an irretrievable `redaction` omission, it is not published, and
//! `CausallyClosed` is **not** claimed: "Where redaction removes an item a guarantee needs,
//! the guarantee is not claimed and the reason is recorded" (RFC 0028, "Redaction and
//! privacy"). A pack that answered anyway would be the confident smaller answer that section
//! prohibits.
//!
//! **A drop is only `slice-irrelevant` if the declaration it was decided against is
//! complete.** See [`crate::causal::Completeness`] for stage 2 and
//! [`crate::property::Coverage`] for stage 3. Under a partial declaration the same drop is
//! `heuristic-cutoff`, because non-ancestry in an incomplete order, and non-relevance in an
//! incomplete relevance set, prove nothing.
//!
//! # Why both checks run on the published selection
//!
//! [`crate::causal::CausalOrder::backward_closure`] produces a set that is closed by
//! construction — and that is exactly why the licence does not come from it. RFC 0028's
//! Validation section requires a checker independent of the producer, so
//! [`CausalCompile::run`] takes the selection it is actually about to publish — after
//! redaction has removed items from it, after stage 3 narrowed it and after stage 4 grew it —
//! and hands it to [`crate::causal::CausalOrder::closure_violation`], which reads the order's
//! edges and nothing else, and to [`crate::monitor::PropertyMonitor`], which runs the
//! automaton over the whole order and over that selection and compares. Both licences are
//! issued from a checker's verdict. A slicer bug that dropped a predecessor, or a filter bug
//! that dropped a property-relevant event, therefore costs the guarantee rather than
//! producing a false one.
//!
//! # Which stage's reason a drop is recorded under
//!
//! Every candidate is dispositioned exactly once, and the reason names the stage that
//! decided it. The ladder, in the order it is applied:
//!
//! 1. In the published selection → selected.
//! 2. Withheld by field policy → an irretrievable `redaction` omission.
//! 3. Claimed by the dependence join and declined by stage 4 → `heuristic-cutoff`
//!    ([`crate::dependence::Inadmissible::omission_reason`]). Stage 4's decision is taken
//!    *before* stage 3's on purpose: stage 3 drops every `source` and `model` candidate,
//!    because no property automaton steps on a source span, and recording those as *provably*
//!    outside the property-directed slice would be false about exactly the items stage 4
//!    exists to decide.
//! 4. In the stage-2 slice and not in stage 3's → the automaton's
//!    [`crate::property::Coverage::omission_reason`].
//! 5. Otherwise → the order's [`crate::causal::Completeness::omission_reason`].
//!
//! Where two stages both declined an item the manifest records the weaker claim. A drop is
//! `slice-irrelevant` only where the stage that decided it could prove it.
//!
//! # Determinism (INV-005)
//!
//! No clock, no entropy, no float, no hash-map iteration: every set is a `BTree`, the
//! frontier walk is order-independent because it computes a set, and the trail's digest is a
//! function of content alone. Two runs of one compile produce equal [`Compilation`]s.
//!
//! # What is declined here, and named rather than implied
//!
//! - **The `redactions[]` stub.** The pre-pass records *that* an item was withheld and under
//!   which [`RedactionReason`]; the pack's `Redacted(redacted, reason, commitment,
//!   original_class)` stub needs a commitment and an original class, which are properties of
//!   the artifact store and of whole-pack assembly. That is the last child's bullet.
//! - **Rule C1's manifest record.** A requested-but-unachieved guarantee is owed an omission
//!   record (RFC 0028 C1), but the manifest's `kind` is the closed `SelectionKind`
//!   vocabulary and a guarantee is not a selection kind.
//!   [`crate::guarantee::RequestedGuarantees::unachieved`] computes the list; deciding which
//!   items carry it into the manifest is whole-pack assembly's, and inventing a shape here
//!   would be the second unchecked authority RFC 0028 keeps refusing.
//! - **`ReplayPreserving`.** Stage 2 supplies its precondition and nothing more; the replay
//!   checker is not this crate's, and rule C2 is enforced in
//!   [`crate::guarantee::GuaranteeSet::seal`] so the later group that adds it finds the rule
//!   already in force.
//! - **A seal-level rule for `PropertyPreserving`.** RFC 0028's pipeline table reads
//!   "`PropertyPreserving`, with stage 2", and the preservation obligation in its "Formal
//!   model" has causal closure as a hypothesis — so this pipeline never claims the one without
//!   the other, because [`crate::monitor::PropertyMonitor`] checks closure itself before
//!   concluding. It is deliberately *not* added to
//!   [`crate::guarantee::GuaranteeSet::seal`]: the RFC states six composition rules and this is
//!   not one of them, and inventing a seventh would put a rule in the artifact contract that
//!   the RFC does not carry. `a_property_preserving_pack_is_always_causally_closed` pins the
//!   pipeline-level fact instead.
//! - **Stage 4's `id` and array position.** [`Compilation::admissible_items`] builds the items
//!   through the landed typed constructors and under the candidate's own identity; deciding
//!   the published `selected[]` order and the pack they land in is whole-pack assembly's.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | redaction precedes root selection | RFC 0028 correction 12 | `the_pre_pass_records_before_stage_one` |
//! | a redacted root is refused | RFC 0028 correction 12 | `a_redacted_root_is_not_a_root` |
//! | a root must be a candidate | stage 1's own premise | `a_root_from_outside_the_order_is_refused` |
//! | stage 2 licenses `CausallyClosed` | RFC 0028, "Compiler pipeline" | `a_clean_slice_licenses_causal_closure` |
//! | the licence comes from the independent checker | RFC 0028, "Validation" | `a_redaction_hole_costs_the_guarantee` |
//! | closure = selection + Σ counts | INV-007 | `every_compile_reconciles` |
//! | complete vs partial decides the reason | RFC 0028, "Omission manifest" | `a_partial_order_drops_to_heuristic_cutoff` |
//! | the residual anchor resolves in the selection | RFC 0028, "Expansion protocol" | `a_residual_anchor_outside_the_selection_is_refused` |
//! | stage 3 licenses `PropertyPreserving` from the monitor | RFC 0028, "Validation" | `stage_three_licenses_property_preservation` |
//! | a monitor that disagrees costs the guarantee | RFC 0028, "Validation" | `a_monitor_that_disagrees_costs_the_guarantee` |
//! | `PropertyPreserving` never travels without closure | RFC 0028, "Compiler pipeline" | `a_property_preserving_pack_is_always_causally_closed` |
//! | stage 3's drops carry the automaton's coverage | RFC 0028, "Omission manifest" | `stage_three_drops_carry_the_automatons_coverage` |
//! | stage 4 admits only a corroborated dependence | INV-016 | `stage_four_admits_only_a_corroborated_dependence` |
//! | a join claim must name a `source` or `model` candidate | RFC 0028, "Compiler pipeline" | `a_join_claim_about_another_kind_is_refused` |
//! | a withheld candidate stays a redaction omission | plan §18.4 | `a_withheld_candidate_is_never_admitted` |
//! | deterministic (INV-005) | INV-005, ADR-0003 | `two_runs_of_one_compile_agree` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Name;

use crate::accounting::{Accounting, AccountingError, CandidateSet, ClosedAccounting, Omitted};
use crate::causal::{CausalError, CausalOrder, ClosureViolation};
use crate::dependence::{Admissibility, DependenceJoin, Inadmissible};
use crate::expansion::ExpansionQuery;
use crate::guarantee::{Guarantee, GuaranteeSet, License, SealedGuarantees};
use crate::monitor::{MonitorVerdict, PropertyMonitor};
use crate::omission::IrretrievableReason;
use crate::property::{PropertyAutomaton, PropertyFilter};
use crate::selection::{SelectedItem, SelectionKind};
use crate::stage::{Phase, Stage, StageIntermediate, StageTrail, TrailError};

/// Why a candidate is withheld from *this* caller — the three members of the schema's shared
/// `Redacted.reason` vocabulary (`redacted.schema.json`; the IDL's `RedactionReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RedactionReason {
    /// The content was replaced by a summary.
    Summarized,
    /// The content is no longer held.
    Lost,
    /// The content was deleted under policy.
    Purged,
}

impl RedactionReason {
    /// All three members, in the schema's order.
    pub const ALL: [Self; 3] = [Self::Summarized, Self::Lost, Self::Purged];

    /// The schema's wire token.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Summarized => "summarized",
            Self::Lost => "lost",
            Self::Purged => "purged",
        }
    }

    /// Parse a wire token, failing closed.
    #[must_use]
    pub fn from_wire_str(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|member| member.as_wire_str() == text)
    }
}

impl fmt::Display for RedactionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// Field policy, resolved against this caller's scope, as it stands *before* stage 1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactionPolicy {
    withheld: BTreeMap<Name, RedactionReason>,
}

impl RedactionPolicy {
    /// A policy that withholds nothing.
    #[must_use]
    pub fn permitting_everything() -> Self {
        Self {
            withheld: BTreeMap::new(),
        }
    }

    /// A policy withholding the named candidates, each with the reason it is withheld.
    #[must_use]
    pub fn withholding(entries: impl IntoIterator<Item = (Name, RedactionReason)>) -> Self {
        Self {
            withheld: entries.into_iter().collect(),
        }
    }

    /// Whether this candidate is withheld.
    #[must_use]
    pub fn withholds(&self, id: &Name) -> bool {
        self.withheld.contains_key(id)
    }

    /// Why this candidate is withheld, when it is.
    #[must_use]
    pub fn reason(&self, id: &Name) -> Option<RedactionReason> {
        self.withheld.get(id).copied()
    }

    /// Whether the policy withholds nothing at all.
    #[must_use]
    pub fn is_permissive(&self) -> bool {
        self.withheld.is_empty()
    }

    /// The withheld identities, in canonical order.
    #[must_use]
    pub fn withheld(&self) -> Vec<Name> {
        self.withheld.keys().cloned().collect()
    }
}

/// The property inputs stage 3 needs: the automaton the filter is directed by, and the
/// **independent** monitor that licenses the guarantee.
///
/// They are two fields rather than one because they are two authorities. RFC 0028: "an
/// implementation that reuses the compiler's own automaton has checked nothing" — see
/// [`crate::monitor`]'s "The independence argument" for what the separation does and does not
/// establish.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PropertyStage {
    automaton: PropertyAutomaton,
    monitor: PropertyMonitor,
}

/// A compile of stages 1–4 over one causal order under one field policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalCompile {
    order: CausalOrder,
    policy: RedactionPolicy,
    property: Option<PropertyStage>,
    dependence: Option<DependenceJoin>,
}

impl CausalCompile {
    /// Prepare a compile of stages 1–2.
    #[must_use]
    pub const fn new(order: CausalOrder, policy: RedactionPolicy) -> Self {
        Self {
            order,
            policy,
            property: None,
            dependence: None,
        }
    }

    /// Add stage 3, directed by `automaton` and checked by `monitor`.
    ///
    /// The two are separate arguments on purpose: the filter is directed by the first and the
    /// licence is issued from the second, so a caller may supply a stricter monitor than the
    /// filter was given and the guarantee is then refused rather than assumed.
    #[must_use]
    pub fn with_property(mut self, automaton: PropertyAutomaton, monitor: PropertyMonitor) -> Self {
        self.property = Some(PropertyStage { automaton, monitor });
        self
    }

    /// Add stage 4, joining an untrusted source correspondence with a trusted execution
    /// record (INV-016; see [`crate::dependence`]).
    #[must_use]
    pub fn with_dependence(mut self, join: DependenceJoin) -> Self {
        self.dependence = Some(join);
        self
    }

    /// The causal order this compile slices over.
    #[must_use]
    pub const fn order(&self) -> &CausalOrder {
        &self.order
    }

    /// The policy in force before stage 1.
    #[must_use]
    pub const fn policy(&self) -> &RedactionPolicy {
        &self.policy
    }

    /// The property automaton stage 3 filters by, when stage 3 runs.
    #[must_use]
    pub fn automaton(&self) -> Option<&PropertyAutomaton> {
        self.property.as_ref().map(|stage| &stage.automaton)
    }

    /// The independent monitor that licenses `PropertyPreserving`, when stage 3 runs.
    #[must_use]
    pub fn monitor(&self) -> Option<&PropertyMonitor> {
        self.property.as_ref().map(|stage| &stage.monitor)
    }

    /// The dependence join stage 4 decides admissibility from, when stage 4 runs.
    #[must_use]
    pub const fn dependence(&self) -> Option<&DependenceJoin> {
        self.dependence.as_ref()
    }

    /// Run the pre-pass, stage 1, stage 2, and the stages 3 and 4 that were configured.
    ///
    /// `roots` is stage 1's output — the handles the question names. `residual` is the
    /// expansion query that retrieves the candidates the pipeline leaves behind; its anchor
    /// MUST resolve in the published selection, because "expansion is navigation over a
    /// published pack, not a general graph query" (RFC 0028, "Expansion protocol").
    ///
    /// # Errors
    ///
    /// [`CompileError::RedactedRoot`] for a root field policy withheld (correction 12);
    /// [`CompileError::Causal`] for a root, or a dependence-join claim, that is not a node of
    /// the order; [`CompileError::NotACorrespondenceKind`] for a join claim about a candidate
    /// whose kind is neither `source` nor `model`, which is stage 4's column and nothing else;
    /// [`CompileError::AnchorNotSelected`] for a residual query nothing published anchors;
    /// [`CompileError::Accounting`] and [`CompileError::Trail`] where the closed-accounting
    /// or trail disciplines refuse — neither can be reached by a caller of this function, and
    /// both are surfaced rather than unwrapped so a future stage group's bug is a typed
    /// failure rather than a panic.
    pub fn run(
        &self,
        roots: &BTreeSet<Name>,
        residual: &ExpansionQuery,
    ) -> Result<Compilation, CompileError> {
        let universe: BTreeSet<Name> = self.order.nodes().map(|(id, _)| id.clone()).collect();
        let mut trail = StageTrail::over(universe.iter().cloned());

        // --- pre-pass: field policy, before root selection (RFC 0028 correction 12) -----
        let visible: BTreeSet<Name> = universe
            .iter()
            .filter(|id| !self.policy.withholds(id))
            .cloned()
            .collect();
        trail.record(StageIntermediate::noted(
            Phase::Redaction,
            visible.iter().cloned(),
            if self.policy.is_permissive() {
                "no-policy"
            } else {
                "policy-applied"
            },
        ))?;

        // --- stage 1: root selection ---------------------------------------------------
        for root in roots {
            if !self.order.contains(root) {
                return Err(CompileError::Causal(CausalError::UnknownEndpoint {
                    id: root.clone(),
                }));
            }
            if let Some(reason) = self.policy.reason(root) {
                return Err(CompileError::RedactedRoot {
                    id: root.clone(),
                    reason,
                });
            }
        }
        trail.record(StageIntermediate::of(
            Phase::Stage(Stage::RootSelection),
            roots.iter().cloned(),
        ))?;

        // --- stage 2: backward causal slicing ------------------------------------------
        // Over the *whole* order, withheld nodes included: a sub-order with them deleted
        // would report a closure that is not one.
        let closure = self.order.backward_closure(roots)?;
        let redaction_gap: BTreeSet<Name> = closure
            .iter()
            .filter(|id| self.policy.withholds(id))
            .cloned()
            .collect();
        let selection: BTreeSet<Name> = closure.difference(&redaction_gap).cloned().collect();
        trail.record(StageIntermediate::noted(
            Phase::Stage(Stage::CausalSlicing),
            selection.iter().cloned(),
            if redaction_gap.is_empty() {
                "closed"
            } else {
                "redaction-gap"
            },
        ))?;

        // --- stage 3: property-automaton relevance filtering ---------------------------
        // A narrowing, and the only stage of the four that is one. Seeded from the
        // automaton's *relevance* set — the alphabet together with the abstraction-relevant
        // hidden events — and re-closed, so the output is still downward closed and no hidden
        // event is excluded "on the grounds that no observer publishes it".
        let mut working = selection.clone();
        if let Some(property) = &self.property {
            working = PropertyFilter::retain(&property.automaton, &self.order, &selection)?;
            trail.record(StageIntermediate::noted(
                Phase::Stage(Stage::PropertyRelevance),
                working.iter().cloned(),
                if working.len() == selection.len() {
                    "property-retained"
                } else {
                    "property-filtered"
                },
            ))?;
        }
        let property_core = working.clone();

        // --- stage 4: the static/dynamic dependence join --------------------------------
        let mut admissible: BTreeMap<Name, crate::dependence::Admissible> = BTreeMap::new();
        let mut declined: BTreeMap<Name, Inadmissible> = BTreeMap::new();
        if let Some(join) = &self.dependence {
            for id in join.claimed() {
                // Structure is checked: a claim about a candidate this compile never held, or
                // about a candidate of another kind, is a mismatch between the join and the
                // order rather than an assertion to corroborate.
                let Some(kind) = self.order.kind(id) else {
                    return Err(CompileError::Causal(CausalError::UnknownEndpoint {
                        id: id.clone(),
                    }));
                };
                if !matches!(kind, SelectionKind::Source | SelectionKind::Model) {
                    return Err(CompileError::NotACorrespondenceKind {
                        id: id.clone(),
                        kind,
                    });
                }
                // Redaction wins: a withheld candidate is not stage 4's to admit or decline,
                // and its omission stays the irretrievable `redaction` record the pre-pass
                // earned it.
                if self.policy.withholds(id) {
                    continue;
                }
                match join.admissibility(id, &property_core) {
                    Admissibility::Admissible(admitted) => {
                        working.insert(id.clone());
                        admissible.insert(id.clone(), admitted);
                    }
                    Admissibility::Inadmissible(reason) => {
                        declined.insert(id.clone(), reason);
                    }
                }
            }
            trail.record(StageIntermediate::noted(
                Phase::Stage(Stage::DependenceJoin),
                working.iter().cloned(),
                if admissible.is_empty() {
                    "no-admissible-correspondence"
                } else {
                    "correspondence-joined"
                },
            ))?;
        }
        let published = working;

        // --- the independent checks, on the selection that will be published -------------
        let violation = self.order.closure_violation(&published);
        let mut guarantees = GuaranteeSet::empty();
        if violation.is_none() {
            guarantees.claim(License::issue(Guarantee::CausallyClosed));
        }
        let monitor_verdict = self
            .property
            .as_ref()
            .map(|property| property.monitor.verdict(&self.order, &published));
        if monitor_verdict
            .as_ref()
            .is_some_and(MonitorVerdict::is_preserving)
        {
            guarantees.claim(License::issue(Guarantee::PropertyPreserving));
        }

        // --- accounting: every candidate dispositioned ----------------------------------
        let candidates = CandidateSet::new(
            self.order
                .nodes()
                .map(|(id, kind)| (id.clone(), kind))
                .collect::<Vec<_>>(),
        )?;
        let mut accounting = Accounting::over(candidates);
        let causal_reason = self.order.completeness().omission_reason();
        let property_reason = self
            .property
            .as_ref()
            .map(|property| property.automaton.coverage().omission_reason());
        let mut residual_used = false;
        for (id, _) in self.order.nodes() {
            if published.contains(id) {
                accounting.select(id)?;
                continue;
            }
            if self.policy.withholds(id) {
                // Withheld, whether or not the slice wanted it. `expandable: false` and the
                // reason that explains irretrievability — RFC 0028's expansion table's
                // redacted row, read at compile time rather than at expansion time.
                accounting.omit(id, Omitted::Irretrievable(IrretrievableReason::Redaction))?;
                continue;
            }
            // The reason names the stage that decided the drop; see the module
            // documentation's "Which stage's reason a drop is recorded under".
            let reason = if let Some(inadmissible) = declined.get(id) {
                inadmissible.omission_reason()
            } else if selection.contains(id) {
                property_reason.unwrap_or(causal_reason)
            } else {
                causal_reason
            };
            residual_used = true;
            accounting.omit(
                id,
                Omitted::Expandable {
                    reason,
                    query: residual.clone(),
                },
            )?;
        }
        let accounting = accounting.close()?;
        accounting.reconcile()?;

        if residual_used && !published.contains(residual.anchor()) {
            return Err(CompileError::AnchorNotSelected {
                anchor: residual.anchor().clone(),
            });
        }

        Ok(Compilation {
            trail,
            accounting,
            guarantees: guarantees.seal().map_err(|_| CompileError::Unreachable)?,
            closure_violation: violation,
            redaction_gap,
            monitor_verdict,
            admissible,
            declined,
        })
    }
}

/// What the pipeline produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    trail: StageTrail,
    accounting: ClosedAccounting,
    guarantees: SealedGuarantees,
    closure_violation: Option<ClosureViolation>,
    redaction_gap: BTreeSet<Name>,
    monitor_verdict: Option<MonitorVerdict>,
    admissible: BTreeMap<Name, crate::dependence::Admissible>,
    declined: BTreeMap<Name, Inadmissible>,
}

impl Compilation {
    /// The auditable intermediates, retrievable per phase (RFC 0030, compared artifact 5).
    #[must_use]
    pub const fn trail(&self) -> &StageTrail {
        &self.trail
    }

    /// The selection and the manifest, read off one ledger (INV-007).
    #[must_use]
    pub const fn accounting(&self) -> &ClosedAccounting {
        &self.accounting
    }

    /// The guarantees these stages established — never the ones that were requested.
    #[must_use]
    pub const fn guarantees(&self) -> &SealedGuarantees {
        &self.guarantees
    }

    /// The published selection, in canonical order.
    #[must_use]
    pub fn selected(&self) -> &[Name] {
        self.accounting.selected()
    }

    /// Why closure was not licensed, when it was not.
    #[must_use]
    pub fn closure_violation(&self) -> Option<&ClosureViolation> {
        self.closure_violation.as_ref()
    }

    /// The causal ancestors the slice needed and field policy withheld.
    ///
    /// Non-empty exactly when redaction cost this compile its `CausallyClosed` claim.
    #[must_use]
    pub const fn redaction_gap(&self) -> &BTreeSet<Name> {
        &self.redaction_gap
    }

    /// Whether this compile established causal closure.
    #[must_use]
    pub fn is_causally_closed(&self) -> bool {
        self.guarantees
            .members()
            .contains(&Guarantee::CausallyClosed)
    }

    /// What the independent property monitor concluded, when stage 3 ran.
    ///
    /// `None` means stage 3 did not run — a different fact from a monitor that ran and
    /// declined, which is [`MonitorVerdict::Inapplicable`].
    #[must_use]
    pub const fn monitor_verdict(&self) -> Option<&MonitorVerdict> {
        self.monitor_verdict.as_ref()
    }

    /// Whether this compile established property preservation.
    #[must_use]
    pub fn is_property_preserving(&self) -> bool {
        self.guarantees
            .members()
            .contains(&Guarantee::PropertyPreserving)
    }

    /// The `source` and `model` candidates stage 4 admitted, in canonical order.
    #[must_use]
    pub const fn admissible(&self) -> &BTreeMap<Name, crate::dependence::Admissible> {
        &self.admissible
    }

    /// The `source` and `model` candidates stage 4 declined, each with its typed reason.
    #[must_use]
    pub const fn declined(&self) -> &BTreeMap<Name, Inadmissible> {
        &self.declined
    }

    /// The admitted items, projected into the pack's `selected[]` shape under their own
    /// candidate identities.
    ///
    /// Built through [`crate::source::SourceRef::into_selected_item`] and
    /// [`crate::model::ModelActionRef::into_selected_item`], which are this crate's only
    /// constructors for those two kinds and take no string parameter (INV-016).
    #[must_use]
    pub fn admissible_items(&self) -> Vec<SelectedItem> {
        self.admissible
            .iter()
            .map(|(id, admitted)| admitted.clone().into_selected_item(id.clone()))
            .collect()
    }
}

/// A way a compile of stages 1–2 refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// Field policy withheld a root. "A redacted root is not a root" (RFC 0028
    /// correction 12).
    RedactedRoot {
        /// The root policy withheld.
        id: Name,
        /// Why it is withheld.
        reason: RedactionReason,
    },
    /// The causal order refused the request.
    Causal(CausalError),
    /// The residual expansion query anchors at something the pack does not publish.
    AnchorNotSelected {
        /// The anchor that does not resolve in the selection.
        anchor: Name,
    },
    /// The dependence join claims a candidate whose kind is neither `source` nor `model`.
    NotACorrespondenceKind {
        /// The claimed candidate.
        id: Name,
        /// The kind it actually has.
        kind: SelectionKind,
    },
    /// Closed accounting refused.
    Accounting(AccountingError),
    /// The trail refused an intermediate.
    Trail(TrailError),
    /// Rule C2 refused a set this module cannot build — `ReplayPreserving` is not
    /// licensable here, so the arm exists to be unreachable rather than to be unwrapped.
    Unreachable,
}

impl From<CausalError> for CompileError {
    fn from(error: CausalError) -> Self {
        Self::Causal(error)
    }
}

impl From<AccountingError> for CompileError {
    fn from(error: AccountingError) -> Self {
        Self::Accounting(error)
    }
}

impl From<TrailError> for CompileError {
    fn from(error: TrailError) -> Self {
        Self::Trail(error)
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RedactedRoot { id, reason } => write!(
                f,
                "the root `{id}` is withheld ({reason}); a redacted root is not a root, and \
                 slicing from one leaks the shape of what it dropped (RFC 0028)"
            ),
            Self::Causal(error) => write!(f, "{error}"),
            Self::AnchorNotSelected { anchor } => write!(
                f,
                "the residual expansion query anchors at `{anchor}`, which this pack does \
                 not publish; expansion is navigation over a published pack (RFC 0028)"
            ),
            Self::NotACorrespondenceKind { id, kind } => write!(
                f,
                "the dependence join claims `{id}`, whose kind is `{kind}`; stage 4 licenses \
                 the admissibility of `source` and `model` items and nothing else (RFC 0028, \
                 \"Compiler pipeline\")"
            ),
            Self::Accounting(error) => write!(f, "{error}"),
            Self::Trail(error) => write!(f, "{error}"),
            Self::Unreachable => {
                f.write_str("a guarantee set this stage group cannot build was refused by rule C2")
            }
        }
    }
}

impl core::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal::PartialReason;
    use crate::dependence::{
        CorrespondenceClaim, CorrespondenceRef, ExecutionDependence, SourceCorrespondence,
    };
    use crate::expansion::ExpansionRelation;
    use crate::model::ModelActionRef;
    use crate::monitor::{Inapplicable, MonitorDisagreement};
    use crate::omission::OmissionReason;
    use crate::property::{AutomatonState, Coverage, CoverageGap};
    use crate::source::{SourceRef, SourceSpan};
    use continuum_workspace::snapshot::WorkspacePath;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    /// The shape of the G0-DX-01 case in miniature: a causal chain
    /// `begin -> submit -> ack -> loss`, plus four causally isolated noise events and a
    /// source span.
    fn order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_submit"), SelectionKind::Event),
                (name("e_ack"), SelectionKind::Event),
                (name("e_loss"), SelectionKind::Event),
                (name("n_beat_1"), SelectionKind::Event),
                (name("n_beat_2"), SelectionKind::Event),
                (name("n_scrub"), SelectionKind::Event),
                (name("n_telemetry"), SelectionKind::Event),
                (name("s_writer"), SelectionKind::Source),
            ],
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
        )
        .expect("a well-formed order")
    }

    fn residual() -> ExpansionQuery {
        ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_loss"))
    }

    fn clean() -> CausalCompile {
        CausalCompile::new(order(), RedactionPolicy::permitting_everything())
    }

    #[test]
    fn the_pre_pass_records_before_stage_one() {
        let compiled = clean()
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");
        assert_eq!(
            compiled.trail().phases(),
            [
                Phase::Redaction,
                Phase::Stage(Stage::RootSelection),
                Phase::Stage(Stage::CausalSlicing),
            ]
        );
        let pre = compiled
            .trail()
            .intermediate(Phase::Redaction)
            .expect("the pre-pass recorded");
        assert_eq!(pre.len(), 9);
        assert_eq!(pre.note(), Some("no-policy"));
    }

    #[test]
    fn a_clean_slice_licenses_causal_closure() {
        let compiled = clean()
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");
        // `Name`'s order is shortlex (`continuum_value::value::Name`), not plain
        // lexicographic — so the canonical selection is by length first.
        assert_eq!(
            compiled.selected(),
            [
                name("e_ack"),
                name("e_loss"),
                name("e_begin"),
                name("e_submit"),
            ]
        );
        assert!(compiled.is_causally_closed());
        assert_eq!(compiled.closure_violation(), None);
        assert_eq!(compiled.guarantees().members(), [Guarantee::CausallyClosed]);
        // Five candidates dropped, and the whole drop is `slice-irrelevant` because the
        // order was declared complete.
        assert_eq!(compiled.accounting().manifest().total(), 5);
        for record in compiled.accounting().manifest().records() {
            assert_eq!(record.reason(), OmissionReason::SliceIrrelevant);
            assert!(record.retrievability().is_expandable());
        }
    }

    #[test]
    fn every_compile_reconciles() {
        for roots in [
            ids(&["e_loss"]),
            ids(&["e_ack"]),
            ids(&["e_begin"]),
            ids(&["s_writer"]),
            ids(&["e_loss", "n_scrub"]),
            ids(&[]),
        ] {
            let anchor = roots
                .iter()
                .next_back()
                .cloned()
                .unwrap_or_else(|| name("e_loss"));
            let query = ExpansionQuery::new(ExpansionRelation::SameOwner, anchor);
            let Ok(compiled) = clean().run(&roots, &query) else {
                continue;
            };
            compiled
                .accounting()
                .reconcile()
                .expect("the two halves agree");
            assert_eq!(
                u64::from(compiled.accounting().candidate_count()),
                compiled.accounting().manifest().total()
                    + compiled.accounting().selected().len() as u64
            );
        }
    }

    #[test]
    fn a_root_from_outside_the_order_is_refused() {
        assert_eq!(
            clean().run(&ids(&["e_nowhere"]), &residual()),
            Err(CompileError::Causal(CausalError::UnknownEndpoint {
                id: name("e_nowhere"),
            }))
        );
    }

    #[test]
    fn a_redacted_root_is_not_a_root() {
        let compile = CausalCompile::new(
            order(),
            RedactionPolicy::withholding([(name("e_loss"), RedactionReason::Purged)]),
        );
        assert_eq!(
            compile.run(&ids(&["e_loss"]), &residual()),
            Err(CompileError::RedactedRoot {
                id: name("e_loss"),
                reason: RedactionReason::Purged,
            })
        );
    }

    /// The load-bearing negative: policy withholds a causal *ancestor* of the root. The
    /// selection is published without it, the withheld item is an irretrievable `redaction`
    /// omission, and the independent checker refuses the licence — a confident smaller
    /// answer is exactly what plan §18.4 prohibits.
    #[test]
    fn a_redaction_hole_costs_the_guarantee() {
        let compile = CausalCompile::new(
            order(),
            RedactionPolicy::withholding([(name("e_submit"), RedactionReason::Purged)]),
        );
        let compiled = compile
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");

        assert_eq!(compiled.redaction_gap(), &ids(&["e_submit"]));
        assert!(!compiled.is_causally_closed());
        assert!(compiled.guarantees().members().is_empty());
        assert_eq!(
            compiled.closure_violation(),
            Some(&ClosureViolation::MissingPredecessor {
                selected: name("e_ack"),
                predecessor: name("e_submit"),
            })
        );
        // And the hole is named, counted, and marked irretrievable rather than hidden.
        let redacted: Vec<_> = compiled
            .accounting()
            .manifest()
            .records()
            .iter()
            .filter(|record| record.reason() == OmissionReason::Redaction)
            .collect();
        assert_eq!(redacted.len(), 1);
        assert_eq!(redacted[0].count(), 1);
        assert!(!redacted[0].retrievability().is_expandable());
        compiled
            .accounting()
            .reconcile()
            .expect("the two halves agree");
    }

    #[test]
    fn a_redaction_outside_the_slice_does_not_cost_the_guarantee() {
        // The other side of the same rule: withholding something the core never needed is an
        // ordinary omission, and closure still holds.
        let compile = CausalCompile::new(
            order(),
            RedactionPolicy::withholding([(name("n_scrub"), RedactionReason::Summarized)]),
        );
        let compiled = compile
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");
        assert!(compiled.is_causally_closed());
        assert!(compiled.redaction_gap().is_empty());
        assert_eq!(compiled.accounting().manifest().total(), 5);
    }

    #[test]
    fn a_partial_order_drops_to_heuristic_cutoff() {
        let partial = CausalOrder::partial(
            order().nodes().map(|(id, kind)| (id.clone(), kind)),
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
            PartialReason::InsufficientTelemetry,
        )
        .expect("a well-formed order");
        let compiled = CausalCompile::new(partial, RedactionPolicy::permitting_everything())
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");

        // The same four items are selected and closure still holds — closure is a statement
        // about the edges that ARE named. What changes is the honesty of the drop.
        assert!(compiled.is_causally_closed());
        for record in compiled.accounting().manifest().records() {
            assert_eq!(record.reason(), OmissionReason::HeuristicCutoff);
        }
    }

    #[test]
    fn a_residual_anchor_outside_the_selection_is_refused() {
        let query = ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("n_scrub"));
        assert_eq!(
            clean().run(&ids(&["e_loss"]), &query),
            Err(CompileError::AnchorNotSelected {
                anchor: name("n_scrub"),
            })
        );
    }

    #[test]
    fn an_empty_manifest_needs_no_anchor() {
        // Boundary: roots whose closure is the whole order. Nothing is omitted, so the
        // residual query is never used and its anchor is never required to resolve.
        let compile = CausalCompile::new(
            CausalOrder::new(
                [
                    (name("e_1"), SelectionKind::Event),
                    (name("e_2"), SelectionKind::Event),
                ],
                [(name("e_2"), name("e_1"))],
            )
            .expect("a well-formed order"),
            RedactionPolicy::permitting_everything(),
        );
        let query = ExpansionQuery::new(ExpansionRelation::SameOwner, name("e_9999"));
        let compiled = compile.run(&ids(&["e_2"]), &query).expect("compiles");
        assert!(compiled.accounting().manifest().is_empty());
        assert!(compiled.is_causally_closed());
    }

    #[test]
    fn an_empty_root_set_is_a_wrong_pack_and_not_an_unsound_one() {
        // Stage 1 licenses nothing, so a root set that names nothing is not an error here —
        // it is a pack that selects nothing. It is refused one step later, because a pack
        // that dropped everything cannot name where to get any of it back.
        assert_eq!(
            clean().run(&ids(&[]), &residual()),
            Err(CompileError::AnchorNotSelected {
                anchor: name("e_loss"),
            })
        );
    }

    #[test]
    fn a_causally_isolated_root_selects_only_itself() {
        let compiled = clean()
            .run(
                &ids(&["s_writer"]),
                &ExpansionQuery::new(ExpansionRelation::SourceSpan, name("s_writer")),
            )
            .expect("compiles");
        assert_eq!(compiled.selected(), [name("s_writer")]);
        assert!(compiled.is_causally_closed());
        assert_eq!(compiled.accounting().manifest().total(), 8);
    }

    #[test]
    fn two_runs_of_one_compile_agree() {
        let one = clean()
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");
        let other = clean()
            .run(&ids(&["e_loss"]), &residual())
            .expect("compiles");
        assert_eq!(one, other);
        assert_eq!(
            one.trail().to_json().to_canonical_bytes(),
            other.trail().to_json().to_canonical_bytes()
        );
    }

    // =================================================================================
    // stages 3 and 4
    // =================================================================================

    fn state(text: &str) -> AutomatonState {
        AutomatonState::new(name(text))
    }

    /// The stage-1–2 fixture, plus an isolated abstraction-relevant hidden `e_flush`, an
    /// isolated property-irrelevant `e_probe`, and a `model` candidate.
    fn wider_order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_submit"), SelectionKind::Event),
                (name("e_ack"), SelectionKind::Event),
                (name("e_loss"), SelectionKind::Event),
                (name("e_flush"), SelectionKind::Event),
                (name("e_probe"), SelectionKind::Event),
                (name("n_beat_1"), SelectionKind::Event),
                (name("s_writer"), SelectionKind::Source),
                (name("m_commit"), SelectionKind::Model),
            ],
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
        )
        .expect("a well-formed order")
    }

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

    /// `s_writer` is corroborated on `e_ack`; `m_commit` is claimed on `e_submit` and
    /// attested nowhere.
    fn join() -> DependenceJoin {
        DependenceJoin::new(
            SourceCorrespondence::of([
                (
                    name("s_writer"),
                    CorrespondenceClaim::new(
                        CorrespondenceRef::Source(SourceRef::new(
                            SourceSpan::new(
                                WorkspacePath::new("crates/a/src/lib.rs").expect("well formed"),
                                12,
                                1,
                                12,
                                9,
                            )
                            .expect("well ordered"),
                        )),
                        [name("e_ack")],
                    ),
                ),
                (
                    name("m_commit"),
                    CorrespondenceClaim::new(
                        CorrespondenceRef::Model(ModelActionRef::action_only(name("Commit"))),
                        [name("e_submit")],
                    ),
                ),
            ])
            .expect("a well-formed correspondence"),
            ExecutionDependence::attesting([(name("s_writer"), ids(&["e_ack"]))])
                .expect("a well-formed execution record"),
        )
    }

    fn staged() -> CausalCompile {
        CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
            .with_property(
                automaton(Coverage::Total),
                PropertyMonitor::over(automaton(Coverage::Total)),
            )
            .with_dependence(join())
    }

    fn roots() -> BTreeSet<Name> {
        ids(&["e_loss", "e_flush", "e_probe"])
    }

    #[test]
    fn stage_three_licenses_property_preservation() {
        let compiled = staged().run(&roots(), &residual()).expect("compiles");

        assert_eq!(
            compiled.trail().phases(),
            [
                Phase::Redaction,
                Phase::Stage(Stage::RootSelection),
                Phase::Stage(Stage::CausalSlicing),
                Phase::Stage(Stage::PropertyRelevance),
                Phase::Stage(Stage::DependenceJoin),
            ]
        );
        assert_eq!(
            compiled.monitor_verdict(),
            Some(&MonitorVerdict::Preserving)
        );
        assert!(compiled.is_property_preserving());
        assert!(compiled.is_causally_closed());
        assert_eq!(
            compiled.guarantees().members(),
            [Guarantee::PropertyPreserving, Guarantee::CausallyClosed]
        );
    }

    #[test]
    fn stage_three_narrows_and_stage_four_grows() {
        let compiled = staged().run(&roots(), &residual()).expect("compiles");
        let trail = compiled.trail();

        // Stage 2 grew the three roots into the chain; stage 3 dropped `e_probe`; stage 4
        // added the one corroborated source span.
        assert_eq!(
            trail
                .intermediate(Phase::Stage(Stage::CausalSlicing))
                .expect("stage 2")
                .working_set(),
            &ids(&[
                "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe"
            ])
        );
        assert_eq!(
            trail
                .intermediate(Phase::Stage(Stage::PropertyRelevance))
                .expect("stage 3")
                .working_set(),
            &ids(&["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"])
        );
        assert_eq!(
            trail
                .intermediate(Phase::Stage(Stage::PropertyRelevance))
                .expect("stage 3")
                .note(),
            Some("property-filtered")
        );
        assert_eq!(
            trail
                .intermediate(Phase::Stage(Stage::DependenceJoin))
                .expect("stage 4")
                .working_set(),
            &ids(&[
                "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "s_writer"
            ])
        );
        assert_eq!(
            trail
                .intermediate(Phase::Stage(Stage::DependenceJoin))
                .expect("stage 4")
                .note(),
            Some("correspondence-joined")
        );
    }

    /// The independence claim at pipeline grain: the filter is directed by one automaton and
    /// the licence is issued by a monitor over another. A stricter monitor refuses.
    #[test]
    fn a_monitor_that_disagrees_costs_the_guarantee() {
        let stricter = PropertyAutomaton::new(
            state("q_0"),
            [
                (state("q_0"), name("e_submit"), state("q_1")),
                (state("q_1"), name("e_ack"), state("q_2")),
                (state("q_2"), name("e_loss"), state("q_bad")),
                // The monitor also observes the diagnostic chain stage 3's automaton ignores.
                (state("q_bad"), name("e_probe"), state("q_bad")),
            ],
            [state("q_bad")],
            [name("e_flush")],
            Coverage::Total,
        )
        .expect("a well-formed automaton");
        let compiled = CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
            .with_property(automaton(Coverage::Total), PropertyMonitor::over(stricter))
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("compiles");

        assert!(!compiled.is_property_preserving());
        assert_eq!(
            compiled.monitor_verdict(),
            Some(&MonitorVerdict::Divergent(
                MonitorDisagreement::ObservedEventDropped {
                    id: name("e_probe")
                }
            ))
        );
        // The rest of the pack is unchanged: the guarantee is dropped, never the honesty.
        assert!(compiled.is_causally_closed());
        compiled.accounting().reconcile().expect("halves agree");
    }

    #[test]
    fn a_monitor_that_observes_nothing_here_is_inapplicable_not_preserving() {
        let elsewhere = PropertyAutomaton::new(
            state("q_0"),
            [(state("q_0"), name("e_elsewhere"), state("q_1"))],
            [state("q_1")],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton");
        let compiled = CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
            .with_property(automaton(Coverage::Total), PropertyMonitor::over(elsewhere))
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("compiles");

        assert!(!compiled.is_property_preserving());
        assert_eq!(
            compiled.monitor_verdict(),
            Some(&MonitorVerdict::Inapplicable(Inapplicable::NoObservedEvent))
        );
    }

    #[test]
    fn a_property_preserving_pack_is_always_causally_closed() {
        // RFC 0028's pipeline table reads "`PropertyPreserving`, with stage 2". The monitor
        // checks closure itself, so the pipeline cannot claim the one without the other.
        for policy in [
            RedactionPolicy::permitting_everything(),
            RedactionPolicy::withholding([(name("e_submit"), RedactionReason::Purged)]),
            RedactionPolicy::withholding([(name("n_beat_1"), RedactionReason::Lost)]),
        ] {
            let compiled = CausalCompile::new(wider_order(), policy)
                .with_property(
                    automaton(Coverage::Total),
                    PropertyMonitor::over(automaton(Coverage::Total)),
                )
                .with_dependence(join())
                .run(&roots(), &residual())
                .expect("compiles");
            assert!(!compiled.is_property_preserving() || compiled.is_causally_closed());
        }
    }

    #[test]
    fn a_redaction_hole_costs_property_preservation_too() {
        let compiled = CausalCompile::new(
            wider_order(),
            RedactionPolicy::withholding([(name("e_submit"), RedactionReason::Purged)]),
        )
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .run(&roots(), &residual())
        .expect("compiles");

        assert!(!compiled.is_causally_closed());
        assert!(!compiled.is_property_preserving());
        assert_eq!(
            compiled.monitor_verdict(),
            Some(&MonitorVerdict::Divergent(
                MonitorDisagreement::ObservedEventDropped {
                    id: name("e_submit")
                }
            ))
        );
    }

    /// One manifest, three deciding stages, and each cell's reason traceable to the stage
    /// that decided it — the first compile in which `slice-irrelevant` and `heuristic-cutoff`
    /// are distinguishable at all.
    #[test]
    fn stage_three_drops_carry_the_automatons_coverage() {
        let cells = |compiled: &Compilation| {
            compiled
                .accounting()
                .manifest()
                .records()
                .iter()
                .map(|record| ((record.kind(), record.reason()), record.count()))
                .collect::<BTreeMap<(SelectionKind, OmissionReason), u32>>()
        };

        // Total coverage: a stage-3 drop (`e_probe`) is a proof of irrelevance, and so is a
        // stage-2 drop (`n_beat_1`) under a complete order — one cell of two.
        let total = staged().run(&roots(), &residual()).expect("compiles");
        assert_eq!(
            cells(&total),
            [
                ((SelectionKind::Event, OmissionReason::SliceIrrelevant), 2),
                ((SelectionKind::Model, OmissionReason::HeuristicCutoff), 1),
            ]
            .into()
        );

        // Partial coverage: the same drops, and `e_probe` splits off into its own honest
        // cell while the stage-2 drop keeps its proof. The two reasons now live in one
        // manifest, each naming the stage that could — or could not — decide.
        let partial = CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
            .with_property(
                automaton(Coverage::Partial {
                    reason: CoverageGap::UnenumeratedAbstraction,
                }),
                PropertyMonitor::over(automaton(Coverage::Total)),
            )
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("compiles");
        assert_eq!(
            cells(&partial),
            [
                ((SelectionKind::Event, OmissionReason::HeuristicCutoff), 1),
                ((SelectionKind::Event, OmissionReason::SliceIrrelevant), 1),
                ((SelectionKind::Model, OmissionReason::HeuristicCutoff), 1),
            ]
            .into()
        );
        // The selection is unchanged — coverage is a statement about what a drop *proves*.
        assert_eq!(total.selected(), partial.selected());
        assert!(partial.is_property_preserving());
        partial.accounting().reconcile().expect("halves agree");
    }

    #[test]
    fn stage_four_admits_only_a_corroborated_dependence() {
        let compiled = staged().run(&roots(), &residual()).expect("compiles");

        assert!(compiled.selected().contains(&name("s_writer")));
        assert!(!compiled.selected().contains(&name("m_commit")));
        assert_eq!(
            compiled.declined().get(&name("m_commit")),
            Some(&Inadmissible::UncorroboratedSource)
        );
        assert_eq!(
            compiled.admissible().keys().collect::<Vec<_>>(),
            [&name("s_writer")]
        );
        let items = compiled.admissible_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind(), SelectionKind::Source);
        assert_eq!(items[0].summary(), "crates/a/src/lib.rs:12:1-12:9");
        // The declined model candidate is a `heuristic-cutoff` record, never `slice-irrelevant`.
        let model_record = compiled
            .accounting()
            .manifest()
            .records()
            .iter()
            .find(|record| record.kind() == SelectionKind::Model)
            .expect("the model candidate is in the manifest");
        assert_eq!(model_record.reason(), OmissionReason::HeuristicCutoff);
        assert_eq!(model_record.count(), 1);
        compiled.accounting().reconcile().expect("halves agree");
    }

    #[test]
    fn a_join_claim_about_another_kind_is_refused() {
        let miscast = DependenceJoin::new(
            SourceCorrespondence::of([(
                name("e_ack"),
                CorrespondenceClaim::new(
                    CorrespondenceRef::Model(ModelActionRef::action_only(name("Commit"))),
                    [name("e_loss")],
                ),
            )])
            .expect("a well-formed correspondence"),
            ExecutionDependence::none(),
        );
        assert_eq!(
            CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
                .with_dependence(miscast)
                .run(&roots(), &residual()),
            Err(CompileError::NotACorrespondenceKind {
                id: name("e_ack"),
                kind: SelectionKind::Event,
            })
        );
    }

    #[test]
    fn a_join_claim_about_a_candidate_from_nowhere_is_refused() {
        let stray = DependenceJoin::new(
            SourceCorrespondence::of([(
                name("s_nowhere"),
                CorrespondenceClaim::new(
                    CorrespondenceRef::Model(ModelActionRef::action_only(name("Commit"))),
                    [name("e_loss")],
                ),
            )])
            .expect("a well-formed correspondence"),
            ExecutionDependence::none(),
        );
        assert_eq!(
            CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
                .with_dependence(stray)
                .run(&roots(), &residual()),
            Err(CompileError::Causal(CausalError::UnknownEndpoint {
                id: name("s_nowhere"),
            }))
        );
    }

    #[test]
    fn a_withheld_candidate_is_never_admitted() {
        let compiled = CausalCompile::new(
            wider_order(),
            RedactionPolicy::withholding([(name("s_writer"), RedactionReason::Purged)]),
        )
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("compiles");

        assert!(!compiled.selected().contains(&name("s_writer")));
        assert!(compiled.admissible().is_empty());
        assert!(!compiled.declined().contains_key(&name("s_writer")));
        let redacted = compiled
            .accounting()
            .manifest()
            .records()
            .iter()
            .find(|record| record.reason() == OmissionReason::Redaction)
            .expect("the withheld span is a redaction omission");
        assert_eq!(redacted.kind(), SelectionKind::Source);
        assert!(!redacted.retrievability().is_expandable());
        // The property core is untouched, so the property guarantee survives.
        assert!(compiled.is_property_preserving());
    }

    #[test]
    fn a_stage_that_did_not_run_leaves_no_intermediate() {
        let compiled = CausalCompile::new(wider_order(), RedactionPolicy::permitting_everything())
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("compiles");
        assert!(
            compiled
                .trail()
                .intermediate(Phase::Stage(Stage::PropertyRelevance))
                .is_none()
        );
        assert!(
            compiled
                .trail()
                .intermediate(Phase::Stage(Stage::DependenceJoin))
                .is_some()
        );
        assert_eq!(compiled.monitor_verdict(), None);
        assert!(!compiled.is_property_preserving());
    }

    #[test]
    fn two_runs_of_a_four_stage_compile_agree() {
        let one = staged().run(&roots(), &residual()).expect("compiles");
        let other = staged().run(&roots(), &residual()).expect("compiles");
        assert_eq!(one, other);
        assert_eq!(
            one.trail().to_json().to_canonical_bytes(),
            other.trail().to_json().to_canonical_bytes()
        );
    }

    #[test]
    fn the_redaction_reasons_are_the_schemas_three() {
        let tokens: Vec<&str> = RedactionReason::ALL
            .into_iter()
            .map(RedactionReason::as_wire_str)
            .collect();
        assert_eq!(tokens, ["summarized", "lost", "purged"]);
        for reason in RedactionReason::ALL {
            assert_eq!(
                RedactionReason::from_wire_str(reason.as_wire_str()),
                Some(reason)
            );
        }
        assert_eq!(RedactionReason::from_wire_str("redacted"), None);
        assert_eq!(RedactionReason::from_wire_str("Purged"), None);
    }
}
