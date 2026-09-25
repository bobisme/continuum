//! Which claims a lane bears on, and what quarantine does to them.
//!
//! # Lanes
//!
//! A [`Lane`] pairs a subject engine with the oracle it is measured against and names
//! the docs/18 claims (`C0xx`) whose evidence depends on the two agreeing. [`LANES`] is
//! the table. A disagreement on a lane **halts** each of its claims: the claim is
//! entered in a [`QuarantineLedger`], and the run is admissible only while every
//! quarantined claim is (a) recorded in the committed quarantine register and (b) not
//! asserted at a positive evidence state in the claims registry.
//!
//! # What "quarantine" means here
//!
//! The dossier defines quarantine for the incremental audit: RFC 0030 "Quarantine"
//! scopes it to a `(reuse-edge class, function_id, function_version)` triple, makes it
//! durable, and clears it only by evidence, never by time or restart. docs/19 §5 uses
//! the same word's substance for engines: "Disagreement halts the relevant claim and
//! creates a minimized fixture." This module applies RFC 0030's rules with a docs/18
//! claim as the unit: a quarantine is durable (the committed register), it is cleared
//! only by removing the register line together with clean evidence (a passing run), and
//! while it stands the claim may not be asserted `OBSERVED` or stronger — docs/18's
//! "Public claims must never outrun the evidence state."
//!
//! The claim states are read from `tools/governance/claims-baseline.json`, the pinned
//! copy of docs/18 that `tools/governance/check_t09_evidence.py` keeps equal to the
//! live rows. No schema in `notes/plan/schemas/` covers claim quarantine, so the
//! register's shape is this module's and is stated at [`QuarantineLedger::parse`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::engine::{CLOSURE, DPOR, EXPLICIT, Fields, KERNEL, REFERENCE, SEMANTIC_ORACLE};

/// A subject engine, its oracle, and the claims their agreement is evidence for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    /// The engine under test.
    pub subject: &'static str,
    /// The engine it is measured against.
    pub oracle: &'static str,
    /// The fields the subject's engine must declare; [`super::Harness::new`] refuses
    /// an engine whose declaration differs, so an engine cannot opt out of a field
    /// the lane's claims rest on.
    pub subject_fields: Fields,
    /// The fields the oracle's engine must declare.
    pub oracle_fields: Fields,
    /// The docs/18 claims a disagreement halts.
    pub claims: &'static [&'static str],
    /// Why these claims, in one line.
    pub basis: &'static str,
}

/// The lane table. A lane whose engine is not plugged in is reported absent, never
/// passed.
pub const LANES: [Lane; 5] = [
    Lane {
        subject: CLOSURE,
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &["C006"],
        basis: "C006 is observed on the reference explorer's exact reachable set; an independent search that finds a different set, deadlock set, or verdict contradicts that record",
    },
    Lane {
        subject: KERNEL,
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &["C018", "C023"],
        basis: "the reference certificate checked by the kernel from wire bytes is C018's scope and one of C023's cross-path arrows; a kernel rejection of a set or invariant the producer reports breaks both. The kernel's state count is the producer's own closed set (closure does not prove minimality), so on that field the lane checks closure, not exactness; the closure lane checks exactness. Undefined reads (bn-24a5c) on this lane are the reference emitter refusing to certify, which is the reference's own definedness scan, checked against the model: a consistency check of the producer, not kernel evidence. They carry no path and the lane has no exact reachable set, so they are unproven and recorded undecided (undefined-unproven), never agreed. The kernel decides definedness on its own only where it accepts a certificate (no undefined read); its undefined-read rejections are reached only by a certificate that carries one, which the reference emitter never writes",
    },
    Lane {
        subject: SEMANTIC_ORACLE,
        oracle: REFERENCE,
        subject_fields: Fields::PROJECTION_ONLY,
        oracle_fields: Fields::ALL,
        claims: &["C005"],
        basis: "the tiny exhaustive oracle is the enumerator C005's exhaustive differential corpus is measured with; if it disagrees with the reference path it cannot serve as that oracle. Its reachable set comes from the same bfs::explore, so agreement on that field is a consistency check, not independent evidence; its quiescent states are its own computation",
    },
    Lane {
        subject: EXPLICIT,
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &["C006"],
        basis: "C006's record says the optimized explicit engine needs new evidence under ADR-0013 when it lands; a disagreement with the reference explorer is that evidence failing",
    },
    Lane {
        subject: DPOR,
        oracle: SEMANTIC_ORACLE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::PROJECTION_ONLY,
        claims: &["C005"],
        basis: "docs/19 §5 'exhaustive enumerator versus DPOR' is C005's exhaustive differential corpus",
    },
];

