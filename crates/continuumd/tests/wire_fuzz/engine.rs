//! The oracles, and the loop that applies them.
//!
//! # The oracles
//!
//! Four, applied to every target on every input, in this order:
//!
//! 1. **budget** — the input is measured against the target's declared ceiling *before*
//!    the decoder is called. Over-ceiling is [`CaseOutcome::BudgetExhausted`], which is
//!    neither a pass nor a finding.
//! 2. **[`Oracle::Panicked`]** — the probe runs inside [`std::panic::catch_unwind`].
//!    docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker and
//!    RFC 0026 requires "a typed error, never a panic".
//! 3. **[`Oracle::Nondeterministic`]** — the probe runs twice and the two answers must be
//!    equal. A decoder is a pure function of its bytes (INV-005); a second, different
//!    answer is hidden state or ambient input.
//! 4. **[`Oracle::Untyped`]** — the landing must be in the target's declared closed
//!    vocabulary (INV-008).
//! 5. **[`Oracle::RoundTrip`]** — an accepted input must re-encode to itself, or, where
//!    the decoder's own contract is the weaker one, its normal form must be a fixpoint.
//!
//! # Why `catch_unwind` and not "let the test crash"
//!
//! Because the bone's third clause is "every crash becomes a minimized regression". A
//! panic that aborts the test binary carries the input away with it; a caught one is a
//! [`CaseOutcome`] the minimizer can shrink and the corpus can keep. The unwinding
//! itself is the finding either way.

use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::corpus::Case;
use super::generate::{Generator, Xorshift};
use super::outcome::{Budget, CaseOutcome, Oracle, Probe, Reencoding};
use super::target::{ACCEPTED, Target};

/// How many nested calls to [`silenced`] are in progress.
static SILENCE: AtomicUsize = AtomicUsize::new(0);

/// Installed exactly once, and only ever chained onto whatever hook was already there.
static HOOK: OnceLock<()> = OnceLock::new();

fn install_hook() {
    HOOK.get_or_init(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if SILENCE.load(Ordering::SeqCst) == 0 {
                previous(info);
            }
        }));
    });
}

/// Run `body` with panic *messages* suppressed.
///
/// Only the anti-vacuity mutants use this: their panics are the expected result, and a
/// gate that printed a stack trace for a passing test would be training its readers to
/// ignore stack traces. The suppression is a message-printing concern only — the panic
/// still unwinds and is still caught, so nothing about the finding changes.
pub fn silenced<T>(body: impl FnOnce() -> T) -> T {
    install_hook();
    SILENCE.fetch_add(1, Ordering::SeqCst);
    let out = body();
    SILENCE.fetch_sub(1, Ordering::SeqCst);
    out
}

/// Probe once, catching an unwind.
fn catching(target: &dyn Target, bytes: &[u8]) -> Option<Probe> {
    install_hook();
    panic::catch_unwind(AssertUnwindSafe(|| target.probe(bytes))).ok()
}

