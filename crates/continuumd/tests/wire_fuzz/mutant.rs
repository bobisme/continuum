//! Deliberately defective decoders, so that "the harness found nothing" means something.
//!
//! # The problem this solves
//!
//! A fuzz lane that has never failed is a hypothesis. Every oracle in `engine.rs` is a
//! claim that a certain kind of defect would be caught, and until a defect of that kind is
//! actually caught the claim is untested. So each of the four oracles has a mutant here
//! that violates exactly the property it checks, and `wire_fuzz.rs` asserts that the
//! *real* decoder is clean on the same bytes the mutant fails on. If an oracle is ever
//! weakened, its mutant stops being caught and the gate goes red.
//!
//! # Why these four defects and not others
//!
//! Each is a real failure mode of a hand-rolled canonical decoder, not a synthetic one:
//!
//! - [`Defect::PanicOnDeepNesting`] — the unbounded-recursion bug every recursive-descent
//!   parser has before someone adds a depth counter. docs/12 §11 classes it as a release
//!   blocker.
//! - [`Defect::TolerateTrailingWhitespace`] — the lenient-parser bug: two byte strings
//!   decode to one value, so the encoding stops being an identity (ADR-0013).
//! - [`Defect::DriftOnRepeat`] — the cached-state bug: the decoder's answer depends on how
//!   many times it has been called, which is exactly the ambient nondeterminism INV-005
//!   forbids.
//! - [`Defect::UndeclaredLanding`] — the vocabulary-drift bug: a new answer that no
//!   consumer has a case for, which is how a typed outcome quietly becomes an untyped one
//!   (INV-008).
//!
//! Every mutant lives behind `#[cfg(test)]` by construction — this is a test target — and
//! nothing in it is reachable from `continuumd`'s library.

use std::sync::atomic::{AtomicUsize, Ordering};

use continuumd::codec::json::{Json, JsonError};

use super::outcome::{Budget, Probe};
use super::target::{CODEC_MAX_DEPTH, JSON_LANDINGS, Target};

/// Which property the mutant violates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defect {
    /// Unwinds on deeply nested input instead of reporting a typed depth error.
    PanicOnDeepNesting,
    /// Accepts a second spelling: trailing whitespace is trimmed rather than refused, so
    /// two byte strings decode to one document.
    TolerateTrailingWhitespace,
    /// Answers differently on the second call for the same bytes.
    DriftOnRepeat,
    /// Reports a landing outside its own declared vocabulary.
    UndeclaredLanding,
}

impl Defect {
    /// The oracle this defect is built to trip.
    #[must_use]
    pub const fn oracle(self) -> super::outcome::Oracle {
        match self {
            Self::PanicOnDeepNesting => super::outcome::Oracle::Panicked,
            Self::TolerateTrailingWhitespace => super::outcome::Oracle::RoundTrip,
            Self::DriftOnRepeat => super::outcome::Oracle::Nondeterministic,
            Self::UndeclaredLanding => super::outcome::Oracle::Untyped,
        }
    }

    /// Every defect, so the anti-vacuity matrix cannot silently lose one.
    pub const ALL: [Self; 4] = [
        Self::PanicOnDeepNesting,
        Self::TolerateTrailingWhitespace,
        Self::DriftOnRepeat,
        Self::UndeclaredLanding,
    ];
}

/// A mutated `canonical.json` decoder.
///
/// Everything except the injected defect delegates to the real
/// [`Json::parse`](continuumd::codec::json::Json::parse), so a mutant differs from the
/// production decoder in exactly one behaviour and the oracle that fires names that
/// behaviour and no other.
#[derive(Debug)]
pub struct MutantJson {
    defect: Defect,
    calls: AtomicUsize,
}

impl MutantJson {
    /// A mutant carrying `defect`.
    #[must_use]
    pub const fn new(defect: Defect) -> Self {
        Self {
            defect,
            calls: AtomicUsize::new(0),
        }
    }

