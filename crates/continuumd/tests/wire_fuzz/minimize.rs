//! Deterministic minimization: a finding shrunk to something a reviewer can read.
//!
//! # Why determinism is the requirement, not speed
//!
//! A minimized case is committed to the corpus and replayed as a named regression. If the
//! minimizer's output depended on a random restart, a thread interleaving, or a time
//! budget, then the committed case would be one of many possible shrinkings of the same
//! finding and nobody could re-derive it. Everything below is a fixed traversal in a fixed
//! order with a fixed probe ceiling (INV-005).
//!
//! # What is preserved
//!
//! The [`CaseOutcome::signature`] — the outcome's class and, for a defect, which oracle
//! failed and what the target reported. A reduction is accepted only when it produces the
//! *same* signature, so the minimizer cannot walk from one defect to a different one and
//! present the result as a shrinking of the first.
//!
//! # The two passes
//!
//! 1. **Deletion** (`ddmin` over contiguous chunks): halving chunk widths, left to right,
//!    accepting the first reduction that preserves the signature. Bounded by
//!    [`MAX_PROBES`].
//! 2. **Flattening**: each surviving byte is set to zero, left to right, keeping the
//!    change when the signature survives. This does not shorten the case; it removes the
//!    incidental bytes, so what is left is the part that matters.

use super::engine::evaluate;
use super::outcome::CaseOutcome;
use super::target::Target;

/// The probe ceiling for one minimization.
///
/// An explicit resource limit like every other in this harness: minimization is a search,
/// and a search without a bound is a way to hang a gate.
pub const MAX_PROBES: usize = 4096;

/// The result of one minimization.
#[derive(Debug, Clone)]
pub struct Minimized {
    /// The shrunk input.
    pub bytes: Vec<u8>,
    /// The outcome it still produces.
    pub outcome: CaseOutcome,
    /// How many probes the search spent.
    pub probes: usize,
    /// Whether the search stopped because it ran out of probes rather than because it
    /// reached a fixpoint.
    ///
    /// Reported rather than hidden: "this is the smallest input reproducing the finding"
    /// and "this is the smallest input the budget reached" are different claims.
    pub exhausted: bool,
}

/// Shrink `bytes` while preserving the outcome signature `target` produces for them.
///
/// # Panics
///
/// Never. The input is always a valid answer, so the search has a fixpoint to fall back
/// on.
#[must_use]
pub fn minimize(target: &dyn Target, bytes: &[u8]) -> Minimized {
    let original = evaluate(target, bytes);
    let signature = original.signature();
    let mut current = bytes.to_vec();
    let mut probes = 0_usize;
    let mut exhausted = false;

    // --- pass 1: contiguous deletion -------------------------------------------------
    let mut partitions = 2_usize;
    while current.len() >= 2 {
        let width = current.len().div_ceil(partitions);
        let mut reduced = false;
        let mut start = 0_usize;
        while start < current.len() {
            if probes >= MAX_PROBES {
                exhausted = true;
                break;
            }
            let end = (start + width).min(current.len());
            let mut candidate = Vec::with_capacity(current.len() - (end - start));
            candidate.extend_from_slice(&current[..start]);
            candidate.extend_from_slice(&current[end..]);
            probes = probes.saturating_add(1);
            if evaluate(target, &candidate).signature() == signature {
                current = candidate;
                reduced = true;
                break;
            }
            start = end;
        }
        if exhausted {
            break;
        }
        if reduced {
            partitions = partitions.saturating_sub(1).max(2);
        } else if partitions >= current.len() {
            break;
        } else {
            partitions = partitions.saturating_mul(2).min(current.len());
        }
    }

    // --- pass 2: flattening ----------------------------------------------------------
    let mut index = 0_usize;
    while index < current.len() {
        if probes >= MAX_PROBES {
            exhausted = true;
            break;
        }
        if current[index] == 0 {
            index += 1;
            continue;
        }
        let mut candidate = current.clone();
        candidate[index] = 0;
        probes = probes.saturating_add(1);
        if evaluate(target, &candidate).signature() == signature {
            current = candidate;
        }
        index += 1;
    }

    let outcome = evaluate(target, &current);
    // The search only ever accepts signature-preserving steps, so this holds by
    // construction; asserting it here makes a future edit that breaks the invariant fail
    // loudly rather than commit a regression that reproduces something else.
    assert_eq!(
        outcome.signature(),
        signature,
        "minimization changed the finding: {} became {}",
        signature,
        outcome.signature()
    );
    Minimized {
        bytes: current,
        outcome,
        probes,
        exhausted,
    }
}