/// Apply every oracle to one (target, input) pair.
///
/// The whole harness funnels through this function, so a target, a mutant, a corpus
/// replay and a minimizer candidate are all judged by exactly the same rules.
#[must_use]
pub fn evaluate(target: &dyn Target, bytes: &[u8]) -> CaseOutcome {
    let budget = target.budget();
    if bytes.len() > budget.input_bytes {
        return CaseOutcome::BudgetExhausted {
            limit: Budget::INPUT_BYTES,
            needed: bytes.len(),
            ceiling: budget.input_bytes,
        };
    }

    let Some(first) = catching(target, bytes) else {
        return CaseOutcome::Defect {
            oracle: Oracle::Panicked,
            landing: None,
        };
    };
    let Some(second) = catching(target, bytes) else {
        return CaseOutcome::Defect {
            oracle: Oracle::Panicked,
            landing: Some(first.landing),
        };
    };
    if first != second {
        return CaseOutcome::Defect {
            oracle: Oracle::Nondeterministic,
            landing: Some(first.landing),
        };
    }
    if !target.vocabulary().contains(&first.landing.as_str()) {
        return CaseOutcome::Defect {
            oracle: Oracle::Untyped,
            landing: Some(first.landing),
        };
    }

    match &first.reencoding {
        Reencoding::Rejected => CaseOutcome::Clean {
            landing: first.landing,
        },
        Reencoding::Canonical(reencoded) => {
            if reencoded.as_slice() == bytes {
                CaseOutcome::Clean {
                    landing: first.landing,
                }
            } else {
                CaseOutcome::Defect {
                    oracle: Oracle::RoundTrip,
                    landing: Some(first.landing),
                }
            }
        }
        Reencoding::NormalForm(normal) => {
            if normal.len() > budget.input_bytes {
                // The normal form outgrew the ceiling. Nothing is decided about the
                // fixpoint — reporting it as clean would be reporting a check that did
                // not run.
                return CaseOutcome::BudgetExhausted {
                    limit: "normal-form-bytes",
                    needed: normal.len(),
                    ceiling: budget.input_bytes,
                };
            }
            let Some(again) = catching(target, normal) else {
                return CaseOutcome::Defect {
                    oracle: Oracle::Panicked,
                    landing: Some(first.landing),
                };
            };
            let fixed = again.landing == ACCEPTED
                && match &again.reencoding {
                    Reencoding::NormalForm(second_normal) => second_normal == normal,
                    Reencoding::Canonical(second_normal) => second_normal == normal,
                    Reencoding::Rejected => false,
                };
            if fixed {
                CaseOutcome::Clean {
                    landing: first.landing,
                }
            } else {
                CaseOutcome::Defect {
                    oracle: Oracle::RoundTrip,
                    landing: Some(first.landing),
                }
            }
        }
    }
}

/// One (target, input, outcome) triple the campaign produced.
#[derive(Debug, Clone)]
pub struct Finding {
    /// The target that produced it.
    pub target: &'static str,
    /// The input, verbatim.
    pub bytes: Vec<u8>,
    /// What the oracles decided.
    pub outcome: CaseOutcome,
}

/// The result of one campaign over one target.
#[derive(Debug, Clone, Default)]
pub struct CampaignReport {
    /// How many inputs were judged.
    pub probes: usize,
    /// How many landed clean.
    pub clean: usize,
    /// How many did not fit the budget. A large number here means the generator is
    /// over-producing, not that the decoder is healthy.
    pub over_budget: usize,
    /// Every defect, in generation order.
    pub findings: Vec<Finding>,
    /// Every distinct landing the campaign reached, sorted.
    ///
    /// The campaign's own coverage statement: a run that only ever reached
    /// `json::truncated` explored one rule, whatever its probe count says.
    pub landings: Vec<String>,
}

/// Run `iterations` generated inputs per seed against `target`.
///
/// Deterministic in `(target, seeds, iterations, corpus)` and nothing else: no clock, no
/// entropy, no filesystem order (the corpus arrives already sorted), no thread
/// interleaving. Two runs of the same arguments produce the same report on any platform,
/// which is what makes a finding reproducible from the committed seed alone (INV-005).
#[must_use]
pub fn run_campaign(
    target: &'static dyn Target,
    seeds: &[u64],
    iterations: usize,
    corpus: &[Case],
) -> CampaignReport {
    let generator = Generator::new(target, corpus);
    let mut report = CampaignReport::default();
    let mut landings: Vec<String> = Vec::new();
    for seed in seeds {
        let mut rng = Xorshift::new(*seed);
        for _ in 0..iterations {
            let bytes = generator.generate(&mut rng, target.budget());
            let outcome = evaluate(target, &bytes);
            report.probes = report.probes.saturating_add(1);
            if let Some(landing) = outcome.landing()
                && !landings.iter().any(|seen| seen == landing)
            {
                landings.push(landing.to_owned());
            }
            match &outcome {
                CaseOutcome::Clean { .. } => report.clean = report.clean.saturating_add(1),
                CaseOutcome::BudgetExhausted { .. } => {
                    report.over_budget = report.over_budget.saturating_add(1);
                }
                CaseOutcome::Defect { .. } => report.findings.push(Finding {
                    target: target.name(),
                    bytes,
                    outcome,
                }),
            }
        }
    }
    landings.sort();
    report.landings = landings;
    report
}
