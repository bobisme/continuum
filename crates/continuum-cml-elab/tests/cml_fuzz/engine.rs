//! The budgets, the outcome vocabulary, the oracles, and the loop that applies them.
//!
//! # The budgets
//!
//! Every probe runs under three budgets, and each one is enforced by something other
//! than the code under test:
//!
//! 1. **input size** — measured *before* the target runs. Over the ceiling is
//!    [`Outcome::BudgetExhausted`], which is neither a pass nor a finding (INV-008).
//! 2. **stack** — the probe runs on a fresh thread with [`SMALL_STACK`], half of Rust's
//!    default 2 MiB thread stack (the parser's own stated bound for its deepest accepted
//!    tree, unoptimized), so a recursion that a default-sized thread would barely
//!    survive still aborts the test binary here. An overflow is not catchable; the binary dies
//!    and the gate fails, which is the finding. `cml_fuzz.rs` shows in a child process
//!    that the runner's stack really is that small.
//! 3. **work and wall clock** — the elaborator and the lowering run under explicit
//!    [`Limits`] ([`FUZZ_LIMITS`], far below the production defaults), and the harness
//!    checks the [`Usage`] they report never passes them ([`Oracle::Overspent`]). The
//!    work budget is the real bound: it is counted, so it is deterministic. The wall
//!    clock ([`Budget::wall`]) is only the backstop for a loop that the work budget does
//!    not meter: the parent waits on a channel with a timeout and reports
//!    [`Oracle::Hung`] if the probe thread has not answered. It never decides a pass.
//!
//! # The oracles
//!
//! Applied to every target on every input, in this order:
//!
//! - [`Oracle::Panicked`] — the probe unwound. Source is untrusted (INV-016), and a
//!   malformed source must be a typed error, never a panic.
//! - [`Oracle::Hung`] — the probe did not answer within the wall budget.
//! - [`Oracle::Nondeterministic`] — two probes of the same source disagree on the
//!   landing, the identity fingerprint, the error text, the span, or the usage.
//! - [`Oracle::Untyped`] — the landing is outside the target's closed vocabulary.
//! - [`Oracle::Mislocated`] — an error span is out of bounds, splits a character, or its
//!   line and column disagree with its byte offset.
//! - [`Oracle::Overspent`] — a reported [`Usage`] passes the [`Limits`] it ran under.
//! - [`Oracle::RoundTrip`] — the printed form of an accepted parse does not re-parse to
//!   the same tree, or does not elaborate or lower to the same identity or refusal.
//! - [`Oracle::TooDeep`] — an accepted tree nests deeper than the stage's depth bound
//!   allows, measured without recursion: exactly, on the parse tree
//!   (`depth::file_tree_depth` against `MAX_NESTING`), and within a factor of two on the
//!   normalized model's dump ([`dump_nesting`]). A tree past the bound is the
//!   precondition of a stack overflow in every later recursive pass, so this oracle fires
//!   on a tree just past the bound, before a deeper one overflows the measurement's
//!   neighbours (dump, print) and aborts the binary instead.
//!
//! Each oracle has a deliberately defective mutant in `mutant.rs` that trips it, so a
//! clean campaign is evidence rather than a hypothesis.

use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use continuum_cml_elab::{Limits, Usage};
use continuum_cml_syntax::Span;

/// The probe thread's stack: half of Rust's default 2 MiB thread stack.
///
/// The parser states that its deepest accepted tree fits in half a default stack,
/// unoptimized (`continuum-cml-syntax/tests/errors.rs`), and not in a quarter: an
/// `if … else` chain at the nesting bound needs more than 512 KiB in a debug build, and
/// so does elaborating a chain of builtin calls (`min(0, min(0, …))`) forty levels deep.
/// Both fit here with a factor of two to the default thread stack.
pub const SMALL_STACK: usize = 1024 * 1024;

/// The elaboration and lowering limits of a fuzz probe.
///
/// 64 Ki nodes and 16 Mi work units: a sixteenth and a hundred-and-twenty-eighth of the
/// production [`continuum_cml_elab::budget::MAX_NODES`] and
/// [`continuum_cml_elab::budget::MAX_WORK`]. The `cases/budget-*.ctm` cases show the
/// limits bind: each is refused here and accepted under the defaults. One seed is refused
/// here too: the replicated register under its run configuration predicts about 7·10⁸
/// work units, so its mutants reach the configured lowering's bindings and preflight but
/// not its build. The manifest pins that refusal.
pub const FUZZ_LIMITS: Limits = Limits {
    nodes: 1 << 16,
    work: 1 << 24,
};

/// The resource ceilings of one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// The largest source, in bytes, the target is handed. Checked before it runs.
    pub input_bytes: usize,
    /// How long the parent waits for one evaluation (two probes and their round trips).
    pub wall: Duration,
}

impl Budget {
    /// 32 KiB of source and 30 seconds of wall clock per evaluation. The wall budget is
    /// three orders of magnitude above what a probe under [`FUZZ_LIMITS`] takes, so it
    /// fires only for a loop the work budget does not meter.
    pub const DEFAULT: Self = Self {
        input_bytes: 32 * 1024,
        wall: Duration::from_secs(30),
    };

