//! `query.explain_invalidation` over two revisions (RFC 0030, "Wire surface").
//!
//! The question is "why did this re-run, and which edge caused the rebuild". The
//! answer partitions every query of either revision into exactly one of three sets,
//! in label order (deterministic, so pagination over it is too):
//!
//! - **invalidated** — recomputed, or no longer demanded. Each entry carries its
//!   [`Cause`]: the chain of edges from the changed input, each step with its
//!   dependency reason and its reuse-edge class, or the downgrade that refused a
//!   licence (quarantine, lane, a witness the kernel did not verify).
//! - **unknown** — reused only through an `Experimental` licence, or derived from
//!   such a reuse. Its independence from the edit is heuristic, so it is `Unknown`,
//!   and RFC 0030 forbids reporting it as `reused`.
//! - **reused** — reused through an `Exact`, `Validated` or `Conservative` licence
//!   on a derivation with no `Experimental` edge.
//!
//! A chain step is taken only across a *changed* input: one whose canonical output
//! differs between the two revisions, or that exists in only one of them. The walk
//! follows the declared edge order and stops at the source, so it is bounded by the
//! pipeline's depth and each step is one ordered-map lookup. Each invalidation
//! carries **one** chain: the first changed input in declared edge order at each
//! step. When several changed inputs reach one query, the others are not listed;
//! this is an explanation for a reader, not the complete edge set.
//!
//! This is not the wire answer. `query.explain_invalidation`'s response
//! (`rule query.invalidation_edges`) needs one `<handle>:<reason>` edge per changed
//! reason, sorted by handle then reason, over artifact handles the daemon holds;
//! the spike neither builds that projection nor claims to. It is an ADR residual,
//! with the daemon wiring that needs an RFC 0031 `diff_*` artifact.
//!
//! Reads do not decide: nothing here changes a class, the quarantine set, or a
//! cache entry.

use std::collections::BTreeSet;

use crate::engine::{Cause as RecomputeCause, Outcome, Record, Revision};
use crate::query::{Label, Stage};
use crate::vocab::{DependencyReason, EvidenceForm, Independence, ReuseClass};

/// One edge of an invalidation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The changed input.
    pub from: Label,
    /// The query that read it.
    pub to: Label,
    /// Why it read it.
    pub reason: DependencyReason,
    /// The edge's declared class.
    pub class: ReuseClass,
}

/// Why a query is invalidated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cause {
    /// An input changed: the chain from the source, in dependency order.
    Input(Vec<Step>),
    /// No input changed and a licence existed, but it was refused.
    Downgrade(RecomputeCause),
    /// No input changed and no cached entry matched (a cold cache or an evicted
    /// entry). Over-invalidation, never under-invalidation.
    NotCached,
    /// A budget-dependent result is never stored, so it is always recomputed.
    NotMemoized(&'static str),
}

/// Whether an invalidated query was recomputed or dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Recomputed in the later revision.
    Recomputed,
    /// Demanded by the earlier revision only (an upstream failure, a removed
    /// invariant, or a renamed one): its result is unaddressable.
    Removed,
}

/// One invalidated query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalidated {
    /// The query.
    pub label: Label,
    /// The handle of the key it is invalidated under (the later revision's, or the
    /// earlier one's when removed).
    pub handle: String,
    /// Recomputed or removed.
    pub status: Status,
    /// Why.
    pub cause: Cause,
}

/// One query whose reuse is heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknown {
    /// The query.
    pub label: Label,
    /// Its handle.
    pub handle: String,
    /// Always [`Independence::Unknown`]; kept typed so a consumer cannot read the
    /// entry as independent.
    pub independence: Independence,
}

/// One reused query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reused {
    /// The query.
    pub label: Label,
    /// Its handle.
    pub handle: String,
    /// The licence's class.
    pub licence: ReuseClass,
    /// The evidence form of the licence.
    pub evidence: EvidenceForm,
    /// The derivation's class (the meet).
    pub class: ReuseClass,
    /// The assumptions of the `Conservative` edges the licence rested on.
    pub assumptions: Vec<&'static str>,
}

/// The answer to `query.explain_invalidation` for the edit between two revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalidation {
    /// Whether the source changed at all (typed, not a bare flag).
    pub source: SourceChange,
    /// Recomputed or removed queries.
    pub invalidated: Vec<Invalidated>,
    /// Heuristically reused queries.
    pub unknown: Vec<Unknown>,
    /// Soundly reused queries.
    pub reused: Vec<Reused>,
}

