//! A deterministic delta minimizer over source text.
//!
//! It removes runs of characters of halving length (the whole text, halves, quarters,
//! …, single characters), repeated until a pass removes nothing, and keeps a removal
//! only when the outcome's [`Outcome::signature`] — the oracle and the landing — is
//! unchanged. The candidate order is fixed, so two minimizations of one finding give the
//! same text, and a committed regression is re-derivable.

use super::engine::{Outcome, Target, evaluate};

/// The most candidates one minimization evaluates.
pub const MAX_PROBES: usize = 4000;

/// The result of one minimization.
#[derive(Debug, Clone)]
pub struct Minimized {
    /// The smallest source found.
    pub source: String,
    /// Its outcome (same signature as the input's).
    pub outcome: Outcome,
    /// Candidates evaluated.
    pub probes: usize,
    /// Whether [`MAX_PROBES`] stopped it before a fixpoint.
    pub exhausted: bool,
}

/// Shrink `src` while `target` keeps the same outcome signature.
#[must_use]
pub fn minimize(target: &'static dyn Target, src: &str) -> Minimized {
    let want = evaluate(target, src);
    let signature = want.signature();
    let mut best: Vec<char> = src.chars().collect();
    let mut outcome = want;
    let mut probes = 0;
    // Character runs of halving length, until a pass changes nothing.
    loop {
        let before = best.len();
        let mut chunk = best.len().max(1);
        while chunk >= 1 {
            let mut at = 0;
            while at < best.len() {
                if probes >= MAX_PROBES {
                    return Minimized {
                        source: best.into_iter().collect(),
                        outcome,
                        probes,
                        exhausted: true,
                    };
                }
                let end = (at + chunk).min(best.len());
                let mut candidate = best.clone();
                candidate.drain(at..end);
                probes += 1;
                let text: String = candidate.iter().collect();
                let got = evaluate(target, &text);
                if got.signature() == signature {
                    best = candidate;
                    outcome = got;
                } else {
                    at += chunk;
                }
            }
            chunk /= 2;
        }
        if best.len() == before {
            break;
        }
    }
    Minimized {
        source: best.into_iter().collect(),
        outcome,
        probes,
        exhausted: false,
    }
}
