//! The G0-DX-10 falsification artifact: per-attack verdicts, the sensitivity table, and the
//! statement of what survives.
//!
//! # Why this is a second artifact and not a section of the first
//!
//! [`crate::report::Report`] is the *instrument's* output: what the two arms did under the
//! accounting rule bn-134i fixed before taking a number. This is the output of the campaign
//! against that instrument: what the same runs say under every other defensible accounting
//! rule, which of the instrument's choices are load-bearing, and which conclusions hold
//! whichever way each choice is made. Folding the second into the first would let a reader
//! quote a variant as if it were the measurement, which is exactly the confusion a
//! falsification campaign exists to prevent.
//!
//! Same rules as the first artifact, for the same reasons: a typed value, a total rendering
//! function, deterministic order, no float, no clock, no path, every section derived
//! (INV-003, INV-005).
//!
//! # What a verdict means
//!
//! Per INV-008's spirit — an inconclusive answer names its reason rather than being read as
//! a negative:
//!
//! - [`Outcome::Landed`] — the attack found an artefact. The harness choice it attacked
//!   changes a number, and the variant number is reported beside the landed one.
//! - [`Outcome::Held`] — the instrument is honest on this axis: the choice was attacked and
//!   the number did not move enough to change anything an adjudicator would rely on.
//! - [`Outcome::Inconclusive`] — the attack could not be decided on this instrument, with
//!   the reason typed rather than left to a reader.
//!
//! A `Landed` verdict is **not** a defect in bn-134i's work. Every choice this campaign
//! attacked is one bn-134i named in the module that makes it; the campaign's contribution is
//! the number that says what the choice was worth.

use crate::variants::{Decomposition, Sensitivity};

/// Which bias an attack was looking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    /// The harness choice favours the shell baseline: does the measured native loss survive
    /// removing it?
    ProShell,
    /// The harness choice favours the typed arm: does the claimed typed advantage survive
    /// removing it?
    ProNative,
}

impl Direction {
    /// The stable token the report writes this direction as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ProShell => "pro-shell",
            Self::ProNative => "pro-native",
        }
    }
}

/// What one attack concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// An artefact was found: the number moves.
    Landed,
    /// The instrument is honest on this axis.
    Held,
    /// Undecidable on this instrument, for a typed reason.
    Inconclusive(String),
}

impl Outcome {
    /// The stable token the report writes this outcome as.
    #[must_use]
    pub fn token(&self) -> String {
        match self {
            Self::Landed => "LANDED".to_owned(),
            Self::Held => "HELD".to_owned(),
            Self::Inconclusive(reason) => format!("INCONCLUSIVE({reason})"),
        }
    }
}

/// One attack, its target, its verdict, and the measured number that carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attack {
    /// The campaign identifier: `S`n for pro-shell-bias, `N`n for pro-native-bias.
    pub id: &'static str,
    /// Which bias it looked for.
    pub direction: Direction,
    /// The harness choice under attack, in one line.
    pub target: &'static str,
    /// What the attack concluded.
    pub outcome: Outcome,
    /// The measured evidence, rendered from real numbers by the test that took them.
    pub evidence: String,
}

impl Attack {
    /// The attack, as one stable line.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} {} {} :: {} :: {}",
            self.id,
            self.direction.token(),
            self.outcome.token(),
            self.target,
            self.evidence,
        )
    }
}

/// The falsification artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalsificationReport {
    /// Every attack, in campaign order.
    pub attacks: Vec<Attack>,
    /// The sensitivity table: the three ratified margins under every variant.
    pub sensitivity: Vec<Sensitivity>,
    /// Where the typed arm's bytes go, and how much of that a redesign could reach.
    pub decomposition: Decomposition,
    /// What an adjudicator may rely on, whichever way each attacked choice is decided.
    pub surviving: Vec<String>,
}

impl FalsificationReport {
    /// The canonical rendering.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("continuum-benchmark dx10-falsification v1\n");
        out.push_str("# bn-2c0a. The instrument attacked from both sides. This artifact\n");
        out.push_str("# reports verdicts on the INSTRUMENT; it does not adjudicate G0-DX-10.\n");
        out.push('\n');

        out.push_str("[attacks]\n");
        out.push_str("# id direction verdict :: harness choice attacked :: measured evidence\n");
        for attack in &self.attacks {
            out.push_str(&attack.render());
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[sensitivity]\n");
        out.push_str("# the three ratified margins under every accounting and instrument\n");
        out.push_str("# variant. The first row is the landed artifact, unchanged.\n");
        for row in &self.sensitivity {
            out.push_str(&row.render());
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[decomposition]\n");
        out.push_str(&format!(
            "answers: {}\nresult_bytes: {}\nrequest_bytes: {}\nfloor_bytes: {}\n\
             snapshot_component_bytes: {} over {} workspace.create requests\n",
            self.decomposition.answers,
            self.decomposition.result_bytes,
            self.decomposition.request_bytes,
            self.decomposition.floor_bytes,
            self.decomposition.snapshot_component_bytes,
            self.decomposition.snapshot_component_calls,
        ));
        out.push_str("field class bytes\n");
        for field in &self.decomposition.fields {
            out.push_str(&format!(
                "{} {} {}\n",
                field.field, field.class, field.bytes
            ));
        }
        out.push_str(&format!(
            "redesign_reducible: {}\nmaximal_reducible: {}\n",
            self.decomposition.redesign_reducible(),
            self.decomposition.maximal_reducible(),
        ));
        out.push('\n');

        out.push_str("[surviving]\n");
        out.push_str("# what an adjudicator may rely on, whichever way each attacked\n");
        out.push_str("# choice is decided.\n");
        for line in &self.surviving {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');

        out.push_str("[limits]\n");
        out.push_str(
            "policies: still scripted. Every variant here varies the *interface* or the \
             *accounting*; none varies the agent, so none answers whether a live model \
             would discover better use of either surface.\n\
             rates: the stale-read and misnamed-argument variants report what one occurrence \
             costs and the break-even count. Neither declares a rate, because this \
             instrument cannot measure one.\n\
             baseline: still a text projection of this daemon, not PR 13's CLI.\n\
             verdict: none on G0-DX-10. These verdicts are on the INSTRUMENT. The row's own \
             pass condition is adjudicated by bn-762i.\n",
        );
        out
    }
}