/// Whether the source text differs between the two revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChange {
    /// Byte-identical.
    Unchanged,
    /// Different bytes.
    Changed,
}

/// Whether `label`'s output differs between the revisions (or it exists in one).
fn changed(before: &Revision, after: &Revision, label: &Label) -> bool {
    if label.stage == Stage::Source {
        return before.source() != after.source();
    }
    match (before.get(label), after.get(label)) {
        (Some(a), Some(b)) => a.output != b.output,
        (None, None) => false,
        _ => true,
    }
}

/// The chain of changed inputs into `label`, walking `record`'s edges (and each
/// parent's, in whichever revision holds it) back to the source.
fn chain(before: &Revision, after: &Revision, record: &Record) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut current = record;
    // Each step moves to a strictly earlier stage, so this ends within the stage
    // count; the explicit bound keeps that a fact of the loop, not of the registry.
    for _ in 0..16 {
        let Some(edge) = current
            .edges
            .iter()
            .find(|edge| changed(before, after, &edge.from))
        else {
            break;
        };
        steps.push(Step {
            from: edge.from.clone(),
            to: current.label.clone(),
            reason: edge.spec.reason,
            class: edge.spec.class,
        });
        if edge.from.stage == Stage::Source {
            break;
        }
        match after.get(&edge.from).or_else(|| before.get(&edge.from)) {
            Some(parent) => current = parent,
            None => break,
        }
    }
    steps.reverse();
    steps
}

/// Explain the invalidation between `before` and `after`.
#[must_use]
pub fn explain_invalidation(before: &Revision, after: &Revision) -> Invalidation {
    let labels: BTreeSet<&Label> = before.records.keys().chain(after.records.keys()).collect();
    let mut invalidated = Vec::new();
    let mut unknown = Vec::new();
    let mut reused = Vec::new();
    for label in labels {
        let Some(record) = after.get(label) else {
            if let Some(old) = before.get(label) {
                invalidated.push(Invalidated {
                    label: label.clone(),
                    handle: old.handle.clone(),
                    status: Status::Removed,
                    cause: Cause::Input(chain(before, after, old)),
                });
            }
            continue;
        };
        match &record.outcome {
            Outcome::Reused {
                licence,
                evidence,
                assumptions,
                ..
            } if record.class != ReuseClass::Experimental => {
                // The assumptions are the ones the decision recorded, for the side
                // inputs it actually compared, not a re-derivation from this pair
                // of revisions.
                let assumptions = assumptions.clone();
                reused.push(Reused {
                    label: label.clone(),
                    handle: record.handle.clone(),
                    licence: *licence,
                    // A strong or conservative licence always carries its form.
                    evidence: evidence.unwrap_or(EvidenceForm::ContentIdentity),
                    class: record.class,
                    assumptions,
                });
            }
            Outcome::Reused { .. } => unknown.push(Unknown {
                label: label.clone(),
                handle: record.handle.clone(),
                independence: Independence::Unknown,
            }),
            Outcome::Recomputed { cause, memo } => {
                // A refused licence means the key matched: the downgrade is the
                // cause, whatever else changed. Otherwise the changed input is.
                let why = match cause {
                    RecomputeCause::Quarantined(_) | RecomputeCause::LaneRefused(_) => {
                        Cause::Downgrade(cause.clone())
                    }
                    RecomputeCause::KeyMiss
                    | RecomputeCause::WitnessRefused(_)
                    | RecomputeCause::AlternateRefused(_) => {
                        let steps = chain(before, after, record);
                        if !steps.is_empty() {
                            Cause::Input(steps)
                        } else if let crate::engine::Memo::NotMemoized(reason) = memo {
                            Cause::NotMemoized(reason)
                        } else if *cause == RecomputeCause::KeyMiss {
                            Cause::NotCached
                        } else {
                            Cause::Downgrade(cause.clone())
                        }
                    }
                };
                invalidated.push(Invalidated {
                    label: label.clone(),
                    handle: record.handle.clone(),
                    status: Status::Recomputed,
                    cause: why,
                });
            }
        }
    }
    Invalidation {
        source: if before.source() == after.source() {
            SourceChange::Unchanged
        } else {
            SourceChange::Changed
        },
        invalidated,
        unknown,
        reused,
    }
}