    /// The token of the input-size ceiling.
    pub const INPUT_BYTES: &'static str = "input-bytes";
}

/// Whether the printed form of the input agrees with the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundTrip {
    /// The input did not parse, so there is no printed form.
    NotApplicable,
    /// The printed form re-parsed and agreed.
    Held,
    /// The printed form disagreed; the text says how.
    Broke(String),
}

/// What one probe of one target answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// `accepted`, or the stable code of the typed refusal.
    pub landing: String,
    /// What must be reproducible: an identity fingerprint, a dump fingerprint, or the
    /// whole error text.
    pub detail: String,
    /// The error's source span, when the target refused.
    pub span: Option<Span>,
    /// Every budgeted pass the probe ran, with the limits it ran under.
    pub usage: Vec<(Limits, Usage)>,
    /// The round-trip verdict.
    pub round_trip: RoundTrip,
    /// The depth of the accepted tree (parse: exact expression depth; elaborate: the
    /// canonical dump's parenthesis nesting), or 0.
    pub depth: usize,
}

/// One fuzz target.
pub trait Target: Sync {
    /// The stable name.
    fn name(&self) -> &'static str;
    /// Every landing this target may report.
    fn vocabulary(&self) -> &'static [&'static str];
    /// The ceilings.
    fn budget(&self) -> Budget {
        Budget::DEFAULT
    }
    /// The deepest dump nesting an accepted tree may have.
    fn depth_limit(&self) -> usize {
        usize::MAX
    }
    /// Run the target once.
    fn probe(&self, src: &str) -> Probe;
}

/// The parenthesis nesting of an S-expression dump, skipping string literals (which are
/// written quoted, with backslash escapes). Iterative, so it measures a tree of any depth
/// without recursing.
#[must_use]
pub fn dump_nesting(text: &str) -> usize {
    let (mut depth, mut max, mut in_string, mut escaped) = (0_usize, 0_usize, false, false);
    for c in text.chars() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => {
                depth += 1;
                max = max.max(depth);
            }
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// Which property failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Oracle {
    /// The probe unwound.
    Panicked,
    /// The probe did not answer within the wall budget.
    Hung,
    /// Two probes of one source disagreed.
    Nondeterministic,
    /// The landing is outside the closed vocabulary.
    Untyped,
    /// An error span does not locate a place in the source.
    Mislocated,
    /// A reported usage passes its limits.
    Overspent,
    /// The printed form disagrees with the source.
    RoundTrip,
    /// An accepted tree nests past the stage's depth bound.
    TooDeep,
}

impl Oracle {
    /// Every arm, for the anti-vacuity matrix.
    pub const ALL: [Self; 8] = [
        Self::Panicked,
        Self::Hung,
        Self::Nondeterministic,
        Self::Untyped,
        Self::Mislocated,
        Self::Overspent,
        Self::RoundTrip,
        Self::TooDeep,
    ];
}

/// What the harness decided about one (target, source) pair. Three facts, never two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every oracle held.
    Clean {
        /// Where the target landed.
        landing: String,
    },
    /// The source did not fit the budget, so nothing was decided about it.
    BudgetExhausted {
        /// Which ceiling.
        limit: &'static str,
        /// What the source needed.
        needed: usize,
        /// The ceiling.
        ceiling: usize,
    },
    /// An oracle failed.
    Defect {
        /// Which one.
        oracle: Oracle,
        /// The first probe's landing, if there was one.
        landing: Option<String>,
        /// What was seen.
        detail: String,
    },
}

impl Outcome {
    /// Whether this is a finding.
    #[must_use]
    pub fn is_defect(&self) -> bool {
        matches!(self, Self::Defect { .. })
    }

    /// The landing, when the target ran.
    #[must_use]
    pub fn landing(&self) -> Option<&str> {
        match self {
            Self::Clean { landing } => Some(landing),
            Self::Defect { landing, .. } => landing.as_deref(),
            Self::BudgetExhausted { .. } => None,
        }
    }

    /// What a minimizer must preserve: the oracle and the landing, not the detail text
    /// (which carries spans and so changes as the source shrinks).
    #[must_use]
    pub fn signature(&self) -> (Option<Oracle>, Option<String>) {
        match self {
            Self::Clean { landing } => (None, Some(landing.clone())),
            Self::Defect {
                oracle, landing, ..
            } => (Some(*oracle), landing.clone()),
            Self::BudgetExhausted { .. } => (None, None),
        }
    }
}

// --- panic-message silencing, for the mutants only -------------------------------------

static SILENCE: AtomicUsize = AtomicUsize::new(0);
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