    /// The bytes this mutant's defect is triggered by.
    ///
    /// Kept beside the mutant so the anti-vacuity test cannot drift away from the defect
    /// it is meant to exercise. Every one is small and linear: the deep case is one-sided
    /// nesting at the production bound plus one, which costs one byte per level.
    #[must_use]
    pub fn trigger(defect: Defect) -> Vec<u8> {
        match defect {
            Defect::PanicOnDeepNesting => {
                let mut out = vec![b'['; CODEC_MAX_DEPTH + 1];
                out.extend(std::iter::repeat_n(b']', CODEC_MAX_DEPTH + 1));
                out
            }
            Defect::TolerateTrailingWhitespace => b"{} ".to_vec(),
            Defect::DriftOnRepeat | Defect::UndeclaredLanding => b"null".to_vec(),
        }
    }
}

/// The real decoder's landing spelling, shared with `target.rs` through
/// [`JSON_LANDINGS`] so a mutant cannot invent a token by accident.
fn json_landing(error: &JsonError) -> &'static str {
    match error {
        JsonError::NotUtf8 => "json::not-utf8",
        JsonError::Truncated => "json::truncated",
        JsonError::TrailingBytes => "json::trailing-bytes",
        JsonError::TooDeep => "json::too-deep",
        JsonError::DuplicateKey { .. } => "json::duplicate-key",
        JsonError::KeyOrder { .. } => "json::key-order",
        JsonError::Unexpected { .. } => "json::unexpected",
        JsonError::FloatingPoint { .. } => "json::floating-point",
        JsonError::Signed { .. } => "json::signed",
        JsonError::LeadingZero { .. } => "json::leading-zero",
        JsonError::IntegerRange { .. } => "json::integer-range",
        JsonError::RawControl { .. } => "json::raw-control",
        JsonError::BadEscape { .. } => "json::bad-escape",
    }
}

impl Target for MutantJson {
    fn name(&self) -> &'static str {
        "mutant.canonical.json"
    }

    fn budget(&self) -> Budget {
        Budget {
            input_bytes: 2048,
            ..Budget::DEFAULT
        }
    }

    fn vocabulary(&self) -> &'static [&'static str] {
        // Deliberately the *real* target's vocabulary. `UndeclaredLanding` is caught
        // because it answers outside this list, not because the list was narrowed for it.
        static CELL: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        CELL.get_or_init(|| {
            let mut out: Vec<&'static str> = vec![super::target::ACCEPTED];
            out.extend_from_slice(JSON_LANDINGS);
            out.sort_unstable();
            out
        })
        .as_slice()
    }

    fn probe(&self, bytes: &[u8]) -> Probe {
        match self.defect {
            Defect::PanicOnDeepNesting => {
                let mut run = 0_usize;
                for byte in bytes {
                    if *byte == b'[' {
                        run += 1;
                        assert!(
                            run <= CODEC_MAX_DEPTH,
                            "mutant: recursion is not bounded here"
                        );
                    } else {
                        run = 0;
                    }
                }
            }
            Defect::TolerateTrailingWhitespace => {
                if let Err(JsonError::TrailingBytes) = Json::parse(bytes) {
                    let trimmed: Vec<u8> = bytes
                        .iter()
                        .copied()
                        .filter(|byte| !byte.is_ascii_whitespace())
                        .collect();
                    if let Ok(document) = Json::parse(&trimmed) {
                        // A lenient parser's answer: accepted, and its canonical form is
                        // the trimmed spelling — which is not the input.
                        return Probe::canonical(document.to_canonical_bytes());
                    }
                }
            }
            Defect::DriftOnRepeat => {
                if self.calls.fetch_add(1, Ordering::SeqCst) % 2 == 1 {
                    return Probe::rejected("json::truncated");
                }
            }
            Defect::UndeclaredLanding => {
                return Probe::rejected("json::a-token-this-target-never-declared");
            }
        }
        match Json::parse(bytes) {
            Err(error) => Probe::rejected(json_landing(&error)),
            Ok(document) => Probe::canonical(document.to_canonical_bytes()),
        }
    }
}