/// A docs/18 evidence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaimState {
    /// `HYPOTHESIS`.
    Hypothesis,
    /// `TARGET`.
    Target,
    /// `OBSERVED`.
    Observed,
    /// `OBSERVED-BOUNDED` (used by C025).
    ObservedBounded,
    /// `ESTABLISHED-BOUNDED`.
    EstablishedBounded,
    /// `ESTABLISHED`.
    Established,
    /// `BLOCKED`.
    Blocked,
    /// `REFUTED`.
    Refuted,
}

impl ClaimState {
    /// Parse the registry spelling. `None` for anything else: an unknown state is not
    /// guessed at.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "HYPOTHESIS" => Self::Hypothesis,
            "TARGET" => Self::Target,
            "OBSERVED" => Self::Observed,
            "OBSERVED-BOUNDED" => Self::ObservedBounded,
            "ESTABLISHED-BOUNDED" => Self::EstablishedBounded,
            "ESTABLISHED" => Self::Established,
            "BLOCKED" => Self::Blocked,
            "REFUTED" => Self::Refuted,
            _ => return None,
        })
    }

    /// The registry spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hypothesis => "HYPOTHESIS",
            Self::Target => "TARGET",
            Self::Observed => "OBSERVED",
            Self::ObservedBounded => "OBSERVED-BOUNDED",
            Self::EstablishedBounded => "ESTABLISHED-BOUNDED",
            Self::Established => "ESTABLISHED",
            Self::Blocked => "BLOCKED",
            Self::Refuted => "REFUTED",
        }
    }

    /// Whether the state asserts positive evidence. docs/18's wording table allows
    /// "observed on …" and "established …" only for these.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        matches!(
            self,
            Self::Observed | Self::ObservedBounded | Self::EstablishedBounded | Self::Established
        )
    }
}

/// One quarantined claim: the claim, the lane that halted it, and the defect report
/// that records why.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quarantine {
    /// The claim (`C0xx`).
    pub claim: String,
    /// The lane's subject slot.
    pub subject: String,
    /// The lane's oracle slot.
    pub oracle: String,
    /// The `defect_*` handle of the report.
    pub defect: String,
}

/// A set of quarantines, from a run or from the committed register.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuarantineLedger {
    entries: BTreeSet<Quarantine>,
}

/// Why a register line is not a quarantine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterError {
    /// One-based line number.
    pub line: usize,
    /// What is wrong.
    pub problem: &'static str,
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "quarantine register line {}: {}",
            self.line, self.problem
        )
    }
}

/// Whether `text` is an engine slot the register can carry: non-empty
/// `[A-Za-z0-9_:.-]`. No space, so a register line always has four fields.
#[must_use]
pub fn is_slot(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'.' | b'-'))
}

/// Whether `text` is a docs/18 claim id: `C` and three digits.
#[must_use]
pub fn is_claim_id(text: &str) -> bool {
    text.len() == 4
        && text.starts_with('C')
        && text.bytes().skip(1).all(|byte| byte.is_ascii_digit())
}

fn is_defect_handle(text: &str) -> bool {
    text.strip_prefix("defect_").is_some_and(|identity| {
        !identity.is_empty()
            && identity
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    })
}