/// Run `body` with panic *messages* suppressed. The panics still unwind and are still
/// caught; only the printing stops, so an expected mutant panic does not look like a
/// failure in the log.
pub fn silenced<T>(body: impl FnOnce() -> T) -> T {
    install_hook();
    SILENCE.fetch_add(1, Ordering::SeqCst);
    let out = body();
    SILENCE.fetch_sub(1, Ordering::SeqCst);
    out
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a non-text panic payload".to_owned())
}

type Pair = (Result<Probe, String>, Option<Result<Probe, String>>);

/// Run both probes on one fresh [`SMALL_STACK`] thread, and wait at most `wall`.
fn run_bounded(target: &'static dyn Target, src: &str, wall: Duration) -> Result<Pair, ()> {
    install_hook();
    let (tx, rx) = mpsc::channel::<Pair>();
    let owned = src.to_owned();
    let spawned = thread::Builder::new()
        .name(format!("cml-fuzz-{}", target.name()))
        .stack_size(SMALL_STACK)
        .spawn(move || {
            let once = || {
                panic::catch_unwind(AssertUnwindSafe(|| target.probe(&owned)))
                    .map_err(|p| panic_text(p.as_ref()))
            };
            let first = once();
            let second = first.is_ok().then(once);
            // The parent may have given up (the wall budget); a closed channel is fine.
            let _ = tx.send((first, second));
        })
        .expect("spawn a probe thread");
    match rx.recv_timeout(wall) {
        Ok(pair) => {
            let _ = spawned.join();
            Ok(pair)
        }
        // A timed-out probe thread is left running and detached: it cannot be killed
        // from safe Rust. The finding is reported either way.
        Err(mpsc::RecvTimeoutError::Timeout) => Err(()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok((
            Err("the probe thread ended without answering".to_owned()),
            None,
        )),
    }
}

/// The 1-based line and character column of byte `offset` in `src`.
fn locate(src: &str, offset: usize) -> (u32, u32) {
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(col).unwrap_or(u32::MAX),
    )
}

/// Why `span` does not locate a place in `src`, if it does not.
#[must_use]
pub fn span_defect(src: &str, span: Span) -> Option<String> {
    let (start, end) = (span.start as usize, span.end as usize);
    if start > end || end > src.len() {
        return Some(format!("span {start}..{end} is outside 0..{}", src.len()));
    }
    if !src.is_char_boundary(start) || !src.is_char_boundary(end) {
        return Some(format!("span {start}..{end} splits a character"));
    }
    let at = locate(src, start);
    if at != (span.line, span.col) {
        return Some(format!(
            "span {start}..{end} says {}:{} but its offset is at {}:{}",
            span.line, span.col, at.0, at.1
        ));
    }
    None
}

/// Apply every oracle to one (target, source) pair.
///
/// Everything funnels through here: a campaign input, a corpus replay, a mutant, and a
/// minimizer candidate are judged by exactly the same rules.
#[must_use]
pub fn evaluate(target: &'static dyn Target, src: &str) -> Outcome {
    let budget = target.budget();
    if src.len() > budget.input_bytes {
        return Outcome::BudgetExhausted {
            limit: Budget::INPUT_BYTES,
            needed: src.len(),
            ceiling: budget.input_bytes,
        };
    }
    let Ok((first, second)) = run_bounded(target, src, budget.wall) else {
        return Outcome::Defect {
            oracle: Oracle::Hung,
            landing: None,
            detail: format!("no answer within {:?}", budget.wall),
        };
    };
    let first = match first {
        Ok(p) => p,
        Err(text) => {
            return Outcome::Defect {
                oracle: Oracle::Panicked,
                landing: None,
                detail: text,
            };
        }
    };
    let second = match second {
        Some(Ok(p)) => p,
        Some(Err(text)) => {
            return Outcome::Defect {
                oracle: Oracle::Panicked,
                landing: Some(first.landing),
                detail: text,
            };
        }
        None => unreachable!("a second probe runs whenever the first answered"),
    };
    let defect = |oracle, detail: String| Outcome::Defect {
        oracle,
        landing: Some(first.landing.clone()),
        detail,
    };
    if first != second {
        return defect(
            Oracle::Nondeterministic,
            format!("first {first:?}\nsecond {second:?}"),
        );
    }
    if !target.vocabulary().contains(&first.landing.as_str()) {
        return defect(Oracle::Untyped, first.detail.clone());
    }
    if let Some(span) = first.span
        && let Some(why) = span_defect(src, span)
    {
        return defect(Oracle::Mislocated, format!("{why}: {}", first.detail));
    }
    for (limits, usage) in &first.usage {
        if usage.nodes > limits.nodes || usage.work > limits.work {
            return defect(Oracle::Overspent, format!("{usage:?} passes {limits:?}"));
        }
    }
    if let RoundTrip::Broke(why) = &first.round_trip {
        return defect(Oracle::RoundTrip, why.clone());
    }
    if first.depth > target.depth_limit() {
        return defect(
            Oracle::TooDeep,
            format!(
                "the accepted tree is {} deep by the target's measure, past {}",
                first.depth,
                target.depth_limit()
            ),
        );
    }
    Outcome::Clean {
        landing: first.landing,
    }
}
