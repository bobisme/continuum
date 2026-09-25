//! Delta debugging over a fixture's declarations (Zeller and Hildebrandt, TSE 2002).
//!
//! `ddmin` takes the list of deletable declarations ([`super::fixture::Element`]) and
//! a test that says whether a subset still reproduces the failure. It splits the list
//! into `n` chunks, keeps a chunk or a complement that still reproduces, and refines
//! `n` until every chunk is one element. On termination without a budget stop, the
//! result is **1-minimal**: removing any single remaining element no longer
//! reproduces, because the last round tested exactly those complements.
//!
//! Deterministic: chunks are contiguous in the input order and are tried in order, so
//! one input and one test give one result. The test is charged against
//! [`MinimizeBudget::tests`] *before* it runs; when the budget is spent the smallest
//! reproducing subset found so far is returned, marked [`Minimality::BudgetExhausted`]
//! rather than claimed minimal (INV-008).
//!
//! Decision records: RFC 0026 (a `defect_*` pins an automatically minimized
//! reproduction) and ADR-0003 (deterministic order).

/// How many subset tests minimization may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimizeBudget {
    /// Most tests.
    pub tests: usize,
}

/// What the returned subset is known to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Minimality {
    /// No single element can be removed and still reproduce.
    OneMinimal,
    /// The test budget ran out first; the subset reproduces and may not be minimal.
    BudgetExhausted,
    /// The minimized subset did not reproduce when it was rebuilt and re-run for the
    /// report (an engine that is not a function of its input), so the report carries
    /// the original fixture, unminimized.
    NotReproduced,
}

impl Minimality {
    /// `one-minimal` or `budget-exhausted`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneMinimal => "one-minimal",
            Self::BudgetExhausted => "budget-exhausted",
            Self::NotReproduced => "not-reproduced",
        }
    }
}

/// The minimized subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minimized<E> {
    /// The subset, in input order.
    pub kept: Vec<E>,
    /// Tests run.
    pub tests: usize,
    /// What it is known to be.
    pub minimality: Minimality,
}

fn chunks<E: Clone>(items: &[E], n: usize) -> Vec<Vec<E>> {
    let n = n.clamp(1, items.len().max(1));
    let base = items.len().checked_div(n).unwrap_or(0);
    let extra = items.len().checked_rem(n).unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    let mut rest = items;
    for index in 0..n {
        let size = if index < extra {
            base.saturating_add(1)
        } else {
            base
        };
        let (head, tail) = rest.split_at(size.min(rest.len()));
        out.push(head.to_vec());
        rest = tail;
    }
    out
}

/// Minimize `items` under `reproduces`. `items` itself is assumed to reproduce; the
/// caller established that before asking.
pub fn ddmin<E: Clone, F: FnMut(&[E]) -> bool>(
    items: Vec<E>,
    budget: MinimizeBudget,
    mut reproduces: F,
) -> Minimized<E> {
    let mut current = items;
    let mut tests: usize = 0;
    let mut n: usize = 2;
    let mut try_subset = |candidate: &[E], tests: &mut usize| -> Option<bool> {
        if *tests >= budget.tests {
            return None;
        }
        *tests = tests.saturating_add(1);
        Some(reproduces(candidate))
    };
    while current.len() >= 2 {
        let parts = chunks(&current, n);
        let mut next: Option<(Vec<E>, usize)> = None;
        for part in &parts {
            match try_subset(part, &mut tests) {
                None => {
                    return Minimized {
                        kept: current,
                        tests,
                        minimality: Minimality::BudgetExhausted,
                    };
                }
                Some(true) => {
                    next = Some((part.clone(), 2));
                    break;
                }
                Some(false) => {}
            }
        }
        if next.is_none() && parts.len() > 2 {
            for skip in 0..parts.len() {
                let complement: Vec<E> = parts
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != skip)
                    .flat_map(|(_, part)| part.iter().cloned())
                    .collect();
                match try_subset(&complement, &mut tests) {
                    None => {
                        return Minimized {
                            kept: current,
                            tests,
                            minimality: Minimality::BudgetExhausted,
                        };
                    }
                    Some(true) => {
                        next = Some((complement, n.saturating_sub(1).max(2)));
                        break;
                    }
                    Some(false) => {}
                }
            }
        }
        match next {
            Some((smaller, granularity)) => {
                current = smaller;
                n = granularity;
            }
            None if n >= current.len() => break,
            None => n = n.saturating_mul(2).min(current.len()),
        }
    }
    // A one-element list is 1-minimal only if the empty list does not reproduce.
    if current.len() == 1 {
        match try_subset(&[], &mut tests) {
            None => {
                return Minimized {
                    kept: current,
                    tests,
                    minimality: Minimality::BudgetExhausted,
                };
            }
            Some(true) => current.clear(),
            Some(false) => {}
        }
    }
    Minimized {
        kept: current,
        tests,
        minimality: Minimality::OneMinimal,
    }
}