impl QuarantineLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter a quarantine.
    pub fn insert(&mut self, entry: Quarantine) {
        self.entries.insert(entry);
    }

    /// Every entry, in order.
    pub fn entries(&self) -> impl Iterator<Item = &Quarantine> {
        self.entries.iter()
    }

    /// The quarantined claims.
    #[must_use]
    pub fn claims(&self) -> BTreeSet<&str> {
        self.entries
            .iter()
            .map(|entry| entry.claim.as_str())
            .collect()
    }

    /// Whether the ledger has no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Both ledgers' entries.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            entries: self.entries.union(&other.entries).cloned().collect(),
        }
    }

    /// Parse the committed register.
    ///
    /// Format, one quarantine per line: `<claim> <subject slot> <oracle slot>
    /// <defect handle>`, separated by single spaces. `#` starts a comment line; blank
    /// lines are ignored. A claim is `C` and three digits, a handle is `defect_` and an
    /// identity in `[A-Za-z0-9_-]` (the handle grammar
    /// `crates/continuum-workspace/src/artifact_path.rs` accepts).
    ///
    /// # Errors
    ///
    /// The first malformed line. A register that does not parse quarantines nothing,
    /// so the caller fails closed.
    pub fn parse(text: &str) -> Result<Self, RegisterError> {
        let mut ledger = Self::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index.saturating_add(1);
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = trimmed.split(' ').collect();
            let [claim, subject, oracle, defect] = fields.as_slice() else {
                return Err(RegisterError {
                    line,
                    problem: "expected four space-separated fields",
                });
            };
            if !is_claim_id(claim) {
                return Err(RegisterError {
                    line,
                    problem: "the claim is not C and three digits",
                });
            }
            if !is_slot(subject) || !is_slot(oracle) {
                return Err(RegisterError {
                    line,
                    problem: "an engine slot is not [A-Za-z0-9_:.-]+",
                });
            }
            if !is_defect_handle(defect) {
                return Err(RegisterError {
                    line,
                    problem: "the defect is not a defect_ handle",
                });
            }
            ledger.insert(Quarantine {
                claim: (*claim).to_owned(),
                subject: (*subject).to_owned(),
                oracle: (*oracle).to_owned(),
                defect: (*defect).to_owned(),
            });
        }
        Ok(ledger)
    }

    /// The register line for every entry, in order; [`Self::parse`] reads it back.
    #[must_use]
    pub fn render(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                format!(
                    "{} {} {} {}\n",
                    entry.claim, entry.subject, entry.oracle, entry.defect
                )
            })
            .collect()
    }
}

/// Why the gate refuses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GateFinding {
    /// A quarantined claim is asserted at a positive evidence state.
    AssertedPositive {
        /// The claim.
        claim: String,
        /// Its registry state.
        state: ClaimState,
    },
    /// A quarantined claim is not in the registry at all.
    UnknownClaim {
        /// The claim.
        claim: String,
    },
    /// A run quarantined a claim the committed register does not record.
    Unregistered {
        /// The claim.
        claim: String,
        /// The report's handle.
        defect: String,
    },
}

impl fmt::Display for GateFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssertedPositive { claim, state } => write!(
                f,
                "{claim} is quarantined but docs/18 asserts it {}",
                state.as_str()
            ),
            Self::UnknownClaim { claim } => {
                write!(f, "{claim} is quarantined but is not a docs/18 claim")
            }
            Self::Unregistered { claim, defect } => write!(
                f,
                "{claim} was halted by {defect} and is not in the committed quarantine register"
            ),
        }
    }
}

/// The quarantine gate: no quarantined claim may be asserted at a positive state, and
/// every quarantined claim must exist. Fails closed on a claim the registry does not
/// know.
#[must_use]
pub fn gate(ledger: &QuarantineLedger, states: &BTreeMap<String, ClaimState>) -> Vec<GateFinding> {
    let mut out: BTreeSet<GateFinding> = BTreeSet::new();
    for claim in ledger.claims() {
        match states.get(claim) {
            None => {
                out.insert(GateFinding::UnknownClaim {
                    claim: claim.to_owned(),
                });
            }
            Some(state) if state.is_positive() => {
                out.insert(GateFinding::AssertedPositive {
                    claim: claim.to_owned(),
                    state: *state,
                });
            }
            Some(_) => {}
        }
    }
    out.into_iter().collect()
}

/// The halting rule for a run: every quarantine the run produced must be in the
/// committed register **as a whole entry** (claim, lane, and defect handle), and the
/// gate must hold over both. Matching the whole entry means a register line recorded
/// for one defect never covers a different defect of the same claim: a new
/// disagreement halts again until it is recorded itself.
#[must_use]
pub fn halts(
    run: &QuarantineLedger,
    register: &QuarantineLedger,
    states: &BTreeMap<String, ClaimState>,
) -> Vec<GateFinding> {
    let mut out: BTreeSet<GateFinding> = gate(&run.union(register), states).into_iter().collect();
    for entry in run.entries() {
        if !register.entries.contains(entry) {
            out.insert(GateFinding::Unregistered {
                claim: entry.claim.clone(),
                defect: entry.defect.clone(),
            });
        }
    }
    out.into_iter().collect()
}
