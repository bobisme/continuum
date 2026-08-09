//! The compiler's first stage group, run: the redaction pre-pass, stage 1 (root selection),
//! and stage 2 (backward causal slicing), ending in a closed accounting, an auditable trail,
//! and the guarantees those stages actually established.
//!
//! # Scope: bn-21vno — stages 1–2 of RFC 0028's ten
//!
//! | Phase | RFC 0028 | Licenses |
//! |---|---|---|
//! | pre-pass | "Redaction runs before stage 1" (correction 12) | nothing |
//! | 1 | root selection | "nothing on its own; a bad root is a wrong pack, not an unsound one" |
//! | 2 | backward causal slicing | `CausallyClosed`, and the precondition of `ReplayPreserving` |
//!
//! Stages 3–8 are their own bones and append to [`crate::stage::StageTrail`]; stage 9 is the
//! ranker RFC 0028 requires a deployment be able to run *without* ("the register row's named
//! fallback for the whole capability is 'plain causal slice + expansion' — that is, stages
//! 1–8 plus the expansion protocol, with stage 9 disabled"); stage 10 is
//! [`crate::budget::BudgetPacker`] in substance.
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
//! **A drop is only `slice-irrelevant` if the order it was decided against is complete.**
//! See [`crate::causal::Completeness`]. Under a partial order the same drop is
//! `heuristic-cutoff`, because non-ancestry in an incomplete order proves nothing.
//!
//! # Why the closure check runs on the published selection
//!
//! [`crate::causal::CausalOrder::backward_closure`] produces a set that is closed by
//! construction — and that is exactly why the licence does not come from it. RFC 0028's
//! Validation section requires a checker independent of the producer, so
//! [`CausalCompile::run`] takes the selection it is actually about to publish (after
//! redaction has removed items from it) and hands it to
//! [`crate::causal::CausalOrder::closure_violation`], which reads the order's edges and
//! nothing else. The licence is issued from the checker's verdict. A slicer bug that dropped
//! a predecessor therefore costs the guarantee rather than producing a false one.
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
//! | deterministic (INV-005) | INV-005, ADR-0003 | `two_runs_of_one_compile_agree` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Name;

use crate::accounting::{Accounting, AccountingError, CandidateSet, ClosedAccounting, Omitted};
use crate::causal::{CausalError, CausalOrder, ClosureViolation};
use crate::expansion::ExpansionQuery;
use crate::guarantee::{Guarantee, GuaranteeSet, License, SealedGuarantees};
use crate::omission::IrretrievableReason;
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

/// A compile of stages 1–2 over one causal order under one field policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalCompile {
    order: CausalOrder,
    policy: RedactionPolicy,
}

impl CausalCompile {
    /// Prepare a compile.
    #[must_use]
    pub const fn new(order: CausalOrder, policy: RedactionPolicy) -> Self {
        Self { order, policy }
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

    /// Run the pre-pass, stage 1, and stage 2.
    ///
    /// `roots` is stage 1's output — the handles the question names. `residual` is the
    /// expansion query that retrieves the candidates stage 2 leaves behind; its anchor MUST
    /// resolve in the published selection, because "expansion is navigation over a published
    /// pack, not a general graph query" (RFC 0028, "Expansion protocol").
    ///
    /// # Errors
    ///
    /// [`CompileError::RedactedRoot`] for a root field policy withheld (correction 12);
    /// [`CompileError::Causal`] for a root that is not a node of the order;
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

        // --- the independent closure check, on the selection that will be published -----
        let violation = self.order.closure_violation(&selection);
        let mut guarantees = GuaranteeSet::empty();
        if violation.is_none() {
            guarantees.claim(License::issue(Guarantee::CausallyClosed));
        }

        // --- accounting: every candidate dispositioned ----------------------------------
        let candidates = CandidateSet::new(
            self.order
                .nodes()
                .map(|(id, kind)| (id.clone(), kind))
                .collect::<Vec<_>>(),
        )?;
        let mut accounting = Accounting::over(candidates);
        let slice_reason = self.order.completeness().omission_reason();
        let mut residual_used = false;
        for (id, _) in self.order.nodes() {
            if selection.contains(id) {
                accounting.select(id)?;
            } else if self.policy.withholds(id) {
                // Withheld, whether or not the slice wanted it. `expandable: false` and the
                // reason that explains irretrievability — RFC 0028's expansion table's
                // redacted row, read at compile time rather than at expansion time.
                accounting.omit(id, Omitted::Irretrievable(IrretrievableReason::Redaction))?;
            } else {
                residual_used = true;
                accounting.omit(
                    id,
                    Omitted::Expandable {
                        reason: slice_reason,
                        query: residual.clone(),
                    },
                )?;
            }
        }
        let accounting = accounting.close()?;
        accounting.reconcile()?;

        if residual_used && !selection.contains(residual.anchor()) {
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
        })
    }
}

/// What stages 1–2 produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    trail: StageTrail,
    accounting: ClosedAccounting,
    guarantees: SealedGuarantees,
    closure_violation: Option<ClosureViolation>,
    redaction_gap: BTreeSet<Name>,
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
    use crate::expansion::ExpansionRelation;
    use crate::omission::OmissionReason;
    use crate::selection::SelectionKind;

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
